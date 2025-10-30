//! DNS Server implementation with LLM integration
//!
//! This module provides the core DNS server functionality with proper
//! separation of concerns, dependency injection, and testability.
//!
//! # Architecture
//!
//! - `Server`: Main server struct managing lifecycle and components
//! - `LlmDnsHandler`: DNS query processor integrating LLM responses
//! - Graceful shutdown support with proper resource cleanup
//! - Dependency injection for testing and flexibility
//!
//! # Example
//!
//! ```no_run
//! use llm_over_dns::{Config, Server};
//! use std::sync::Arc;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let config = Config::from_env()?;
//! let server = Server::new(config)?;
//! server.start().await?;
//! # Ok(())
//! # }
//! ```

use anyhow::{Context, Result};
use hickory_server::proto::op::{Message, MessageType, OpCode, ResponseCode};
use hickory_server::proto::rr::rdata::TXT;
use hickory_server::proto::rr::{Name, RData, Record, RecordType};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

use crate::{Chunker, Config, DnsHandler, LlmClient};

/// DNS query handler that integrates with LLM
///
/// This handler processes DNS TXT queries by:
/// 1. Parsing the query subdomain to extract the prompt
/// 2. Querying the LLM with the extracted prompt
/// 3. Chunking the response into DNS-compliant TXT records
/// 4. Building and returning DNS records
pub struct LlmDnsHandler {
    llm_client: Arc<LlmClient>,
    chunker: Arc<Chunker>,
    dns_handler: Arc<DnsHandler>,
}

impl LlmDnsHandler {
    /// Creates a new LLM DNS handler with injected dependencies
    ///
    /// # Arguments
    ///
    /// * `llm_client` - Client for LLM API interaction
    /// * `chunker` - Text chunking utility for DNS TXT record limits
    /// * `dns_handler` - DNS protocol handler
    pub fn new(
        llm_client: Arc<LlmClient>,
        chunker: Arc<Chunker>,
        dns_handler: Arc<DnsHandler>,
    ) -> Self {
        Self {
            llm_client,
            chunker,
            dns_handler,
        }
    }

    /// Processes a single DNS query and returns DNS records
    ///
    /// # Arguments
    ///
    /// * `query_name` - The DNS name from the query
    ///
    /// # Returns
    ///
    /// Vector of DNS records containing the chunked LLM response
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Subdomain parsing fails
    /// - LLM API call fails
    /// - Response chunking fails
    pub async fn process_query(&self, query_name: &Name) -> Result<Vec<Record>> {
        // Extract the query domain from the DNS name
        let query_str = query_name.to_utf8();
        debug!("Raw query string: {}", query_str);

        // Parse subdomain to get the prompt
        let prompt = self.dns_handler.parse_subdomain(&query_str)?;
        debug!("Parsed prompt: {}", prompt);

        // Query the LLM with the prompt
        let response_text = self.llm_client.query(&prompt).await?;
        debug!("LLM response length: {}", response_text.len());

        // Chunk the response for DNS TXT records
        let chunks = self.chunker.chunk_text(&response_text);
        debug!("Chunked into {} parts", chunks.len());

        // Build TXT records from chunks
        let mut records = Vec::new();

        for (index, chunk) in chunks.iter().enumerate() {
            let txt_record = TXT::new(vec![chunk.clone()]);

            let record = Record::from_rdata(
                query_name.clone(),
                300, // TTL in seconds
                RData::TXT(txt_record),
            );

            records.push(record);
            debug!("Created TXT record {}: {} bytes", index + 1, chunk.len());
        }

        info!(
            "Successfully processed query '{}': {} chunks",
            prompt,
            records.len()
        );
        Ok(records)
    }
}

/// Main DNS server with LLM integration
///
/// Manages the complete server lifecycle including:
/// - UDP socket binding and management
/// - Request handling and routing
/// - Graceful shutdown coordination
/// - Resource cleanup
pub struct Server {
    config: Config,
    handler: Arc<LlmDnsHandler>,
    shutdown_tx: broadcast::Sender<()>,
}

