# LLM over DNS - Architecture & Design

## System Overview

LLM over DNS is a DNS server that bridges DNS queries with large language models via the OpenRouter API. The system is designed for simplicity, performance, and reliability.

### High-Level Flow

```
DNS Client                DNS Server              OpenRouter API
    │                         │                          │
    │──── DNS Query ─────────>│                          │
    │    (TXT record)         │                          │
    │                         │──> Parse Question        │
    │                         │──> Chunk Text            │
    │                         │──> Make LLM Request ────>│
    │                         │                          │
    │                         │<──── LLM Response ───────│
    │                         │──> Chunk Response        │
    │<── DNS Response ────────│                          │
    │   (TXT record)          │                          │
```

## Component Architecture

### 1. Configuration Module (`config.rs`)

**Responsibility**: Load and validate server configuration from environment variables.

**Key Types**:
- `Config` - Central configuration structure

**Features**:
- Automatic `.env` file loading
- Sensible defaults for DNS settings
- Validation of required fields
- Type-safe configuration

**Error Handling**:
- Clear error messages for missing API key
- Validation of port numbers
- Context about configuration issues

**Example**:
```rust
let config = Config::from_env()?;
println!("{}:{}", config.dns_address, config.dns_port);
```

### 2. DNS Handler Module (`dns_handler.rs`)

**Responsibility**: Process incoming DNS queries and route to LLM client.

**Key Types**:
- `DnsHandler` - DNS protocol handler
- `DnsMessage` - Parsed DNS query/response

**Features**:
- TXT record query handling
- Subdomain label parsing
- DNS response formatting
- Error response generation

**Protocol Details**:
- Handles DNS over UDP (standard)
- Supports DNS over TCP (for fallback)
- Returns chunked TXT responses
- Implements proper DNS headers and flags

**Request Format**:
```
question.example.com TXT

Labels are parsed as:
- "hello-world" → "hello world" (hyphens become spaces)
- "why-is-rust-great" → "why is rust great"
- Multiple labels combined: "why.is.rust.great" → "why is rust great"
```

**Response Format**:
```
example.com.   IN TXT "Response part 1 (up to 255 chars)"
               IN TXT "Response part 2 (up to 255 chars)"
               IN TXT ...
```

### 3. LLM Client Module (`llm_client.rs`)

**Responsibility**: Communicate with OpenRouter API for LLM inference.

**Key Types**:
- `LlmClient` - API client wrapper
- `LlmRequest` - Request to LLM
- `LlmResponse` - Response from LLM

**Features**:
- HTTP client using reqwest
- JSON request/response handling
- Error handling and retries
- Timeout management
- Model selection

**API Integration**:
- Endpoint: `https://openrouter.ai/api/v1/chat/completions`
- Authentication: Bearer token in headers
- Request format: OpenAI-compatible chat API
- Response format: Streaming or non-streaming

**Request Structure**:
```json
{
  "model": "nvidia/nemotron-nano-12b-v2-vl:free",
  "messages": [
    {"role": "user", "content": "Your question here"}
  ]
}
```

**Error Handling**:
- Network errors with retries
- API rate limiting (429 status)
- Invalid model errors
- Timeout handling

### 4. Chunker Module (`chunker.rs`)

**Responsibility**: Handle text chunking due to DNS 255-character limit per TXT record.

**Key Types**:
- `Chunker` - Text splitting/joining utility

**Features**:
- Split long text into 255-byte chunks
- Preserve word boundaries when possible
- Reconstruct chunked text
- Handle UTF-8 edge cases

**Design Rationale**:
DNS TXT records have a 255-character limit per string within the record. Long LLM responses must be split across multiple TXT strings within a single response.

**Algorithm**:
```
1. Split text into chunks ≤ 255 bytes
2. Prefer breaking at word boundaries
3. Each chunk is a separate TXT string in DNS response
4. Client automatically concatenates all TXT strings
```

**Example**:
```rust
let chunker = Chunker::new();
let chunks = chunker.chunk("Very long text...", 255);
// Returns: ["Chunk 1...", "Chunk 2...", ...]

let full = chunker.dechunk(&chunks);
// Returns: "Very long text..."
```

## Data Flow

### Request Processing

