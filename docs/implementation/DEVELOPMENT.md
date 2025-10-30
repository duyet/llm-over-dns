# Development Guide

This guide explains how to set up your development environment and work with the LLM over DNS project.

## Prerequisites

- Rust 1.70.0 or later
- Git
- GitHub account (for contributions)
- Optional: Docker (for containerized testing)

## Installation

### 1. Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

### 2. Clone Repository

```bash
git clone https://github.com/duyet/llm-over-dns.git
cd llm-over-dns
```

### 3. Install Development Tools

```bash
# Install toolchain components
rustup component add rustfmt clippy

# Install development tools
cargo install cargo-tarpaulin cargo-audit cargo-watch
```

### 4. Install Pre-commit Hooks (Recommended)

Create `.git/hooks/pre-commit`:

```bash
#!/bin/bash
set -e

echo "🔍 Running pre-commit checks..."

# Format check
echo "📝 Checking format..."
cargo fmt --all -- --check

# Clippy lint
echo "🎯 Running clippy..."
cargo clippy --all-targets --all-features -- -D warnings

# Tests
echo "✅ Running tests..."
cargo test --all-features --quiet

# Coverage check
echo "📊 Checking coverage..."
cargo tarpaulin --all-features --fail-under 100 --quiet

echo "✨ All checks passed!"
```

Make it executable:
```bash
chmod +x .git/hooks/pre-commit
```

## Quick Start

### Build

```bash
# Debug build
cargo build

# Release build
cargo build --release
```

### Run

```bash
# With environment variables from .env
cargo run

# Pass arguments
cargo run -- --config path/to/config.toml
```

### Test

```bash
# Run all tests
cargo test --all-features

# Run specific test
cargo test test_name

# Run with output
cargo test --all-features -- --nocapture

# Run doc tests
cargo test --doc
```

### Code Quality

```bash
# Format code
cargo fmt --all

# Check formatting
cargo fmt --all -- --check

# Run clippy (auto-fix)
cargo clippy --fix --all-targets --all-features

# Check clippy without fixing
cargo clippy --all-targets --all-features -- -D warnings

# Security audit
cargo audit

# Coverage check (requires 100%)
cargo tarpaulin --all-features --fail-under 100
```

### Documentation

```bash
# Generate and open docs
cargo doc --no-deps --open

# Build docs with private items
cargo doc --no-deps --document-private-items --open
```

## Project Structure

```
llm-over-dns/
├── src/
│   ├── main.rs           # CLI entry point
│   ├── lib.rs            # Library root
│   └── config.rs         # Configuration module
├── tests/                # Integration tests
├── .github/
│   ├── workflows/        # GitHub Actions workflows
│   ├── CI_CD.md          # CI/CD documentation
│   └── GITHUB_ACTIONS_SETUP.md
├── Cargo.toml            # Project manifest
├── Cargo.lock            # Dependency lock file
├── .env.example          # Example environment variables
└── DEVELOPMENT.md        # This file
```

## Development Workflow

### 1. Create Feature Branch

```bash
git checkout -b feature/my-feature
```

### 2. Make Changes

```bash
# Edit code
vim src/main.rs

# Run tests frequently
cargo test

# Format as you go
cargo fmt --all
```

### 3. Check Quality

```bash
# Run all quality checks
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo tarpaulin --all-features --fail-under 100
```

### 4. Commit Changes

```bash
git add .
git commit -m "feat: add my feature"
```

**Commit Message Format**:
```
<type>(<scope>): <subject>

<body>

<footer>
```

**Types**: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`, `ci`

**Examples**:
```
feat(dns): add support for custom DNS ports
fix(config): handle missing environment variables
docs: update README with examples
test: add coverage for error handling
```

### 5. Push and Create PR

```bash
git push origin feature/my-feature
```

Go to GitHub and create a Pull Request. The PR template will guide you through:
- Describing changes
- Linking related issues
- Verifying testing
- Confirming quality requirements

### 6. CI Verification

GitHub Actions will automatically:
- Check code formatting
- Run Clippy lints
- Execute all tests
- Verify 100% code coverage
- Run security audit

All checks must pass before merging.

## Common Tasks

### Adding a New Dependency

```bash
# Add to Cargo.toml
cargo add dependency-name

# Or specify version
cargo add dependency-name@1.2.3

# Add as dev dependency
cargo add --dev dependency-name
```

Then:
1. Update tests if needed
2. Run `cargo test` to verify
3. Commit the changes

### Updating Dependencies

```bash
# Check outdated
cargo outdated

# Update all
cargo update

# Update specific dependency
cargo update -p dependency-name
```

### Running with Environment Variables

Create `.env` file:
```bash
cp .env.example .env
# Edit .env with your values
```

Then run:
```bash
cargo run
```

### Debugging

```bash
# Run with backtrace
RUST_BACKTRACE=1 cargo run

