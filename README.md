# Thoetd - Refact Self-Hosted API Setup

This repository contains the necessary code, scripts, and instructions to set up a self-hosted Refact AI server and a backend service to interact with it. This setup is intended to be deployed on a Google Cloud VM.

## Overview

The goal is to:
1. Set up a Google Cloud VM.
2. Install necessary dependencies (Git, Python, Docker, Docker Compose).
3. Configure and run the Refact self-hosted server and the backend API service using Docker Compose.
4. Configure firewall rules on Google Cloud.
5. Test the setup.

## Steps

### 1. Prepare Google Cloud VM
   - Create a new Google Cloud VM instance (e.g., Ubuntu 20.04 LTS or 22.04 LTS).
     - When creating, under "Firewall", allow HTTP and HTTPS traffic initially. This can be refined later.
     - Choose a machine type with adequate resources (CPU, RAM). GPU is optional and requires further setup if Refact is to use it.
   - Note down the **Public IP Address** of your VM.
   - SSH into your VM.

### 2. Clone This Repository and Install Dependencies
   - **Inside your VM's terminal:**
   - Clone this repository:
     ```bash
     git clone https://github.com/protae5544/thoetd.git
     cd thoetd
     ```
   - Make the installation script executable and run it:
     ```bash
     chmod +x scripts/install_dependencies.sh
     sudo ./scripts/install_dependencies.sh
     ```
     *(Running with sudo if the script itself doesn't use sudo for apt commands)*
     Alternatively, if the script handles sudo internally for commands that need it:
     ```bash
     chmod +x scripts/install_dependencies.sh
     ./scripts/install_dependencies.sh
     ```
   - **Important:** After the script finishes, you **must** either logout and log back in, or run `newgrp docker` in your current session. This is for Docker group permissions to take effect, allowing you to run `docker` commands without `sudo`. Test with `docker ps`. If it fails with a permission error, you haven't successfully applied the group change.

### 3. Prepare Environment File for Docker Compose
   - **Inside the `thoetd` directory on your VM:**
   - Create a file named `.env`.
   - Add your Refact API key to this file. This key is `1xSVoqlBp923mC7fyQaIQVJU`.
     ```env
     # Contents of .env file
     REFACT_API_KEY=1xSVoqlBp923mC7fyQaIQVJU
     ```
   - The `.gitignore` file in this repository is already configured to ignore `.env` files, so this key won't be accidentally committed.

### 4. Configure and Run Services with Docker Compose
   - **Ensure you are in the `thoetd` directory in your VM terminal.**
   - Start all services (Refact server and Backend service) using Docker Compose:
     ```bash
     docker compose up -d
     ```
     The `-d` flag runs the containers in detached mode (in the background).
   - **Check container status:**
     ```bash
     docker compose ps
     ```
     You should see `refact_server_container` and `backend_service_container` running.
   - **View logs (especially on first run or if there are issues):**
     ```bash
     docker compose logs -f refact_server_container
     docker compose logs -f backend_service_container
     ```
     The Refact server might take some time to initialize or download models on its first run.
   - **To stop the services:**
     ```bash
     docker compose down
     ```

   **Notes on Services:**
   - **Refact Server (`refact_server_container`):**
     - Uses the `smallcloud/refact:latest` Docker image (verify if a more specific tag is recommended by Refact for your use case).
     - VM's port `8008` is mapped to the container's port `8008`.
     - Attempts to use the `REFACT_API_KEY` from the `.env` file. The exact mechanism (if this env var is directly used by the image's entrypoint or if it needs to be passed as a command-line argument via `command:` in `docker-compose.yml`) **must be verified with official Refact documentation for self-hosted setups.**
     - Uses a Docker named volume `refact_dot_cache` for persisting downloaded models (e.g_., in `/root/.cache/refact` inside the container).
     - GPU configuration lines are present but commented out in `docker-compose.yml`. For GPU usage, ensure NVIDIA drivers and the NVIDIA Container Toolkit are installed on the VM, then uncomment relevant GPU sections in `docker-compose.yml`.
   - **Backend Service (`backend_service_container`):**
     - Built from `backend_service/Dockerfile`.
     - VM's port `5000` is mapped to the container's port `5000` (Flask app).
     - Connects to the Refact server via `http://refact_server_container:8008` (Docker's internal network).
     - The Flask app (`backend_service/app.py`) contains **placeholders** for the Refact API endpoint (e.g., `/v1/completions`) and the structure of the request/response payload. **These placeholders MUST be verified against the actual API provided by your self-hosted Refact server instance and updated in `app.py` if necessary.**

### 5. Configure Google Cloud Firewall
   - Allow incoming TCP traffic to ports `5000` (for your backend service) and potentially `8008` (if you want direct access to Refact server, though usually access via the backend is preferred).
   - In the Google Cloud Console:
     1. Navigate to "VPC network" -> "Firewall".
     2. Click "CREATE FIREWALL RULE".
     3. **Name:** e.g., `allow-thoetd-services`
     4. **Network:** Select the network your VM is on.
     5. **Priority:** `1000` (default).
     6. **Direction of traffic:** `Ingress`.
     7. **Action on match:** `Allow`.
     8. **Targets:** `Specified target tags`. Add a network tag (e.g., `thoetd-vm`) to your VM in its settings, and use that same tag here. (Using tags is more specific than `All instances`).
     9. **Source filter:** `IP ranges`.
     10. **Source IPv4 ranges:**
         - For initial testing: `0.0.0.0/0` (allows all IPs - be cautious).
         - For better security later: Restrict to your specific IP address or known IP ranges.
     11. **Protocols and ports:**
         - Select `Specified protocols and ports`.
         - Check `TCP` and enter `5000,8008` in the "Ports" field.
     12. Click "Create".

### 6. Test the Services

   **a) Test Backend Service Liveness:**
   From your local machine (not the VM), open a web browser or use `curl`:
   `http://<YOUR_VM_PUBLIC_IP>:5000/`
   You should see: `Backend service for Refact is running!`

   **b) Test Refact Chat Endpoint (via Backend Service):**
   Use a tool like Postman or `curl` from your local machine.

   Example using `curl` (replace `<YOUR_VM_PUBLIC_IP>`):
   ```bash
   curl -X POST http://<YOUR_VM_PUBLIC_IP>:5000/api/refact/chat \
   -H "Content-Type: application/json" \
   -d '{
     "prompt": "def hello_world_python():",
     "model": "smallcloud/Refact-1_6B-fim" # This model name is an EXAMPLE, verify correct available model names
   }'
   ```

   **Expected Response:**
   A JSON response from the backend service, including the Refact server's output. The structure of `raw_refact_response` and the extracted `response` field in `backend_service/app.py` depends on the actual API of *your* Refact server instance. **You will likely need to inspect the `raw_refact_response` from your first successful calls and adjust the parsing logic in `backend_service/app.py` to correctly extract the desired completion text.**

   **Troubleshooting:**
   - **Container Logs:** `docker compose logs -f refact_server_container` and `docker compose logs -f backend_service_container` are your primary tools.
   - **Refact Server Initialization:** The `refact_server_container` might take considerable time on its first startup to download models. Monitor its logs.
   - **API Key for Refact Server:** The method of providing the API key to the `smallcloud/refact` Docker image (currently assumed as `REFACT_API_KEY` env var) is critical. If the server fails to authenticate or unlock features, this is a key area to investigate based on Refact's official documentation for Pro plans / self-hosting.
   - **Refact API Endpoint/Payload:** The endpoint (`REFACT_COMPLETION_ENDPOINT`) and payload structure in `backend_service/app.py` are common defaults but **must be verified**. The Refact self-hosted server might use a different path or expect a different JSON structure.
   - **Firewall:** Double-check Google Cloud firewall rules and ensure your VM's network tag matches the firewall rule's target tag.
   - **Connectivity from Backend to Refact:** If `backend_service_container` logs show errors connecting to `http://refact_server_container:8008`, ensure `refact_server_container` is running and healthy.

---
This `README.md` provides a setup guide. Always refer to the **official Refact documentation** for the most accurate and up-to-date information on configuring their self-hosted server, especially concerning API key usage, model management, and Docker image specifics for Pro plans.
