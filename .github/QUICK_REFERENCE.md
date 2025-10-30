# CI/CD Quick Reference Card

## Local Development Commands

```bash
# Format code
cargo fmt --all

# Check format
cargo fmt --all -- --check

# Lint code (with auto-fix)
cargo clippy --fix --all-targets --all-features

# Check lint (no changes)
cargo clippy --all-targets --all-features -- -D warnings

# Run all tests
cargo test --all-features --verbose

# Run tests with output
cargo test --all-features -- --nocapture

# Run doc tests
cargo test --doc

# Check coverage
cargo tarpaulin --all-features --fail-under 100

# Generate coverage HTML report
cargo tarpaulin --all-features --out Html

# Security audit
cargo audit

# Build project
cargo build --release

# Generate documentation
cargo doc --no-deps --open
```

## Pre-Commit Checks

Run these before pushing:

```bash
#!/bin/bash
set -e

cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo tarpaulin --all-features --fail-under 100
cargo audit
```

## GitHub Actions Setup

1. **Add Repository Secret**
   - Go to Settings → Secrets and variables → Actions
   - Add `CODECOV_TOKEN` from codecov.io

2. **Configure Branch Protection**
   - Go to Settings → Branches → Add rule
   - Branch: `main`
   - Require: Status checks pass
   - Require: 1 review approval

3. **Enable Security Features**
   - Settings → Code security and analysis
   - Enable: Dependabot alerts, security updates, secret scanning

## Common Issues & Fixes

### Format Fails
```bash
cargo fmt --all
git add .
git commit --amend --no-edit
```

### Clippy Warnings
```bash
cargo clippy --fix --all-targets --all-features
cargo test --all-features
```

### Tests Fail
```bash
RUST_BACKTRACE=full cargo test --all-features -- --nocapture
```

### Coverage Below 100%
```bash
cargo tarpaulin --all-features --out Html
# Open tarpaulin-report.html to see uncovered lines
# Add tests for uncovered code
```

### Security Audit Fails
```bash
cargo audit --json
# Update vulnerable dependency:
cargo update -p vulnerable_crate
```

## Job Status Check

Click on Actions tab → CI workflow → latest run

### Expected Results
- ✅ Format Check (5 sec)
- ✅ Clippy Lint (20 sec)
- ✅ Security Audit (15 sec)
- ✅ Test Suite - stable (60 sec)
- ✅ Test Suite - beta (60 sec)
- ✅ Code Coverage (90 sec)
- ✅ All Checks Pass (1 sec)

**Total: ~2-3 minutes** (with warm cache)

## Commit Message Format

```
<type>(<scope>): <subject>

<body (optional)>

<footer (optional)>
```

**Types**:
- `feat` - New feature
- `fix` - Bug fix
- `docs` - Documentation
- `style` - Formatting
- `refactor` - Code restructuring
- `test` - Test updates
- `chore` - Dependency/build updates
- `ci` - CI/CD updates

**Examples**:
```
feat(dns): add TXT record support
fix(config): handle missing env vars
docs: update README
test(coverage): add error path tests
chore(deps): update tokio to 1.36
ci: add dependabot configuration
```

## Pull Request Workflow

1. **Create branch**: `git checkout -b feature/my-feature`
2. **Make changes**: Edit code
3. **Test locally**: Run all checks
4. **Commit**: `git commit -m "type(scope): message"`
5. **Push**: `git push origin feature/my-feature`
6. **Create PR**: Go to GitHub, click "Compare & pull request"
7. **Fill template**: Use provided PR template
8. **Wait for CI**: All checks must pass
9. **Request review**: Add reviewers
10. **Address feedback**: Make requested changes
11. **Merge**: Once approved and CI passes

## Dependency Management

### Check for Updates
```bash
cargo outdated
cargo update --dry-run
```

### Update All Dependencies
```bash
cargo update
cargo test --all-features
```

### Update Specific Dependency
```bash
cargo update -p dependency_name
cargo test --all-features
```

### Add New Dependency
```bash
cargo add dependency_name
# or with version
cargo add dependency_name@1.2.3
# or dev dependency
cargo add --dev test_dependency_name
```

