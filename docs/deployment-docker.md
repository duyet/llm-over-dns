# Docker & Production Deployment Guide

This guide details how to build, run, and host the **LLM over DNS** server in production environments using Docker and Docker Compose.

---

## 🐳 Docker Architecture & Optimizations

The project includes an optimized multi-stage `Dockerfile` to produce secure, minimal production container images:

1. **Builder Stage**: Compiles the Rust binary using the official Cargo toolchain with full compiler optimizations (`--release`).
2. **Runtime Stage**: Copies only the compiled binary and essential root SSL certificates (`ca-certificates`) into a minimal `debian:bookworm-slim` base image.
* **Security**: Runs under a non-root system user (`app`) to prevent host system exposure.
* **Final Image Size**: `~45MB`.

---

## 🚀 Quick Start with Docker Compose

Using Docker Compose is the recommended way to deploy and maintain the server.

### 1. Create a `docker-compose.yml`
Create a `docker-compose.yml` file in your deployment directory:

```yaml
version: '3.8'

services:
  llm-dns:
    image: ghcr.io/duyet/llm-over-dns:latest
    container_name: llm-dns
    restart: unless-stopped
    ports:
      # Maps host port 53 (UDP) to container port 5353
      - "53:5353/udp"
    environment:
      - ANYROUTER_API_KEY=${ANYROUTER_API_KEY}
      - ANYROUTER_MODEL=google/gemini-2.5-flash-lite,meta/llama-3.2-3b-instruct
      - DNS_PORT=5353
      - DNS_ADDRESS=0.0.0.0
      - RUST_LOG=info
    env_file:
      - .env
```

### 2. Configure Environment Variables
Create a local `.env` file alongside your compose file:

```env
# Required: AnyRouter API Key (Recommended)
ANYROUTER_API_KEY=sk-ar-v1-your-key-here

# Or OpenRouter:
# OPENROUTER_API_KEY=sk-or-v1-your-key-here
```

### 3. Launch the Stack
Start the container in detached background mode:

```bash
# Start
docker compose up -d

# Check startup logs
docker compose logs -f

# Stop
docker compose down
```

---

## 🐳 Manual Docker Commands

If you prefer building and running the container manually:

### 1. Build the Container
```bash
docker build -t llm-over-dns:latest .
```

### 2. Run the Container
```bash
docker run -d \
  --name llm-dns \
  --restart unless-stopped \
  -p 53:5353/udp \
  -e ANYROUTER_API_KEY="sk-ar-v1-yourkey" \
  -e DNS_PORT=5353 \
  llm-over-dns:latest
```

---

## 🛡️ Production Deployment & Network Routing

Deploying DNS servers in production requires attention to network binding:

### 1. Binding Port 53
Port `53` is a privileged port on Linux hosts. Rather than running the Docker container with root privileges, we bind the host UDP port `53` directly to a high, non-privileged port inside the container (e.g. `-p 53:5353/udp`).

### 2. Systemd-resolved Conflicts
Most Linux distributions (like Ubuntu Server) run a local resolver (`systemd-resolved`) that already binds to port `53` on the loopback interface.

To verify if port `53` is blocked:
```bash
sudo lsof -i :53
```

If `systemd-resolved` is blocking the port, edit `/etc/systemd/resolved.conf`:
```ini
[Resolve]
DNSStubListener=no
```
Then restart the service:
```bash
sudo systemctl restart systemd-resolved
```

### 3. Firewall Configuration
Ensure that UDP port `53` is explicitly allowed in your cloud provider's firewall policy (and on-server firewall like UFW):
```bash
sudo ufw allow 53/udp
```
