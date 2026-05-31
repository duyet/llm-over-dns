# Contributing Guide

Thank you for contributing to **LLM over DNS**! We appreciate your help in making this project even better.

---

## 🛠️ Local Development Setup

To set up a local development environment:

1. **Install Prerequisites**:
   * Rust 1.70 or later (via [rustup.rs](https://rustup.rs/))
   * Standard network testing tools (like `dig` or `nslookup`)

2. **Clone and Build**:
   ```bash
   git clone https://github.com/duyet/llm-over-dns.git
   cd llm-over-dns
   cargo build
   ```

3. **Configure Local Environment**:
   Create a local override config file:
   ```bash
   cp .env.example .env.local
   # Edit .env.local and add your ANYROUTER_API_KEY
   ```

---

## 🧪 Testing Guidelines

The project maintains high code quality and test coverage standards. All contributions must pass the local test suites.

```bash
# Run unit and integration tests
cargo test

# Run tests with stdout output
cargo test -- --nocapture
```

### Smoke Testing
To verify live AnyRouter gateway integration:
```bash
ANYROUTER_API_KEY=sk-ar-v1-... cargo test --lib llm_client::tests::test_anyrouter_smoke -- --nocapture
```

---

## 🎨 Code Style & Quality Gates

Before submitting a Pull Request, please ensure your changes conform to formatting and quality requirements:

### 1. Formatting
The project strictly enforces standard Rust formatting:
```bash
cargo fmt -- --check
```
To auto-format changes:
```bash
cargo fmt
```

### 2. Linting
Verify there are no compiler warnings or common code smell warnings:
```bash
cargo clippy -- -D warnings
```

---

## 📥 Pull Request Workflow

1. **Fork** the repository and create your feature branch:
   ```bash
   git checkout -b feature/amazing-feature
   ```
2. **Commit** your changes with clear, descriptive commit messages.
3. **Format & Test** to ensure the build compiles, lints pass, and tests succeed.
4. **Push** to your fork and submit a **Pull Request** to the `master` branch.
