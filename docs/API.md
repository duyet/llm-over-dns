# LLM over DNS - API Documentation

Complete reference documentation for the LLM over DNS API and library.

## Table of Contents

- [DNS Protocol API](#dns-protocol-api)
- [Library API](#library-api)
- [Configuration API](#configuration-api)
- [Data Types](#data-types)
- [Error Handling](#error-handling)
- [Examples](#examples)

## DNS Protocol API

### Query Format

LLM over DNS uses DNS TXT queries to send questions and receive responses.

#### Query Syntax

```
[question].example.com TXT
```

Where `[question]` is your question encoded as DNS labels:

```bash
# Simple question (spaces become hyphens)
dig hello-world.example.com TXT

# Multi-word question
dig what-is-rust-programming.example.com TXT

# Single letter/short labels
dig a.b.c.example.com TXT
```

#### Label Rules

- Labels are separated by dots
- Spaces within a label are represented as hyphens
- Each label can be up to 63 characters
- Case-insensitive (normalized to lowercase)
- ASCII alphanumerics, hyphens, and underscores supported

#### Examples

```
Question: "Hello"
Query:    hello.example.com TXT

Question: "What is Rust?"
Query:    what.is.rust.example.com TXT

Question: "How do DNS queries work?"
Query:    how.do.dns.queries.work.example.com TXT

Question: "Explain neural networks in detail"
Query:    explain.neural.networks.in.detail.example.com TXT
```

### Response Format

The server responds with DNS TXT records containing the LLM's response.

#### Response Structure

```
example.com.   IN TXT "Response text part 1 (≤255 chars)"
               IN TXT "Response text part 2 (≤255 chars)"
               IN TXT ...
```

#### Response Characteristics

- **Record Type**: TXT (16)
- **Class**: IN (Internet)
- **TTL**: 300 seconds (configurable)
- **Data**: Multiple TXT strings, each ≤255 characters
- **Chunking**: Automatic splitting of long responses
- **Encoding**: UTF-8 with proper escape sequences

#### Example Response

```
$ dig what-is-ai.example.com TXT +short

"AI (Artificial Intelligence) is a branch of computer science that aims"
" to create systems that can perform tasks that typically require human i"
"ntelligence. These tasks include learning, reasoning, problem-solving, p"
"erception, and language understanding."
```

### Error Responses

The server returns standard DNS error codes:

| Code | Name | Meaning |
|------|------|---------|
| 0 | NOERROR | Success |
| 1 | FORMERR | Malformed query |
| 2 | SERVFAIL | Server error |
| 3 | NXDOMAIN | Domain not found |
| 4 | NOTIMPL | Not implemented |
| 5 | REFUSED | Request refused |

#### Error Examples

```bash
# Invalid query format
$ dig invalid..example.com TXT
; <<>> DiG 9.10.6 <<>> invalid..example.com TXT
; (1 server found)
;; global options: +cmd
;; Got answer:
;; ->>HEADER<<- opcode: QUERY, status: FORMERR

# Server error (e.g., API failure)
$ dig any-query.example.com TXT
; <<>> DiG 9.10.6 <<>> any-query.example.com TXT
;; Got answer:
;; ->>HEADER<<- opcode: QUERY, status: SERVFAIL
```

### Query Tools

Any DNS client can query the server:

```bash
# dig (recommended)
dig @localhost hello.example.com TXT

# nslookup
nslookup -type=TXT hello.example.com localhost

# host
host -t TXT hello.example.com localhost

# drill
drill @localhost hello.example.com TXT

# Python
import dns.resolver
answers = dns.resolver.resolve('hello.example.com', 'TXT')
for rdata in answers:
    print(rdata)

# Node.js
const dns = require('dns');
dns.resolveTxt('hello.example.com', (err, records) => {
  if (err) throw err;
  console.log(records);
});

# Rust
use dns_lookup::lookup_host;
let ips = lookup_host("example.com")?;
```

## Library API

The Rust library provides programmatic access to the DNS server components.

### Module Structure

```rust
use llm_over_dns::{
    Config,       // Configuration management
    DnsHandler,   // DNS protocol handling
    LlmClient,    // OpenRouter API client
    Chunker,      // Text chunking utilities
};
```

### Core Types

#### Config

Configuration for the DNS server.

```rust
pub struct Config {
    pub openrouter_api_key: String,
    pub openrouter_model: String,
    pub dns_port: u16,
    pub dns_address: String,
}
```

**Methods**:

```rust
impl Config {
    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self>
}
```

**Example**:

```rust
use llm_over_dns::Config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::from_env()?;

    println!("API Key: {}", config.openrouter_api_key);
    println!("Model: {}", config.openrouter_model);
    println!("DNS Port: {}", config.dns_port);
    println!("DNS Address: {}", config.dns_address);

    Ok(())
}
```

#### DnsHandler

Handles DNS protocol operations.

```rust
pub struct DnsHandler {
    // Internal implementation
}
```

**Methods**:

```rust
impl DnsHandler {
    /// Create a new DNS handler
    pub fn new() -> Self

    /// Handle an incoming DNS query
    pub async fn handle_query(&self, query: &[u8]) -> Result<Vec<u8>>

    /// Parse DNS message
    pub fn parse_message(&self, data: &[u8]) -> Result<DnsMessage>

    /// Build DNS response
    pub fn build_response(&self, response_text: &str) -> Result<Vec<u8>>
}
```

**Example**:

```rust
use llm_over_dns::DnsHandler;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let handler = DnsHandler::new();

    // Handle incoming DNS query
    let query_bytes = b"..."; // DNS protocol bytes
    let response = handler.handle_query(query_bytes).await?;

    Ok(())
}
```

#### LlmClient

Client for OpenRouter API.

```rust
pub struct LlmClient {
    api_key: String,
    model: String,
    client: reqwest::Client,
}
```

**Methods**:

```rust
impl LlmClient {
    /// Create a new LLM client
    pub fn new(api_key: String, model: String) -> Self

    /// Get response from LLM
    pub async fn get_response(&self, question: &str) -> Result<String>

    /// Get response with custom parameters
    pub async fn get_response_with_params(
        &self,
        question: &str,
        temperature: f32,
        max_tokens: u32,
    ) -> Result<String>
}
```

**Example**:

```rust
use llm_over_dns::LlmClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = LlmClient::new(
        "sk_free_xxxxx".to_string(),
        "nvidia/nemotron-nano-12b-v2-vl:free".to_string(),
    );

    let response = client.get_response("What is Rust?").await?;
    println!("Response: {}", response);

    Ok(())
}
```

#### Chunker

Utility for splitting and joining text.

```rust
pub struct Chunker {
    // Internal implementation
}
```

**Methods**:

```rust
impl Chunker {
    /// Create a new chunker
    pub fn new() -> Self

    /// Split text into chunks of max_size bytes
    pub fn chunk(&self, text: &str, max_size: usize) -> Vec<String>

    /// Join chunks back into original text
    pub fn dechunk(&self, chunks: &[String]) -> String

    /// Get optimal chunk size for DNS TXT records
    pub fn dns_optimal_size(&self) -> usize
}
```

**Example**:

```rust
use llm_over_dns::Chunker;

fn main() {
    let chunker = Chunker::new();

    let long_text = "Very long text that exceeds 255 characters...";
    let chunks = chunker.chunk(long_text, 255);

    println!("Chunks: {:?}", chunks);

    let reconstructed = chunker.dechunk(&chunks);
    assert_eq!(long_text, reconstructed);
}
```

## Configuration API

### Environment Variables

All configuration is loaded from environment variables:

```bash
# Required
OPENROUTER_API_KEY=sk_free_xxxxx

# Optional (with defaults)
OPENROUTER_MODEL=nvidia/nemotron-nano-12b-v2-vl:free
DNS_PORT=53
DNS_ADDRESS=0.0.0.0
RUST_LOG=info
```

### Configuration Methods

#### From Environment

```rust
use llm_over_dns::Config;

let config = Config::from_env()?;
```

#### From .env File

```bash
# Create .env
OPENROUTER_API_KEY=sk_free_xxxxx
OPENROUTER_MODEL=nvidia/nemotron-nano-12b-v2-vl:free
DNS_PORT=5353
DNS_ADDRESS=127.0.0.1
```

The library automatically loads `.env` file if present.

#### Programmatic Configuration

```rust
use std::env;

// Set environment variables
env::set_var("OPENROUTER_API_KEY", "sk_free_xxxxx");
env::set_var("OPENROUTER_MODEL", "nvidia/nemotron-nano-12b-v2-vl:free");
env::set_var("DNS_PORT", "5353");
env::set_var("DNS_ADDRESS", "127.0.0.1");

// Load config
let config = Config::from_env()?;
```

### Available Models

Popular free models on OpenRouter:

| Model | Speed | Quality | Recommended |
|-------|-------|---------|-------------|
| `nvidia/nemotron-nano-12b-v2-vl:free` | Very Fast | Good | ✓ |
| `meta-llama/llama-2-7b-chat:free` | Fast | Very Good | - |
| `google/flan-t5-base:free` | Very Fast | Fair | - |
| `bigcode/starling-lm-7b:free` | Fast | Good | - |

For complete list: https://openrouter.ai/models

## Data Types

### Standard Types

#### Result Type

All fallible operations return `Result<T>`:

```rust
use anyhow::Result;

pub async fn example() -> Result<String> {
    let config = Config::from_env()?;
    Ok(config.openrouter_model)
}
```

Error types:

- `anyhow::Error` - Generic error with context
- `std::io::Error` - I/O errors
- `reqwest::Error` - HTTP client errors
- `serde_json::Error` - JSON parsing errors

#### String Types

- `String` - Owned, mutable UTF-8 string
- `&str` - String slice reference
- `&'static str` - Borrowed static string

### DNS Types

#### Message

Represents a complete DNS message (query or response).

**Fields**:
- `id: u16` - Message identifier
- `is_response: bool` - True if response, false if query
- `opcode: Opcode` - Query operation code
- `authoritative: bool` - Authoritative answer flag
- `recursion_desired: bool` - Recursion desired flag
- `recursion_available: bool` - Recursion available flag
- `response_code: ResponseCode` - Response code (for responses)
- `questions: Vec<Question>` - Questions in message
- `answers: Vec<RData>` - Answer records
- `authorities: Vec<RData>` - Authority records
- `additionals: Vec<RData>` - Additional records

#### Question

Represents a DNS question.

**Fields**:
- `name: String` - Domain name being queried
- `qtype: RecordType` - Type of record (TXT, A, AAAA, etc.)
- `qclass: Class` - Query class (IN, CH, HS, etc.)

#### RData

Represents DNS resource record data.

**Variants**:
- `TXT(Vec<String>)` - Text records
- `A(Ipv4Addr)` - IPv4 address
- `AAAA(Ipv6Addr)` - IPv6 address
- `CNAME(String)` - Canonical name
- `SOA(SoaData)` - Start of Authority
- ... (other record types)

#### RecordType

DNS record type enumeration.

**Values**:
- `A` - IPv4 address (1)
- `NS` - Nameserver (2)
- `CNAME` - Canonical name (5)
- `SOA` - Start of Authority (6)
- `MX` - Mail exchange (15)
- `TXT` - Text record (16)
- `AAAA` - IPv6 address (28)
- ... (other types)

#### ResponseCode

DNS response code enumeration.

**Values**:
- `NOERROR` - No error (0)
- `FORMERR` - Format error (1)
- `SERVFAIL` - Server failure (2)
- `NXDOMAIN` - Non-existent domain (3)
- `NOTIMPL` - Not implemented (4)
- `REFUSED` - Query refused (5)

## Error Handling

### Error Types

The library uses `anyhow::Result<T>` for error handling:

```rust
use anyhow::{Result, Context};

fn example() -> Result<String> {
    let config = Config::from_env()
        .context("Failed to load configuration")?;
    Ok(config.openrouter_api_key)
}
```

### Common Errors

| Error | Cause | Resolution |
|-------|-------|-----------|
| `OPENROUTER_API_KEY not set` | Missing API key | Set `OPENROUTER_API_KEY` env var |
| `Invalid DNS_PORT` | Port not a number | Use valid u16 (1-65535) |
| `Connection refused` | DNS server not running | Start DNS server on port |
| `Request timeout` | API took too long | Check OpenRouter API status |
| `Invalid API response` | API returned unexpected data | Check API documentation |

### Error Handling Patterns

#### With Context

```rust
use anyhow::{Context, Result};

fn load_config() -> Result<Config> {
    Config::from_env()
        .context("Failed to load configuration from environment")
}
```

#### With Custom Messages

```rust
use anyhow::{anyhow, Result};

fn validate_port(port: u16) -> Result<u16> {
    if port == 0 {
        return Err(anyhow!("Port cannot be 0"));
    }
    Ok(port)
}
```

#### Propagation

```rust
use anyhow::Result;

async fn chain_operations() -> Result<String> {
    let config = Config::from_env()?;
    let client = LlmClient::new(
        config.openrouter_api_key,
        config.openrouter_model,
    );
    let response = client.get_response("Hello").await?;
    Ok(response)
}
```

## Examples

### Example 1: Basic Query

```rust
use llm_over_dns::LlmClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = LlmClient::new(
        "sk_free_xxxxx".to_string(),
        "nvidia/nemotron-nano-12b-v2-vl:free".to_string(),
    );

    let response = client.get_response("What is Rust?").await?;
    println!("Response: {}", response);

    Ok(())
}
```

### Example 2: Chunked Response

```rust
use llm_over_dns::Chunker;

fn main() {
    let chunker = Chunker::new();

    let long_response = "This is a very long response that exceeds the 255-character DNS TXT record limit...";
    let chunks = chunker.chunk(long_response, 255);

    for (i, chunk) in chunks.iter().enumerate() {
        println!("Chunk {}: {}", i + 1, chunk);
    }

    // Reconstruct
    let reconstructed = chunker.dechunk(&chunks);
    assert_eq!(long_response, reconstructed);
}
```

### Example 3: Configuration Loading

```rust
use llm_over_dns::Config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load from environment or .env file
    let config = Config::from_env()?;

    println!("API Key: {}", config.openrouter_api_key);
    println!("Model: {}", config.openrouter_model);
    println!("Server: {}:{}", config.dns_address, config.dns_port);

    Ok(())
}
```

### Example 4: DNS Handler

```rust
use llm_over_dns::DnsHandler;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let handler = DnsHandler::new();

    // Simulate incoming DNS query (bytes)
    let query_bytes = b"..."; // Actual DNS packet

    // Handle the query
    let response_bytes = handler.handle_query(query_bytes).await?;

    // Send response back to client
    println!("Sent {} bytes", response_bytes.len());

    Ok(())
}
```

### Example 5: Error Handling

```rust
use anyhow::{Result, Context};
use llm_over_dns::{Config, LlmClient};

#[tokio::main]
async fn main() -> Result<()> {
    // Load config with error context
    let config = Config::from_env()
        .context("Failed to load configuration")?;

    // Create client
    let client = LlmClient::new(
        config.openrouter_api_key,
        config.openrouter_model,
    );

    // Make request with error handling
    match client.get_response("Test").await {
        Ok(response) => println!("Response: {}", response),
        Err(e) => eprintln!("Error: {:?}", e),
    }

    Ok(())
}
```

### Example 6: Command-Line Interface

```bash
# Query with dig
dig @localhost what-is-ai.example.com TXT

# Pretty output
dig @localhost what-is-ai.example.com TXT +short

# Verbose output
dig @localhost what-is-ai.example.com TXT +trace

# Using nslookup
nslookup -type=TXT what-is-ai.example.com localhost

# Using host
host -t TXT what-is-ai.example.com localhost
```

## Testing

### Unit Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunker() {
        let chunker = Chunker::new();
        let text = "Hello, world!";
        let chunks = chunker.chunk(text, 255);
        let reconstructed = chunker.dechunk(&chunks);
        assert_eq!(text, reconstructed);
    }

    #[tokio::test]
    async fn test_llm_client() -> anyhow::Result<()> {
        let client = LlmClient::new(
            "sk_free_test".to_string(),
            "test_model".to_string(),
        );
        // Test implementation
        Ok(())
    }
}
```

### Integration Testing

```bash
# Start server in background
DNS_PORT=5353 cargo run &
SERVER_PID=$!

# Run tests
sleep 1
dig @localhost -p 5353 hello.example.com TXT

# Stop server
kill $SERVER_PID
```

## Generating Documentation

```bash
# Generate and open documentation
cargo doc --open

# Include private items
cargo doc --document-private-items --open

# For published crate
cargo doc --release --open
```

The generated documentation includes:
- Type definitions and methods
- Module documentation
- Examples from doc comments
- Links to related items

---

For more information, see:
- [README.md](./README.md) - User guide
- [ARCHITECTURE.md](./ARCHITECTURE.md) - System design
- [CONTRIBUTING.md](./CONTRIBUTING.md) - Development guide
