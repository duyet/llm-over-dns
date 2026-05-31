# LLM over DNS - Documentation Index

Welcome to the official documentation for the **LLM over DNS** server. This project implements a high-performance DNS server in Rust that translates DNS queries into AI responses via LLM gateway providers.

---

## 📚 Documentation Structure

### 🚀 Getting Started
- **[Getting Started Guide](GETTING_STARTED.md)** - Learn how to set up the server and run your first DNS query in under 2 minutes.
- **[Configuration Guide](configuration.md)** - Detailed overview of environment variables, fallback models, and configuration priorities.

### 🏗️ Technical Guides
- **[System Architecture](ARCHITECTURE.md)** - In-depth look at the architecture, async Tokio server engine, and data flow.
- **[API Reference](API.md)** - Deep dive into the DNS wire protocol mapping (TXT records, limits, and chunking) and the Rust internal crate APIs.

### 🐳 DevOps & Operations
- **[Docker & Production Deployment](deployment-docker.md)** - Comprehensive deployment guide covering Docker, Docker Compose, systemd, and VPS networking.
- **[Project Status](PROJECT_STATUS.md)** - System metrics, current feature set, and planned next steps.

### 🤝 Developer Guides
- **[Contributing Guide](CONTRIBUTING.md)** - Development guidelines, workflow, code formatting, and test suites.

---

## 🔗 Quick Navigation

| Target | Recommended Guide |
|---|---|
| First-time installation | [Getting Started Guide](GETTING_STARTED.md) |
| Setting API keys and fallback models | [Configuration Guide](configuration.md) |
| Running under Docker / Docker Compose | [Docker Deployment Guide](deployment-docker.md) |
| Exploring DNS chunking & wire formatting | [API Reference](API.md) |
| Understanding stateless handlers | [System Architecture](ARCHITECTURE.md) |
| Submitting a Pull Request | [Contributing Guide](CONTRIBUTING.md) |

---

*For the main project landing page, see [../README.md](../README.md)*
