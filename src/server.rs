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
use tokio::sync::{watch, Semaphore};
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

/// Resolve once shutdown has been requested, including a request made before
/// this receiver existed.
///
/// `watch` latches: the sender keeps the flag for subscribers that arrive later,
/// so a `shutdown()` that beat the socket bind is still visible here.
/// `borrow_and_update` consumes the current value first, so the following
/// `changed()` waits for a genuinely new transition instead of firing again on
/// the old one.
async fn wait_for_shutdown(shutdown_rx: &mut watch::Receiver<bool>) {
    if *shutdown_rx.borrow_and_update() {
        return;
    }

    // `changed()` only fails once every sender is gone, which cannot happen
    // while a `Server` holding one is alive. Treat it as a shutdown either way.
    if shutdown_rx.changed().await.is_err() {
        debug!("Shutdown sender dropped, treating it as a shutdown request");
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
    /// Latching shutdown flag.
    ///
    /// `broadcast` does not latch: `send` fails outright when nothing has
    /// subscribed yet, and the only subscription happens after the socket bind.
    /// The caller races `start()` against Ctrl+C, so a shutdown arriving in that
    /// window was dropped and `shutdown()` reported a failure on a clean
    /// interrupt. `watch` retains the flag for subscribers created later.
    shutdown_tx: watch::Sender<bool>,
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
        let (shutdown_tx, _) = watch::channel(false);

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
        let (shutdown_tx, _) = watch::channel(false);
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
        // Subscribe before the bind, which is awaited: a shutdown that arrives
        // in that window must stop us before we claim a port, not after.
        let mut shutdown_rx = self.shutdown_tx.subscribe();
        if *shutdown_rx.borrow_and_update() {
            info!(
                "Shutdown already requested, not binding {}",
                self.bind_address()
            );
            return Ok(());
        }

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
                    _ = wait_for_shutdown(&mut shutdown_rx_cleanup) => {
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

        // Main server loop
        loop {
            tokio::select! {
                // Shutdown signal received
                _ = wait_for_shutdown(&mut shutdown_rx) => {
                    info!("Shutdown signal received, stopping server");
                    break;
                }

                // Receive DNS query
                result = socket.recv_from(&mut buffer) => {
                    match result {
                        Ok((n, remote_addr)) => {
                            debug!("Received {} bytes from {}", n, remote_addr);

                            if let Err(e) = dispatch_datagram(
                                &buffer[..n],
                                remote_addr,
                                self.handler.clone(),
                                self.rate_limiter.clone(),
                                socket.clone(),
                            )
                            .await
                            {
                                error!("Failed to dispatch datagram from {}: {}", remote_addr, e);
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
    /// complete in-flight requests and clean up resources. The request is
    /// latched, so it is honoured even if `start()` has not subscribed yet.
    ///
    /// # Returns
    ///
    /// Ok(()) once the shutdown request is recorded
    ///
    /// # Errors
    ///
    /// Never fails. The flag is retained until a subscriber appears, so a
    /// shutdown that beats the socket bind no longer reports a failure on a
    /// perfectly clean interrupt.
    pub fn shutdown(&self) -> Result<()> {
        self.shutdown_tx.send_replace(true);
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

/// Build the shell shared by every reply: header fields plus the echoed question.
///
/// RFC 1035 §4.1 requires a response to repeat the question, and that echo is
/// the only way a client can match a reply to the question it is waiting on.
/// Both the normal path and the rate-limited REFUSED go through here so neither
/// can ship a `QDCOUNT = 0` reply that a strict resolver throws away.
fn response_for(request_msg: &Message) -> Message {
    let mut response = Message::new(
        request_msg.metadata.id,
        MessageType::Response,
        OpCode::Query,
    );
    response.metadata.recursion_available = false;
    response.metadata.recursion_desired = request_msg.metadata.recursion_desired;
    // Set authoritative answer bit
    response.metadata.authoritative = true;
    response.add_queries(request_msg.queries.iter().cloned());
    response
}

/// Rank an rcode by how strongly it should outrank another.
///
/// A message carries a single rcode, so a request with several questions has to
/// report one verdict for all of them. `NoError` is the floor. `NotImp` says the
/// query type is not implemented, which is permanent and unactionable for the
/// client. `ServFail` says this server failed to do the work, which both the
/// client should retry and an operator needs to see, so it outranks `NotImp`:
/// letting `NotImp` win hid real LLM failures behind a capability gap. Any other
/// code ranks above all of these, so a newly used one is never downgraded.
fn rcode_severity(code: ResponseCode) -> u8 {
    match code {
        ResponseCode::NoError => 0,
        ResponseCode::NotImp => 1,
        ResponseCode::ServFail => 2,
        _ => 3,
    }
}

/// Combine the rcode so far with the outcome of one more question.
///
/// Escalation only: a later question can raise the reported rcode but never
/// lower it. Overwriting instead meant the reply was decided by whichever
/// question happened to be asked last, so `[TXT, A]` and `[A, TXT]` were
/// reported differently and a real failure could be relabelled `NOTIMP`.
fn worst_response_code(current: ResponseCode, candidate: ResponseCode) -> ResponseCode {
    if rcode_severity(candidate) > rcode_severity(current) {
        candidate
    } else {
        current
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
    rate_limiter: Arc<IpRateLimiter>,
    socket: Arc<UdpSocket>,
) -> Result<()> {
    // Check rate limit first
    if !rate_limiter.check_allowed(remote_addr.ip()) {
        warn!("Rate limit exceeded for client {}", remote_addr);
        let mut response = response_for(&request_msg);
        response.metadata.response_code = ResponseCode::Refused;

        let response_bytes = response.to_vec()?;
        socket.send_to(&response_bytes, remote_addr).await?;
        return Ok(());
    }

    // Create DNS response message
    let mut response = response_for(&request_msg);

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
            response_code = worst_response_code(response_code, ResponseCode::NotImp);
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
                response_code = worst_response_code(response_code, ResponseCode::ServFail);
            }
        }
    }

    // Set response code
    response.metadata.response_code = response_code;

    // Cap the datagram at what the client is entitled to receive. UDP source
    // addresses are trivially spoofed, so an unbounded response turns this
    // server into an amplifier: a ~50 byte query would otherwise return up to
    // the chunker's 4096 byte limit. Over the budget the trailing answers that
    // do not fit are dropped and TC is set, which tells a legitimate client to
    // retry over TCP (RFC 1035 §4.2.1) while giving a spoofing attacker almost
    // no amplification.
    let max_response_size = client_udp_payload_size(&request_msg);
    let response = fit_response_to_budget(response, max_response_size);

    // Serialize DNS response to bytes
    let response_bytes = response.to_vec()?;

    debug!(
        "Serialized response: {} bytes, code: {:?}",
        response_bytes.len(),
        response.metadata.response_code
    );

    // Send response back to client
    socket
        .send_to(&response_bytes, remote_addr)
        .await
        .context("Failed to send DNS response")?;

    debug!("Successfully sent response to {}", remote_addr);
    Ok(())
}

/// Fit a response into the client's UDP budget, shedding as little as possible.
///
/// [`Message::truncate`] is all or nothing: it drops every answer and sets TC.
/// A TXT answer costs ~263 bytes on the wire, so against the 512 byte plain-DNS
/// budget only one chunk fits and against the usual 1232 byte EDNS budget only
/// about four - past that the client received nothing at all for a question it
/// had already paid for. Dropping only the trailing answers that do not fit
/// turns that into a partial answer plus TC, which is exactly what TC is for.
///
/// TC is set only when records were genuinely left behind, so a response that
/// fits keeps both its answers and a clear TC bit.
fn fit_response_to_budget(mut response: Message, max_size: usize) -> Message {
    if encoded_len(&response).is_some_and(|len| len <= max_size) {
        return response;
    }

    debug!(
        "Response exceeds client budget of {} bytes, shedding trailing answers",
        max_size
    );

    // The answer section holds at most a couple of dozen records, so
    // re-encoding after each drop is cheaper than tracking per-record wire cost.
    let total = response.answers.len();
    let mut dropped = 0usize;
    while !response.answers.is_empty() {
        response.answers.pop();
        dropped += 1;

        if encoded_len(&response).is_some_and(|len| len <= max_size) {
            // Records were left undelivered, so the client is owed a TCP retry.
            response.metadata.truncation = true;
            debug!(
                "Dropped {} of {} answers to fit the {} byte budget",
                dropped, total, max_size
            );
            return response;
        }
    }

    // No answers fit, so the echoed question section alone is over budget.
    // `truncate` re-emits that section, hence the re-check: its output can still
    // be too large for a client that asked a lot of questions.
    let mut truncated = response.truncate();
    if encoded_len(&truncated).is_some_and(|len| len <= max_size) {
        return truncated;
    }

    // Nothing but the header is within budget. Send it anyway: the client can
    // still match it by transaction ID and TC still tells it to retry over TCP,
    // which beats silently exceeding the budget or answering with silence.
    truncated.queries.clear();
    warn!(
        "Question section alone exceeds client budget of {} bytes, sending header-only response",
        max_size
    );
    truncated
}

/// Wire length of a message, or `None` when it cannot be encoded at all.
fn encoded_len(msg: &Message) -> Option<usize> {
    msg.to_vec().ok().map(|bytes| bytes.len())
}

/// Decode one received datagram and dispatch it to [`handle_dns_request`].
///
/// A datagram that will not decode is still answered. The transaction ID is the
/// first two bytes of every DNS message (RFC 1035 §4.1.1), so it is recoverable
/// from a datagram whose remainder is garbage, and a reply carrying it is
/// matchable where silence is not: `recv_from` silently truncates anything
/// larger than [`MAX_UDP_REQUEST`], and before this existed such a datagram
/// produced nothing at all but a client timeout.
async fn dispatch_datagram(
    raw: &[u8],
    remote_addr: SocketAddr,
    handler: Arc<LlmDnsHandler>,
    rate_limiter: Arc<IpRateLimiter>,
    socket: Arc<UdpSocket>,
) -> Result<()> {
    match Message::from_vec(raw) {
        Ok(request_msg) => {
            // Answer off the receive loop: an LLM call takes seconds and the
            // socket must keep serving every other client meanwhile.
            tokio::spawn(async move {
                if let Err(e) =
                    handle_dns_request(request_msg, remote_addr, handler, rate_limiter, socket)
                        .await
                {
                    error!("Failed to handle DNS request from {}: {}", remote_addr, e);
                }
            });
        }
        Err(e) => {
            warn!("Failed to parse DNS message from {}: {}", remote_addr, e);
            reply_format_error(&socket, remote_addr, raw).await?;
        }
    }

    Ok(())
}

/// Send FORMERR for a datagram that failed to decode, carrying its transaction ID.
async fn reply_format_error(socket: &UdpSocket, remote_addr: SocketAddr, raw: &[u8]) -> Result<()> {
    let Some(id_bytes) = raw.get(..2) else {
        // Too short to hold even a transaction ID: there is nothing for a client
        // to match a reply against, so silence is the only honest answer.
        warn!(
            "Datagram from {} is too short to hold a transaction ID",
            remote_addr
        );
        return Ok(());
    };
    let id = u16::from_be_bytes([id_bytes[0], id_bytes[1]]);

    let mut response = Message::new(id, MessageType::Response, OpCode::Query);
    response.metadata.authoritative = true;
    response.metadata.response_code = ResponseCode::FormErr;

    let response_bytes = response.to_vec()?;
    socket
        .send_to(&response_bytes, remote_addr)
        .await
        .context("Failed to send FORMERR response")?;

    debug!("Sent FORMERR with id {} to {}", id, remote_addr);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hickory_server::proto::op::{Edns, Query};
    use mockito::{Mock, ServerGuard};
    use std::net::Ipv4Addr;

    /// How long a test waits for a datagram before giving up.
    ///
    /// Every exchange below is loopback plus a local mock, so this only turns a
    /// hang into a failed test instead of a stuck job.
    const REPLY_TIMEOUT: Duration = Duration::from_secs(10);

    /// A base URL no test ever calls, for fixtures that only inspect a handler.
    const UNUSED_BASE_URL: &str = "http://127.0.0.1:1/never-called";

    /// Body returned by mocks that must fail the query.
    const LLM_ERROR_BODY: &str = r#"{"error": "upstream failure"}"#;

    /// Config for a loopback server on `port`, with limits loose enough that
    /// they never interfere with a test.
    fn test_config(port: u16) -> Config {
        Config {
            openrouter_api_key: "test_key".to_string(),
            openrouter_models: vec!["test_model".to_string()],
            llm_base_url: "https://openrouter.ai/api/v1/chat/completions".to_string(),
            system_prompt: "Test system prompt".to_string(),
            dns_address: "127.0.0.1".to_string(),
            dns_port: port,
            temperature: None,
            max_tokens: None,
            top_p: None,
            top_k: None,
            frequency_penalty: None,
            presence_penalty: None,
            cache_ttl_seconds: 300,
            rate_limit_rps: 1000.0,
            rate_limit_burst: 1000.0,
            max_concurrent_llm_requests: 32,
            cache_max_entries: 10000,
        }
    }

    /// A well-formed LLM reply carrying `content`.
    fn llm_reply(content: &str) -> String {
        format!(r#"{{"choices": [{{"message": {{"content": "{content}"}}}}]}}"#)
    }

    /// A local stand-in for the LLM endpoint answering every query with `status`
    /// and `body`.
    ///
    /// The guard must outlive every client pointed at `url()`: dropping it shuts
    /// the mock server down. Tests keep a mock endpoint so that a regression
    /// fails immediately instead of blocking on the real API's 30 second
    /// connection timeout.
    async fn mock_llm(status: usize, body: &str) -> (Mock, ServerGuard) {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", mockito::Matcher::Regex(r"^/.*".to_string()))
            .with_status(status)
            .with_header("content-type", "application/json")
            .with_body(body)
            .create_async()
            .await;
        (mock, server)
    }

    /// A mock endpoint that must never be reached: `assert` fails the test if
    /// anything calls it.
    async fn forbidden_llm() -> (Mock, ServerGuard) {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", mockito::Matcher::Regex(r"^/.*".to_string()))
            .with_status(500)
            .with_body(LLM_ERROR_BODY)
            .expect(0)
            .create_async()
            .await;
        (mock, server)
    }

    /// Handler whose LLM calls go to `base_url`, under a global concurrency cap.
    ///
    /// Tests must pass a mock endpoint here. Against the production URL a
    /// regression that reached the LLM would hang for the client's 30 second
    /// timeout and still fail, turning a fast failure into a slow one.
    fn test_handler_with_limit(limit: usize, base_url: &str) -> LlmDnsHandler {
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
            .expect("Failed to create LLM client")
            .with_base_url(base_url.to_string()),
        );
        LlmDnsHandler::new(
            llm_client,
            Arc::new(Chunker::new()),
            Arc::new(DnsHandler::new()),
            Arc::new(DnsCache::new(Duration::from_secs(300))),
        )
        .with_max_concurrent_llm_requests(limit)
    }

    /// Handler backed by a local mock, for tests that do issue queries. The
    /// returned guard keeps the mock alive.
    async fn mock_backed_handler(status: usize, body: &str) -> (LlmDnsHandler, ServerGuard) {
        let (_mock, server) = mock_llm(status, body).await;
        (test_handler_with_limit(0, &server.url()), server)
    }

    /// Rate limiter that lets every test query through.
    fn open_rate_limiter() -> Arc<IpRateLimiter> {
        Arc::new(IpRateLimiter::new(1000.0, 1000.0))
    }

    /// Rate limiter whose single token for the loopback client is already spent.
    ///
    /// One token refills about every 16 minutes, so the exhausted state cannot
    /// lapse into a pass on a slow test runner.
    fn exhausted_rate_limiter() -> Arc<IpRateLimiter> {
        let limiter = Arc::new(IpRateLimiter::new(0.001, 1.0));
        assert!(
            limiter.check_allowed(Ipv4Addr::LOCALHOST.into()),
            "the first request should consume the bucket"
        );
        assert!(
            !limiter.check_allowed(Ipv4Addr::LOCALHOST.into()),
            "the bucket should now be empty"
        );
        limiter
    }

    /// A single-question TXT request for `name`.
    fn txt_request(id: u16, name: &str) -> Message {
        let mut request = Message::new(id, MessageType::Query, OpCode::Query);
        request.metadata.recursion_desired = true;
        request.add_query(Query::query(
            Name::from_utf8(name).expect("valid name"),
            RecordType::TXT,
        ));
        request
    }

    /// A request asking `count` A-record questions, each with a name long enough
    /// that the echoed question section alone overruns a 512 byte budget.
    fn long_question_request(id: u16, count: usize) -> Message {
        let mut request = Message::new(id, MessageType::Query, OpCode::Query);
        for index in 0..count {
            let name = format!("q{index}-{}.example.com.", "a".repeat(40));
            request.add_query(Query::query(
                Name::from_utf8(&name).expect("valid name"),
                RecordType::A,
            ));
        }
        request
    }

    /// A datagram whose header promises a question section that is not there.
    ///
    /// This is the shape `recv_from` hands us for anything larger than
    /// [`MAX_UDP_REQUEST`], because it silently truncates.
    fn undecodable_datagram(id: u16) -> Vec<u8> {
        let mut raw = vec![0u8; 12];
        raw[0..2].copy_from_slice(&id.to_be_bytes());
        raw[2] = 0x01; // standard query, recursion desired
        raw[5] = 0x01; // QDCOUNT = 1, with no question to read
        raw
    }

    /// A loopback socket pair: the second socket plays the server that answers
    /// whatever the first one sends.
    async fn socket_pair() -> (UdpSocket, Arc<UdpSocket>) {
        let client = UdpSocket::bind("127.0.0.1:0")
            .await
            .expect("bind client socket");
        let server = Arc::new(
            UdpSocket::bind("127.0.0.1:0")
                .await
                .expect("bind server socket"),
        );
        (client, server)
    }

    /// Await a single datagram, failing the test rather than hanging forever.
    async fn recv_datagram(socket: &UdpSocket) -> Vec<u8> {
        let mut buf = vec![0u8; MAX_UDP_REQUEST];
        let n = tokio::time::timeout(REPLY_TIMEOUT, socket.recv_from(&mut buf))
            .await
            .expect("server did not reply in time")
            .expect("receive failed")
            .0;
        buf.truncate(n);
        buf
    }

    /// Drive one request through `handle_dns_request` over a real socket pair
    /// and return the exact bytes the server sent back.
    async fn dns_exchange(
        request: &Message,
        handler: Arc<LlmDnsHandler>,
        rate_limiter: Arc<IpRateLimiter>,
    ) -> Vec<u8> {
        let (client, server) = socket_pair().await;
        let remote = server.local_addr().expect("server address");

        let task = tokio::spawn(handle_dns_request(
            request.clone(),
            client.local_addr().expect("client address"),
            handler,
            rate_limiter,
            server,
        ));

        client
            .send_to(&request.to_vec().expect("encode request"), remote)
            .await
            .expect("send request");

        let bytes = recv_datagram(&client).await;
        task.await
            .expect("handler task panicked")
            .expect("handling the request failed");
        bytes
    }

    #[test]
    fn test_server_creation() -> Result<()> {
        let server = Server::new(test_config(15353))?;
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
    fn test_worst_response_code_only_escalates() {
        // The rule behind the rcode fix: a later question may raise the reported
        // code but never lower it, and a real failure outranks a capability gap.
        assert_eq!(
            worst_response_code(ResponseCode::NoError, ResponseCode::NotImp),
            ResponseCode::NotImp
        );
        assert_eq!(
            worst_response_code(ResponseCode::NotImp, ResponseCode::NoError),
            ResponseCode::NotImp
        );
        assert_eq!(
            worst_response_code(ResponseCode::NotImp, ResponseCode::ServFail),
            ResponseCode::ServFail
        );
        assert_eq!(
            worst_response_code(ResponseCode::ServFail, ResponseCode::NotImp),
            ResponseCode::ServFail
        );
        assert_eq!(
            worst_response_code(ResponseCode::ServFail, ResponseCode::ServFail),
            ResponseCode::ServFail
        );
    }

    #[test]
    fn test_fit_response_sheds_trailing_answers_and_sets_tc_bit() {
        // The question section is part of the fixture: without it this measured
        // a ~12 byte message and asserted nothing about truncation.
        let name = Name::from_utf8("what.is.rust.").unwrap();
        let mut response = Message::new(42, MessageType::Response, OpCode::Query);
        response.metadata.authoritative = true;
        response.add_query(Query::query(name.clone(), RecordType::TXT));
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

        let fitted = fit_response_to_budget(response.clone(), DEFAULT_MAX_UDP_RESPONSE);
        let bytes = fitted.to_vec().unwrap();

        assert!(fitted.metadata.truncation, "TC bit must be set");
        assert!(
            !fitted.answers.is_empty(),
            "answers that fit must be kept, not thrown away wholesale"
        );
        assert_eq!(
            fitted.answers,
            response.answers[..fitted.answers.len()],
            "the leading records are the ones kept"
        );
        assert_eq!(
            fitted.queries, response.queries,
            "the question section must survive shedding"
        );
        assert!(
            bytes.len() <= DEFAULT_MAX_UDP_RESPONSE,
            "fitted response should fit the smallest budget, got {} bytes",
            bytes.len()
        );
        assert_eq!(fitted.metadata.id, 42, "query id must be preserved");
    }

    #[test]
    fn test_fit_response_falls_back_to_a_bare_header_when_questions_do_not_fit() {
        // `truncate` re-emits the question section, so even an answer-less reply
        // can be over budget and has to be re-checked.
        let mut response = Message::new(43, MessageType::Response, OpCode::Query);
        for index in 0..12 {
            let name =
                Name::from_utf8(format!("q{index}-{}.example.com.", "a".repeat(40)).as_str())
                    .expect("valid name");
            response.add_query(Query::query(name, RecordType::TXT));
        }
        let txt = TXT::new(vec!["x".repeat(250)]);
        response.add_answer(Record::from_rdata(
            Name::from_utf8("what.is.rust.").unwrap(),
            300,
            RData::TXT(txt),
        ));

        let fitted = fit_response_to_budget(response, DEFAULT_MAX_UDP_RESPONSE);
        let bytes = fitted.to_vec().unwrap();

        assert!(fitted.metadata.truncation, "TC bit must be set");
        assert!(fitted.answers.is_empty(), "no answer fits");
        assert!(
            fitted.queries.is_empty(),
            "the echoed question section is what overflowed"
        );
        assert_eq!(bytes.len(), 12, "only the DNS header is left");
        assert_eq!(fitted.metadata.id, 43, "query id must be preserved");
    }

    #[tokio::test]
    async fn test_rate_limited_reply_echoes_question_section() {
        // The reply every client above the rate limit receives. Without the
        // question section the client cannot match it to its query, strict
        // resolvers discard it, and the user sees a timeout instead of REFUSED.
        let (handler, _server) = mock_backed_handler(200, &llm_reply("unused")).await;
        let request = txt_request(0xBEEF, "what.is.rust.");

        let bytes = dns_exchange(&request, Arc::new(handler), exhausted_rate_limiter()).await;
        let response = Message::from_vec(&bytes).expect("valid reply");

        assert_eq!(response.metadata.id, 0xBEEF, "reply must carry the id");
        assert_eq!(response.metadata.response_code, ResponseCode::Refused);
        assert_eq!(
            response.queries, request.queries,
            "RFC 1035 §4.1: a response must repeat the question"
        );
        assert!(
            response.answers.is_empty(),
            "a refused request has no answers"
        );
    }

    /// One row of the mixed-question table: the question types asked, in order,
    /// and the rcode the single reply must carry.
    struct MixedQuestions {
        label: &'static str,
        question_types: &'static [RecordType],
        llm_fails: bool,
        expected: ResponseCode,
    }

    #[tokio::test]
    async fn test_mixed_questions_report_the_worst_rcode() {
        // A message carries one rcode, so the reply must report the worst outcome
        // among its questions rather than whichever was asked last. Overwriting
        // instead meant `[TXT, A]` and `[A, TXT]` were reported differently, a
        // good answer rode along with NOTIMP, and a real LLM failure was
        // relabelled NOTIMP so nobody could see its cause.
        let cases = [
            MixedQuestions {
                label: "answered TXT then unsupported A",
                question_types: &[RecordType::TXT, RecordType::A],
                llm_fails: false,
                expected: ResponseCode::NotImp,
            },
            MixedQuestions {
                label: "unsupported A then answered TXT",
                question_types: &[RecordType::A, RecordType::TXT],
                llm_fails: false,
                expected: ResponseCode::NotImp,
            },
            MixedQuestions {
                label: "failing TXT then unsupported A",
                question_types: &[RecordType::TXT, RecordType::A],
                llm_fails: true,
                expected: ResponseCode::ServFail,
            },
            MixedQuestions {
                label: "unsupported A then failing TXT",
                question_types: &[RecordType::A, RecordType::TXT],
                llm_fails: true,
                expected: ResponseCode::ServFail,
            },
            MixedQuestions {
                label: "two answered TXT questions",
                question_types: &[RecordType::TXT, RecordType::TXT],
                llm_fails: false,
                expected: ResponseCode::NoError,
            },
        ];

        for (case_index, case) in cases.iter().enumerate() {
            let (status, body) = if case.llm_fails {
                (500, LLM_ERROR_BODY.to_string())
            } else {
                (200, llm_reply("mocked answer"))
            };
            let (handler, _server) = mock_backed_handler(status, &body).await;

            // Distinct names per row: the handler caches by query string, so a
            // shared name would let one row's answer leak into the next.
            let mut request =
                Message::new(1000 + case_index as u16, MessageType::Query, OpCode::Query);
            for (query_index, query_type) in case.question_types.iter().enumerate() {
                let name = format!("case{case_index}-q{query_index}.example.com.");
                request.add_query(Query::query(
                    Name::from_utf8(&name).expect("valid name"),
                    *query_type,
                ));
            }

            let bytes = dns_exchange(&request, Arc::new(handler), open_rate_limiter()).await;
            let response = Message::from_vec(&bytes).expect("valid reply");

            assert_eq!(
                response.metadata.response_code, case.expected,
                "{}: wrong response code",
                case.label
            );
            assert_eq!(
                response.queries, request.queries,
                "{}: the question must be echoed",
                case.label
            );
            assert_eq!(
                !response.answers.is_empty(),
                !case.llm_fails,
                "{}: answers are present exactly when the TXT question succeeded",
                case.label
            );
        }
    }

    #[tokio::test]
    async fn test_undecodable_datagram_is_answered_with_formerr() {
        // Such a datagram used to be logged and dropped, so a malformed query -
        // including anything larger than the receive buffer, which recv_from
        // silently truncates - cost the client a timeout. The transaction ID in
        // the first two bytes makes the FORMERR trivially recoverable.
        let (handler, _server) = mock_backed_handler(200, &llm_reply("unused")).await;
        let (client, server) = socket_pair().await;

        dispatch_datagram(
            &undecodable_datagram(0xBEEF),
            client.local_addr().expect("client address"),
            Arc::new(handler),
            open_rate_limiter(),
            server,
        )
        .await
        .expect("dispatching a malformed datagram should not fail");

        let bytes = recv_datagram(&client).await;
        let response = Message::from_vec(&bytes).expect("FORMERR must be a valid message");

        assert_eq!(response.metadata.id, 0xBEEF, "reply must carry the id");
        assert_eq!(response.metadata.message_type, MessageType::Response);
        assert_eq!(response.metadata.response_code, ResponseCode::FormErr);
    }

    #[tokio::test]
    async fn test_datagram_without_a_transaction_id_is_not_answered() {
        // Shorter than an ID: there is nothing for a client to match a reply
        // against, so a reply would be pure noise.
        let (handler, _server) = mock_backed_handler(200, &llm_reply("unused")).await;
        let (client, server) = socket_pair().await;

        dispatch_datagram(
            &[0x00],
            client.local_addr().expect("client address"),
            Arc::new(handler),
            open_rate_limiter(),
            server,
        )
        .await
        .expect("dispatching a short datagram should not fail");

        let mut buf = [0u8; 64];
        let received =
            tokio::time::timeout(Duration::from_millis(200), client.recv_from(&mut buf)).await;
        assert!(
            received.is_err(),
            "a datagram too short to hold an ID must not be answered"
        );
    }

    #[tokio::test]
    async fn test_shutdown_before_start_is_latched() {
        // A broadcast sender has no subscriber until after the socket bind, so
        // `send` failed there: the caller propagated the error and exited
        // non-zero on a perfectly clean interrupt.
        let (handler, _server) = mock_backed_handler(200, &llm_reply("unused")).await;
        let server = Server::with_handler(test_config(0), Arc::new(handler));

        server
            .shutdown()
            .expect("shutdown before start must succeed");
        server.shutdown().expect("shutdown must be idempotent");

        assert!(*server.shutdown_tx.borrow(), "the flag must be latched");
    }

    #[tokio::test]
    async fn test_start_returns_promptly_when_shutdown_was_requested() {
        let (handler, _server) = mock_backed_handler(200, &llm_reply("unused")).await;

        // Hold the port so a bind attempt would fail loudly: returning Ok proves
        // start() never got as far as binding.
        let squatter = UdpSocket::bind("127.0.0.1:0")
            .await
            .expect("bind squatter socket");
        let port = squatter.local_addr().expect("squatter address").port();

        let server = Server::with_handler(test_config(port), Arc::new(handler));
        server
            .shutdown()
            .expect("shutdown before start must succeed");

        tokio::time::timeout(REPLY_TIMEOUT, server.start())
            .await
            .expect("start() must return promptly once shutdown was requested")
            .expect("start() must succeed after a clean shutdown");
    }

    #[tokio::test]
    async fn test_start_serves_datagrams_and_stops_on_shutdown() {
        let (handler, _server) = mock_backed_handler(200, &llm_reply("mocked answer")).await;

        // Take a free port, then release it for the server to claim.
        let probe = UdpSocket::bind("127.0.0.1:0")
            .await
            .expect("bind probe socket");
        let port = probe.local_addr().expect("probe address").port();
        drop(probe);

        let server = Arc::new(Server::with_handler(test_config(port), Arc::new(handler)));
        let server_task = {
            let server = server.clone();
            tokio::spawn(async move { server.start().await })
        };

        let client = UdpSocket::bind("127.0.0.1:0")
            .await
            .expect("bind client socket");
        let server_addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);

        // A malformed datagram is the cheapest proof that the receive loop is
        // live: its FORMERR can only come from a running server. Retry until the
        // bind has happened, since the port was only just released.
        let request_bytes = undecodable_datagram(0x0102);
        let mut reply = None;
        for _ in 0..50 {
            client
                .send_to(&request_bytes, server_addr)
                .await
                .expect("send probe datagram");

            let mut buf = [0u8; MAX_UDP_REQUEST];
            if let Ok(Ok((n, _))) =
                tokio::time::timeout(Duration::from_millis(100), client.recv_from(&mut buf)).await
            {
                reply = Some(buf[..n].to_vec());
                break;
            }
        }

        let reply = reply.expect("server never answered the probe datagram");
        let response = Message::from_vec(&reply).expect("valid reply");
        assert_eq!(response.metadata.id, 0x0102);
        assert_eq!(response.metadata.response_code, ResponseCode::FormErr);

        server.shutdown().expect("shutdown the running server");
        tokio::time::timeout(REPLY_TIMEOUT, server_task)
            .await
            .expect("start() must return after shutdown")
            .expect("server task panicked")
            .expect("start() must succeed");
    }

    #[tokio::test]
    async fn test_response_within_budget_keeps_everything() {
        let (handler, _server) =
            mock_backed_handler(200, &llm_reply("Rust is a systems language.")).await;
        let request = txt_request(77, "what.is.rust.");

        let bytes = dns_exchange(&request, Arc::new(handler), open_rate_limiter()).await;
        let response = Message::from_vec(&bytes).expect("valid reply");

        assert_eq!(response.metadata.response_code, ResponseCode::NoError);
        assert_eq!(
            response.answers.len(),
            1,
            "the whole answer must be delivered"
        );
        assert!(
            !response.metadata.truncation,
            "nothing was left behind, so TC must stay clear"
        );
        assert_eq!(response.queries, request.queries);
        assert!(
            bytes.len() <= DEFAULT_MAX_UDP_RESPONSE,
            "got {} bytes",
            bytes.len()
        );
    }

    #[tokio::test]
    async fn test_oversized_response_sheds_overflow_and_keeps_what_fits() {
        // ~3000 characters is 12 TXT records, well past the 512 byte plain-DNS
        // budget. Message::truncate used to drop all 12, so the client got
        // nothing at all for a question it had already paid for.
        let (handler, _server) = mock_backed_handler(200, &llm_reply(&"a".repeat(3000))).await;
        let request = txt_request(78, "what.is.rust.");

        let bytes = dns_exchange(&request, Arc::new(handler), open_rate_limiter()).await;
        let response = Message::from_vec(&bytes).expect("valid reply");

        assert_eq!(response.metadata.response_code, ResponseCode::NoError);
        assert!(
            !response.answers.is_empty(),
            "the client must receive the answers that fit"
        );
        assert_eq!(
            response.answers.len(),
            1,
            "a TXT answer costs ~263 bytes, so exactly one fits in 512"
        );
        assert!(
            response.metadata.truncation,
            "TC must be set when records were left behind"
        );
        assert_eq!(
            response.queries, request.queries,
            "the question must survive shedding"
        );
        assert!(
            bytes.len() <= DEFAULT_MAX_UDP_RESPONSE,
            "got {} bytes",
            bytes.len()
        );
    }

    #[tokio::test]
    async fn test_edns_client_receives_more_chunks_than_plain_dns() {
        // The 1232 byte EDNS budget is the common case, and it must carry several
        // chunks where plain DNS carries one.
        let (handler, _server) = mock_backed_handler(200, &llm_reply(&"a".repeat(3000))).await;
        let mut request = txt_request(79, "what.is.rust.");
        let mut edns = Edns::new();
        edns.set_max_payload(1232);
        request.set_edns(edns);

        let bytes = dns_exchange(&request, Arc::new(handler), open_rate_limiter()).await;
        let response = Message::from_vec(&bytes).expect("valid reply");

        assert!(
            response.answers.len() > 1,
            "an EDNS client should get more than one chunk, got {}",
            response.answers.len()
        );
        assert!(
            response.metadata.truncation,
            "TC must be set when records were left behind"
        );
        assert!(bytes.len() <= MAX_UDP_RESPONSE, "got {} bytes", bytes.len());
    }

    #[tokio::test]
    async fn test_question_section_over_budget_still_gets_an_in_budget_reply() {
        // `truncate` re-emits the question section, so a client asking a dozen
        // long questions was sent an over-budget datagram unless the final size
        // is re-checked.
        let request = long_question_request(80, 12);
        assert!(
            request.to_vec().expect("encode request").len() > DEFAULT_MAX_UDP_RESPONSE,
            "the fixture's question section must overrun the budget on its own"
        );

        let (handler, _server) = mock_backed_handler(200, &llm_reply("unused")).await;
        let bytes = dns_exchange(&request, Arc::new(handler), open_rate_limiter()).await;
        let response = Message::from_vec(&bytes).expect("valid reply");

        assert_eq!(response.metadata.id, 80, "reply must stay matchable");
        assert_eq!(response.metadata.response_code, ResponseCode::NotImp);
        assert!(response.metadata.truncation, "TC must be set");
        assert!(
            bytes.len() <= DEFAULT_MAX_UDP_RESPONSE,
            "got {} bytes",
            bytes.len()
        );
    }

    #[tokio::test]
    async fn test_query_is_shed_when_llm_concurrency_exhausted() {
        // With every permit held, a further query must be refused *before* the
        // outbound call. The mock endpoint asserts that nothing reached it.
        let (mock, server) = forbidden_llm().await;
        let handler = test_handler_with_limit(1, &server.url());
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
        mock.assert();
    }

    #[tokio::test]
    async fn test_permit_is_released_after_query() {
        // A shed query must not leak its permit, or the server would wedge shut
        // after the first burst.
        let (mock, server) = forbidden_llm().await;
        let handler = test_handler_with_limit(1, &server.url());
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
        mock.assert();
    }

    #[test]
    fn test_zero_limit_disables_llm_concurrency_cap() {
        assert!(test_handler_with_limit(0, UNUSED_BASE_URL)
            .llm_permits
            .is_none());
        assert!(test_handler_with_limit(4, UNUSED_BASE_URL)
            .llm_permits
            .is_some());
    }
}