## Coverage Requirements

### 100% Coverage Means

- Every line of code executed ✓
- Every branch tested ✓
- Every error path covered ✓
- No dead code allowed ✓

### Excluded From Coverage

- Test code itself
- Build scripts
- Unreachable code (marked unreachable!())
- Integration test files

### Checking Coverage

```bash
# Quick check
cargo tarpaulin --all-features

# Detailed report
cargo tarpaulin --all-features --out Html
open tarpaulin-report.html

# With exclusions
cargo tarpaulin --all-features --exclude-files tests/*
```

## Environment Setup

### First Time Only

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Install components
rustup component add rustfmt clippy

# Install tools
cargo install cargo-tarpaulin cargo-audit cargo-watch
```

### Development Environment

```bash
# Clone repo
git clone https://github.com/duyet/llm-over-dns.git
cd llm-over-dns

# Copy environment file
cp .env.example .env

# Verify setup
cargo test --all-features
```

## Documentation References

| Topic | File |
|-------|------|
| Full CI/CD details | `.github/CI_CD.md` |
| GitHub Actions setup | `.github/GITHUB_ACTIONS_SETUP.md` |
| Local development | `DEVELOPMENT.md` |
| Setup summary | `CI_CD_SETUP_SUMMARY.md` |
| Issue templates | `.github/ISSUE_TEMPLATE/` |
| PR template | `.github/pull_request_template.md` |

## Key Workflow Files

| File | Purpose |
|------|---------|
| `.github/workflows/ci.yml` | Main CI workflow |
| `.github/dependabot.yml` | Dependency updates |
| `.github/CODEOWNERS` | Code review ownership |
| `.github/pull_request_template.md` | PR template |
| `.github/ISSUE_TEMPLATE/bug_report.md` | Bug template |
| `.github/ISSUE_TEMPLATE/feature_request.md` | Feature template |

## Performance Tips

### Build Optimization
```bash
# Use sccache for incremental builds
cargo install sccache
export RUSTC_WRAPPER=sccache

# Use mold for linking (Linux only)
RUSTFLAGS="-C link-arg=-fuse-ld=mold" cargo build
```

### Test Optimization
```bash
# Run tests in parallel (default)
cargo test

# Single-threaded for debugging
cargo test -- --test-threads=1

# Skip slow tests
cargo test --lib --doc
```

## Troubleshooting Checklist

When things go wrong, try:

- [ ] Update Rust: `rustup update`
- [ ] Clean build: `cargo clean && cargo build`
- [ ] Check versions: `rustc --version && cargo --version`
- [ ] Run locally: `cargo test --all-features`
- [ ] Check logs: Click job in Actions tab
- [ ] Review recent changes: `git log --oneline -5`
- [ ] Check environment: `env | grep RUST`
- [ ] Verify secrets: Go to Settings → Secrets
- [ ] Review PR template: Verify all checkboxes
- [ ] Ask for help: Create GitHub issue

## Useful Links

- [Rust Documentation](https://doc.rust-lang.org/)
- [Cargo Documentation](https://doc.rust-lang.org/cargo/)
- [GitHub Actions](https://github.com/features/actions)
- [Codecov](https://codecov.io/)
- [Cargo Tarpaulin](https://github.com/xd009642/tarpaulin)
- [Clippy Lints](https://doc.rust-lang.org/clippy/)
- [Rustfmt](https://rust-lang.github.io/rustfmt/)

## One-Liner Checks

```bash
# Run everything locally (like CI does)
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-features && cargo tarpaulin --all-features --fail-under 100 && cargo audit

# Just the essentials
cargo fmt --all && cargo clippy && cargo test

# Quick validation
cargo check && cargo test --lib
```

## GitHub Actions Dashboard

**View Results**:
1. Go to GitHub repository
2. Click **Actions** tab
3. Select **CI** workflow
4. Click latest run
5. Expand job to see details

**Expected Green Checks** ✅:
- All 6 jobs completed
- No failed steps
- All status checks passed

---

**Keep this card handy for quick reference!** 📋
