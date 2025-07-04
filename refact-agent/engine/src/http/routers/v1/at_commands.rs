use axum::response::Result;
use axum::Extension;
use hyper::{Body, Response, StatusCode};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use serde_json::{json, Value};
use tokio::sync::RwLock as ARwLock;
use tokio::sync::Mutex as AMutex;
use strsim::jaro_winkler;
use itertools::Itertools;
use tokenizers::Tokenizer;
use tracing::info;

use crate::at_commands::execute_at::run_at_commands_locally;
use crate::indexing_utils::wait_for_indexing_if_needed;
use crate::postprocessing::pp_utils::pp_resolve_ctx_file_paths;
use crate::tokens;
use crate::at_commands::at_commands::AtCommandsContext;
use crate::at_commands::execute_at::{execute_at_commands_in_query, parse_words_from_line};
use crate::call_validation::{ChatMeta, PostprocessSettings, SubchatParameters};
use crate::caps::resolve_chat_model;
use crate::custom_error::ScratchError;
use crate::global_context::try_load_caps_quickly_if_not_present;
use crate::global_context::GlobalContext;
use crate::call_validation::{ChatMessage, ChatContent, ContextEnum};
use crate::at_commands::at_commands::filter_only_context_file_from_context_tool;
use crate::http::routers::v1::chat::deserialize_messages_from_post;
use crate::scratchpads::scratchpad_utils::HasRagResults;


#[derive(Serialize, Deserialize, Clone)]
struct CommandCompletionPost {
    query: String,
    cursor: i64,
    top_n: usize,
}
#[derive(Serialize, Deserialize, Clone)]
struct CommandCompletionResponse {
    completions: Vec<String>,
    replace: (i64, i64),
    is_cmd_executable: bool,
}

#[derive(Serialize, Deserialize, Clone)]
struct CommandPreviewPost {
    #[serde(default)]
    pub messages: Vec<Value>,
    #[serde(default)]
    model: String,
    #[serde(default)]
    provider: String,
    #[serde(default)]
    pub meta: ChatMeta,
}

