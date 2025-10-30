# Quality Assurance & Coverage Guide

This document outlines the quality assurance processes, coverage requirements, and tools for the LLM over DNS project.

## Quick Start

### Install Git Hooks

Run this once to set up automatic quality checks before commits:

```bash
chmod +x scripts/install-git-hooks.sh
./scripts/install-git-hooks.sh
```

### Run Quality Checks Locally

Before committing:

```bash
# Run pre-commit checks
chmod +x scripts/pre-commit.sh
./scripts/pre-commit.sh

# Or run comprehensive quality audit
chmod +x scripts/quality-check.sh
./scripts/quality-check.sh

# Or run coverage analysis
chmod +x scripts/test-coverage.sh
./scripts/test-coverage.sh
```

## Quality Gates

The project enforces the following quality standards:

### Code Coverage
- **Target:** 100% code coverage
- **Threshold:** 0% (all code must be covered)
- **Tool:** `cargo-tarpaulin`
- **Report:** Generated in `coverage/` directory

### Testing
- **Requirement:** All tests must pass
- **Command:** `cargo test`
- **Scope:** Unit tests + Integration tests
- **Framework:** Rust built-in test framework + dev dependencies

### Linting (Clippy)
- **Policy:** Zero warnings allowed
- **Command:** `cargo clippy --all-targets --all-features -- -D warnings`
- **Strictness:** Warnings are treated as errors
- **Scope:** All targets (bin, lib, tests, examples)

### Code Formatting
- **Standard:** Rust formatting (rustfmt)
- **Command:** `cargo fmt --check`
- **Auto-fix:** `cargo fmt`
- **Policy:** Enforced on all Rust source files

### Documentation
- **Requirement:** Public API must be documented
- **Command:** `cargo doc --no-deps`
- **Format:** Markdown in doc comments
- **Standards:** Doc tests must compile and run

## Scripts Overview

### `scripts/pre-commit.sh`
Runs essential quality checks before committing:
- Unit tests
- Clippy linting (zero warnings)
- Code formatting check
- Dependency validation

**Usage:**
```bash
./scripts/pre-commit.sh
```

**Exit Codes:**
- `0`: All checks passed
- `1`: One or more checks failed

### `scripts/test-coverage.sh`
Generates detailed code coverage reports:
- Runs tests with coverage instrumentation
- Generates HTML coverage report
- Calculates overall coverage percentage
- Validates against 100% target

**Usage:**
```bash
./scripts/test-coverage.sh
# View report: open coverage/index.html
```

**Output:**
- `coverage/index.html`: Interactive HTML report
- Console output: Coverage percentage and summary

### `scripts/quality-check.sh`
Comprehensive quality audit (all 6 checks):
1. Code Coverage Analysis
2. Unit Tests
3. Clippy Analysis (Zero Warnings Policy)
4. Code Format Check (rustfmt)
5. Dependency Audit
6. Documentation Check

**Usage:**
```bash
./scripts/quality-check.sh
```

**Output:**
- Detailed report with pass/fail for each check
- Warnings summary
- Clear guidance on fixing issues

### `scripts/install-git-hooks.sh`
Sets up automatic pre-commit quality checks:

**Installs:**
- `pre-commit`: Runs quality checks before each commit
- `commit-msg`: Validates conventional commit format
- `post-merge`: Updates dependencies after merge

**Usage:**
```bash
./scripts/install-git-hooks.sh
```

**Bypass Hooks (if needed):**
```bash
git commit --no-verify
```

## CI/CD Integration

### GitHub Actions Workflows

Two workflows are configured:

#### 1. Quality Checks (`.github/workflows/quality.yml`)
Runs on every push and PR:
- Tests
- Clippy linting
- Code formatting
- Documentation generation
- Security audit

#### 2. Code Coverage (`.github/workflows/coverage.yml`)
Runs on main/develop and PRs:
- Full coverage analysis
- Uploads to Codecov
- Comments on PRs with coverage report

### Codecov Configuration (`codecov.yml`)

Key settings:
- **Target Coverage:** 100%
- **Threshold:** 0% (no slack)
- **PR Comments:** Enabled with detailed breakdown
- **Base Comparison:** Automatic

## Code Coverage Details

### Coverage Tool: cargo-tarpaulin

Installation:
```bash
cargo install cargo-tarpaulin
```

### Running Coverage Locally

Basic run:
```bash
cargo tarpaulin --out Html
```

Advanced options:
```bash
cargo tarpaulin \
  --out Html \
  --output-dir coverage \
  --timeout 120 \
  --exclude-files build.rs \
  --verbose
```

### Understanding Coverage Reports

