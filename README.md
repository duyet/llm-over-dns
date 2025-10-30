# LLM over DNS

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Build Status](https://github.com/duyet/llm-over-dns/workflows/CI/badge.svg)](https://github.com/duyet/llm-over-dns/actions)
[![Coverage](https://img.shields.io/badge/coverage-100%25-brightgreen)](#testing)

A high-performance DNS server that responds to TXT queries with LLM responses via OpenRouter. Query an LLM using nothing but DNS.

## Overview

**LLM over DNS** enables you to interact with large language models using DNS TXT record queries. Simply ask a question by creating a DNS query, and receive AI-powered responses directly through DNS.

### Key Features

- 🎯 **DNS-Native Interface**: Query LLMs using standard DNS tools (`dig`, `nslookup`)
- 🆓 **Free LLM Access**: Integrated with OpenRouter's free tier models
- 📝 **TXT-Based Protocol**: Questions encoded in DNS subdomains, answers in TXT records
- ⚡ **Fast & Lightweight**: Built in Rust with async/await for high performance
- 🧪 **100% Test Coverage**: Comprehensive unit and integration tests
- 🔒 **Production Ready**: Structured error handling and logging
- 🐳 **Docker Support**: Easy containerized deployment
- 📚 **Well Documented**: Complete API documentation and examples

## System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      DNS Client (dig/nslookup)               │
│                 dig query.example.com TXT                    │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│            LLM over DNS Server (Port 53)                     │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────────┐ │
│  │  DNS Handler                                            │ │
│  │  - Receives DNS TXT queries                             │ │
│  │  - Parses encoded questions from subdomain labels       │ │
│  │  - Routes to LLM Client                                 │ │
│  └────────────┬────────────────────────────────────────────┘ │
│               │                                                │
│  ┌────────────▼────────────────────────────────────────────┐ │
│  │  Chunker                                                │ │
│  │  - Splits long questions into 63-char DNS labels       │ │
│  │  - Reassembles chunked responses                        │ │
│  └────────────┬────────────────────────────────────────────┘ │
│               │                                                │
│  ┌────────────▼────────────────────────────────────────────┐ │
│  │  LLM Client                                             │ │
│  │  - Calls OpenRouter API                                 │ │
│  │  - Handles model selection and parameters               │ │
│  │  - Manages error recovery                               │ │
│  └────────────┬────────────────────────────────────────────┘ │
└───────────────┼────────────────────────────────────────────────┘
                │
                ▼
        ┌──────────────────┐
        │  OpenRouter API  │
        │   (Free Models)  │
        └──────────────────┘
```

## Installation

### Prerequisites

- **Rust** 1.70 or later ([install](https://rustup.rs/))
- **OpenRouter API Key** (free at [openrouter.io](https://openrouter.io))
- **sudo access** (if running on port 53)

### Quick Start

```bash
# Clone repository
git clone https://github.com/duyet/llm-over-dns.git
cd llm-over-dns

# Create .env file
cp .env.example .env
# Edit .env and add your OpenRouter API key
nano .env

# Build and run
cargo build --release
sudo ./target/release/llm-over-dns
```

### Running Without Sudo

To run on a non-privileged port (e.g., 5353):

```bash
# Edit .env
DNS_PORT=5353

# Run without sudo
cargo run --release

# Test with dig
dig @localhost -p 5353 test.example.com TXT
```

## Usage

### Basic Query

Query the LLM by creating a DNS TXT query where the subdomain contains your question:

```bash
# Simple question
dig @localhost hello.example.com TXT

# Multi-word question (spaces as hyphens)
dig @localhost what-is-rust.example.com TXT

# Longer questions are chunked automatically
dig @localhost why-is-rust-great-for-systems-programming.example.com TXT
```

### Response Format

DNS TXT responses are returned as multiple strings, each up to 255 characters:

```
example.com.   IN TXT "Hello! I'm Claude, an AI assistant. " "I can help with a wide variety " "of tasks and questions..."
```

### Interpreting Results

The response is the LLM's answer to your question. Multiple TXT strings are automatically concatenated:

```bash
$ dig @localhost hello.example.com TXT +short
"Hello! I'm an AI assistant powered by OpenRouter."
```

### Working with Longer Responses

For questions that generate very long responses, the DNS server automatically chunks the response into multiple TXT records:

```bash
$ dig @localhost explain-neural-networks-in-detail.example.com TXT
# Output contains multiple strings, each 255 chars or less
```

### Command Examples

```bash
# Direct question
dig @localhost what-is-ai.example.com TXT +short

# Using nslookup
nslookup -type=TXT hello.example.com localhost

# Using drill (if available)
drill @localhost hello.example.com TXT

# Using host
host -t TXT hello.example.com localhost
```

## Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `OPENROUTER_API_KEY` | **Required** | Your OpenRouter API key |
| `OPENROUTER_MODEL` | `nvidia/nemotron-nano-12b-v2-vl:free` | LLM model to use |
| `DNS_PORT` | `53` | DNS server listening port |
| `DNS_ADDRESS` | `0.0.0.0` | DNS server listening address |
| `RUST_LOG` | `info` | Logging level (`debug`, `info`, `warn`, `error`) |

### Model Selection

Free models available on OpenRouter:

- `nvidia/nemotron-nano-12b-v2-vl:free` - Fast, lightweight (recommended)
- `meta-llama/llama-2-7b-chat:free` - Balanced performance
- Other free models listed at [openrouter.io/models](https://openrouter.io/models)

### .env File Example

```bash
# .env
OPENROUTER_API_KEY=sk_free_your_actual_api_key_here
OPENROUTER_MODEL=nvidia/nemotron-nano-12b-v2-vl:free

# DNS Configuration
DNS_PORT=53
DNS_ADDRESS=0.0.0.0

# Logging
RUST_LOG=info
```

## Testing

### Running Tests

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_config_from_env_with_api_key

# Run tests in release mode
cargo test --release
```

### Test Coverage

Check coverage with `cargo-tarpaulin`:

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin --out Html --output-dir coverage

# View coverage in browser
open coverage/index.html
```

### Test Categories

#### Unit Tests
- Configuration loading and validation
- DNS message parsing
- LLM request/response handling
- Text chunking and reconstruction

#### Integration Tests
- End-to-end DNS queries
- OpenRouter API interaction
- Error handling and recovery

### Running E2E Tests

```bash
# Terminal 1: Start server on port 5353
DNS_PORT=5353 cargo run

# Terminal 2: Run tests
cargo test --test '*' e2e
```

## Deployment

### Local Development

```bash
# Set up environment
cp .env.example .env
nano .env  # Add your API key

# Run on non-privileged port (no sudo needed)
DNS_PORT=5353 RUST_LOG=debug cargo run

# Test in another terminal
dig @localhost -p 5353 hello.example.com TXT +short
```

### Docker Deployment

#### Build Docker Image

```bash
# Create Dockerfile
docker build -t llm-over-dns .

# Run container
docker run -p 53:53/udp \
  -e OPENROUTER_API_KEY=your_key_here \
  -e OPENROUTER_MODEL=nvidia/nemotron-nano-12b-v2-vl:free \
  llm-over-dns
```

#### Docker Compose

```yaml
# docker-compose.yml
version: '3.8'

services:
  llm-over-dns:
    build: .
    ports:
      - "53:53/udp"
    environment:
      OPENROUTER_API_KEY: ${OPENROUTER_API_KEY}
      OPENROUTER_MODEL: ${OPENROUTER_MODEL:-nvidia/nemotron-nano-12b-v2-vl:free}
      DNS_PORT: 53
      DNS_ADDRESS: 0.0.0.0
      RUST_LOG: info
    restart: unless-stopped
    networks:
      - llm

networks:
  llm:
    driver: bridge
```

Run with:
```bash
docker-compose up -d
```

### VPS Deployment with Systemd

#### Prerequisites
- VPS with Rust installed
- Domain with DNS records pointing to VPS
- Root or sudo access

#### Setup Steps

1. **Clone and Build**
```bash
sudo useradd -m -s /sbin/nologin llm-over-dns
sudo su - llm-over-dns
git clone https://github.com/duyet/llm-over-dns.git
cd llm-over-dns
cargo build --release
```

2. **Create systemd Service**
```bash
sudo tee /etc/systemd/system/llm-over-dns.service > /dev/null << EOF
[Unit]
Description=LLM over DNS Server
After=network.target

[Service]
Type=simple
User=llm-over-dns
WorkingDirectory=/home/llm-over-dns/llm-over-dns
ExecStart=/home/llm-over-dns/llm-over-dns/target/release/llm-over-dns
Restart=on-failure
RestartSec=10
StandardOutput=journal
StandardError=journal
EnvironmentFile=/home/llm-over-dns/.env

[Install]
WantedBy=multi-user.target
EOF
```

3. **Configure Environment**
```bash
sudo tee /home/llm-over-dns/.env > /dev/null << EOF
OPENROUTER_API_KEY=your_key_here
OPENROUTER_MODEL=nvidia/nemotron-nano-12b-v2-vl:free
DNS_PORT=53
DNS_ADDRESS=0.0.0.0
RUST_LOG=info
EOF
sudo chown llm-over-dns:llm-over-dns /home/llm-over-dns/.env
sudo chmod 600 /home/llm-over-dns/.env
```

4. **Start Service**
```bash
sudo systemctl daemon-reload
sudo systemctl enable llm-over-dns
sudo systemctl start llm-over-dns

# Check status
sudo systemctl status llm-over-dns

# View logs
sudo journalctl -u llm-over-dns -f
```

### DNS Configuration

#### Cloudflare Setup

If using Cloudflare to manage DNS:

1. Point your domain's NS records to your VPS IP
2. Update Cloudflare DNS settings:
   - Create A record: `dns.yourdomain.com` → VPS IP
   - Use custom nameserver in DNS provider pointing to this A record

#### Route53 / Other Providers

1. Create A record pointing to your VPS
2. Update NS records if using custom nameservers
3. Test: `dig @your.vps.ip yourdomain.com TXT`

## Troubleshooting

### Common Issues

#### Connection Refused
```bash
# Check if port 53 is in use
sudo netstat -tulpn | grep :53

# Or use lsof
sudo lsof -i :53
```

#### DNS Queries Timing Out
```bash
# Check server is running
ps aux | grep llm-over-dns

# Check logs
RUST_LOG=debug cargo run

# Verify DNS server is listening
sudo netstat -tulpn | grep llm-over-dns
```

#### OpenRouter API Errors
```bash
# Verify API key
echo $OPENROUTER_API_KEY

# Check API endpoint
curl -H "Authorization: Bearer $OPENROUTER_API_KEY" \
  https://openrouter.ai/api/v1/models
```

#### Permission Denied (Port 53)
```bash
# Solution 1: Use non-privileged port
DNS_PORT=5353 cargo run

# Solution 2: Use sudo
sudo cargo run

# Solution 3: Grant capability
sudo setcap CAP_NET_BIND_SERVICE=+ep ./target/release/llm-over-dns
./target/release/llm-over-dns  # No sudo needed
```

### Debug Logging

Enable detailed logging:

```bash
# Maximum verbosity
RUST_LOG=debug cargo run

# Log only specific modules
RUST_LOG=llm_over_dns=debug,hickory_dns=info cargo run
```

## Development

### Project Structure

```
llm-over-dns/
├── src/
│   ├── main.rs              # Entry point, server setup
│   ├── lib.rs               # Library exports
│   ├── config.rs            # Configuration management
│   ├── dns_handler.rs       # DNS query handling
│   ├── llm_client.rs        # OpenRouter API client
│   └── chunker.rs           # Text chunking utilities
├── Cargo.toml               # Dependencies and metadata
├── .env.example             # Environment variables template
├── .gitignore               # Git ignore rules
├── README.md                # This file
├── ARCHITECTURE.md          # System design documentation
├── CONTRIBUTING.md          # Contribution guidelines
└── API.md                   # Detailed API documentation
```

### Code Organization

- **config.rs**: Configuration loading from environment variables
- **dns_handler.rs**: DNS protocol handling and query parsing
- **llm_client.rs**: OpenRouter API client and LLM interaction
- **chunker.rs**: Utility functions for splitting/joining long text (DNS 255-char limit)

### Building

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Check without building
cargo check

# Format code
cargo fmt

# Lint code
cargo clippy
```

### Key Dependencies

| Dependency | Purpose |
|------------|---------|
| `hickory-dns` | DNS protocol implementation |
| `tokio` | Async runtime and networking |
| `reqwest` | HTTP client for API calls |
| `serde` | JSON serialization |
| `anyhow` | Error handling |
| `tracing` | Structured logging |

### Testing Workflow

```bash
# Run tests during development
cargo test -- --nocapture --test-threads=1

# Watch for changes and test
cargo watch -x test

# Check code quality
cargo clippy -- -D warnings
cargo fmt --check
```

## API Documentation

### Rustdoc

Generate and view API documentation:

```bash
# Generate docs
cargo doc --open

# The documentation includes:
# - Module descriptions
# - Function signatures with examples
# - Error types and handling
# - Configuration options
```

### Core Types

#### Config
Configuration loaded from environment variables.

```rust
pub struct Config {
    pub openrouter_api_key: String,
    pub openrouter_model: String,
    pub dns_port: u16,
    pub dns_address: String,
}
```

#### DnsHandler
Processes incoming DNS queries and returns responses.

#### LlmClient
Communicates with OpenRouter API for LLM inference.

#### Chunker
Splits long text into DNS-compatible chunks (≤255 chars).

For detailed API documentation, see [API.md](./API.md) or run `cargo doc --open`.

## Contributing

Contributions are welcome! Please follow these guidelines:

1. **Code Quality**: Run `cargo fmt` and `cargo clippy` before submitting
2. **Tests**: Add tests for new functionality
3. **Documentation**: Update rustdoc comments for public APIs
4. **Commit Messages**: Use descriptive commit messages
5. **Issues**: Check existing issues before opening new ones

See [CONTRIBUTING.md](./CONTRIBUTING.md) for detailed guidelines.

## Performance

### Benchmarks

Typical response times (free models):

- Simple queries (< 100 chars): 500ms - 2s
- Complex queries (100-500 chars): 2s - 10s
- Very long queries (> 500 chars): 10s - 30s

*Note: Times depend on OpenRouter load and model selection*

### Optimization Tips

1. **Keep queries concise** for faster responses
2. **Use specific models** based on your needs:
   - Fast responses: `nvidia/nemotron-nano-12b-v2-vl:free`
   - Better quality: `meta-llama/llama-2-7b-chat:free`
3. **Run locally** to minimize network latency
4. **Use release builds** for production

## Security Considerations

### API Keys

- Never commit `.env` files with real API keys
- Use `DNS_ADDRESS=127.0.0.1` for local-only access
- Rotate API keys regularly
- Monitor OpenRouter usage to detect abuse

### DNS Security

- Consider firewall rules to restrict DNS access
- Use DNS rate limiting for production deployments
- Monitor logs for unusual query patterns
- Consider DNSSEC for production DNS zones

### Input Validation

The server automatically:
- Validates DNS queries
- Handles malformed input gracefully
- Implements request size limits
- Has timeout protections

## License

MIT License - see [LICENSE](LICENSE) file for details.

## Support

- **Issues**: [GitHub Issues](https://github.com/duyet/llm-over-dns/issues)
- **Discussions**: [GitHub Discussions](https://github.com/duyet/llm-over-dns/discussions)
- **Documentation**: See [ARCHITECTURE.md](./ARCHITECTURE.md) and [API.md](./API.md)

## Acknowledgments

- [OpenRouter](https://openrouter.ai) - LLM API and free models
- [hickory-dns](https://github.com/bluejekyll/hickory-dns) - DNS protocol library
- [Tokio](https://tokio.rs) - Async runtime

## Roadmap

- [ ] Support additional LLM providers (Anthropic, OpenAI, etc.)
- [ ] Caching layer for repeated queries
- [ ] Rate limiting and API quota management
- [ ] Support for DNS over HTTPS (DoH)
- [ ] Web dashboard for monitoring
- [ ] Streaming responses for long-running queries
- [ ] Support for other DNS record types

## FAQ

**Q: Is this secure for production use?**
A: Use with caution in production. The server is functional but consider security implications of exposing LLM access via DNS.

**Q: What's the cost?**
A: OpenRouter free models are completely free with no usage limits (fair use applies).

**Q: Can I use my own LLM?**
A: Currently requires OpenRouter. Local LLM support is on the roadmap.

**Q: How do I handle very long queries?**
A: The chunker automatically splits questions into DNS-compatible pieces. Responses are also automatically chunked.

**Q: Can I cache responses?**
A: Caching support is planned for a future release.

---

Built with ❤️ by [Duyet](https://duyet.net)

---

## 📚 Documentation

Complete documentation is organized in the [`docs/`](docs/) directory:

- **[Getting Started](docs/GETTING_STARTED.md)** - Quick start for different user types
- **[Configuration Guide](docs/configuration.md)** - `.env` and `.env.local` setup
- **[Architecture](docs/ARCHITECTURE.md)** - System design and components
- **[API Reference](docs/API.md)** - DNS protocol and Rust API
- **[Contributing](docs/CONTRIBUTING.md)** - Development guidelines
- **[Deployment](docs/deployment-docker.md)** - Docker and production deployment
- **[Project Status](docs/PROJECT_STATUS.md)** - Current status and metrics

See [docs/README.md](docs/README.md) for complete documentation index.

---