#[derive(Serialize, Deserialize, Clone)]
struct Highlight {
    kind: String,
    pos1: i64,
    pos2: i64,
    ok: bool,
    reason: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CommandExecutePost {
    pub messages: Vec<ChatMessage>,
    pub n_ctx: usize,
    pub maxgen: usize,
    pub subchat_tool_parameters: IndexMap<String, SubchatParameters>, // tool_name: {model, allowed_context, temperature}
    pub postprocess_parameters: PostprocessSettings,
    pub model_name: String,
    pub chat_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CommandExecuteResponse {
    pub messages: Vec<ChatMessage>,
    pub undroppable_msg_number: usize,
    pub any_context_produced: bool,
    pub messages_to_stream_back: Vec<serde_json::Value>,
}

pub async fn handle_v1_command_completion(
    Extension(global_context): Extension<Arc<ARwLock<GlobalContext>>>,
    body_bytes: hyper::body::Bytes,
) -> Result<Response<Body>, ScratchError> {
    let post = serde_json::from_slice::<CommandCompletionPost>(&body_bytes)
        .map_err(|e| ScratchError::new(StatusCode::UNPROCESSABLE_ENTITY, format!("JSON problem: {}", e)))?;
    let top_n = post.top_n;

    let fake_n_ctx = 4096;
    let ccx: Arc<AMutex<AtCommandsContext>> = Arc::new(AMutex::new(AtCommandsContext::new(
        global_context.clone(),
        fake_n_ctx,
        top_n,
        true,
        vec![],
        "".to_string(),
        false,
        "".to_string(),
    ).await));

    let at_commands = ccx.lock().await.at_commands.clone();
    let at_command_names = at_commands.keys().map(|x|x.clone()).collect::<Vec<_>>();

    let mut completions: Vec<String> = vec![];
    let mut pos1 = -1; let mut pos2 = -1;
    let mut is_cmd_executable = false;

    if let Ok((query_line_val, cursor_rel, cursor_line_start)) = get_line_with_cursor(&post.query, post.cursor) {
        let query_line_val = query_line_val.chars().take(cursor_rel as usize).collect::<String>();
        let args = query_line_args(&query_line_val, cursor_rel, cursor_line_start, &at_command_names);
        info!("args: {:?}", args);
        (completions, is_cmd_executable, pos1, pos2) = command_completion(ccx.clone(), args,  post.cursor).await;
    }
    let completions: Vec<_> = completions.into_iter().unique().map(|x|format!("{} ", x)).collect();

    let response = CommandCompletionResponse {
        completions,
        replace: (pos1, pos2),
        is_cmd_executable,
    };

    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(Body::from(serde_json::to_string(&response).unwrap()))
        .unwrap())
}

async fn count_tokens(tokenizer_arc: Option<Arc<Tokenizer>>, messages: &Vec<ChatMessage>) -> Result<u64, ScratchError> {
    let mut accum: u64 = 0;

    for message in messages {
        accum += message.content.count_tokens(tokenizer_arc.clone(), &None)
            .map_err(|e| ScratchError {
                status_code: StatusCode::INTERNAL_SERVER_ERROR,
                message: format!("v1_chat_token_counter: count_tokens failed: {}", e),
                telemetry_skip: false})? as u64;
    }
    Ok(accum)
}

pub async fn handle_v1_command_preview(
    Extension(global_context): Extension<Arc<ARwLock<GlobalContext>>>,
    body_bytes: hyper::body::Bytes,
) -> Result<Response<Body>, ScratchError> {
    let post = serde_json::from_slice::<CommandPreviewPost>(&body_bytes)
        .map_err(|e| ScratchError::new(StatusCode::UNPROCESSABLE_ENTITY, format!("JSON problem: {}", e)))?;
    let mut messages = deserialize_messages_from_post(&post.messages)?;

    let last_message = messages.pop();
    let mut query = if let Some(last_message) = &last_message {
        match &last_message.content {
            ChatContent::SimpleText(query) => query.clone(),
            ChatContent::Multimodal(elements) => {
                let mut query = String::new();
                for element in elements {
                    if element.is_text() { // use last text, but expected to be only one
                        query = element.m_content.clone();
                    }
                }
                query
            }
        }
    } else {
        String::new()
    };

    let caps = crate::global_context::try_load_caps_quickly_if_not_present(global_context.clone(), 0).await?;
    let model_rec = resolve_chat_model(caps, &post.model)
        .map_err(|e| ScratchError::new(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let tokenizer_arc = match tokens::cached_tokenizer(global_context.clone(), &model_rec.base).await {
        Ok(x) => x,
        Err(e) => {
            tracing::error!(e);
            return Err(ScratchError::new(StatusCode::BAD_REQUEST, e));
        }
    };

    let ccx = Arc::new(AMutex::new(AtCommandsContext::new(
        global_context.clone(),
        model_rec.base.n_ctx,
        crate::http::routers::v1::chat::CHAT_TOP_N,
        true,
        vec![],
        "".to_string(),
        false,
        model_rec.base.id.clone(),
    ).await));

    let (messages_for_postprocessing, vec_highlights) = execute_at_commands_in_query(
        ccx.clone(),
        &mut query
    ).await;

    let mut preview: Vec<ChatMessage> = vec![];
    for exec_result in messages_for_postprocessing.iter() {
        // at commands exec() can produce both role="user" and role="assistant" messages
        if let ContextEnum::ChatMessage(raw_msg) = exec_result {
            preview.push(raw_msg.clone());
        }
    }

    let mut pp_settings = {
        let ccx_locked = ccx.lock().await;
        ccx_locked.postprocess_parameters.clone()
    };
    if pp_settings.max_files_n == 0 {
        pp_settings.max_files_n = crate::http::routers::v1::chat::CHAT_TOP_N;
    }

    let mut context_files = filter_only_context_file_from_context_tool(&messages_for_postprocessing);
    let ctx_file_paths = pp_resolve_ctx_file_paths(global_context.clone(), &mut context_files).await;
    for (context_file, (_, short_path)) in context_files.iter_mut().zip(ctx_file_paths.into_iter()) {
        context_file.file_name = short_path;
    }

    if !context_files.is_empty() {
        let message = ChatMessage {
            role: "context_file".to_string(),
            content: ChatContent::SimpleText(serde_json::to_string(&context_files).unwrap()),
            tool_calls: None,
            tool_call_id: "".to_string(),
            ..Default::default()
        };
        preview.push(message.clone());
    }

    let mut highlights = vec![];
    for h in vec_highlights {
        highlights.push(Highlight {
            kind: h.kind.clone(),
            pos1: h.pos1 as i64,
            pos2: h.pos2 as i64,
            ok: h.ok,
            reason: h.reason.unwrap_or_default(),
        })
    }

    let messages_to_count = if let Some(mut last_message) = last_message {
        match &mut last_message.content {
            ChatContent::SimpleText(_) => {last_message.content = ChatContent::SimpleText(query.clone());}
            ChatContent::Multimodal(elements) => {
                for elem in elements {
                    if elem.is_text() {
                        elem.m_content = query.clone();
                    }
                }
            }
        };
        itertools::concat(vec![preview.clone(), vec![last_message]])
    } else {
        preview.clone()
    };
    let tokens_number = count_tokens(tokenizer_arc.clone(), &messages_to_count).await?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(Body::from(serde_json::to_string_pretty(
            &json!({"messages": preview, "model": model_rec.base.id, "highlight": highlights,
                "current_context": tokens_number, "number_context": model_rec.base.n_ctx})
        ).unwrap()))
        .unwrap())
}

pub async fn handle_v1_at_command_execute(
    Extension(global_context): Extension<Arc<ARwLock<GlobalContext>>>,
    body_bytes: hyper::body::Bytes,
) -> Result<Response<Body>, ScratchError> {
    wait_for_indexing_if_needed(global_context.clone()).await;

    let post = serde_json::from_slice::<CommandExecutePost>(&body_bytes)
        .map_err(|e| ScratchError::new(StatusCode::UNPROCESSABLE_ENTITY, format!("JSON problem: {}", e)))?;

    let caps = try_load_caps_quickly_if_not_present(global_context.clone(), 0).await?;
    let model_rec = resolve_chat_model(caps, &post.model_name)
        .map_err(|e| ScratchError::new(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let tokenizer = tokens::cached_tokenizer(global_context.clone(), &model_rec.base).await
        .map_err(|e| ScratchError::new(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let mut ccx = AtCommandsContext::new(
        global_context.clone(),
        post.n_ctx,
        crate::http::routers::v1::chat::CHAT_TOP_N,
        true,
        vec![],
        "".to_string(),
        false,
        model_rec.base.id.clone(),
    ).await;
    ccx.subchat_tool_parameters = post.subchat_tool_parameters.clone();
    ccx.postprocess_parameters = post.postprocess_parameters.clone();
    let ccx_arc = Arc::new(AMutex::new(ccx));

    let mut has_rag_results = HasRagResults::new();
    let (messages, any_context_produced) = run_at_commands_locally(
        ccx_arc.clone(), tokenizer.clone(), post.maxgen, post.messages, &mut has_rag_results).await;
    let messages_to_stream_back = has_rag_results.in_json;
    let undroppable_msg_number = messages.iter().rposition(|msg| msg.role == "user").unwrap_or(0);

    let response = CommandExecuteResponse {
        messages, messages_to_stream_back, undroppable_msg_number, any_context_produced };

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string_pretty(&response).unwrap()))
        .unwrap())
}

fn get_line_with_cursor(query: &String, cursor: i64) -> Result<(String, i64, i64), ScratchError> {
    let mut cursor_rel = cursor;
    for line in query.lines() {
        let line_length = line.len() as i64;
        if cursor_rel <= line_length {
            return Ok((line.to_string(), cursor_rel, cursor - cursor_rel));
        }
        cursor_rel -= line_length + 1; // +1 to account for the newline character
    }
    return Err(ScratchError::new(StatusCode::EXPECTATION_FAILED, "incorrect cursor provided".to_string()));
}

async fn command_completion(
    ccx: Arc<AMutex<AtCommandsContext>>,
    args: Vec<QueryLineArg>,
    cursor_abs: i64,
) -> (Vec<String>, bool, i64, i64) {    // returns ([possible, completions], good_as_it_is)
    let mut args = args;
    let at_commands = ccx.lock().await.at_commands.clone();
    let at_command_names = at_commands.keys().map(|x|x.clone()).collect::<Vec<_>>();

    let q_cmd_with_index = args.iter().enumerate().find_map(|(index, x)| {
        x.value.starts_with("@").then(|| (x, index))
    });
    let (q_cmd, q_cmd_idx) = match q_cmd_with_index {
        Some((x, idx)) => (x.clone(), idx),
        None => return (vec![], false, -1, -1),
    };

    let cmd = match at_command_names.iter().find(|x|x == &&q_cmd.value).and_then(|x| at_commands.get(x)) {
        Some(x) => x,
        None => {
            return if !q_cmd.focused {
                (vec![], false, -1, -1)
            } else {
                (command_completion_options(ccx.clone(), &q_cmd.value).await, false, q_cmd.pos1, q_cmd.pos2)
            }
        }
    };
    args = args.iter().skip(q_cmd_idx + 1).map(|x|x.clone()).collect::<Vec<_>>();
    let cmd_params_cnt = cmd.params().len();
    args.truncate(cmd_params_cnt);

    let can_execute = args.len() == cmd.params().len();

    for (arg, param) in args.iter().zip(cmd.params()) {
        let is_valid = param.is_value_valid(ccx.clone(), &arg.value).await;
        if !is_valid {
            return if arg.focused {
                (param.param_completion(ccx.clone(), &arg.value).await, can_execute, arg.pos1, arg.pos2)
            } else {
                (vec![], false, -1, -1)
            }
        }
        if is_valid && arg.focused && param.param_completion_valid() {
            return (param.param_completion(ccx.clone(), &arg.value).await, can_execute, arg.pos1, arg.pos2);
        }
    }

    if can_execute {
        return (vec![], true, -1, -1);
    }

    // if command is not focused, and the argument is empty we should make suggestions
    if !q_cmd.focused {
        match cmd.params().get(args.len()) {
            Some(param) => {
                return (param.param_completion(ccx.clone(), &"".to_string()).await, false, cursor_abs, cursor_abs);
            },
            None => {}
        }
    }

    (vec![], false, -1, -1)
}

async fn command_completion_options(
    ccx: Arc<AMutex<AtCommandsContext>>,
    q_cmd: &String,
) -> Vec<String> {
    let at_commands = ccx.lock().await.at_commands.clone();
    let at_command_names = at_commands.keys().map(|x|x.clone()).collect::<Vec<_>>();
    at_command_names
        .iter()
        .filter(|command| command.starts_with(q_cmd))
        .map(|command| {
            (command.to_string(), jaro_winkler(&command, q_cmd))
        })
        .sorted_by(|(_, dist1), (_, dist2)| dist1.partial_cmp(dist2).unwrap())
        .rev()
        .take(5)
        .map(|(command, _)| command.clone())
        .collect()
}

pub fn query_line_args(line: &String, cursor_rel: i64, cursor_line_start: i64, at_command_names: &Vec<String>) -> Vec<QueryLineArg> {
    let mut args: Vec<QueryLineArg> = vec![];
    for (text, pos1, pos2) in parse_words_from_line(line).iter().rev().cloned() {
        if at_command_names.contains(&text) && args.iter().any(|x|(x.value.contains("@") && x.focused) || at_command_names.contains(&x.value)) {
            break;
        }
        let mut x = QueryLineArg {
            value: text.clone(),
            pos1: pos1 as i64, pos2: pos2 as i64,
            focused: false,
        };
        x.focused = cursor_rel >= x.pos1 && cursor_rel <= x.pos2;
        x.pos1 += cursor_line_start;
        x.pos2 += cursor_line_start;
        args.push(x)
    }
    args.iter().rev().cloned().collect::<Vec<_>>()
}

#[derive(Debug, Clone)]
pub struct QueryLineArg {
    pub value: String,
    pub pos1: i64,
    pub pos2: i64,
    pub focused: bool,
}
