# Thoetd - Refact Self-Hosted API Setup

This repository contains the necessary code, scripts, and instructions to set up a self-hosted Refact AI server and a backend service to interact with it. This setup is intended to be deployed on a Google Cloud VM.

## Overview

The goal is to:
1. Set up a Google Cloud VM.
2. Install necessary dependencies (Git, Python, Docker).
3. Run the Refact self-hosted server using Docker.
4. Run a Python Flask backend service that acts as a bridge between your Android application and the Refact server.
5. Configure firewall rules on Google Cloud.
6. Test the setup.

## Steps

(Details will be filled in as the project progresses)

### 1. Prepare Google Cloud VM
   - Create a new Google Cloud VM instance (e.g., Ubuntu 20.04 LTS or 22.04 LTS).
   - Ensure you have noted down the Public IP Address of the VM.
   - SSH into your VM.

### 2. Clone this Repository
   ```bash
   git clone https://github.com/protae5544/thoetd.git
   cd thoetd
   ```

### 3. Install Dependencies on VM
   - (A script `scripts/install_dependencies.sh` will be provided here)

### 4. Configure and Run Refact Self-Hosted Server
   - (Instructions for running Refact Docker container with your API key will be provided)

### 5. Build and Run Backend Service
   - (Instructions for building and running the Flask backend service, possibly with Docker, will be provided)

### 6. Configure Google Cloud Firewall
   - (Instructions for opening necessary ports will be provided)

### 7. Test the Services
   - (Instructions for testing the endpoints will be provided)

---
*This `README.md` is a work in progress and will be updated.*
