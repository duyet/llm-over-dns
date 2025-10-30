//! LLM over DNS: Query large language models using DNS
//!
//! This library provides a high-performance DNS server that responds to TXT queries
//! with LLM responses via OpenRouter. Ask questions using DNS, get answers powered by AI.
//!
//! # Architecture
//!
//! The system consists of five main components:
//!
//! 1. **[`Config`]** - Configuration management from environment variables
//! 2. **[`Server`]** - Main DNS server with lifecycle management
//! 3. **[`DnsHandler`]** - DNS protocol handling and query routing
//! 4. **[`LlmClient`]** - OpenRouter API client for LLM inference
//! 5. **[`Chunker`]** - Text chunking/dechunking for DNS 255-char limit
//!
//! # Quick Start
//!
//! ## Library Usage
//!
//! ```no_run
//! use llm_over_dns::{Config, Server};
//!
//! # async fn example() -> anyhow::Result<()> {
//! // Load configuration
//! let config = Config::from_env()?;
//!
//! // Create and start server
//! let server = Server::new(config)?;
//! server.start().await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Usage
//!
//! Query the server using `dig`:
//!
//! ```bash
//! dig @localhost hello-world.llm.duyet.net TXT
//! ```
//!
//! The server will:
//! 1. Parse the question from DNS subdomain labels
//! 2. Call OpenRouter LLM API
//! 3. Return chunked response in TXT records
//!
//! # Configuration
//!
//! Set environment variables or create a `.env` file:
//!
//! ```text
//! OPENROUTER_API_KEY=sk_your_key_here
//! OPENROUTER_MODEL=nvidia/nemotron-nano-12b-v2-vl:free
//! DNS_PORT=53
//! DNS_ADDRESS=0.0.0.0
//! RUST_LOG=info
//! ```
//!
//! # Module Organization
//!
//! - [`config`] - Configuration loading and validation
//! - [`server`] - DNS server lifecycle management
//! - [`dns_handler`] - DNS query parsing and response building
//! - [`llm_client`] - OpenRouter API client with error handling
//! - [`chunker`] - Text chunking utilities for DNS limitations
//!
//! # Examples
//!
//! See the `examples/` directory for complete working examples:
//! - `simple_query.rs` - Basic LLM query
//! - `custom_config.rs` - Custom configuration
//! - `error_handling.rs` - Comprehensive error handling

pub mod chunker;
pub mod config;
pub mod dns_handler;
pub mod llm_client;
pub mod server;

pub use chunker::Chunker;
pub use config::Config;
pub use dns_handler::DnsHandler;
pub use llm_client::LlmClient;
pub use server::{LlmDnsHandler, Server};
