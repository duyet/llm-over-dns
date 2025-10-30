# Environment Configuration Guide

This guide explains how to configure the LLM over DNS server using environment variables and `.env` files.

## Quick Start

1. **Copy the example file**:
   ```bash
   cp .env.example .env
   ```

2. **Edit the `.env` file**:
   ```bash
   nano .env  # or your preferred editor
   ```

3. **Add your OpenRouter API key**:
   ```env
   OPENROUTER_API_KEY=sk-or-v1-your-actual-api-key-here
   ```

4. **Run the server**:
   ```bash
   cargo run
   ```

The server will automatically load configuration from the `.env` file.

---

## Configuration Methods

The server supports four ways to provide configuration (in order of precedence):

### 1. Environment Variables (Highest Priority)
```bash
export OPENROUTER_API_KEY="sk-or-v1-..."
export OPENROUTER_MODEL="nvidia/nemotron-nano-12b-v2-vl:free"
export DNS_PORT=53
export DNS_ADDRESS=0.0.0.0

cargo run
```

### 2. `.env.local` File (Local Overrides)
```bash
# Create .env.local for local-only overrides
# This file is gitignored and takes precedence over .env
cat > .env.local << EOF
OPENROUTER_API_KEY=my-local-dev-key
DNS_PORT=5353
RUST_LOG=debug
EOF

cargo run
```

**Use case**: Personal settings that shouldn't be committed (API keys, local ports).

### 3. `.env` File (Shared Configuration)
```bash
# Create .env file for team-shared defaults
cp .env.example .env

# Edit with your settings
# The server automatically loads this file on startup
cargo run
```

**Use case**: Team-shared configuration that can be committed (if keys are not included).

### 4. Default Values (Fallback)
If not specified, the following defaults are used:
- `OPENROUTER_MODEL`: `nvidia/nemotron-nano-12b-v2-vl:free`
- `DNS_PORT`: `53`
- `DNS_ADDRESS`: `0.0.0.0`
- `RUST_LOG`: `info`

**Note**: `OPENROUTER_API_KEY` is **required** and has no default.

---

## Configuration Options

### OpenRouter API Configuration

#### `OPENROUTER_API_KEY` (Required)
Your OpenRouter API key for authentication.

**Format**: `sk-or-v1-...`

