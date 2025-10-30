# GitHub Configuration

This directory contains GitHub-specific configuration for the LLM over DNS project.

## Directory Structure

### Workflows
- **`workflows/ci.yml`** - Main CI/CD pipeline with all quality checks, testing, and coverage
- **`workflows/docker.yml`** - Multi-platform Docker image builds and GHCR deployment
- **`workflows/release.yml`** - Automated GitHub releases with cross-platform binaries

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
- ✅ 90% code coverage requirement with PR comments
- ✅ Security vulnerability scanning (cargo-audit + Trivy)
- ✅ Code formatting and linting (rustfmt + clippy)
- ✅ Multi-platform Docker builds (amd64, arm64)
- ✅ Automated GitHub releases with binaries
- ✅ Automated dependency updates (Dependabot)
- ✅ Code review workflow
- ✅ Status checks for branch protection

## Workflow Jobs

### CI Pipeline (`workflows/ci.yml`)
Runs on: Push to master, Pull Requests, Manual trigger
- **fmt** - Format code with rustfmt
- **clippy** - Lint code with cargo clippy (-D warnings)
- **audit** - Security audit with cargo-audit
- **test** - Run tests across Rust stable and beta
- **coverage** - Verify 90% code coverage + PR comments
- **all-checks-pass** - Aggregate all results

### Docker Pipeline (`workflows/docker.yml`)
Runs on: Push to master, Tags (v*.*.*), Pull Requests, Manual trigger
- **build-and-push** - Multi-platform build (amd64, arm64) + push to GHCR
- **docker-test** - Test image + Trivy security scan (PRs only)

### Release Pipeline (`workflows/release.yml`)
Runs on: Tags (v*.*.*), Manual trigger
- **create-release** - Create GitHub release with changelog
- **build-binaries** - Cross-compile for 6 platforms (Linux, macOS, Windows)

## Environment Secrets

Required secrets (Settings → Secrets and variables → Actions):
- `CODECOV_TOKEN` - For uploading coverage to Codecov (optional for public repos)
- `GITHUB_TOKEN` - Automatically provided by GitHub (no setup needed)

Permissions required:
- **packages: write** - For pushing Docker images to GHCR
- **contents: write** - For creating releases

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