```
1. DNS Client sends query
   ↓
2. Server receives UDP packet on port 53
   ↓
3. DnsHandler parses DNS query
   ↓
4. Extract question name (subdomain labels)
   ↓
5. Parse as plain text question
   ↓
6. LlmClient makes request to OpenRouter
   ↓
7. Await API response
   ↓
8. Chunker splits response into 255-char chunks
   ↓
9. DnsHandler builds DNS TXT response
   ↓
10. Server sends DNS response back to client
```

### Error Flow

```
Error occurs (network, API, parsing)
   ↓
Generate appropriate DNS RCODE:
   - SERVFAIL: Server error
   - NXDOMAIN: Invalid query
   - REFUSED: Access denied
   ↓
Return DNS error response to client
   ↓
Log error for debugging
```

## Message Formats

### DNS Query Message

```
Header:
  ID: Random
  QR: 0 (Query)
  Opcode: 0 (Standard query)
  RD: 1 (Recursion desired)

Question:
  Name: question.example.com
  Type: TXT (16)
  Class: IN (1)
```

### DNS Response Message

```
Header:
  ID: Same as query
  QR: 1 (Response)
  AA: 1 (Authoritative)
  RD: 1 (Recursion desired)
  RA: 0 (Recursion not available)
  RCODE: 0 (No error)

Question:
  (Echo of query)

Answer:
  Name: question.example.com
  Type: TXT (16)
  Class: IN (1)
  TTL: 300
  RDATA:
    - "Response part 1 (≤255 chars)"
    - "Response part 2 (≤255 chars)"
    - ...
```

## Key Design Decisions

### 1. DNS-Based Protocol

**Why DNS?**
- Universal availability (port 53 open almost everywhere)
- Works through firewalls and proxies
- No additional software needed (built-in tools like `dig`)
- Simple text-based interface

**Implications**:
- 255-character limit per TXT string (solved with chunking)
- DNS response caching (not ideal for dynamic content)
- DNS resolution timeouts

### 2. OpenRouter API

**Why OpenRouter?**
- Free tier with no usage limits
- Multiple models available
- OpenAI-compatible API
- Handles authentication centrally

**Models**:
- `nvidia/nemotron-nano-12b-v2-vl:free` - Fast, lightweight (recommended)
- `meta-llama/llama-2-7b-chat:free` - Balanced
- Others available at openrouter.io

### 3. Async/Await with Tokio

**Why Tokio?**
- Handle multiple concurrent DNS queries
- Non-blocking I/O for API calls
- Efficient resource usage
- Industry standard in Rust

**Benefits**:
- Single-threaded event loop (simple concurrency)
- Minimal overhead per query
- Built-in timeout support

### 4. Chunking Strategy

**Why split at 255 chars?**
- DNS TXT record limitation
- Automatic by clients (transparent to users)
- Preserves response integrity
- Simple to implement correctly

**Alternative Approaches**:
- Multiple DNS queries (complex UX)
- Compression (reduces readability)
- Truncation (loses data)

## Security Considerations

### Input Validation

- DNS queries validated against RFC 1035
- Invalid queries rejected with FORMERR
- Question name sanitized before LLM request
- Size limits on chunked text

### Output Validation

- LLM responses validated as valid UTF-8
- Chunking preserves all content
- DNS response format verified

### API Key Security

- Stored in environment variables (not in code)
- `.env` file excluded from git
- Never logged or exposed in responses
- Should be rotated regularly

### Network Security

- Runs on localhost by default in development
- Firewall recommended for production
- Consider DNS rate limiting
- Monitor for abuse patterns

### DNS Spoofing Protection

- Implement DNSSEC for production
- Use known resolvers for queries
- Monitor for anomalous patterns

## Performance Characteristics

### Latency

```
Client Query
  └─> DNS Resolution: ~5-50ms
      └─> LLM API Call: 500ms - 30s (depends on model and query)
          └─> Response Chunking: 1-5ms
              └─> DNS Response: ~50ms
                  └─> Total: 600ms - 30s
```

### Throughput

- Single server instance: 100-1000 QPS (depends on system)
- Limited by OpenRouter API rate limits
- Network bandwidth typically not bottleneck

### Resource Usage

