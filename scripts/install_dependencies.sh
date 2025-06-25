#!/bin/bash

# Exit immediately if a command exits with a non-zero status.
set -e

echo "Starting dependency installation script..."

# Update package lists
echo "Updating package lists..."
sudo apt-get update -y

# Upgrade installed packages
echo "Upgrading installed packages..."
sudo apt-get upgrade -y

# Install Git (if not already installed, though it should be if cloning this repo)
if ! command -v git &> /dev/null
then
    echo "Installing Git..."
    sudo apt-get install git -y
else
    echo "Git is already installed."
fi

# Install Python 3, pip, and venv
echo "Installing Python 3, pip, and python3-venv..."
sudo apt-get install python3 python3-pip python3-venv -y
echo "Python 3, pip, and python3-venv installation complete."

# Install Docker
echo "Installing Docker..."
# Add Docker's official GPG key:
sudo apt-get install ca-certificates curl -y
sudo install -m 0755 -d /etc/apt/keyrings
sudo curl -fsSL https://download.docker.com/linux/ubuntu/gpg -o /etc/apt/keyrings/docker.asc
sudo chmod a+r /etc/apt/keyrings/docker.asc

# Add the repository to Apt sources:
echo \
  "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.asc] https://download.docker.com/linux/ubuntu \
  $(. /etc/os-release && echo "$VERSION_CODENAME") stable" | \
  sudo tee /etc/apt/sources.list.d/docker.list > /dev/null
sudo apt-get update -y

# Install Docker Engine, CLI, Containerd, and Docker Compose plugin
sudo apt-get install docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin -y
echo "Docker installation complete."

# Add current user to the docker group (requires logout/login or newgrp to take effect)
echo "Adding current user to the docker group..."
sudo usermod -aG docker $USER
echo "User $USER added to the docker group. Please logout and login again, or run 'newgrp docker' in your shell for this to take effect immediately."

# Verify Docker installation
echo "Verifying Docker installation (this might show an error if group permissions haven't taken effect yet)..."
if command -v docker &> /dev/null
then
    docker --version
else
    echo "Docker command not found. This might be due to group permissions not yet applied."
fi


# Install Docker Compose (Standalone version, if docker-compose-plugin is not preferred or for older systems)
# Check if docker-compose is already available via the plugin
if ! docker compose version &> /dev/null; then
    echo "Docker Compose plugin not found or not working, installing standalone Docker Compose..."
    LATEST_COMPOSE_VERSION=$(curl -s https://api.github.com/repos/docker/compose/releases/latest | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')
    if [ -z "$LATEST_COMPOSE_VERSION" ]; then
        echo "Could not fetch latest Docker Compose version. Installing a known version."
        LATEST_COMPOSE_VERSION="v2.20.0" # Fallback to a known version
    fi
    echo "Installing Docker Compose version $LATEST_COMPOSE_VERSION..."
    DESTINATION=/usr/local/bin/docker-compose
    sudo curl -L "https://github.com/docker/compose/releases/download/${LATEST_COMPOSE_VERSION}/docker-compose-$(uname -s | tr '[:upper:]' '[:lower:]')-$(uname -m)" -o $DESTINATION
    sudo chmod +x $DESTINATION
    docker-compose --version
else
    echo "Docker Compose (plugin) is already available."
    docker compose version
fi

echo "Dependency installation script finished."
echo "IMPORTANT: For Docker group changes to take full effect, you might need to logout and log back in, or open a new terminal session after running 'newgrp docker'."
