# Contributing to LLM over DNS

Thank you for your interest in contributing to LLM over DNS! This document provides guidelines and instructions for contributing to the project.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Making Changes](#making-changes)
- [Testing](#testing)
- [Code Quality](#code-quality)
- [Commit Messages](#commit-messages)
- [Pull Requests](#pull-requests)
- [Reporting Issues](#reporting-issues)

## Code of Conduct

This project adheres to the Rust Community Code of Conduct. By participating, you are expected to uphold this code. Please report unacceptable behavior to the project maintainers.

## Getting Started

### Prerequisites

- Rust 1.70 or later (install from [rustup.rs](https://rustup.rs/))
- Git
- An OpenRouter API key (free at [openrouter.io](https://openrouter.io))

### Fork and Clone

```bash
# Fork the repository on GitHub
# Clone your fork
git clone https://github.com/your-username/llm-over-dns.git
cd llm-over-dns

# Add upstream remote for syncing
git remote add upstream https://github.com/duyet/llm-over-dns.git
```

## Development Setup

### Environment Configuration

```bash
# Copy example environment file
cp .env.example .env

# Edit with your OpenRouter API key
nano .env
# Set: OPENROUTER_API_KEY=your_key_here
# Optional: Change DNS_PORT=5353 (no sudo needed)
```

### Build Project

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Check without building
cargo check
```

### Run Tests

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_config_from_env

# Run in release mode
cargo test --release
```

### Run Development Server

```bash
# Terminal 1: Start server on non-privileged port
DNS_PORT=5353 RUST_LOG=debug cargo run

# Terminal 2: Test the server
dig @localhost -p 5353 hello.example.com TXT +short
```

## Making Changes

### Workflow

1. **Create a feature branch**
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. **Make your changes**
   - One feature/fix per branch
   - Keep commits atomic and logical
   - Write tests for new functionality

3. **Ensure code quality**
   ```bash
   # Format code
   cargo fmt

   # Check for common mistakes
   cargo clippy -- -D warnings

   # Run tests
   cargo test
   ```

4. **Push to your fork**
   ```bash
   git push origin feature/your-feature-name
   ```

5. **Create a Pull Request**
   - Describe what you changed and why
   - Link any related issues
   - Ensure all checks pass

### Code Organization

The project is organized as follows:

```
src/
├── config.rs        # Configuration management
├── dns_handler.rs   # DNS protocol handling
├── llm_client.rs    # OpenRouter API client
├── chunker.rs       # Text chunking utilities
├── lib.rs           # Library root
└── main.rs          # Binary entry point
```

### Adding a New Module

When adding a new module:

1. Create the module file in `src/`
2. Add `pub mod your_module;` to `src/lib.rs`
3. Add `pub use your_module::*;` for public items
4. Add comprehensive rustdoc comments
5. Add unit tests in the same file
6. Update `ARCHITECTURE.md` if architectural impact

### Naming Conventions

- **Files**: `snake_case.rs`
- **Functions**: `snake_case()`
- **Types**: `PascalCase`
- **Constants**: `SCREAMING_SNAKE_CASE`
- **Modules**: `snake_case`

## Testing

### Test Structure

Tests are organized in the same files as the code they test:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_descriptive_name() {
        // Arrange
        let input = "test input";

        // Act
        let result = function_under_test(input);

        // Assert
        assert_eq!(result, expected_value);
    }
}
```

### Test Requirements

- **Unit tests**: Test individual functions and types
- **Integration tests**: Test module interactions
- **Coverage**: Aim for >90% coverage on new code
- **Edge cases**: Test boundary conditions and error cases

### Writing Good Tests

```rust
#[test]
fn test_chunker_respects_256_byte_limit() {
    let chunker = Chunker::new();
    let text = "a".repeat(1000);
    let chunks = chunker.chunk(&text, 255);

    for chunk in chunks {
        assert!(chunk.len() <= 255, "Chunk exceeds limit: {}", chunk.len());
    }
}

#[test]
fn test_chunker_preserves_content() {
    let chunker = Chunker::new();
    let original = "Hello, DNS world!";
    let chunks = chunker.chunk(original, 255);
    let reconstructed = chunker.dechunk(&chunks);

    assert_eq!(original, reconstructed);
}
```

### Running Coverage

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate coverage
cargo tarpaulin --out Html --output-dir coverage

# View in browser
open coverage/index.html
```

## Code Quality

### Formatting

The project uses `rustfmt` for consistent formatting:

```bash
# Format all code
cargo fmt

# Check formatting without changes
cargo fmt -- --check
```

### Linting

Use `clippy` to catch common mistakes:

```bash
# Run clippy
cargo clippy

# Run with all warnings as errors
cargo clippy -- -D warnings

# Fix automatically where possible
cargo clippy --fix --allow-dirty
```

### Documentation

All public items must be documented with rustdoc comments:

```rust
/// Brief description of what this does.
///
/// More detailed explanation if needed. Explain the purpose, behavior,
/// and any important considerations.
///
/// # Arguments
///
/// * `param1` - Description of param1
/// * `param2` - Description of param2
///
/// # Returns
///
/// Description of return value
///
/// # Errors
///
/// Describes errors that can occur
///
/// # Examples
///
/// ```
/// let result = my_function("arg");
/// assert_eq!(result, expected);
/// ```
///
/// # Panics
///
/// Describes what can cause panics (if applicable)
pub fn my_function(param1: &str, param2: u32) -> Result<String> {
    // Implementation
}
```

### Documentation Generation

```bash
# Generate and open documentation
cargo doc --open

# Generate with private items included
cargo doc --document-private-items --open
```

## Commit Messages

Follow conventional commits format:

```
type(scope): subject

body

footer
```

### Types

- `feat`: A new feature
- `fix`: A bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting, missing semicolons, etc.)
- `refactor`: Code refactoring without feature changes
- `perf`: Performance improvements
- `test`: Adding or updating tests
- `chore`: Build, CI/CD, dependency updates

### Examples

```
feat(dns-handler): add support for AAAA queries

- Implement AAAA record parsing
- Update handler to route IPv6 queries
- Add comprehensive tests for IPv6

Fixes #123

feat(config): validate port range

Ensure DNS port is between 1024 and 65535.
Previously allowed invalid port numbers.

fix(chunker): handle UTF-8 boundary correctly

Properly split UTF-8 sequences at chunk boundaries
instead of breaking multi-byte characters.

docs(readme): add Docker deployment section

Added Docker and docker-compose examples
for easier deployment.
```

### Guidelines

- Use imperative mood ("add feature" not "added feature")
- Keep subject line under 50 characters
- Capitalize the subject line
- Describe the "why" not just the "what"
- Reference issues and PRs

## Pull Requests

### Before Creating PR

- [ ] Code compiles without warnings: `cargo build`
- [ ] Code is formatted: `cargo fmt`
- [ ] Lint passes: `cargo clippy -- -D warnings`
- [ ] Tests pass: `cargo test`
- [ ] Documentation updated: `cargo doc --no-deps`
- [ ] Tests added for new features
- [ ] CHANGELOG.md updated (if applicable)

### PR Description Template

```markdown
## Description

Brief description of what this PR does.

## Motivation

Why is this change needed? What problem does it solve?

## Testing

How was this tested? Include steps to reproduce if applicable.

## Checklist

- [ ] Code compiles without warnings
- [ ] `cargo fmt` passes
- [ ] `cargo clippy -- -D warnings` passes
- [ ] Tests added/updated
- [ ] Documentation updated
- [ ] CHANGELOG.md updated

## Related Issues

Closes #XXX
```

### Review Process

- At least one maintainer review required
- All CI checks must pass
- Discussion of any requested changes
- Approval and merge by maintainer

## Reporting Issues

### Bug Reports

Use the GitHub issue tracker with the following template:

```markdown
## Description

Clear and concise description of the bug.

## Steps to Reproduce

1. Set up with X configuration
2. Run command Y
3. Observe error Z

## Expected Behavior

What should happen?

## Actual Behavior

What actually happens?

## Environment

- Rust version: `rustc --version`
- OS: macOS/Linux/Windows
- DNS server version: `git log -1 --oneline`

## Additional Context

Any additional information that helps.
```

### Feature Requests

```markdown
## Description

Clear description of the feature.

## Motivation

Why is this feature needed?

## Examples

Example usage or API design.

## Alternatives Considered

Other approaches considered.
```

## Development Tips

### Useful Commands

```bash
# Watch for changes and run tests
cargo watch -x test

# Generate documentation and watch
cargo watch -x 'doc --no-deps --open'

# Run specific test with output
cargo test test_name -- --nocapture --exact

# Show test output even for passed tests
cargo test -- --nocapture --test-threads=1

# Profile binary size
cargo build --release
ls -lh target/release/llm-over-dns
```

### Debugging

```bash
# Enable verbose logging
RUST_LOG=debug cargo run

# Log specific modules
RUST_LOG=llm_over_dns=debug,hickory_dns=info cargo run

# Use rust-gdb for debugging
rust-gdb ./target/debug/llm-over-dns
```

### Performance Profiling

```bash
# Time a test
time cargo test test_name

# Profile with perf (Linux)
cargo install flamegraph
cargo flamegraph --bin llm-over-dns
```

## Project Structure

### Key Files

- `Cargo.toml` - Dependencies and project metadata
- `.env.example` - Environment variables template
- `.gitignore` - Git ignore rules
- `README.md` - User documentation
- `ARCHITECTURE.md` - System design documentation
- `CONTRIBUTING.md` - This file
- `API.md` - API documentation

### Directories

- `src/` - Rust source code
- `target/` - Build artifacts (gitignored)
- `coverage/` - Coverage reports (gitignored)

## Questions?

- Open an issue for discussion
- Join the community discussions
- Contact maintainers for guidance

## Recognition

Contributors will be recognized in:
- GitHub contributors page
- CHANGELOG.md for significant contributions
- README.md acknowledgments section

## License

By contributing to this project, you agree that your contributions will be licensed under its MIT License.

---

Thank you for contributing to LLM over DNS!
