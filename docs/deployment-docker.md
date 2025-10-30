# Docker Deployment Guide

This guide covers building, running, and deploying the LLM over DNS service using Docker.

## Overview

The Docker setup consists of:
- **Multi-stage Dockerfile**: Optimized for minimal image size (<50MB)
- **docker-compose.yml**: Local development and deployment configuration
- **.dockerignore**: Optimized build context exclusions

## Prerequisites

- Docker 20.10+
- Docker Compose 2.0+
- OpenRouter API key (get one at https://openrouter.ai)

## Quick Start

### 1. Set up environment variables

Create a `.env` file in the project root:

```bash
# Required: Your OpenRouter API key
OPENROUTER_API_KEY=your_api_key_here

# Optional: LLM model selection (defaults to free tier)
OPENROUTER_MODEL=nvidia/nemotron-nano-12b-v2-vl:free

# Optional: DNS server configuration
DNS_ADDRESS=0.0.0.0
DNS_PORT=53

# Optional: Logging level (trace, debug, info, warn, error)
RUST_LOG=info
```

### 2. Build the Docker image

```bash
# Build using docker-compose
docker-compose build

# Or build directly with Docker
docker build -t llm-over-dns:latest .
```

**Build Time**: ~3-5 minutes (first build), ~1-2 minutes (subsequent builds with cached layers)

### 3. Run the service

```bash
# Start with docker-compose
docker-compose up -d

# View logs
docker-compose logs -f llm-dns

# Stop the service
docker-compose down
```

### 4. Test the DNS server

```bash
# Test with dig (requires dig/dnsutils installed locally)
dig @localhost "hello.world" TXT

# Or using nslookup
nslookup -type=TXT hello.world localhost

# Or using host command
host -t TXT hello.world 127.0.0.1

# Or using nc (netcat) for raw DNS query
echo -ne '\x00\x00\x01\x00\x00\x01\x00\x00\x00\x00\x00\x00\x05hello\x05world\x00\x00\x10\x00\x01' | nc -u localhost 53
```

## Docker Image Optimization

### Image Size Optimization

The multi-stage build achieves significant size reduction:

1. **Stage 1 (Builder)**: Full Rust toolchain
   - Size: ~2GB (not included in final image)
   - Contains: Rust compiler, dependencies, intermediate build artifacts

2. **Stage 2 (Runtime)**: Minimal Debian slim
   - Size: ~35-40MB (final image)
   - Contains: Binary + minimal runtime dependencies only

### What's included in the final image:

- `debian:bookworm-slim` base image (~70MB base)
- Runtime libraries: `ca-certificates`, `libssl3`
- Compiled binary: `llm-over-dns` (~5-10MB)
- Non-root user for security

### What's excluded via .dockerignore:

- Source code (`src/`)
- Build artifacts (`target/`)
- Development files (`.vscode/`, `.github/`)
- Documentation and examples
- Environment files with secrets

## Port Configuration

### Port 53 (DNS)

DNS requires port 53 (UDP). The Docker setup handles privileged port binding with:

1. **CAP_NET_BIND_SERVICE**: Allows binding to port 53 without root
2. **Security hardening**: Dropped all unnecessary capabilities
3. **Non-root user**: Application runs as `llm` user (uid: 1000)

Note: On some systems, you may need:
```bash
# If you get "permission denied" for port 53
sudo docker-compose up -d

# Or use a non-privileged port for testing
ports:
  - "5353:53/udp"
```

## Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `OPENROUTER_API_KEY` | Required | Your OpenRouter API key |
| `OPENROUTER_MODEL` | `nvidia/nemotron-nano-12b-v2-vl:free` | LLM model to use |
| `DNS_ADDRESS` | `0.0.0.0` | DNS bind address (0.0.0.0 = all interfaces) |
| `DNS_PORT` | `53` | DNS port (must be 53 for standard DNS) |
| `RUST_LOG` | `info` | Log level (trace, debug, info, warn, error) |

### Logging Configuration

Logs are stored with rotation:
- Max file size: 10MB
- Max number of files: 3
- Total max logs: ~30MB

View logs:
```bash
# Live logs
docker-compose logs -f llm-dns

# Last 100 lines
docker-compose logs --tail=100 llm-dns

# Logs since specific time
docker-compose logs --since 10m llm-dns
```

## Health Checks

The Docker container includes a health check that validates DNS functionality:

```bash
# Check container health
docker-compose ps

# Inspect health status
docker inspect llm-over-dns --format='{{json .State.Health}}'
```

Health check details:
- **Interval**: 30 seconds
- **Timeout**: 5 seconds per check
- **Start period**: 10 seconds (grace period after start)
- **Retries**: 3 consecutive failures to mark unhealthy

## Resource Limits

By default, no resource limits are set. To add CPU/memory limits, uncomment in `docker-compose.yml`:

```yaml
deploy:
  resources:
    limits:
      cpus: '1'
      memory: 512M
    reservations:
      cpus: '0.5'
      memory: 256M
```

Recommended values:
- **Memory**: 256-512MB (minimum 256MB)
- **CPU**: 1 core for general use, 0.5 for low-traffic scenarios

## Production Deployment

### Docker Registry

Push to Docker registry:
```bash
# Tag image
docker tag llm-over-dns:latest myregistry/llm-over-dns:latest

# Push to registry
docker push myregistry/llm-over-dns:latest
```

### Kubernetes Deployment

Example Kubernetes manifest:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: llm-over-dns
spec:
  replicas: 1
  selector:
    matchLabels:
      app: llm-over-dns
  template:
    metadata:
      labels:
        app: llm-over-dns
    spec:
      containers:
      - name: llm-dns
        image: llm-over-dns:latest
        ports:
        - containerPort: 53
          protocol: UDP
        env:
        - name: OPENROUTER_API_KEY
          valueFrom:
            secretKeyRef:
              name: llm-dns-secrets
              key: api-key
        - name: RUST_LOG
          value: "info"
        resources:
          limits:
            memory: "512Mi"
            cpu: "1000m"
          requests:
            memory: "256Mi"
            cpu: "500m"
        livenessProbe:
          exec:
            command:
            - /bin/sh
            - -c
            - timeout 2 bash -c 'echo "" | nc -u localhost 53' || exit 1
          initialDelaySeconds: 10
          periodSeconds: 30
---
apiVersion: v1
kind: Service
metadata:
  name: llm-over-dns
spec:
  type: LoadBalancer
  ports:
  - port: 53
    targetPort: 53
    protocol: UDP
  selector:
    app: llm-over-dns
```

### Docker Swarm Deployment

```bash
# Create stack from docker-compose.yml
docker stack deploy -c docker-compose.yml llm-dns

# Check status
docker stack ps llm-dns

# Remove stack
docker stack rm llm-dns
```

## Troubleshooting

### Build fails with "cannot find openssl"

The builder stage caches dependencies. Try:
```bash
docker-compose build --no-cache
```

### Port 53 already in use

```bash
# Check what's using port 53
sudo lsof -i :53

# Use alternative port for testing
# Edit docker-compose.yml ports section to "5353:53/udp"
```

### Container starts but DNS doesn't respond

1. Check logs:
```bash
docker-compose logs llm-dns
```

2. Verify API key:
```bash
docker-compose exec llm-dns env | grep OPENROUTER
```

3. Test DNS endpoint directly from container:
```bash
docker-compose exec llm-dns timeout 5 /app/llm-over-dns
```

### High memory usage

1. Check actual usage:
```bash
docker stats llm-over-dns
```

2. Add memory limit in docker-compose.yml
3. Check for memory leaks in logs

## Security Considerations

1. **Secrets Management**:
   - Never commit `.env` file with API keys
   - Use Docker Compose secrets in production
   - Consider using environment variable management tools

2. **Image Security**:
   - Non-root user (`llm` uid: 1000)
   - Dropped capabilities (only NET_BIND_SERVICE)
   - Read-only root filesystem (can be enabled with `read_only: true`)

3. **Network Security**:
   - Runs on isolated network by default
   - No unnecessary exposed ports
   - Consider using firewall rules to restrict DNS access

## Performance Tuning

### DNS Query Caching

The current implementation doesn't cache LLM responses. For high-traffic scenarios:

1. Add Redis cache layer
2. Cache responses by domain/query pattern
3. Set TTL based on response type

### Concurrent Connections

Adjust Tokio runtime in `src/main.rs`:
```rust
let runtime = tokio::runtime::Builder::new_multi_thread()
    .worker_threads(4)  // Adjust based on CPU cores
    .build()
```

### Batch Processing

Consider implementing query batching for bulk DNS operations.

## Maintenance

### Regular Updates

```bash
# Update base image and rebuild
docker-compose pull
docker-compose build --no-cache
docker-compose up -d
```

### Log Rotation

Logs are automatically rotated (10MB max file, 3 files max). Check volume usage:
```bash
docker system df
```

## Development

### Build optimizations for development:

```dockerfile
# For faster builds during development, create Dockerfile.dev:
FROM rust:latest
WORKDIR /workspace
CMD cargo run
```

Use:
```bash
docker build -f Dockerfile.dev -t llm-over-dns:dev .
docker run -v $(pwd):/workspace llm-over-dns:dev
```

## Additional Resources

- [Docker Documentation](https://docs.docker.com/)
- [Docker Compose Documentation](https://docs.docker.com/compose/)
- [Rust Docker Best Practices](https://hub.docker.com/_/rust)
- [OpenRouter API Documentation](https://openrouter.ai/docs)