impl Server {
    /// Creates a new DNS server with the provided configuration
    ///
    /// # Arguments
    ///
    /// * `config` - Server configuration including DNS address/port and LLM settings
    ///
    /// # Returns
    ///
    /// A configured Server instance ready to start
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - LLM client initialization fails
    /// - Configuration is invalid
    pub fn new(config: Config) -> Result<Self> {
        // Initialize LLM client
        let llm_client = Arc::new(
            LlmClient::new(
                config.openrouter_api_key.clone(),
                config.openrouter_models.clone(),
                config.system_prompt.clone(),
            )
            .context("Failed to create LLM client")?,
        );

        // Initialize chunker
        let chunker = Arc::new(Chunker::new());

        // Initialize DNS handler
        let dns_handler = Arc::new(DnsHandler::new());

        // Create the main handler
        let handler = Arc::new(LlmDnsHandler::new(llm_client, chunker, dns_handler));

        // Create shutdown channel
        let (shutdown_tx, _) = broadcast::channel(1);

        Ok(Self {
            config,
            handler,
            shutdown_tx,
        })
    }

    /// Creates a new server with custom dependencies (for testing)
    ///
    /// # Arguments
    ///
    /// * `config` - Server configuration
    /// * `handler` - Custom LLM DNS handler (e.g., with mocked dependencies)
    ///
    /// # Returns
    ///
    /// A configured Server instance with injected dependencies
    #[cfg(test)]
    pub fn with_handler(config: Config, handler: Arc<LlmDnsHandler>) -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);

        Self {
            config,
            handler,
            shutdown_tx,
        }
    }

    /// Starts the DNS server
    ///
    /// This method:
    /// 1. Binds to the configured UDP address
    /// 2. Begins accepting DNS queries
    /// 3. Spawns async tasks for each query
    /// 4. Handles graceful shutdown on signal
    ///
    /// # Returns
    ///
    /// Ok(()) when the server shuts down gracefully
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Socket binding fails
    /// - Address parsing fails
    /// - Fatal UDP errors occur
    pub async fn start(&self) -> Result<()> {
        // Parse bind address
        let bind_addr: SocketAddr = format!("{}:{}", self.config.dns_address, self.config.dns_port)
            .parse()
            .context("Failed to parse bind address")?;

        // Bind UDP socket
        let socket = UdpSocket::bind(&bind_addr)
            .await
            .context("Failed to bind UDP socket")?;

        info!("DNS server listening on {}", bind_addr);
        info!("Waiting for DNS queries...");
        info!("Example: dig @localhost 'hello.world.llm.duyet.net' TXT");

        // Wrap socket in Arc for sharing across tasks
        let socket = Arc::new(socket);
        let mut buffer = vec![0u8; 512];

        // Subscribe to shutdown signal
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        // Main server loop
        loop {
            tokio::select! {
                // Shutdown signal received
                _ = shutdown_rx.recv() => {
                    info!("Shutdown signal received, stopping server");
                    break;
                }

                // Receive DNS query
                result = socket.recv_from(&mut buffer) => {
                    match result {
                        Ok((n, remote_addr)) => {
                            debug!("Received {} bytes from {}", n, remote_addr);

                            // Parse DNS message
                            match Message::from_vec(&buffer[..n]) {
                                Ok(request_msg) => {
                                    let handler_clone = self.handler.clone();
                                    let socket_clone = socket.clone();

                                    // Process DNS query in background task
                                    tokio::spawn(async move {
                                        if let Err(e) = handle_dns_request(
                                            request_msg,
                                            remote_addr,
                                            handler_clone,
                                            socket_clone,
                                        )
                                        .await
                                        {
                                            error!("Failed to handle DNS request from {}: {}", remote_addr, e);
                                        }
                                    });
                                }
                                Err(e) => {
                                    warn!("Failed to parse DNS message from {}: {}", remote_addr, e);
                                }
                            }
                        }
                        Err(e) => {
                            error!("UDP socket error: {}", e);
                            // Small delay to prevent tight loop on persistent errors
                            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                        }
                    }
                }
            }
        }

        info!("DNS server shutdown complete");
        Ok(())
    }

    /// Triggers graceful shutdown of the server
    ///
    /// This sends a shutdown signal to the running server, allowing it to
    /// complete in-flight requests and clean up resources.
    ///
    /// # Returns
    ///
    /// Ok(()) if shutdown signal was sent successfully
    ///
    /// # Errors
    ///
    /// Returns error if no receivers are listening (server not running)
    pub fn shutdown(&self) -> Result<()> {
        self.shutdown_tx
            .send(())
            .context("Failed to send shutdown signal")?;
        Ok(())
    }

    /// Returns the configured bind address
    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.config.dns_address, self.config.dns_port)
    }
}

