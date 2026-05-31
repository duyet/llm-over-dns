# Getting Started with LLM over DNS

Quick reference guide to get up and running with LLM over DNS.

## Choose Your Path

### I want to use the service
**→ Start here: [README.md](./README.md)**

Read:
1. Overview & Features (5 min)
2. Installation (5 min)
3. Usage Examples (10 min)
4. Configuration (5 min)

### I want to deploy it
**→ Start here: [README.md](./README.md) → Deployment Section**

Read:
1. Docker Deployment (10 min)
2. VPS with Systemd (15 min)
3. DNS Configuration (10 min)
4. Troubleshooting (5 min)

### I want to develop features
**→ Start here: [CONTRIBUTING.md](./CONTRIBUTING.md)**

Read:
1. Getting Started (5 min)
2. Development Setup (10 min)
3. Making Changes (5 min)
4. Testing (10 min)
5. Code Quality (5 min)

### I want to understand the architecture
**→ Start here: [ARCHITECTURE.md](./ARCHITECTURE.md)**

Read:
1. System Overview (10 min)
2. Component Architecture (15 min)
3. Data Flow (10 min)
4. Design Decisions (10 min)

### I want the complete API reference
**→ Start here: [API.md](./API.md)**

Read:
1. DNS Protocol API (10 min)
2. Library API (15 min)
3. Configuration API (5 min)
4. Examples (10 min)

## Quick Reference

### Installation (1 minute)

```bash
git clone https://github.com/duyet/llm-over-dns.git
cd llm-over-dns
cp .env.example .env
# Edit .env with your AnyRouter (Recommended) or OpenRouter API key
cargo build --release
sudo ./target/release/llm-over-dns
```

### First Query (1 minute)

```bash
# In another terminal
dig @localhost hello.example.com TXT +short
```

### Docker (1 minute)

```bash
docker build -t llm-over-dns .
docker run -p 53:53/udp \
  -e ANYROUTER_API_KEY=your_key \
  llm-over-dns
```

## Common Tasks

### Run Tests

```bash
cargo test
```

### Check Code Quality

```bash
cargo fmt
cargo clippy -- -D warnings
```

### Generate Documentation

```bash
cargo doc --open
```

### Debug Issues

```bash
RUST_LOG=debug cargo run
```

### Check Coverage

```bash
cargo install cargo-tarpaulin
cargo tarpaulin --out Html
open coverage/index.html
```

## Directory Structure

```
llm-over-dns/
├── README.md                    ← Start here (user guide)
├── ARCHITECTURE.md              ← System design
├── CONTRIBUTING.md              ← Development guide
├── API.md                       ← API reference
├── GETTING_STARTED.md           ← This file
├── DOCUMENTATION_SUMMARY.md     ← Overview of all docs
├── Cargo.toml                   ← Dependencies
├── .env.example                 ← Config template
└── src/
    ├── lib.rs                   ← Library root
    ├── config.rs                ← Configuration
    ├── dns_handler.rs           ← DNS protocol
    ├── llm_client.rs            ← LLM API client
    ├── chunker.rs               ← Text chunking
    └── main.rs                  ← Binary entry point
```

## Documentation Map

| Document | Purpose | Audience | Read Time |
|----------|---------|----------|-----------|
| [README.md](./README.md) | User guide & quick start | Everyone | 20 min |
| [ARCHITECTURE.md](./ARCHITECTURE.md) | System design & reference | Developers, Architects | 25 min |
| [CONTRIBUTING.md](./CONTRIBUTING.md) | Development guidelines | Contributors | 20 min |
| [API.md](./API.md) | Complete API reference | Developers, API users | 25 min |
| [DOCUMENTATION_SUMMARY.md](./DOCUMENTATION_SUMMARY.md) | Overview of all docs | Everyone | 10 min |
| [Cargo.toml](./Cargo.toml) | Dependencies & metadata | Developers | 5 min |
| `.env.example` | Configuration template | Operators | 2 min |

## Frequently Asked Questions

**Q: How do I set up AnyRouter or OpenRouter API?**
A: Create an account at [anyrouter.dev](https://anyrouter.dev) (Recommended) or [openrouter.ai](https://openrouter.ai), get an API key, and add it to your `.env` file.

**Q: Can I run without sudo?**
A: Yes! Use `DNS_PORT=5353` to use non-privileged port.

**Q: What's the difference between free models?**
A: Check ARCHITECTURE.md → Models section for comparison.

**Q: How do I contribute?**
A: Follow CONTRIBUTING.md workflow.

**Q: Where do I report bugs?**
A: Create GitHub issue (see CONTRIBUTING.md).

**Q: Is this production ready?**
A: Yes, with proper configuration and monitoring.

## Key Concepts

### DNS Query Format
```
question-text.example.com TXT
```

### DNS Response Format
```
TXT "Response part 1 (≤255 chars)"
TXT "Response part 2 (≤255 chars)"
```

### Configuration
Set environment variables or create .env file:
```
ANYROUTER_API_KEY=your_key
ANYROUTER_MODEL=google/gemini-2.5-flash-lite,meta/llama-3.2-3b-instruct
DNS_PORT=53
DNS_ADDRESS=0.0.0.0
```

## Next Steps

1. **Read README.md** - Understand the project
2. **Follow installation** - Get it running locally
3. **Try basic query** - Use `dig` to test
4. **Review configuration** - Customize settings
5. **Explore deployment** - Choose your deployment model

## Support

- **Issues**: GitHub Issues
- **Discussions**: GitHub Discussions
- **Documentation**: See links above

## Learning Resources

- [DNS Protocol](https://tools.ietf.org/html/rfc1035) - DNS specification
- [OpenRouter Docs](https://openrouter.ai/docs) - API documentation
- [Rust Book](https://doc.rust-lang.org/book/) - Rust language
- [Tokio Guide](https://tokio.rs/) - Async runtime

---

Happy hacking! 🚀
