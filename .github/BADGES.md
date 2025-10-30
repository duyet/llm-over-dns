# Badge Suggestions for README

Add these badges to your `README.md` to display CI/CD status, coverage, and project metrics.

## Status Badges

### CI/CD Workflow
```markdown
[![CI](https://github.com/duyet/llm-over-dns/actions/workflows/ci.yml/badge.svg)](https://github.com/duet/llm-over-dns/actions/workflows/ci.yml)
```
Shows: ✅ Passing or ❌ Failing

### Code Coverage
```markdown
[![codecov](https://codecov.io/gh/duyet/llm-over-dns/branch/main/graph/badge.svg)](https://codecov.io/gh/duyet/llm-over-dns)
```
Shows: Coverage percentage

### Security Audit
```markdown
[![Security Audit](https://github.com/duyet/llm-over-dns/actions/workflows/ci.yml/badge.svg?label=audit)](https://github.com/duyet/llm-over-dns/security)
```
Shows: Security check status

## Project Information Badges

### Crate Version
```markdown
[![Crate](https://img.shields.io/crates/v/llm-over-dns.svg)](https://crates.io/crates/llm-over-dns)
```
Shows: Published version on crates.io

### License
```markdown
[![License](https://img.shields.io/crates/l/llm-over-dns.svg)](https://github.com/duyet/llm-over-dns/blob/main/LICENSE)
```
Shows: MIT license

### Rust Version
```markdown
[![Rust Version](https://img.shields.io/badge/rust-1.70%2B-blue.svg)](https://www.rust-lang.org)
```
Shows: Minimum supported Rust version

### Downloads
```markdown
[![Downloads](https://img.shields.io/crates/d/llm-over-dns.svg)](https://crates.io/crates/llm-over-dns)
```
Shows: Total crates.io downloads

### Documentation
```markdown
[![Documentation](https://docs.rs/llm-over-dns/badge.svg)](https://docs.rs/llm-over-dns)
```
Shows: docs.rs link

## Combined Badge Section

### Recommended Layout

```markdown
# LLM over DNS

[![CI](https://github.com/duyet/llm-over-dns/actions/workflows/ci.yml/badge.svg)](https://github.com/duyet/llm-over-dns/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/duyet/llm-over-dns/branch/main/graph/badge.svg)](https://codecov.io/gh/duyet/llm-over-dns)
[![Crate](https://img.shields.io/crates/v/llm-over-dns.svg)](https://crates.io/crates/llm-over-dns)
[![License](https://img.shields.io/crates/l/llm-over-dns.svg)](https://github.com/duyet/llm-over-dns/blob/main/LICENSE)

> DNS server that responds to TXT queries with LLM responses via OpenRouter

## Features

- ✅ 100% test coverage
- ✅ Automated CI/CD pipeline
- ✅ Security vulnerability scanning
- ✅ Comprehensive documentation
```

## Shields.io Custom Badges

### Build Status
```markdown
![Build](https://img.shields.io/github/actions/workflow/status/duyet/llm-over-dns/ci.yml?branch=main)
```

### Last Commit
```markdown
![Last Commit](https://img.shields.io/github/last-commit/duyet/llm-over-dns)
```

### Contributors
```markdown
![Contributors](https://img.shields.io/github/contributors/duyet/llm-over-dns)
```

### Issues
```markdown
![Issues](https://img.shields.io/github/issues/duyet/llm-over-dns)
```

### Pull Requests
```markdown
![Pull Requests](https://img.shields.io/github/issues-pr/duyet/llm-over-dns)
```

### Stars
```markdown
![Stars](https://img.shields.io/github/stars/duyet/llm-over-dns?style=social)
```

### Forks
```markdown
![Forks](https://img.shields.io/github/forks/duyet/llm-over-dns?style=social)
```

## Advanced Shields

### Code Size
```markdown
![Code Size](https://img.shields.io/github/languages/code-size/duyet/llm-over-dns)
```

