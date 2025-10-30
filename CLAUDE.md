# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**LLM over DNS** is a high-performance Rust DNS server that responds to TXT queries with LLM responses via OpenRouter. It enables querying large language models using standard DNS tools (`dig`, `nslookup`).

### Key Architecture Components

1. **Config** (`src/config.rs`) - Loads configuration from environment variables with priority: Environment > `.env.local` > `.env` > Defaults
2. **Server** (`src/server.rs`) - Main DNS server with graceful lifecycle management
3. **DnsHandler** (`src/dns_handler.rs`) - DNS protocol handling; queries are sent **directly to LLM** (no subdomain parsing)
4. **LlmClient** (`src/llm_client.rs`) - OpenRouter API client with automatic model fallback
5. **Chunker** (`src/chunker.rs`) - Text chunking/dechunking for DNS 255-char TXT record limit

### Important Query Behavior

**DNS queries are sent directly to the LLM as prompts** - there is NO domain parsing or subdomain extraction. Examples:
- `dig @localhost 'hello world' TXT` → sends "hello world" to LLM
- `dig @localhost 'what is rust' TXT` → sends "what is rust" to LLM

## Development Commands

### Build and Run

```bash
# Build project
cargo build

# Build optimized release
cargo build --release

# Run server (requires OpenRouter API key in .env)
cargo run

# Run on non-privileged port (no sudo needed)
DNS_PORT=5353 cargo run

# Run with debug logging
RUST_LOG=debug cargo run
```

### Testing

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run single test
cargo test test_config_from_env_with_api_key

# Run integration tests
cargo test --test integration_test

# Run tests with specific threads
cargo test -- --test-threads=1
```

### Code Quality

```bash
# Format code
cargo fmt

# Check formatting without modifying
cargo fmt --check

# Lint with clippy
cargo clippy

# Strict clippy (CI standard)
cargo clippy -- -D warnings

# Quick compile check (no build)
cargo check
```

### Coverage

```bash
# Generate coverage report (requires cargo-tarpaulin)
cargo install cargo-tarpaulin
cargo tarpaulin --out Html --output-dir coverage

# View coverage
open coverage/index.html
```

### Docker

```bash
# Quick start
docker-compose build
docker-compose up -d

# Using Makefile
make -f Makefile.docker help
make -f Makefile.docker docker-build
make -f Makefile.docker docker-up
make -f Makefile.docker docker-test

# Using helper scripts
./scripts/build.sh
./scripts/deploy.sh start
./scripts/test.sh
```

## Configuration

### Environment Variables

The application uses the following priority order:
1. **Environment variables** (highest priority)
2. **`.env.local`** (for local overrides, gitignored)
3. **`.env`** (for team-shared config)
4. **Defaults** (lowest priority)

Required:
- `OPENROUTER_API_KEY` - Your OpenRouter API key (get free key at openrouter.io)

Optional:
- `OPENROUTER_MODEL` - Comma-separated list of models for automatic fallback (default: `nvidia/nemotron-nano-12b-v2-vl:free`)
- `DNS_PORT` or `PORT` - DNS listening port (default: 53, `PORT` takes precedence)
- `DNS_ADDRESS` or `HOST` - Bind address (default: 0.0.0.0, `HOST` takes precedence)
- `RUST_LOG` - Logging level: debug, info, warn, error (default: info)

### Local Development Setup

```bash
# Copy example config
cp .env.example .env

# Edit .env with your API key
# OPENROUTER_API_KEY=your_key_here

# For local overrides (not committed to git)
cat > .env.local << EOF
OPENROUTER_API_KEY=my-personal-key
DNS_PORT=5353
RUST_LOG=debug
EOF
```

## Code Architecture

### DNS Query Flow

```
DNS Query (dig) → Server (port 53)
                    ↓
                DnsHandler (treat query as prompt)
                    ↓
                LlmClient (call OpenRouter API with fallback)
                    ↓
                Chunker (split response into 255-char TXT records)
                    ↓
                DNS Response (TXT records)
```

### Model Fallback Strategy

The `LlmClient` supports automatic fallback across multiple models (configured via `OPENROUTER_MODEL`). If a model fails or is unavailable, it automatically retries with the next model in the list.

### Key Design Patterns

1. **Async/Await**: Entire application is async using Tokio runtime
2. **Error Handling**: Uses `anyhow::Result` for error propagation, `thiserror` for custom errors
3. **Graceful Shutdown**: Server supports Ctrl+C signal handling with clean shutdown
4. **Configuration Layering**: Multiple config sources with clear precedence
5. **Stateless Handlers**: DnsHandler is stateless and thread-safe (Clone + Send + Sync)

## Testing Strategy

### Current Coverage: 100%

The project maintains strict 100% test coverage enforced by CI/CD. Key test categories:

1. **Unit Tests**: In-module tests for each component
   - `src/config.rs`: Config loading, validation, environment precedence
   - `src/dns_handler.rs`: Query parsing, validation
   - `src/llm_client.rs`: API interaction, error handling, fallback
   - `src/chunker.rs`: Text chunking, dechunking
   - `src/main.rs`: API key masking

2. **Integration Tests**: `tests/integration_test.rs`
   - End-to-end DNS queries
   - Server lifecycle
   - OpenRouter API interaction

### Writing Tests

When adding new code:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_your_feature() {
        // Test implementation
    }

    #[tokio::test]
    async fn test_async_feature() {
        // Async test implementation
    }
}
```