# Full backtrace
RUST_BACKTRACE=full cargo run

# With logging
RUST_LOG=debug cargo run
```

### Performance Profiling

```bash
# Build with debug symbols
cargo build

# Use profiling tool
cargo flamegraph
# Requires: cargo install flamegraph
```

## Testing Guidelines

### Unit Tests

Write unit tests in the same file:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_something() {
        let result = do_something();
        assert_eq!(result, expected);
    }
}
```

### Integration Tests

Create files in `tests/` directory:

```rust
// tests/my_test.rs
#[test]
fn test_integration() {
    // Test code
}
```

### Test Coverage

100% coverage is required:

```bash
# Generate HTML report
cargo tarpaulin --all-features --out Html

# View report
open tarpaulin-report.html
```

**Coverage Requirements**:
- All branches must be tested
- All error paths must be tested
- All public APIs must have tests

## Documentation Guidelines

### Code Comments

```rust
/// Summary of what this does
///
/// # Arguments
/// * `param1` - Description
///
/// # Returns
/// Description of return value
///
/// # Examples
/// ```
/// let result = my_function(42);
/// assert_eq!(result, expected);
/// ```
pub fn my_function(param1: i32) -> i32 {
    // Implementation
}
```

### Documentation Tests

Doc comments can include tests:

```rust
/// # Examples
/// ```
/// assert_eq!(add(2, 2), 4);
/// ```
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}
```

Run doc tests:
```bash
cargo test --doc
```

## Common Issues

### "Error: toolchain 'X' not found"

```bash
rustup update
rustup component add rustfmt clippy
```

### Tests fail locally but pass in CI

1. Check Rust version matches:
   ```bash
   rustc --version
   ```

2. Clean and rebuild:
   ```bash
   cargo clean
   cargo test --all-features
   ```

3. Check environment variables are set

### Coverage fails unexpectedly

```bash
# Check detailed coverage
cargo tarpaulin --all-features --verbose

# Generate HTML to see uncovered lines
cargo tarpaulin --all-features --out Html
```

### Clippy has suggestions I disagree with

```bash
# Allow specific lint
#[allow(clippy::lint_name)]
fn my_function() {}

// Or in Cargo.toml
[lints.clippy]
lint_name = "allow"
```

## Useful Cargo Commands

```bash
# Build
cargo build                  # Debug build
cargo build --release       # Optimized build

# Test
cargo test                  # Run tests
cargo test --all-features   # With all features
cargo test -- --test-threads=1  # Single-threaded

# Quality
cargo check                 # Quick syntax check
cargo fmt --all             # Format code
cargo clippy                # Lint code
cargo audit                 # Security check

# Documentation
cargo doc --open            # Build and view docs
cargo test --doc            # Test doc examples

# Performance
cargo bench                 # Run benchmarks
cargo profdata              # Profile code

# Maintenance
cargo update                # Update dependencies
cargo outdated              # Show outdated crates
cargo tree                  # Dependency tree
cargo bloat --release       # Binary size analysis
```

## CI/CD Local Simulation

Run the exact same checks as CI:

```bash
#!/bin/bash
set -e

echo "Running CI checks locally..."

echo "✓ Format check"
cargo fmt --all -- --check

echo "✓ Clippy lint"
cargo clippy --all-targets --all-features -- -D warnings

echo "✓ Tests"
cargo test --all-features --verbose

echo "✓ Doc tests"
cargo test --doc

echo "✓ Coverage"
cargo tarpaulin --all-features --fail-under 100

echo "✓ Audit"
cargo audit

echo "✨ All checks passed!"
```

Save as `run_ci_locally.sh`:
```bash
chmod +x run_ci_locally.sh
./run_ci_locally.sh
```

## Performance Tips

### Faster Builds

```bash
# Use sccache for incremental builds
cargo install sccache
export RUSTC_WRAPPER=sccache

# Use mold for linking (Linux)
apt install mold
RUSTFLAGS="-C link-arg=-fuse-ld=mold" cargo build

# Parallel compilation
export CARGO_BUILD_JOBS=$(nproc)
```

### Faster Tests

```bash
# Run tests in parallel (default)
cargo test

# Single-threaded (slower, useful for debugging)
cargo test -- --test-threads=1
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make changes following this guide
4. Ensure all tests pass and coverage is 100%
5. Create a Pull Request
6. Address review comments
7. Merge when CI passes

## References

- [Rust Book](https://doc.rust-lang.org/book/)
- [Cargo Book](https://doc.rust-lang.org/cargo/)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Clippy Lints](https://doc.rust-lang.org/clippy/)
- [GitHub Actions](https://docs.github.com/en/actions)

## Getting Help

- Check existing GitHub issues
- Review CI logs for debugging hints
- Ask questions in pull request discussions
- Refer to project documentation

---

**Happy coding!** 🎉
