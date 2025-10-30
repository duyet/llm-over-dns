# Docker Quick Start Guide

Get the LLM over DNS service running in Docker in under 5 minutes.

## 1️⃣ Prerequisites

- Docker 20.10+ installed
- Docker Compose 2.0+ installed
- OpenRouter API key (get one free at https://openrouter.ai)

Verify installation:
```bash
docker --version
docker-compose --version
```

## 2️⃣ Setup Configuration

Create `.env` file from template:

```bash
cp .env.example .env
```

Edit `.env` and add your OpenRouter API key:

```env
OPENROUTER_API_KEY=sk_free_...your_api_key_here...
OPENROUTER_MODEL=nvidia/nemotron-nano-12b-v2-vl:free
DNS_PORT=53
RUST_LOG=info
```

**Need an API key?**
1. Visit https://openrouter.ai
2. Sign up (free tier includes free models)
3. Go to Keys → Create Key
4. Copy the key and paste into `.env`

## 3️⃣ Build & Start

### Option A: Using docker-compose (Recommended)

```bash
# Build image
docker-compose build

# Start service
docker-compose up -d

# Check status
docker-compose ps
```

### Option B: Using helper script

```bash
# Make scripts executable (first time only)
chmod +x scripts/*.sh

# Build, start, and test
./scripts/build.sh
./scripts/deploy.sh start
./scripts/test.sh
```

## 4️⃣ Test the DNS Server

### Using `dig` (Linux/macOS)

```bash
dig @localhost hello.world TXT
```

Expected response:
```
; <<>> DiG 9.x.x <<>> @localhost hello.world TXT
; (1 server found)
;; global options: +cmd
;; Got answer:
;; ->>HEADER<<- opcode: QUERY, status: NOERROR, id: xxxxx
;; flags: qr aa rd; QUERY: 1, ANSWER: 1, AUTHORITY: 0, ADDITIONAL: 0

;; QUESTION SECTION:
;hello.world.			IN	TXT

;; ANSWER SECTION:
hello.world.		0	IN	TXT	"LLM response here..."
```

### Using `nslookup` (Windows/macOS/Linux)

```bash
nslookup -type=TXT hello.world localhost
```

### Using `host` (macOS/Linux)

```bash
host -t TXT hello.world 127.0.0.1
```

### Using Docker directly

```bash
docker-compose exec llm-dns dig @localhost hello.world TXT
```

## 5️⃣ Common Operations

### View logs

```bash
# Live logs
docker-compose logs -f llm-dns

# Last 50 lines
docker-compose logs --tail=50 llm-dns

# Logs from last 10 minutes
docker-compose logs --since 10m llm-dns
```

### Check container health

```bash
docker-compose ps
docker inspect llm-over-dns --format='{{json .State.Health}}'
```

### Stop service

```bash
docker-compose down
```

### Restart service

```bash
docker-compose restart
```

### View environment variables

```bash
docker-compose exec llm-dns env
```

## 6️⃣ Troubleshooting

### DNS port already in use

If port 53 is already in use:

```bash
# Option 1: Use alternative port
# Edit docker-compose.yml:
ports:
  - "5353:53/udp"

# Then query on port 5353:
dig @localhost -p 5353 hello.world TXT
```

```bash
# Option 2: Find and stop what's using port 53
sudo lsof -i :53
sudo kill -9 <PID>
```

### Container exits immediately

Check logs for errors:
```bash
docker-compose logs llm-dns
```

Common issues:
- Missing `OPENROUTER_API_KEY` in `.env`
- Invalid API key format
- Port 53 permission issues (need sudo/elevated privileges)

### API key not working

1. Verify the key is set:
```bash
docker-compose exec llm-dns env | grep OPENROUTER_API_KEY
```

2. Check it matches your OpenRouter key
3. Verify the model name is valid:
```bash
# Current model in use:
docker-compose exec llm-dns env | grep OPENROUTER_MODEL
```

### DNS queries timeout

1. Check if service is running:
```bash
docker-compose ps
```

2. Check health status:
```bash
docker-compose exec llm-dns timeout 5 /app/llm-over-dns
```

3. View logs to see errors:
```bash
docker-compose logs -f llm-dns
```

## 7️⃣ Performance Notes

### Image Size

- Final Docker image: ~40-50MB
- Build time: ~3-5 minutes (first build), ~30 seconds (cached builds)

### Memory Usage

- Typical memory: 50-100MB
- With caching: up to 200MB

### DNS Query Latency

- First query: 100-500ms (API call latency)
- Subsequent: Same (no caching yet)

## 8️⃣ Next Steps

### Deploy to production

See [DOCKER.md](./DOCKER.md) for:
- Kubernetes deployment
- Docker Swarm
- Docker registry push
- Security hardening

### Develop with Docker

See [DOCKER.md](./DOCKER.md) for:
- Development workflow
- Debugging techniques
- Custom configurations

### API Documentation

See [OpenRouter API](https://openrouter.ai/docs) for:
- Available models
- API rate limits
- Pricing information

## 📚 Full Documentation

For comprehensive Docker deployment information, see [DOCKER.md](./DOCKER.md)

## 🆘 Need Help?

1. Check logs: `docker-compose logs llm-dns`
2. Read full guide: [DOCKER.md](./DOCKER.md)
3. Check OpenRouter status: https://status.openrouter.ai