/// Handles a single incoming DNS request and sends the response
///
/// # Arguments
///
/// * `request_msg` - Parsed DNS request message
/// * `remote_addr` - Address of the client
/// * `handler` - LLM DNS handler for processing queries
/// * `socket` - UDP socket for sending responses
///
/// # Returns
///
/// Ok(()) when response is sent successfully
///
/// # Errors
///
/// Returns error if:
/// - DNS response serialization fails
/// - UDP send fails
async fn handle_dns_request(
    request_msg: Message,
    remote_addr: SocketAddr,
    handler: Arc<LlmDnsHandler>,
    socket: Arc<UdpSocket>,
) -> Result<()> {
    // Create DNS response message
    let mut response = Message::new();
    response.set_id(request_msg.id());
    response.set_message_type(MessageType::Response);
    response.set_op_code(OpCode::Query);
    response.set_recursion_available(false);
    response.set_recursion_desired(request_msg.recursion_desired());

    // Set authoritative answer bit
    response.set_authoritative(true);

    // Process each query in the request
    let mut response_code = ResponseCode::NoError;

    for query in request_msg.queries() {
        debug!(
            "Processing query: {} {:?}",
            query.name(),
            query.query_type()
        );

        // Only handle TXT queries
        if query.query_type() != RecordType::TXT {
            warn!(
                "Unsupported query type {:?} for {}",
                query.query_type(),
                query.name()
            );
            response_code = ResponseCode::NotImp;
            continue;
        }

        // Process the query
        match handler.process_query(query.name()).await {
            Ok(records) => {
                debug!("Adding {} answer records", records.len());
                for record in records {
                    response.add_answer(record);
                }
            }
            Err(e) => {
                warn!("Failed to process query for {}: {}", query.name(), e);
                response_code = ResponseCode::ServFail;
            }
        }
    }

    // Set response code
    response.set_response_code(response_code);

    // Serialize DNS response to bytes
    let response_bytes = response.to_vec()?;
    debug!(
        "Serialized response: {} bytes, code: {:?}",
        response_bytes.len(),
        response_code
    );

    // Send response back to client
    socket
        .send_to(&response_bytes, remote_addr)
        .await
        .context("Failed to send DNS response")?;

    debug!("Successfully sent response to {}", remote_addr);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_creation() -> Result<()> {
        let config = Config {
            openrouter_api_key: "test_key".to_string(),
            openrouter_models: vec!["test_model".to_string()],
            dns_address: "127.0.0.1".to_string(),
            dns_port: 15353,
        };

        let server = Server::new(config)?;
        assert_eq!(server.bind_address(), "127.0.0.1:15353");
        Ok(())
    }

    #[test]
    fn test_handler_creation() {
        let llm_client = Arc::new(LlmClient::new("key".to_string(), vec!["model".to_string()]).unwrap());
        let chunker = Arc::new(Chunker::new());
        let dns_handler = Arc::new(DnsHandler::new());

        let handler = LlmDnsHandler::new(llm_client, chunker, dns_handler);

        // Handler should be created successfully
        assert!(Arc::strong_count(&handler.llm_client) > 0);
    }
}
