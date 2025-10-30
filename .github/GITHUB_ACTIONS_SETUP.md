# GitHub Actions Setup Guide

This guide explains how to fully configure GitHub Actions for the LLM over DNS project.

## Prerequisites

- [ ] Repository pushed to GitHub
- [ ] Repository settings accessible
- [ ] Admin access to repository
- [ ] Codecov account (optional, for coverage tracking)

## Step-by-Step Setup

### 1. Enable GitHub Actions

1. Go to repository **Settings** → **Actions** → **General**
2. Ensure "Allow all actions and reusable workflows" is selected
3. Under "Workflow permissions", select:
   - [ ] "Read and write permissions"
   - [ ] "Allow GitHub Actions to create and approve pull requests"

### 2. Configure Codecov Integration

#### Option A: With Token (Recommended for Private Repos)

1. Sign up at [codecov.io](https://codecov.io)
2. Add your GitHub repository to Codecov
3. Copy the **Repository Upload Token**
4. Go to repository **Settings** → **Secrets and variables** → **Actions**
5. Create new secret:
   - **Name**: `CODECOV_TOKEN`
   - **Value**: Paste the token from Codecov
6. Save

#### Option B: Without Token (Works for Public Repos)

1. The workflow will automatically use GitHub's token for public repositories
2. No additional setup needed

### 3. Set Up Branch Protection Rules

1. Go to **Settings** → **Branches** → **Add rule**
2. Create protection for `main` branch:
   - **Branch name pattern**: `main`
   - Check:
     - [ ] "Require a pull request before merging"
     - [ ] "Require status checks to pass before merging"
     - [ ] "Require branches to be up to date before merging"
   - Click **Refresh and select** and choose:
     - [ ] `all-checks-pass` (or all individual jobs)
   - [ ] "Require code reviews before merging" (minimum 1)
   - [ ] "Dismiss stale pull request approvals when new commits are pushed"
   - [ ] "Require approval of the most recent reviewers review"
   - Click **Create**

### 4. Enable Security Settings

1. Go to **Settings** → **Code security and analysis**
2. Enable:
   - [ ] "Dependabot alerts"
   - [ ] "Dependabot security updates"
   - [ ] "Secret scanning"
   - [ ] "Secret scanning push protection"

### 5. Configure Dependabot

Dependabot is configured via `.github/dependabot.yml`. It will:
- Check for Rust dependency updates weekly
- Check for GitHub Actions updates weekly
- Create automatic PRs for updates
- Run CI against all PRs automatically

**Note**: Dependabot PRs require the same CI checks to pass as regular PRs.

### 6. Set Up PR Templates

PR templates are automatically provided in `.github/pull_request_template.md`.

When creating a PR:
1. The template will auto-populate
2. Follow the checklist before requesting review
3. Ensure all CI checks pass

## Verification

### Check Workflow Status

1. Go to **Actions** tab
2. You should see:
   - [ ] "CI" workflow
   - Previous runs showing status

### Run First Workflow Manually

1. Create a test branch:
   ```bash
   git checkout -b ci-test
   echo "# CI Test" >> README.md
   git add README.md
   git commit -m "test: trigger CI workflow"
   git push origin ci-test
   ```

2. Go to **Actions** → **CI**
3. Click the latest run to see job details
4. Jobs should complete in ~2-3 minutes with warm cache

### Expected Job Results

All jobs should show ✅ (success):
- [ ] Format Check
- [ ] Clippy Lint
- [ ] Security Audit
- [ ] Test Suite (stable)
- [ ] Test Suite (beta)
- [ ] Code Coverage
- [ ] All Checks Pass

## Adding Badges to README

Add these badges to your README.md:

```markdown
# LLM over DNS

[![CI](https://github.com/duyet/llm-over-dns/actions/workflows/ci.yml/badge.svg)](https://github.com/duyet/llm-over-dns/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/duyet/llm-over-dns/branch/main/graph/badge.svg)](https://codecov.io/gh/duyet/llm-over-dns)
[![Crate](https://img.shields.io/crates/v/llm-over-dns.svg)](https://crates.io/crates/llm-over-dns)
[![License](https://img.shields.io/crates/l/llm-over-dns.svg)](https://github.com/duyet/llm-over-dns/blob/main/LICENSE)
```

## Troubleshooting

### Workflow Not Triggering

**Problem**: Workflow doesn't run on push/PR

**Solutions**:
1. Check workflow file syntax:
   ```bash
   cd .github/workflows
   # Validate YAML syntax
   ```
2. Verify branch names match (check `on:` section)
3. Go to **Actions** → **All workflows** → Check if disabled
4. Try manual trigger from Actions tab

### CI Fails on Lint

**Problem**: `cargo clippy` or `cargo fmt` fails

**Solution**: Run locally and fix:
```bash
# Auto-fix formatting
cargo fmt --all

# Fix clippy warnings
cargo clippy --fix --all-targets --all-features
```

### Coverage Falls Below 100%

**Problem**: Coverage requirement not met

**Solution**:
1. Run locally: `cargo tarpaulin --all-features`
2. Review generated HTML report
3. Add tests for uncovered code
4. Ensure all branches are tested

### Codecov Upload Fails

**Problem**: Workflow fails at Codecov upload step

**Solutions**:
1. Verify `CODECOV_TOKEN` is set correctly
2. Check Codecov status page
3. Temporarily disable failure requirement:
   - In `.github/workflows/ci.yml`, change:
   ```yaml
   fail_ci_if_error: false
   ```
   - Keep it false while investigating

### Security Audit Fails

**Problem**: Dependency vulnerability detected

**Solutions**:
1. Update vulnerable dependency:
   ```bash
   cargo update <dependency-name>
   ```
2. Check advisory for details:
   ```bash
   cargo audit --json
   ```
3. If false positive, update in `Cargo.lock`

## Advanced Configuration

### Matrix Testing Additional Rust Versions

Edit `.github/workflows/ci.yml`:

```yaml
strategy:
  matrix:
    rust: [stable, beta, nightly]
  fail-fast: false
```

### Conditional Jobs

Run jobs only on specific conditions:

```yaml
if: |
  github.event_name == 'pull_request' &&
  !contains(github.event.pull_request.labels.*.name, 'skip-ci')
```

### Add Release Workflow

Create `.github/workflows/release.yml`:

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --release
      - uses: softprops/action-gh-release@v1
```

## Performance Optimization

### Cache Statistics

After a few runs, you should see:
- **Cold Run**: 4-5 minutes
- **Warm Cache**: 1-2 minutes

### Improving Speed

1. **Parallel Jobs**: Jobs run in parallel automatically
2. **Skip Tests**: Use `[skip ci]` in commit message (not recommended)
3. **Incremental Builds**: Cargo uses incremental compilation automatically
4. **Pre-built Docker**: For very large projects, consider custom Docker images

## Security Best Practices

### Secrets Management

1. **Never commit secrets**: Use GitHub Secrets
2. **Rotate tokens**: Review and rotate `CODECOV_TOKEN` periodically
3. **Least privilege**: Use restricted tokens when possible
4. **Secret scanning**: GitHub will alert on secrets in code

### Dependency Scanning

1. Review Dependabot PRs carefully
2. Check changelogs for breaking changes
3. Test locally before merging
4. Enable security advisories alerts

### Branch Protection

1. Require PR reviews before merge
2. Require CI to pass
3. Dismiss stale approvals
4. Require up-to-date branches

## Monitoring

### Set Up Notifications

1. Go to **Settings** → **Notifications**
2. Enable:
   - [ ] "Watching" notifications for workflow failures
   - [ ] "Email" for critical failures

### Track Metrics

1. **Codecov Dashboard**: [codecov.io](https://codecov.io/gh/duyet/llm-over-dns)
2. **GitHub Actions**: Repository **Actions** tab
3. **Security**: **Security** → **Code scanning alerts**

## Resources

- [GitHub Actions Docs](https://docs.github.com/en/actions)
- [Codecov Integration](https://docs.codecov.io/docs/github-actions)
- [Rust Toolchain Actions](https://github.com/dtolnay/rust-toolchain)
- [Cargo Cache Action](https://github.com/Swatinem/rust-cache)
- [Dependabot Docs](https://docs.github.com/en/code-security/dependabot)
- [Branch Protection Rules](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/about-protected-branches)

## Completion Checklist

- [ ] Repository pushed to GitHub
- [ ] GitHub Actions enabled
- [ ] Codecov token added (if using private repo)
- [ ] Branch protection rules configured
- [ ] Security features enabled
- [ ] Dependabot configured
- [ ] First workflow run successful
- [ ] Badges added to README
- [ ] Team notified of new process
- [ ] Documentation reviewed

## Questions?

Refer to:
1. **[CI_CD.md](./.CI_CD.md)** - Detailed job descriptions
2. **[.github/workflows/ci.yml](./workflows/ci.yml)** - Workflow source
3. **GitHub Actions Documentation** - Official docs

---

**Last Updated**: 2025-10-30
