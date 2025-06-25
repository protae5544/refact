import os
from flask import Flask, request, jsonify

app = Flask(__name__)

# Configuration for Refact Server
REFACT_SERVER_URL = os.environ.get("REFACT_SERVER_URL", "http://localhost:8008") # Port 8008 is default for refact
REFACT_API_KEY = os.environ.get("REFACT_API_KEY", None) # API Key for Refact

# It's good practice to make the actual API endpoint for Refact configurable if it's not standard
# For example, for a chat-like completion, the endpoint might be /v1/chat/completions or similar.
# This needs to be verified from Refact's documentation for self-hosted server.
# Assuming a hypothetical /v1/completions endpoint for now.
REFACT_COMPLETION_ENDPOINT = "/v1/completions" # Placeholder - VERIFY THIS

@app.route('/')
def hello():
    return "Backend service for Refact is running!"

@app.route('/api/refact/chat', methods=['POST'])
def refact_chat():
    try:
        data = request.get_json()
        if not data:
            return jsonify({"error": "No input data provided"}), 400

        prompt = data.get("prompt")
        # You might want to pass other parameters like max_tokens, temperature, model, etc.
        # These would also come from `data` or be set as defaults.
        # Example:
        # model_name = data.get("model", "default_model_name")
        # max_tokens = data.get("max_tokens", 150)

        if not prompt:
            return jsonify({"error": "Prompt is required"}), 400

        headers = {
            "Content-Type": "application/json",
        }
        if REFACT_API_KEY:
            headers["Authorization"] = f"Bearer {REFACT_API_KEY}"

        # The payload structure will depend heavily on what the Refact server expects.
        # This is a common structure for many LLM APIs.
        # You MUST verify this against Refact's self-hosted API documentation.
        refact_payload = {
            "model": data.get("model", "smallcloud/Refact-1_6B-fim"), # Example model, verify correct name
            "prompt": prompt,
            "max_tokens": data.get("max_tokens", 200),
            "temperature": data.get("temperature", 0.7),
            # Add other parameters as required by Refact:
            # "stream": False,
            # "stop_sequences": ["\n\n"],
        }

        # Construct the full URL for the Refact server endpoint
        full_refact_url = f"{REFACT_SERVER_URL.rstrip('/')}{REFACT_COMPLETION_ENDPOINT}"

        import requests # Using requests library
        response = requests.post(full_refact_url, json=refact_payload, headers=headers, timeout=60) # 60s timeout
        response.raise_for_status()  # Raise an exception for HTTP errors (4xx or 5xx)

        refact_response_data = response.json()

        # Process refact_response_data as needed.
        # The structure of this response also needs to be verified.
        # For example, the completed text might be in `refact_response_data['choices'][0]['text']`
        # This is a common pattern but needs confirmation.

        # Assuming the response contains a 'completion' or similar field:
        # completion_text = refact_response_data.get("completion_text_field_name_here")
        # For now, just returning the whole response for debugging.
        # return jsonify({"success": True, "refact_response": refact_response_data})

        # Let's assume a more standard OpenAI-like response for completion for now
        # This is a GUESS and needs to be confirmed with Refact's API documentation
        if "choices" in refact_response_data and len(refact_response_data["choices"]) > 0:
            completion_text = refact_response_data["choices"][0].get("text") # or message.content
            if not completion_text and "message" in refact_response_data["choices"][0]:
                 completion_text = refact_response_data["choices"][0]["message"].get("content")
        else:
            # Fallback if the structure is different or if there's no clear completion text
            completion_text = str(refact_response_data)


        return jsonify({
            "success": True,
            "prompt": prompt,
            "response": completion_text,
            "raw_refact_response": refact_response_data # for debugging
        })

    except requests.exceptions.RequestException as e:
        app.logger.error(f"Error connecting to Refact server: {e}")
        return jsonify({"error": f"Could not connect to Refact server: {str(e)}"}), 503 # Service Unavailable
    except Exception as e:
        app.logger.error(f"An unexpected error occurred: {e}")
        return jsonify({"error": f"An unexpected error occurred: {str(e)}"}), 500

if __name__ == '__main__':
    # Make sure to set the FLASK_APP environment variable to app.py
    # For development: flask run
    # For production, use a proper WSGI server like Gunicorn
    port = int(os.environ.get("PORT", 5000))
    app.run(host='0.0.0.0', port=port, debug=True) # debug=True is for development only
