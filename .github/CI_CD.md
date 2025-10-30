# CI/CD Pipeline

## Overview

This project uses GitHub Actions for continuous integration and deployment. The pipeline ensures code quality, security, and test coverage through automated checks.

## Workflow Jobs

### 1. Format Check (`fmt`)
- **Tool**: `rustfmt`
- **Purpose**: Enforces consistent code formatting across the project
- **Command**: `cargo fmt --all -- --check`
- **Failure Action**: Blocks PR until formatting is corrected
- **Fix**: Run `cargo fmt --all` locally to auto-fix

### 2. Linting (`clippy`)
- **Tool**: `cargo clippy`
- **Purpose**: Detects common Rust mistakes and idiom violations
- **Command**: `cargo clippy --all-targets --all-features -- -D warnings`
- **Failure Action**: Treats warnings as errors, blocks CI
- **Fix**: Run `cargo clippy --fix --all-targets --all-features` to auto-fix most issues

### 3. Security Audit (`audit`)
- **Tool**: `cargo audit` via `rustsec/audit-check-action`
- **Purpose**: Scans dependencies for known security vulnerabilities
- **Failure Action**: Blocks PR if vulnerabilities are found
- **Fix**: Update affected dependencies or audit exceptions in `Cargo.lock`

### 4. Test Suite (`test`)
- **Tool**: `cargo test`
- **Strategy**: Matrix testing across Rust versions (stable, beta)
- **Command**: `cargo test --all-features --verbose`
- **Includes**: Unit tests and documentation tests
- **Coverage**: All features enabled during testing
- **Failure Action**: Blocks PR if any tests fail

### 5. Code Coverage (`coverage`)
- **Tool**: `cargo-tarpaulin`
- **Purpose**: Measures code coverage and enforces 100% requirement
- **Command**: `cargo tarpaulin --out Xml --all-features --timeout 120 --fail-under 100`
- **Upload**: Sends coverage data to Codecov for historical tracking
- **Failure Action**: Blocks PR if coverage drops below 100%
- **Exclusions**: Test files are excluded from coverage analysis

### 6. All Checks Pass (`all-checks-pass`)
- **Purpose**: Aggregates all job results for clear approval/blocking
- **Depends On**: All previous jobs
- **Failure Action**: Blocks merge until all jobs pass

## Running Locally

### Prerequisites
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.sh/env

# Install components
rustup component add rustfmt clippy
cargo install cargo-tarpaulin cargo-audit
```

### Run All Checks
```bash
# Format check
cargo fmt --all -- --check

# Clippy lint
cargo clippy --all-targets --all-features -- -D warnings

# Tests
cargo test --all-features --verbose

# Coverage (requires 100%)
cargo tarpaulin --all-features --fail-under 100
```

### Fix Issues Automatically
```bash
# Format code
cargo fmt --all

# Fix clippy warnings
cargo clippy --fix --all-targets --all-features

# Run security audit
cargo audit
```

## Caching Strategy

The pipeline uses `Swatinem/rust-cache@v2` for efficient dependency caching:
- **Cache Keys**: Rust version, toolchain, manifest files
- **Preservation**: Caches `~/.cargo` and `target/` directories
- **Speed Improvement**: ~3-4 minute savings per job with warm cache

## Coverage Requirements

### 100% Coverage Enforcement
The project requires 100% code coverage to merge:
- All production code must have corresponding tests
- All branches must be covered
- All error paths must be tested

### Measuring Coverage
```bash
# Generate coverage report
cargo tarpaulin --all-features --out Html

# Open report
open tarpaulin-report.html
```

## Rust Version Matrix

### Tested Versions
- **Stable**: Latest stable release
- **Beta**: Upcoming stable version (early testing)
- **Nightly**: Available on-demand (not required for PR)

### MSRV (Minimum Supported Rust Version)
Currently: **1.70.0** (as per `edition = "2021"`)

## Dependency Management

### Dependabot Configuration
Automated dependency updates are configured via `.github/dependabot.yml`:
- **Frequency**: Weekly on Monday at 4 AM UTC
- **Cargo Dependencies**: All updates tracked with `chore(deps)` prefix
- **GitHub Actions**: Separately tracked with `ci` prefix
- **PR Limits**: Max 10 cargo + 5 action PRs simultaneously

### Handling Dependabot PRs
1. CI automatically runs against all Dependabot PRs
2. Review changes in dependency versions
3. Check changelog for breaking changes
4. Merge once CI passes

## Badge Support

Add these badges to your README:

```markdown
[![CI](https://github.com/duyet/llm-over-dns/actions/workflows/ci.yml/badge.svg)](https://github.com/duyet/llm-over-dns/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/duyet/llm-over-dns/branch/main/graph/badge.svg)](https://codecov.io/gh/duyet/llm-over-dns)
[![Security Audit](https://github.com/duyet/llm-over-dns/actions/workflows/ci.yml/badge.svg?label=audit)](https://github.com/duyet/llm-over-dns/security)
```

## Performance Targets

### Current Performance
- **Cold Cache**: ~4-5 minutes per run
- **Warm Cache**: ~1-2 minutes per run
- **Target**: <5 minutes total

### Optimization Opportunities
1. Matrix testing parallelization (currently serial)
2. Conditional job skipping (lint-only for docs)
3. Incremental coverage builds

## Secrets Configuration

Required GitHub secrets:

### `CODECOV_TOKEN`
- **Purpose**: Upload coverage to Codecov
- **Optional**: Can run without, but uploads fail
- **Get Token**: https://codecov.io/gh/duyet/llm-over-dns/settings

### `GITHUB_TOKEN`
- **Automatic**: Provided by GitHub Actions
- **Usage**: For audit-check-action and API calls

## Troubleshooting

### Coverage Fails Unexpectedly
```bash
# Run tarpaulin locally with verbose output
cargo tarpaulin --all-features --verbose --timeout 120
```

### Tests Pass Locally but Fail in CI
- Check Rust version: `rustc --version`
- Check feature flags in workflow match local testing
- Verify environment variables are set correctly

### Clippy Says Different Things Locally
```bash
# Update toolchain
rustup update

# Ensure same version
cargo clippy --version
```

### Dependabot PRs Failing
1. Check dependency compatibility
2. Run tests locally with new version
3. Update code if breaking changes detected
4. Comment in PR if advisory is false positive

## Advanced Features

### Custom Workflow Triggers
Edit `.github/workflows/ci.yml` to add:
- Schedule-based runs (nightly tests)
- Manual dispatch triggers
- Release workflows

### Conditional Job Skipping
```yaml
if: |
  github.event_name == 'pull_request' &&
  github.event.pull_request.draft == false
```

### Parallel Job Optimization
Currently using sequential dependencies. Can parallelize:
- `fmt` and `clippy` are independent
- `test` and `coverage` can run in parallel
- Modify `needs:` directive for faster runs

## References

- [Rust GitHub Actions](https://github.com/actions-rs)
- [Cargo Tarpaulin](https://github.com/xd009642/tarpaulin)
- [Codecov Integration](https://docs.codecov.io/docs/github-actions)
- [Dependabot Guide](https://docs.github.com/en/code-security/dependabot)
- [Cargo Format](https://rust-lang.github.io/rustfmt/)
- [Clippy Lints](https://doc.rust-lang.org/clippy/)