**How to get**:
1. Sign up at [openrouter.ai](https://openrouter.ai/)
2. Navigate to [Keys](https://openrouter.ai/keys)
3. Create a new API key
4. Copy and paste into `.env`

**Example**:
```env
OPENROUTER_API_KEY=sk-or-v1-1234567890abcdef
```

**Security**:
- ⚠️ Never commit `.env` to git (it's in `.gitignore`)
- ⚠️ Never share your API key publicly
- ⚠️ Rotate keys if exposed

#### `OPENROUTER_MODEL` (Optional)
The LLM model to use for generating responses.

**Default**: `nvidia/nemotron-nano-12b-v2-vl:free`

**Popular free models**:
- `nvidia/nemotron-nano-12b-v2-vl:free` - Fast, good quality (recommended)
- `minimax/minimax-m2:free` - Alternative free model
- `meta-llama/llama-3.2-3b-instruct:free` - Smaller, faster

**Example**:
```env
OPENROUTER_MODEL=nvidia/nemotron-nano-12b-v2-vl:free
```

**Browse models**: [openrouter.ai/models](https://openrouter.ai/models?order=newest&supported_parameters=tools&max_price=0)

---

### DNS Server Configuration

#### `DNS_PORT` (Optional)
Port to listen for DNS queries.

**Default**: `53`

**Valid range**: `1-65535`

**Common values**:
- `53` - Standard DNS port (requires root/sudo on Linux)
- `5353` - Alternative DNS port (no root required)
- `8053` - Development port

**Example**:
```env
DNS_PORT=5353
```

**Linux/macOS note**: Ports <1024 require root:
```bash
sudo -E cargo run  # -E preserves environment variables
```

#### `DNS_ADDRESS` (Optional)
Network address to bind the DNS server to.

**Default**: `0.0.0.0` (all interfaces)

**Common values**:
- `0.0.0.0` - Listen on all network interfaces (public access)
- `127.0.0.1` - Listen only on localhost (local testing)
- `192.168.1.x` - Listen on specific network interface

**Example**:
```env
DNS_ADDRESS=127.0.0.1
```

**Security**: Use `127.0.0.1` for development, `0.0.0.0` for production.

---

### Logging Configuration

#### `RUST_LOG` (Optional)
Logging level for the application.

**Default**: `info`

**Valid levels** (from most to least verbose):
- `trace` - Very detailed debugging information
- `debug` - Debugging information
- `info` - General informational messages (recommended)
- `warn` - Warning messages only
- `error` - Error messages only

**Module-specific logging**:
```env
RUST_LOG=info,llm_over_dns=debug
```

**Example**:
```env
RUST_LOG=debug
```

---

## Example Configurations

### Development (Local Testing)
```env
# .env for local development
OPENROUTER_API_KEY=sk-or-v1-dev-key-here
OPENROUTER_MODEL=nvidia/nemotron-nano-12b-v2-vl:free
DNS_PORT=5353
DNS_ADDRESS=127.0.0.1
RUST_LOG=debug
```

Test with:
```bash
cargo run
dig @localhost -p 5353 "test.query" TXT
```

### Production (VPS/Server)
```env
# .env for production
OPENROUTER_API_KEY=sk-or-v1-prod-key-here
OPENROUTER_MODEL=nvidia/nemotron-nano-12b-v2-vl:free
DNS_PORT=53
DNS_ADDRESS=0.0.0.0
RUST_LOG=info
```

Run with:
```bash
sudo -E cargo run --release
```

### Docker Compose
```yaml
# docker-compose.yml
services:
  llm-dns:
    build: .
    ports:
      - "53:53/udp"
    environment:
      - OPENROUTER_API_KEY=${OPENROUTER_API_KEY}
      - OPENROUTER_MODEL=nvidia/nemotron-nano-12b-v2-vl:free
      - DNS_PORT=53
      - DNS_ADDRESS=0.0.0.0
      - RUST_LOG=info
    env_file:
      - .env
```

---

## Verification

### Check Configuration Loading
The server logs configuration on startup:

```bash
cargo run
```

Expected output:
```
2025-10-30T12:00:00.000Z INFO  llm_over_dns: Starting LLM over DNS server
2025-10-30T12:00:00.000Z INFO  llm_over_dns: DNS Server: 0.0.0.0:53
2025-10-30T12:00:00.000Z INFO  llm_over_dns: Model: nvidia/nemotron-nano-12b-v2-vl:free
2025-10-30T12:00:00.000Z INFO  llm_over_dns: Server listening on 0.0.0.0:53
```

### Test Configuration
```bash
# Create test .env
cat > .env << EOF
OPENROUTER_API_KEY=test-key
OPENROUTER_MODEL=test-model
DNS_PORT=5353
DNS_ADDRESS=127.0.0.1
RUST_LOG=debug
EOF

# Run config test
cargo test test_config_from_env -- --nocapture
```

---

## Troubleshooting

### Error: "OPENROUTER_API_KEY environment variable not set"

**Cause**: API key not provided

**Solutions**:
1. Create `.env` file: `cp .env.example .env`
2. Add your API key to `.env`
3. Or export: `export OPENROUTER_API_KEY="your-key"`

### Error: "Invalid DNS_PORT value"

**Cause**: Port is not a valid number

**Solutions**:
1. Check `.env` file: `DNS_PORT=53` (must be a number)
2. Valid range: `1-65535`
3. Remove line to use default (53)

### Error: "Permission denied (os error 13)"

**Cause**: Port <1024 requires root on Linux/macOS

**Solutions**:
1. Use port ≥1024: `DNS_PORT=5353`
2. Or run with sudo: `sudo -E cargo run`
3. Or use `setcap`: `sudo setcap CAP_NET_BIND_SERVICE=+eip target/debug/llm-over-dns`

### .env File Not Loading

**Diagnostic**:
```bash
# Check file exists
ls -la .env

# Check file contents
cat .env

# Check for hidden characters
cat -A .env
```

**Common issues**:
1. File named `.env.example` instead of `.env`
2. File in wrong directory (must be in project root)
3. Invalid syntax (no quotes needed around values)

**Correct format**:
```env
OPENROUTER_API_KEY=sk-or-v1-key
DNS_PORT=5353
```

**Incorrect format**:
```env
OPENROUTER_API_KEY="sk-or-v1-key"  # ❌ No quotes needed
DNS_PORT = 5353                     # ❌ No spaces around =
```

---

## Security Best Practices

### 1. Protect API Keys
```bash
# Ensure .env is in .gitignore
echo ".env" >> .gitignore

# Verify .env is not tracked
git status

# Set restrictive permissions
chmod 600 .env
```

### 2. Use Environment-Specific Keys
- **Development**: Use test/sandbox keys
- **Production**: Use production keys with rate limits
- **CI/CD**: Use secrets management (GitHub Secrets, etc.)

### 3. Rotate Keys Regularly
1. Generate new key at [openrouter.ai/keys](https://openrouter.ai/keys)
2. Update `.env` with new key
3. Restart server
4. Delete old key from OpenRouter dashboard

### 4. Monitor Usage
- Check OpenRouter dashboard for unusual activity
- Set up usage alerts
- Review API logs regularly

---

## Environment Variables Reference

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `OPENROUTER_API_KEY` | ✅ Yes | None | OpenRouter API key |
| `OPENROUTER_MODEL` | No | `nvidia/nemotron-nano-12b-v2-vl:free` | LLM model ID |
| `DNS_PORT` | No | `53` | DNS server port (1-65535) |
| `DNS_ADDRESS` | No | `0.0.0.0` | DNS server bind address |
| `RUST_LOG` | No | `info` | Logging level (trace/debug/info/warn/error) |

---

## Additional Resources

- **OpenRouter Documentation**: [openrouter.ai/docs](https://openrouter.ai/docs)
- **Model List**: [openrouter.ai/models](https://openrouter.ai/models)
- **API Keys**: [openrouter.ai/keys](https://openrouter.ai/keys)
- **Project README**: [README.md](README.md)
- **Deployment Guide**: [DOCKER.md](DOCKER.md)

---

## Quick Reference

```bash
# Setup
cp .env.example .env
nano .env  # Add OPENROUTER_API_KEY

# Run
cargo run

# Test
dig @localhost "test" TXT

# Debug
RUST_LOG=debug cargo run

# Production
sudo -E cargo run --release
```

---

*Last updated: October 30, 2025*