### Repository Size
```markdown
![Repo Size](https://img.shields.io/github/repo-size/duyet/llm-over-dns)
```

### Top Language
```markdown
![Language](https://img.shields.io/github/languages/top/duyet/llm-over-dns)
```

### Dependencies Status
```markdown
![Dependencies](https://img.shields.io/librariesio/github/duyet/llm-over-dns)
```

## Custom Badge Examples

### Custom Status
```markdown
![Status](https://img.shields.io/badge/status-active-success.svg)
```

### Custom Platform
```markdown
![Platform](https://img.shields.io/badge/platform-linux%20%7C%20macOS-lightgrey.svg)
```

### Custom Coverage
```markdown
![Coverage](https://img.shields.io/badge/coverage-100%25-brightgreen.svg)
```

### Custom Version
```markdown
![Version](https://img.shields.io/badge/version-0.1.0-blue.svg)
```

## Full Example README Header

```markdown
<div align="center">

# LLM over DNS

[![CI](https://github.com/duyet/llm-over-dns/actions/workflows/ci.yml/badge.svg)](https://github.com/duyet/llm-over-dns/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/duyet/llm-over-dns/branch/main/graph/badge.svg)](https://codecov.io/gh/duyet/llm-over-dns)
[![Crate](https://img.shields.io/crates/v/llm-over-dns.svg)](https://crates.io/crates/llm-over-dns)
[![Documentation](https://docs.rs/llm-over-dns/badge.svg)](https://docs.rs/llm-over-dns)
[![License](https://img.shields.io/crates/l/llm-over-dns.svg)](https://github.com/duyet/llm-over-dns/blob/main/LICENSE)

DNS server that responds to TXT queries with LLM responses via OpenRouter

[Features](#features) • [Installation](#installation) • [Usage](#usage) • [Documentation](#documentation)

</div>

---

## Highlights

- ✅ **100% Test Coverage** - Comprehensive test suite
- ✅ **Automated CI/CD** - GitHub Actions workflow
- ✅ **Security First** - Automated vulnerability scanning
- ✅ **Type Safe** - Written in Rust with strict typing
- ✅ **Production Ready** - Enterprise-grade configuration

## Quick Start

\`\`\`bash
# Install
cargo install llm-over-dns

# Run
llm-over-dns --config config.toml
\`\`\`
```

## Badge Customization

### Style Options

Shields.io supports different badge styles:

```markdown
?style=flat            # Default flat style
?style=flat-square     # Flat with square edges
?style=for-the-badge   # Large badge style
?style=plastic         # Plastic style with gradient
?style=social          # Social media style
```

### Color Options

```markdown
?color=brightgreen     # Success/passing
?color=green           # Good
?color=yellowgreen     # OK
?color=yellow          # Warning
?color=orange          # Alert
?color=red             # Error/failing
?color=blue            # Info
?color=lightgrey       # Neutral
```

### Example with Customization

```markdown
![CI](https://img.shields.io/github/actions/workflow/status/duyet/llm-over-dns/ci.yml?style=for-the-badge&label=BUILD&color=brightgreen)
```

## Tips

1. **Keep it Simple**: Don't overload with too many badges
2. **Prioritize**: Show most important metrics first (CI, coverage, version)
3. **Update Links**: Replace `duyet/llm-over-dns` with actual repo path
4. **Test Badges**: Verify all badge links work before publishing
5. **Align Center**: Use `<div align="center">` for professional look
6. **Group Related**: Keep CI/CD badges together, project info together

## Resources

- [Shields.io](https://shields.io) - Badge creation service
- [Simple Icons](https://simpleicons.org) - Icon library for badges
- [Codecov Badges](https://docs.codecov.io/docs/status-badges) - Coverage badges
- [Crates.io Badges](https://doc.rust-lang.org/cargo/reference/publishing.html#badges) - Cargo badges

---

**Last Updated**: 2025-10-30