- Memory: ~50MB baseline + ~5MB per concurrent query
- CPU: ~10% idle, scales with concurrent requests
- Network: Minimal (DNS is low-bandwidth)

## Scaling Strategies

### Horizontal Scaling

```
Load Balancer (DNS round-robin)
  ├── Server Instance 1 (port 53)
  ├── Server Instance 2 (port 53)
  └── Server Instance 3 (port 53)

Each instance:
  - Independent configuration
  - Shared OpenRouter API key
  - Monitor OpenRouter rate limits
```

### Caching Layer

```
DNS Client
  └─> Cache Server
      └─> Authoritative Server
          └─> OpenRouter API
```

Future enhancement: Add response caching to avoid repeated API calls.

### Connection Pooling

```
Single HTTP connection pool across:
- Multiple DNS queries
- Multiple LLM requests
- Connection reuse from reqwest
```

## Testing Strategy

### Unit Tests

- Configuration loading and validation
- Text chunking edge cases
- Error handling

### Integration Tests

- End-to-end DNS queries
- OpenRouter API mocking
- Response formatting

### Manual Testing

```bash
# Basic query
dig @localhost hello.example.com TXT +short

# Longer query
dig @localhost explain-dns-protocol.example.com TXT +short

# Debug output
dig @localhost -t TXT hello.example.com
```

## Future Enhancements

### Planned Features

1. **Response Caching**
   - LRU cache for repeated queries
   - TTL-based expiration
   - Memory-efficient implementation

2. **Alternative LLM Providers**
   - Anthropic Claude API
   - OpenAI API
   - Local LLM support

3. **Advanced DNS Features**
   - DNS over HTTPS (DoH)
   - DNSSEC signing
   - Zone file support

4. **Monitoring & Observability**
   - Prometheus metrics
   - OpenTelemetry integration
   - Query analytics

5. **Rate Limiting**
   - Per-IP rate limiting
   - OpenRouter quota tracking
   - Graceful degradation

## Dependencies

### Core Dependencies

| Dependency | Version | Purpose |
|------------|---------|---------|
| hickory-dns | 0.25 | DNS protocol implementation |
| hickory-server | 0.25 | DNS server implementation |
| tokio | 1.35 | Async runtime |
| reqwest | 0.11 | HTTP client |
| serde | 1.0 | JSON serialization |
| anyhow | 1.0 | Error handling |
| tracing | 0.1 | Structured logging |
| dotenvy | 0.15 | .env file loading |

### Development Dependencies

| Dependency | Version | Purpose |
|------------|---------|---------|
| mockito | 1.2 | HTTP mocking |
| tokio-test | 0.4 | Test utilities |
| hickory-client | 0.25 | DNS client |
| assert_matches | 1.5 | Test assertions |
| pretty_assertions | 1.4 | Pretty test output |

## Deployment Models

### Single Server

```
Client ──> DNS Server (Port 53)
               ├── OpenRouter API
               └── .env configuration
```

### Docker Container

```
Client ──> Docker Container
           ├── DNS Server
           ├── OpenRouter Client
           └── Configuration (env vars)
```

### Systemd Service

```
Client ──> Systemd Service
           ├── Binary: /usr/local/bin/llm-over-dns
           ├── Config: /etc/llm-over-dns/.env
           └── Logs: journalctl
```

## Monitoring & Logging

### Log Levels

```
ERROR: Fatal errors preventing operation
WARN:  Recoverable errors, degraded operation
INFO:  Normal operations, interesting events
DEBUG: Detailed diagnostic information
TRACE: Very detailed tracing
```

### Metrics to Monitor

- Query count per second (QPS)
- Average response time
- Error rate (by error type)
- OpenRouter API usage
- Memory and CPU usage
- Active concurrent connections

### Recommended Tools

- ELK Stack for logs
- Prometheus for metrics
- Grafana for visualization
- AlertManager for alerting

## References

- [RFC 1035 - DNS Protocol](https://tools.ietf.org/html/rfc1035)
- [OpenRouter API Docs](https://openrouter.ai/docs)
- [Hickory DNS Docs](https://docs.rs/hickory-dns/)
- [Tokio Docs](https://tokio.rs/)

---

Last Updated: 2025-10-30
