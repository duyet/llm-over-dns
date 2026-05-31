# Getting Started Guide

Welcome to **LLM over DNS**! This guide will help you install, configure, and query your DNS-based AI server in under 2 minutes.

---

## 🚀 Quick Start (2 Minutes)

### 1. Setup the Server

Clone the repository and prepare your environment configuration:

```bash
# 1. Clone the project
git clone https://github.com/duyet/llm-over-dns.git
cd llm-over-dns

# 2. Initialize your configuration
cp .env.example .env
```

Edit the `.env` file to add your API key. **AnyRouter is highly recommended** for its performance, latency, and native fallback handling.

```env
# Edit .env and configure one of the gateway providers:

# Recommended: AnyRouter (switches automatically if set)
ANYROUTER_API_KEY=sk-ar-v1-your-key-here

# Or OpenRouter:
# OPENROUTER_API_KEY=sk-or-v1-your-key-here
```

### 2. Launch the DNS Server

Run the server on a non-privileged port (e.g. `5454`) to avoid MDNS or system conflicts on port `53`:

```bash
DNS_PORT=5454 cargo run --release
```

**Expected Startup Output:**
```text
=== Configuration ===
Provider: AnyRouter API (https://anyrouter.dev)
API Key: sk-ar-v1...*** (masked)
Models (with fallback): ["google/gemini-2.5-flash-lite", "meta/llama-3.2-3b-instruct"]
DNS Server: 0.0.0.0:5454

=== Components Initialized ===
✓ LLM client ready
✓ Chunker ready (max chunk: 250 bytes, max total: 4096 bytes)
✓ DNS handler ready
✓ DNS server ready

=== Server Ready ===
Press Ctrl+C to stop
Server task starting...
DNS server listening on 0.0.0.0:5454
Waiting for DNS queries...
```

### 3. Send a Test Query

In a separate terminal window, query the local DNS server using standard DNS lookup utility `dig`:

```bash
dig @localhost -p 5454 'what is rust programming in one sentence' TXT +time=30 +short
```

**Expected Response Capture:**
```text
"Rust is a systems programming language focused on safety, speed, and concurrency, designed to prevent memory errors and data races at compile time."
```

---

## 💡 Core Concepts

### DNS Wire Format for Queries
You can ask the server anything by passing the prompt as a subdomain query:
```text
your-question-here.example.com TXT
```
The server is stateless and maps the query directly to an LLM prompt. Any suffix/domain (like `.example.com` or `.local`) is ignored, allowing you to ask questions naturally.

### DNS TXT Response Chunking
DNS has a hard protocol limit of **255 characters** per TXT record. If the AI's response is longer than 255 characters, the server automatically splits the response into multiple TXT records, preserving the correct display order.

---

## 🤔 Frequently Asked Questions

#### Q: How do I get an API Key?
* **AnyRouter (Recommended)**: Sign up at [anyrouter.dev](https://anyrouter.dev), navigate to Keys, and create a key (starts with `sk-ar-`).
* **OpenRouter**: Sign up at [openrouter.ai](https://openrouter.ai), navigate to Keys, and create a key (starts with `sk-or-`).

#### Q: Can I run the server on port 53?
Yes. Port `53` is the standard DNS port. Running on port `53` requires root/administrator privileges:
```bash
sudo DNS_PORT=53 cargo run --release
```
*Note: Make sure to stop any system resolver (like `systemd-resolved` or local DNS servers) first to avoid port binding conflicts.*

#### Q: What if the DNS client times out?
LLM gateway requests can take a few seconds to complete. Standard `dig` utility times out in `5` seconds by default. We recommend adding `+time=30` (or `+timeout=30`) to your `dig` commands to allow ample time for the model to generate the response.

---

## 📚 Next Steps

* **[Configuration Guide](configuration.md)** - Learn about fallback models, bind address custom configurations, and logging levels.
* **[System Architecture](ARCHITECTURE.md)** - Understand the internal design and how the stateless DNS handler operates.
* **[Docker Deployment Guide](deployment-docker.md)** - Step-by-step instructions for deploying to a VPS using Docker and Docker Compose.