Use `serial_test` for tests that share state:

```rust
use serial_test::serial;

#[test]
#[serial]
fn test_with_shared_state() {
    // Test implementation
}
```

## CI/CD

Three GitHub Actions workflows automate quality checks, builds, and releases:

### CI Pipeline (`.github/workflows/ci.yml`)
Runs on: Push to master, Pull Requests, Manual trigger

1. **Format Check**: `cargo fmt --check`
2. **Linting**: `cargo clippy -- -D warnings`
3. **Security Audit**: `cargo audit` for known vulnerabilities
4. **Tests**: Matrix across Rust stable and beta
5. **Coverage**: `cargo tarpaulin` with 90% minimum + PR comments
6. **All Checks**: Aggregate status gate

### Docker Pipeline (`.github/workflows/docker.yml`)
Runs on: Push to master, Tags (v*.*.*), Pull Requests, Manual trigger

1. **Multi-platform Build**: Linux amd64 + arm64
2. **Push to GHCR**: GitHub Container Registry at `ghcr.io/duyet/llm-over-dns`
3. **Security Scan**: Trivy vulnerability scanning on PRs
4. **Smart Tagging**: Version tags, sha, branch, and latest

### Release Pipeline (`.github/workflows/release.yml`)
Runs on: Tags (v*.*.*), Manual trigger

1. **Create Release**: Automated GitHub release with changelog
2. **Cross-compile**: Build binaries for 6 platforms:
   - Linux: x86_64-gnu, x86_64-musl, aarch64-gnu
   - macOS: x86_64, aarch64 (Apple Silicon)
   - Windows: x86_64
3. **Upload Assets**: Binaries with SHA256 checksums

All CI checks must pass before merge. Coverage threshold is 90%.

## Documentation

Complete documentation is in the `docs/` directory:

- **[Getting Started](docs/GETTING_STARTED.md)** - Quick start guide
- **[Configuration](docs/configuration.md)** - Environment setup
- **[Architecture](docs/ARCHITECTURE.md)** - System design details
- **[API Reference](docs/API.md)** - Rust API documentation
- **[Contributing](docs/CONTRIBUTING.md)** - Development guidelines
- **[Deployment](docs/deployment-docker.md)** - Docker and production deployment

Generate API docs: `cargo doc --open`

## Common Development Tasks

### Adding a New Feature

1. Write tests first (TDD approach)
2. Implement feature
3. Ensure `cargo test` passes with ≥90% coverage
4. Run `cargo fmt` and `cargo clippy -- -D warnings`
5. Update documentation if needed
6. Push to branch and create PR
7. Verify all CI checks pass (format, clippy, tests, coverage, audit)

### Modifying Configuration

When adding new config variables:

1. Update `Config` struct in `src/config.rs`
2. Add parsing logic in `Config::from_env()`
3. Update `.env.example` with new variable
4. Add test cases for new configuration
5. Update `docs/configuration.md`

### Testing DNS Queries

```bash
# Terminal 1: Start server on non-privileged port
DNS_PORT=5353 cargo run

# Terminal 2: Test queries
dig @localhost -p 5353 'hello world' TXT +short
dig @localhost -p 5353 'what is rust' TXT +short
```

### Creating a Release

To create a new release with automated builds:

```bash
# Create and push a version tag
git tag v0.1.0
git push origin v0.1.0

# This triggers:
# 1. Release workflow - Creates GitHub release with binaries
# 2. Docker workflow - Builds and pushes versioned Docker images
```

Release artifacts generated:
- Cross-platform binaries (Linux, macOS, Windows) with SHA256 checksums
- Docker images tagged with version, latest, and sha
- Automated changelog from commit messages

### Debugging

Enable debug logging to see detailed execution flow:

```bash
RUST_LOG=debug cargo run
```

Key debug points:
- `src/main.rs`: Server startup, configuration display
- `src/server.rs`: DNS packet handling
- `src/llm_client.rs`: API requests, model fallback
- `src/dns_handler.rs`: Query processing

## Dependencies

Key dependencies (see `Cargo.toml` for full list):

- `hickory-dns` (v0.25.2) - DNS protocol implementation
- `tokio` (v1.35) - Async runtime
- `reqwest` (v0.11) - HTTP client for OpenRouter API
- `serde`/`serde_json` (v1.0) - JSON serialization
- `anyhow`/`thiserror` (v1.0) - Error handling
- `tracing` (v0.1) - Structured logging
- `dotenvy` (v0.15) - `.env` file loading

## Project Conventions

1. **Error Handling**: Use `anyhow::Result` for application errors, `thiserror` for library errors
2. **Logging**: Use `tracing` macros (`info!`, `debug!`, `error!`) with structured fields
3. **Formatting**: Follow `rustfmt` defaults (run `cargo fmt` before commit)
4. **Documentation**: Use `///` doc comments for public APIs with examples
5. **Testing**: Maintain 100% test coverage; tests live in same file as implementation or `tests/`
6. **Async**: All I/O operations must be async; use `tokio::spawn` for concurrent tasks
