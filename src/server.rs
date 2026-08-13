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
use tokio::sync::{broadcast, Semaphore};
use tracing::{debug, error, info, warn};

use crate::{Chunker, Config, DnsCache, DnsHandler, IpRateLimiter, LlmClient};
use std::time::Duration;

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
    pub cache: Arc<DnsCache>,
    /// Global ceiling on concurrent LLM calls. `None` disables the limit.
    llm_permits: Option<Arc<Semaphore>>,
}

impl LlmDnsHandler {
    /// Creates a new LLM DNS handler with injected dependencies
    ///
    /// # Arguments
    ///
    /// * `llm_client` - Client for LLM API interaction
    /// * `chunker` - Text chunking utility for DNS TXT record limits
    /// * `dns_handler` - DNS protocol handler
    /// * `cache` - DNS response cache
    pub fn new(
        llm_client: Arc<LlmClient>,
        chunker: Arc<Chunker>,
        dns_handler: Arc<DnsHandler>,
        cache: Arc<DnsCache>,
    ) -> Self {
        Self {
            llm_client,
            chunker,
            dns_handler,
            cache,
            llm_permits: None,
        }
    }

    /// Sets the global ceiling on concurrent LLM calls.
    ///
    /// A limit of 0 leaves calls unbounded. Per-IP rate limiting cannot provide
    /// this bound, because UDP source addresses are spoofable and each unseen
    /// address starts with a full token bucket.
    pub fn with_max_concurrent_llm_requests(mut self, limit: usize) -> Self {
        self.llm_permits = (limit > 0).then(|| Arc::new(Semaphore::new(limit)));
        self
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

        // Check cache first
        if let Some(cached_records) = self.cache.get(&query_str).await {
            info!("Cache hit for query '{}'", query_str);
            return Ok(cached_records);
        }

        // Parse subdomain to get the prompt
        let prompt = self.dns_handler.parse_subdomain(&query_str)?;
        debug!("Parsed prompt: {}", prompt);

        // Take a permit before spending money. try_acquire sheds load rather
        // than queueing: a spoofed-source flood would otherwise park hundreds of
        // thousands of tasks waiting on the semaphore, which is the exhaustion
        // we are trying to prevent. The client sees SERVFAIL and may retry.
        let _permit = match self.llm_permits.as_ref() {
            Some(sem) => match sem.clone().try_acquire_owned() {
                Ok(permit) => Some(permit),
                Err(_) => {
                    warn!("LLM concurrency limit reached, shedding query '{}'", prompt);
                    anyhow::bail!("LLM concurrency limit reached");
                }
            },
            None => None,
        };

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

        // Cache the records
        self.cache.insert(&query_str, records.clone()).await;

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
    rate_limiter: Arc<IpRateLimiter>,
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
        let mut llm_client = LlmClient::new(
            config.openrouter_api_key.clone(),
            config.openrouter_models.clone(),
            config.system_prompt.clone(),
            config.temperature,
            config.max_tokens,
            config.top_p,
            config.top_k,
            config.frequency_penalty,
            config.presence_penalty,
        )
        .context("Failed to create LLM client")?;

        // Repoint client base URL from config
        llm_client = llm_client.with_base_url(config.llm_base_url.clone());
        let llm_client = Arc::new(llm_client);

        // Initialize chunker
        let chunker = Arc::new(Chunker::new());

        // Initialize DNS handler
        let dns_handler = Arc::new(DnsHandler::new());

        // Initialize cache
        let cache = Arc::new(DnsCache::with_capacity(
            Duration::from_secs(config.cache_ttl_seconds),
            config.cache_max_entries,
        ));

        // Create the main handler
        let handler = Arc::new(
            LlmDnsHandler::new(llm_client, chunker, dns_handler, cache)
                .with_max_concurrent_llm_requests(config.max_concurrent_llm_requests),
        );

        // Initialize rate limiter
        let rate_limiter = Arc::new(IpRateLimiter::new(
            config.rate_limit_rps,
            config.rate_limit_burst,
        ));

        // Create shutdown channel
        let (shutdown_tx, _) = broadcast::channel(1);

        Ok(Self {
            config,
            handler,
            rate_limiter,
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
        let rate_limiter = Arc::new(IpRateLimiter::new(
            config.rate_limit_rps,
            config.rate_limit_burst,
        ));

        Self {
            config,
            handler,
            rate_limiter,
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

        // Spawn background cleanup task for cache and rate limiter
        let cache_clone = self.handler.cache.clone();
        let rate_limiter_clone = self.rate_limiter.clone();
        let mut shutdown_rx_cleanup = self.shutdown_tx.subscribe();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown_rx_cleanup.recv() => {
                        break;
                    }
                    _ = tokio::time::sleep(Duration::from_secs(30)) => {
                        debug!("Running background cleanup for cache and rate limiter...");
                        cache_clone.cleanup().await;
                        rate_limiter_clone.cleanup(Duration::from_secs(300));
                    }
                }
            }
        });

