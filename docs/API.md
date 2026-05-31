# API Reference Guide

This document describes the public interface of the **LLM over DNS** system, covering both the UDP DNS protocol interface and the internal Rust library API.

---

## 🌐 DNS Protocol Interface

The primary way users and clients interact with the server is via the standard UDP DNS protocol on the configured port.

### 1. Query Format

To send a prompt to the LLM, perform a standard DNS `TXT` query. The prompt is encoded directly into the domain name's labels:

```
[prompt-labels].[domain] TXT
```

* **Domain Suffix**: The server ignores the domain suffix (e.g. `example.com` or `local`).
* **Spaces**: Spaces in the prompt are automatically decoded from hyphens (`-`) or dot separators (`.`).
* **Normalization**: The query name is normalized to lowercase and parsed as UTF-8.

#### Examples:
```bash
# Question: "hello"
dig @localhost -p 5454 hello.local TXT +short

# Question: "what is rust" (spaces as dots)
dig @localhost -p 5454 what.is.rust.local TXT +short

# Question: "explain-quantum-physics" (spaces as hyphens)
dig @localhost -p 5454 explain-quantum-physics.local TXT +short
```

---

### 2. Response Format

The server responds with standard DNS `TXT` records. Due to DNS protocol specifications, individual string fields are capped at **255 characters** each.

* **Under 255 Characters**: Returned inside a single `TXT` record.
* **Over 255 Characters**: Automatically split into multiple `TXT` records within the Answer section of the response packet, maintaining exact reading order.

#### Example Response:
```text
$ dig @localhost -p 5454 'what is rust' TXT +short
"Rust is a systems programming language focused on safety, speed, and concurrency, designed to prevent memory errors and data races at compile time."
```

---

### 3. DNS Status Codes

The server maps errors to RFC-compliant DNS header status codes (`RCODE`):

| Status Code | Name | Scenario |
|---|---|---|
| `0` | **NOERROR** | Request succeeded and response populated. |
| `1` | **FORMERR** | Invalid DNS query format or unsupported record type (non-TXT). |
| `2` | **SERVFAIL** | Upstream LLM gateway failure or network timeout. |

---

## 🦀 Rust Library API

If using `llm-over-dns` as a dependency in your Rust project, the crate exposes the following key components:

### 1. `Config`
Manages application settings loaded from environment variables and dotenv configurations.

```rust
pub struct Config {
    pub openrouter_api_key: String,   // Mapped to ANYROUTER_API_KEY if present
    pub openrouter_model: String,     // Mapped to ANYROUTER_MODEL if present
    pub dns_port: u16,
    pub dns_address: String,
    pub is_anyrouter: bool,
    pub llm_base_url: String,
    pub openrouter_models: Vec<String>,
}

impl Config {
    /// Loads configuration from environment variables and active .env files
    pub fn from_env() -> Result<Self, anyhow::Error>;
}
```

---

### 2. `Server`
Manages the UDP socket listener lifecycle.

```rust
pub struct Server {
    pub config: Config,
}

impl Server {
    /// Instantiates a new Server with the given Config
    pub fn new(config: Config) -> Result<Self, anyhow::Error>;

    /// Starts the async UDP loop listening for incoming DNS queries
    pub async fn start(&self) -> Result<(), anyhow::Error>;
}
```

---

### 3. `Chunker`
Helper utility to split long strings into compliant DNS chunks.

```rust
pub struct Chunker;

impl Chunker {
    /// Splits raw text into an array of strings, each <= 255 bytes
    pub fn chunk(text: &str, max_chunk_size: usize, max_total_size: usize) -> Vec<String>;
}
```

---

### 4. `LlmClient`
Communicates with AnyRouter or OpenRouter APIs.

```rust
pub struct LlmClient {
    pub api_key: String,
    pub base_url: String,
    pub models: Vec<String>,
}

impl LlmClient {
    /// Instantiates a new LlmClient
    pub fn new(api_key: String, base_url: String, models: Vec<String>) -> Self;

    /// Queries the active gateway using the fallback chain of models
    pub async fn query(&self, prompt: &str) -> Result<String, anyhow::Error>;
}
```
