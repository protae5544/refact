# Use an official Python runtime as a parent image
FROM python:3.9-slim

# Set the working directory in the container
WORKDIR /app

# Copy the requirements file into the container at /app
COPY requirements.txt .

# Install any needed packages specified in requirements.txt
RUN pip install --no-cache-dir -r requirements.txt

# Copy the current directory contents into the container at /app
COPY . .

# Make port 5000 available to the world outside this container
EXPOSE 5000

# Define environment variables (can be overridden at runtime)
ENV FLASK_APP=app.py
ENV FLASK_RUN_HOST=0.0.0.0
ENV FLASK_RUN_PORT=5000
# You'll need to pass REFACT_SERVER_URL and REFACT_API_KEY at runtime, e.g., via docker run -e or docker-compose.yml
# ENV REFACT_SERVER_URL="http://refact_server_container_name:8008" # Example if refact runs in another container
# ENV REFACT_API_KEY="your_refact_api_key_here"

# Command to run the application
CMD ["flask", "run"]