The HTML report shows:
- **Overall Coverage %:** Percentage of code lines covered by tests
- **Per-file Coverage:** Detailed breakdown by source file
- **Coverage by Line:** Colored line-by-line coverage status
  - Green: Covered by tests
  - Red: Not covered by tests
  - Orange: Partially covered

### Improving Coverage

When coverage is below 100%:

1. **Identify uncovered code:**
   - Review `coverage/index.html` for red lines
   - Check which functions/blocks lack tests

2. **Add test cases:**
   - Write unit tests for uncovered paths
   - Include edge cases and error conditions
   - Test all code branches

3. **Verify improvement:**
   - Run coverage again: `./scripts/test-coverage.sh`
   - Check that previously red lines are now green

Example test additions:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_success_path() {
        // Test happy path
    }

    #[test]
    fn test_function_error_case() {
        // Test error handling
    }

    #[test]
    fn test_function_edge_case() {
        // Test boundary conditions
    }
}
```

## Testing Best Practices

### Unit Tests
- Place in same file as code with `#[cfg(test)]` module
- Test each function thoroughly
- Cover success and error cases
- Use meaningful assertion messages

### Test Organization
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_description() {
        // Arrange
        let input = ...;

        // Act
        let result = function(input);

        // Assert
        assert_eq!(result, expected);
    }
}
```

### Coverage-Driven Testing
- Identify gaps in coverage
- Add tests for missing branches
- Focus on critical paths first
- Ensure error handling is tested

## Pre-commit Workflow

### Local Development

1. Make changes
2. Run pre-commit checks:
   ```bash
   ./scripts/pre-commit.sh
   ```
3. Fix any issues found
4. Commit when all checks pass

### Automatic Checks (with Git Hooks)

After installing hooks:
1. Make changes
2. Run `git commit`
3. Hooks automatically run quality checks
4. Commit succeeds only if all checks pass
5. Bypass with `git commit --no-verify` if absolutely necessary

## Quality Metrics

### Current Standards

| Check | Status | Target |
|-------|--------|--------|
| Test Coverage | Pass | 100% |
| Clippy Warnings | Zero | 0 |
| Format Compliance | Pass | 100% |
| Test Pass Rate | Pass | 100% |
| Documentation | Pass | All public API |
| Security Audit | Pass | No vulnerabilities |

### Tracking Progress

View metrics:
- **Local:** Run `./scripts/quality-check.sh` for full audit
- **CI:** Check GitHub Actions workflow results
- **Codecov:** View dashboard at codecov.io

## Troubleshooting

### Tests fail locally but pass in CI
- Check Rust version: `rustc --version`
- Update: `rustup update`
- Clear cache: `cargo clean`

### Clippy warnings won't go away
- Check for latest clippy: `rustup component add clippy --toolchain stable`
- Some warnings need code changes: follow the suggestion
- Suppress specific: `#[allow(clippy::name)]` (document why)

### Coverage stuck below 100%
1. Run coverage: `./scripts/test-coverage.sh`
2. Open `coverage/index.html`
3. Find red (uncovered) lines
4. Add tests for those code paths
5. Re-run coverage

### Git hooks not working
- Re-install: `./scripts/install-git-hooks.sh`
- Check permissions: `ls -la .git/hooks/`
- Verify hook content: `cat .git/hooks/pre-commit`

### Codecov integration issues
- Check codecov.yml syntax: `cat codecov.yml`
- Verify GitHub Actions workflows exist
- Check repository settings for Codecov app

## Configuration Files

### `codecov.yml`
Main codecov configuration:
- Coverage targets
- Threshold settings
- PR comment format
- Ignore patterns

### `.github/workflows/quality.yml`
GitHub Actions workflow for quality checks:
- Runs on push and PR
- Tests, linting, formatting, docs
- Security audit

### `.github/workflows/coverage.yml`
GitHub Actions workflow for coverage:
- Generates coverage reports
- Uploads to Codecov
- Comments on PRs

## References

- [Codecov Documentation](https://docs.codecov.io/)
- [cargo-tarpaulin](https://github.com/xd009642/tarpaulin)
- [Clippy](https://github.com/rust-lang/rust-clippy)
- [Rustfmt](https://rust-lang.github.io/rustfmt/)
- [Conventional Commits](https://www.conventionalcommits.org/)

## Maintenance

### Regular Tasks

**Weekly:**
- Monitor PR coverage reports
- Address any coverage regressions

**Monthly:**
- Review Codecov dashboard trends
- Update dependencies: `cargo update`
- Run security audit: `cargo audit`

**Quarterly:**
- Review quality gate effectiveness
- Update coverage targets if needed
- Audit dependency security status

## Support

For issues or questions:
1. Check this guide
2. Run `./scripts/quality-check.sh` for diagnostics
3. Review GitHub Actions logs for CI failures
4. Check Codecov dashboard for coverage details
