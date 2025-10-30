# GitHub Configuration

This directory contains GitHub-specific configuration for the LLM over DNS project.

## Directory Structure

### Workflows
- **`workflows/ci.yml`** - Main CI/CD pipeline with all quality checks
- **`workflows/coverage.yml`** - Code coverage generation and upload
- **`workflows/quality.yml`** - Code quality checks and build verification

### Templates
- **`pull_request_template.md`** - Template for pull requests
- **`ISSUE_TEMPLATE/bug_report.md`** - Bug report template
- **`ISSUE_TEMPLATE/feature_request.md`** - Feature request template

### Configuration
- **`CODEOWNERS`** - Code ownership and review assignments
- **`dependabot.yml`** - Automated dependency update configuration

### Documentation
- **`CI_CD.md`** - Detailed CI/CD pipeline documentation
- **`GITHUB_ACTIONS_SETUP.md`** - Step-by-step GitHub Actions setup guide
- **`QUICK_REFERENCE.md`** - Quick reference card for common tasks
- **`README.md`** - This file

## Quick Start

### First Time Setup
1. Read `.github/GITHUB_ACTIONS_SETUP.md`
2. Follow steps 1-6 for GitHub configuration
3. Run first workflow by pushing a test commit

### Local Development
1. Read `../DEVELOPMENT.md`
2. Install development tools
3. Run checks locally before pushing

### Understanding CI/CD
1. Read `.github/CI_CD.md` for detailed job descriptions
2. Read `.github/QUICK_REFERENCE.md` for quick commands
3. Check workflow status in GitHub Actions tab

## Key Features

- ✅ Automated testing on every push and PR
- ✅ 100% code coverage requirement
- ✅ Security vulnerability scanning
- ✅ Code formatting and linting
- ✅ Automated dependency updates
- ✅ Code review workflow
- ✅ Status checks for branch protection

## Workflow Jobs

### CI Pipeline (`workflows/ci.yml`)
- **fmt** - Format code with rustfmt
- **clippy** - Lint code with cargo clippy
- **audit** - Security audit with cargo-audit
- **test** - Run tests across Rust versions
- **coverage** - Verify 100% code coverage
- **all-checks-pass** - Aggregate all results

### Other Workflows
- **coverage** - Code coverage detailed reporting
- **quality** - Additional quality checks and build verification

## Environment Secrets

Required for full functionality:
- `CODECOV_TOKEN` - For uploading coverage to Codecov (optional for public repos)

## Documentation Files

| File | Purpose | Audience |
|------|---------|----------|
| `CI_CD.md` | Detailed CI/CD documentation | DevOps, Maintainers |
| `GITHUB_ACTIONS_SETUP.md` | Step-by-step setup guide | Repository Admins |
| `QUICK_REFERENCE.md` | Command reference card | All Developers |
| `README.md` | This index | All Developers |

## Common Tasks

### Check Workflow Status
1. Go to **Actions** tab on GitHub
2. Click **CI** workflow
3. View latest run status

### Run Checks Locally
```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo tarpaulin --all-features --fail-under 100
```

### Add New Dependency
```bash
cargo add dependency-name
# Make changes, run tests, commit
```

### Update Dependencies
```bash
cargo update
cargo test --all-features
# Commit if tests pass
```

### Debug Failed Workflow
1. Click failed job in Actions tab
2. Review job logs
3. Look for error messages
4. Reference `QUICK_REFERENCE.md` for solutions

## Performance Targets

- **Build Time**: <5 minutes (target achieved at ~2 min with warm cache)
- **Coverage**: 100% required
- **Test Pass Rate**: 100% required
- **Lint Warnings**: 0 allowed

## Support

- 📖 Read documentation files in this directory
- 🔍 Check GitHub Actions logs for detailed error messages
- 💡 Reference `QUICK_REFERENCE.md` for common commands
- 🆘 Create a GitHub issue with detailed information

## Links

- [GitHub Actions Documentation](https://docs.github.com/en/actions)
- [Cargo Documentation](https://doc.rust-lang.org/cargo/)
- [Codecov Documentation](https://docs.codecov.io/)
- [Rust Book](https://doc.rust-lang.org/book/)

---

**Version**: 1.0  
**Last Updated**: 2025-10-30  
**Status**: Active ✅