        // Wrap socket in Arc for sharing across tasks
        let socket = Arc::new(socket);
        // recv_from silently discards whatever does not fit, so a 512-byte
        // buffer truncated any EDNS0-sized query into an unparseable fragment:
        // Message::from_vec then failed and the client got no reply at all,
        // just a timeout. Size for the largest datagram we are willing to read.
        let mut buffer = vec![0u8; MAX_UDP_REQUEST];

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
                                    let rate_limiter_clone = self.rate_limiter.clone();
                                    let socket_clone = socket.clone();

                                    // Process DNS query in background task
                                    tokio::spawn(async move {
                                        if let Err(e) = handle_dns_request(
                                            request_msg,
                                            remote_addr,
                                            handler_clone,
                                            rate_limiter_clone,
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

/// Size of the UDP receive buffer.
///
/// Large enough for any EDNS0 query a client may send; anything beyond this is
/// discarded by the kernel and will fail to parse, which is the correct outcome
/// for a datagram that large.
const MAX_UDP_REQUEST: usize = 4096;

/// Largest UDP response permitted to a client that sent no EDNS0 OPT record.
///
/// RFC 1035 §4.2.1 fixes the plain-DNS UDP message size at 512 bytes.
const DEFAULT_MAX_UDP_RESPONSE: usize = 512;

/// Ceiling applied even when a client advertises a larger EDNS0 buffer.
///
/// 1232 bytes is the DNS Flag Day 2020 recommendation: it stays under the
/// common 1280-byte IPv6 MTU so responses avoid IP fragmentation, and it bounds
/// the amplification factor available to an attacker spoofing a source address.
const MAX_UDP_RESPONSE: usize = 1232;

/// Determine how many bytes of UDP response this client may receive.
///
/// A client advertises its receive buffer in the EDNS0 OPT record (RFC 6891).
/// Absent that, plain DNS limits us to [`DEFAULT_MAX_UDP_RESPONSE`]. The value
/// is clamped into `[DEFAULT_MAX_UDP_RESPONSE, MAX_UDP_RESPONSE]` so a spoofed
/// query cannot request a large datagram.
fn client_udp_payload_size(request: &Message) -> usize {
    request
        .edns
        .as_ref()
        .map(|edns| edns.max_payload() as usize)
        .unwrap_or(DEFAULT_MAX_UDP_RESPONSE)
        .clamp(DEFAULT_MAX_UDP_RESPONSE, MAX_UDP_RESPONSE)
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
    rate_limiter: Arc<IpRateLimiter>,
    socket: Arc<UdpSocket>,
) -> Result<()> {
    // Check rate limit first
    if !rate_limiter.check_allowed(remote_addr.ip()) {
        warn!("Rate limit exceeded for client {}", remote_addr);
        let mut response = Message::new(
            request_msg.metadata.id,
            MessageType::Response,
            OpCode::Query,
        );
        response.metadata.recursion_available = false;
        response.metadata.recursion_desired = request_msg.metadata.recursion_desired;
        response.metadata.authoritative = true;
        response.metadata.response_code = ResponseCode::Refused;

        let response_bytes = response.to_vec()?;
        socket.send_to(&response_bytes, remote_addr).await?;
        return Ok(());
    }

    // Create DNS response message
    let mut response = Message::new(
        request_msg.metadata.id,
        MessageType::Response,
        OpCode::Query,
    );
    response.metadata.recursion_available = false;
    response.metadata.recursion_desired = request_msg.metadata.recursion_desired;
    response.add_queries(request_msg.queries.clone());

    // Set authoritative answer bit
    response.metadata.authoritative = true;

    // Process each query in the request
    let mut response_code = ResponseCode::NoError;

    for query in &request_msg.queries {
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
    response.metadata.response_code = response_code;

    // Serialize DNS response to bytes
    let mut response_bytes = response.to_vec()?;

    // Cap the datagram at what the client is entitled to receive. UDP source
    // addresses are trivially spoofed, so an unbounded response turns this
    // server into an amplifier: a ~50 byte query would otherwise return up to
    // the chunker's 4096 byte limit. Over the budget we drop the answers and
    // set TC, which tells a legitimate client to retry over TCP (RFC 1035
    // §4.2.1) while giving a spoofing attacker almost no amplification.
    let max_response_size = client_udp_payload_size(&request_msg);
    if response_bytes.len() > max_response_size {
        debug!(
            "Response {} bytes exceeds client budget {}, truncating",
            response_bytes.len(),
            max_response_size
        );
        response_bytes = response.truncate().to_vec()?;
    }

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
    use hickory_server::proto::op::Edns;

    #[test]
    fn test_server_creation() -> Result<()> {
        let config = Config {
            openrouter_api_key: "test_key".to_string(),
            openrouter_models: vec!["test_model".to_string()],
            llm_base_url: "https://openrouter.ai/api/v1/chat/completions".to_string(),
            system_prompt: "Test system prompt".to_string(),
            dns_address: "127.0.0.1".to_string(),
            dns_port: 15353,
            temperature: None,
            max_tokens: None,
            top_p: None,
            top_k: None,
            frequency_penalty: None,
            presence_penalty: None,
            cache_ttl_seconds: 300,
            rate_limit_rps: 5.0,
            rate_limit_burst: 10.0,
            max_concurrent_llm_requests: 32,
            cache_max_entries: 10000,
        };

        let server = Server::new(config)?;
        assert_eq!(server.bind_address(), "127.0.0.1:15353");
        Ok(())
    }

    #[test]
    fn test_handler_creation() {
        let llm_client = Arc::new(
            LlmClient::new(
                "key".to_string(),
                vec!["model".to_string()],
                "Test system prompt".to_string(),
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap(),
        );
        let chunker = Arc::new(Chunker::new());
        let dns_handler = Arc::new(DnsHandler::new());
        let cache = Arc::new(DnsCache::new(Duration::from_secs(300)));

        let handler = LlmDnsHandler::new(llm_client, chunker, dns_handler, cache);

        // Handler should be created successfully
        assert!(Arc::strong_count(&handler.llm_client) > 0);
    }

    #[test]
    fn test_udp_payload_size_defaults_to_512_without_edns() {
        // A plain-DNS client advertises nothing, so RFC 1035's 512 byte cap applies.
        let msg = Message::new(1, MessageType::Query, OpCode::Query);
        assert_eq!(client_udp_payload_size(&msg), DEFAULT_MAX_UDP_RESPONSE);
    }

    #[test]
    fn test_udp_payload_size_honors_edns_advertisement() {
        let mut msg = Message::new(1, MessageType::Query, OpCode::Query);
        let mut edns = Edns::new();
        edns.set_max_payload(1000);
        msg.set_edns(edns);

        assert_eq!(client_udp_payload_size(&msg), 1000);
    }

    #[test]
    fn test_udp_payload_size_clamps_oversized_advertisement() {
        // An attacker spoofing a source address would advertise the largest
        // buffer possible to maximise amplification. Cap it.
        let mut msg = Message::new(1, MessageType::Query, OpCode::Query);
        let mut edns = Edns::new();
        edns.set_max_payload(u16::MAX);
        msg.set_edns(edns);

        assert_eq!(client_udp_payload_size(&msg), MAX_UDP_RESPONSE);
    }

    #[test]
    fn test_udp_payload_size_raises_undersized_advertisement() {
        // Below 512 we still owe the client a usable response.
        let mut msg = Message::new(1, MessageType::Query, OpCode::Query);
        let mut edns = Edns::new();
        edns.set_max_payload(0);
        msg.set_edns(edns);

        assert_eq!(client_udp_payload_size(&msg), DEFAULT_MAX_UDP_RESPONSE);
    }

    #[test]
    fn test_oversized_response_truncates_and_sets_tc_bit() {
        // Build a response far larger than any client budget, mirroring what a
        // long LLM answer produces, and confirm the wire form we would send is
        // bounded and carries TC so the client retries over TCP.
        let name = Name::from_utf8("what.is.rust.").unwrap();
        let mut response = Message::new(42, MessageType::Response, OpCode::Query);
        response.metadata.authoritative = true;
        for _ in 0..16 {
            let txt = TXT::new(vec!["x".repeat(250)]);
            response.add_answer(Record::from_rdata(name.clone(), 300, RData::TXT(txt)));
        }

        let full = response.to_vec().unwrap();
        assert!(
            full.len() > MAX_UDP_RESPONSE,
            "fixture should exceed the cap, got {} bytes",
            full.len()
        );

        let truncated = response.truncate();
        let bytes = truncated.to_vec().unwrap();

        assert!(truncated.metadata.truncation, "TC bit must be set");
        assert!(truncated.answers.is_empty(), "answers must be dropped");
        assert!(
            bytes.len() <= DEFAULT_MAX_UDP_RESPONSE,
            "truncated response should fit the smallest budget, got {} bytes",
            bytes.len()
        );
        assert_eq!(truncated.metadata.id, 42, "query id must be preserved");
    }

    fn test_handler_with_limit(limit: usize) -> LlmDnsHandler {
        let llm_client = Arc::new(
            LlmClient::new(
                "key".to_string(),
                vec!["model".to_string()],
                "Test system prompt".to_string(),
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap(),
        );
        LlmDnsHandler::new(
            llm_client,
            Arc::new(Chunker::new()),
            Arc::new(DnsHandler::new()),
            Arc::new(DnsCache::new(Duration::from_secs(300))),
        )
        .with_max_concurrent_llm_requests(limit)
    }

    #[tokio::test]
    async fn test_query_is_shed_when_llm_concurrency_exhausted() {
        // With every permit held, a further query must be refused *before* the
        // outbound call - so this asserts the shed without touching the network.
        let handler = test_handler_with_limit(1);
        let sem = handler.llm_permits.clone().expect("limit should be active");
        let _held = sem.try_acquire_owned().expect("first permit available");

        let name = Name::from_utf8("what.is.rust.").unwrap();
        let err = handler
            .process_query(&name)
            .await
            .expect_err("query should be shed while permits are exhausted");

        assert!(
            err.to_string().contains("concurrency limit"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn test_permit_is_released_after_query() {
        // A shed query must not leak its permit, or the server would wedge shut
        // after the first burst.
        let handler = test_handler_with_limit(1);
        let sem = handler.llm_permits.clone().expect("limit should be active");

        {
            let _held = sem.clone().try_acquire_owned().unwrap();
            let name = Name::from_utf8("first.query.").unwrap();
            assert!(handler.process_query(&name).await.is_err());
        }

        assert_eq!(
            sem.available_permits(),
            1,
            "permit was not returned after the shed query"
        );
    }

    #[test]
    fn test_zero_limit_disables_llm_concurrency_cap() {
        assert!(test_handler_with_limit(0).llm_permits.is_none());
        assert!(test_handler_with_limit(4).llm_permits.is_some());
    }
}
