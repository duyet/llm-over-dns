//! DNS query handler for parsing and routing LLM requests.
//!
//! This module handles DNS protocol interactions, treating DNS queries
//! directly as LLM prompts and building TXT record responses.
//!
//! # Overview
//!
//! The [`DnsHandler`] processes DNS queries by:
//! 1. Treating the query text directly as the LLM prompt (no domain parsing)
//! 2. Validating query types (TXT records only)
//! 3. Building formatted DNS responses
//!
//! # Query Format
//!
//! DNS queries are used directly as prompts to the LLM:
//! - `dig @localhost -p 6000 'hello world' TXT` → sends "hello world" to LLM
//! - `dig @localhost -p 6000 'what is rust' TXT` → sends "what is rust" to LLM
//! - Trailing dots (DNS format) are automatically removed
//!
//! # Examples
//!
//! ```
//! use llm_over_dns::DnsHandler;
//!
//! let handler = DnsHandler::new();
//!
//! // Parse query directly as prompt
//! let prompt = handler.parse_subdomain("hello world").unwrap();
//! assert_eq!(prompt, "hello world");
//!
//! // Trailing dots are removed
//! let prompt = handler.parse_subdomain("what is rust.").unwrap();
//! assert_eq!(prompt, "what is rust");
//!
//! // Check if query type is valid
//! assert!(handler.is_valid_txt_query(16)); // TXT record type
//! assert!(!handler.is_valid_txt_query(1));  // A record type
//! ```

use anyhow::{anyhow, Result};

/// DNS Handler for parsing queries and building responses.
///
/// Provides utilities for:
/// - Treating DNS query text directly as LLM prompts (no parsing)
/// - Validating query types
/// - Building TXT record responses
///
/// # Thread Safety
///
/// `DnsHandler` is stateless and can be safely shared across threads.
/// It implements `Clone`, `Debug`, `PartialEq`, and `Eq`.
///
/// # Examples
///
/// ```
/// use llm_over_dns::DnsHandler;
///
/// let handler = DnsHandler::new();
///
/// // Parse a simple query
/// let prompt = handler.parse_subdomain("hello world").unwrap();
/// assert_eq!(prompt, "hello world");
///
/// // Parse query with trailing dot
/// let prompt = handler.parse_subdomain("what is rust.").unwrap();
/// assert_eq!(prompt, "what is rust");
///
/// // Hyphens and other characters are preserved
/// let prompt = handler.parse_subdomain("hello-world").unwrap();
/// assert_eq!(prompt, "hello-world");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsHandler;

impl DnsHandler {
    /// Creates a new DnsHandler instance.
    ///
    /// # Examples
    ///
    /// ```
    /// use llm_over_dns::DnsHandler;
    ///
    /// let handler = DnsHandler::new();
    /// ```
    pub fn new() -> Self {
        Self
    }

    /// Parses a DNS query into a prompt string.
    ///
    /// Treats the entire query as input text, removing the trailing dot if present.
    /// No domain parsing needed - the query IS the prompt.
    ///
    /// # Arguments
    ///
    /// * `domain` - DNS query text to use as prompt
    ///
    /// # Returns
    ///
    /// * `Ok(String)` - The query text as prompt
    /// * `Err` - If query is empty
    ///
    /// # Examples
    ///
    /// ```
    /// use llm_over_dns::DnsHandler;
    ///
    /// let handler = DnsHandler::new();
    ///
    /// // Simple query
    /// assert_eq!(
    ///     handler.parse_subdomain("hello world").unwrap(),
    ///     "hello world"
    /// );
    ///
    /// // Query with trailing dot (DNS format)
    /// assert_eq!(
    ///     handler.parse_subdomain("what is rust.").unwrap(),
    ///     "what is rust"
    /// );
    ///
    /// // Any text works
    /// assert_eq!(
    ///     handler.parse_subdomain("explain machine learning").unwrap(),
    ///     "explain machine learning"
    /// );
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if the query is empty after trimming.
    ///
    /// ```
    /// use llm_over_dns::DnsHandler;
    ///
    /// let handler = DnsHandler::new();
    ///
    /// // Empty query
    /// assert!(handler.parse_subdomain("").is_err());
    /// assert!(handler.parse_subdomain("  ").is_err());
    /// assert!(handler.parse_subdomain(".").is_err());
    /// ```
    pub fn parse_subdomain(&self, domain: &str) -> Result<String> {
        // Remove trailing dot (DNS format) and trim whitespace
        let query = domain.trim().trim_end_matches('.');

        // Check if query is empty
        if query.is_empty() {
            return Err(anyhow!("Empty query: no text provided"));
        }

        // Return the query as-is - it IS the prompt
        Ok(query.to_string())
    }

    /// Legacy method name for backwards compatibility
    #[deprecated(since = "0.2.0", note = "Use parse_query instead")]
    pub fn parse_subdomain_legacy(&self, domain: &str) -> Result<String> {
        let domain = domain.trim().to_lowercase();

        // Old subdomain parsing logic
        let subdomain = domain
            .strip_suffix(".llm.duyet.net")
            .ok_or_else(|| anyhow!("Failed to extract subdomain"))?;

        if subdomain.is_empty() {
            return Err(anyhow!("Empty subdomain: no labels before base domain"));
        }

        // Split by dots and join with spaces
        let prompt = subdomain.split('.').collect::<Vec<&str>>().join(" ");

        if prompt.is_empty() {
            return Err(anyhow!("Empty prompt after parsing"));
        }

        Ok(prompt)
    }

    /// Builds a DNS TXT record from response chunks.
    ///
    /// Combines multiple response chunks into bytes for DNS TXT record format.
    /// Each chunk is concatenated without separators.
    ///
    /// # Arguments
    ///
    /// * `chunks` - Vector of response chunks to combine
    ///
    /// # Returns
    ///
    /// UTF-8 encoded bytes of combined chunks
    ///
    /// # Examples
    ///
    /// ```
    /// use llm_over_dns::DnsHandler;
    ///
    /// let handler = DnsHandler::new();
    ///
    /// // Single chunk
    /// let record = handler.build_txt_record(vec!["Hello world".to_string()]);
    /// assert_eq!(record, b"Hello world");
    ///
    /// // Multiple chunks
    /// let record = handler.build_txt_record(vec![
    ///     "Hello ".to_string(),
    ///     "world".to_string(),
    /// ]);
    /// assert_eq!(record, b"Hello world");
    ///
    /// // Empty input
    /// let record = handler.build_txt_record(vec![]);
    /// assert_eq!(record, b"");
    /// ```
    ///
    /// # Note
    ///
    /// DNS TXT records have length prefixes added by the DNS library.
    /// This method returns raw UTF-8 bytes which are then formatted
    /// by the DNS protocol implementation.
    ///
    /// # See Also
    ///
    /// * [`parse_subdomain`](#method.parse_subdomain) - Parsing queries
    /// * [`llm_over_dns::Chunker`] - Text chunking utilities
    pub fn build_txt_record(&self, chunks: Vec<String>) -> Vec<u8> {
        // Combine all chunks
        let combined = chunks.join("");

        // For TXT records, we need to return the content as bytes
        // In practice, DNS TXT records have length prefixes, but for this
        // implementation we return the raw bytes
        combined.into_bytes()
    }

    /// Validates if a query is for a TXT record.
    ///
    /// Checks DNS query type code to determine if it's a TXT record request.
    ///
    /// # Arguments
    ///
    /// * `query_type` - DNS query type code (u16)
    ///
    /// # Returns
    ///
    /// `true` if query type is TXT (16), `false` otherwise
    ///
    /// # DNS Query Type Codes
    ///
    /// - `1` = A record (IPv4 address)
    /// - `5` = CNAME record (canonical name)
    /// - `16` = TXT record (text data)
    /// - `28` = AAAA record (IPv6 address)
    ///
    /// # Examples
    ///
    /// ```
    /// use llm_over_dns::DnsHandler;
    ///
    /// let handler = DnsHandler::new();
    ///
    /// // TXT record type
    /// assert!(handler.is_valid_txt_query(16));
    ///
    /// // Other record types
    /// assert!(!handler.is_valid_txt_query(1));   // A record
    /// assert!(!handler.is_valid_txt_query(5));   // CNAME record
    /// assert!(!handler.is_valid_txt_query(28));  // AAAA record
    /// ```
    ///
    /// # Usage in DNS Handler
    ///
    /// ```no_run
    /// use llm_over_dns::DnsHandler;
    /// # use hickory_proto::rr::RecordType;
    ///
    /// let handler = DnsHandler::new();
    ///
    /// // In DNS query processing:
    /// let query_type = 16; // TXT record
    /// if handler.is_valid_txt_query(query_type) {
    ///     // Process TXT query
    /// } else {
    ///     // Return DNS error (NOTIMP)
    /// }
    /// ```
    pub fn is_valid_txt_query(&self, query_type: u16) -> bool {
        // TXT record type is 16
        query_type == 16
    }
}

impl Default for DnsHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_subdomain_simple() {
        let handler = DnsHandler::new();
        let result = handler.parse_subdomain("hello world");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "hello world");
    }

    #[test]
    fn test_parse_subdomain_multiple_labels() {
        let handler = DnsHandler::new();
        let result = handler.parse_subdomain("what is rust");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "what is rust");
    }

    #[test]
    fn test_parse_subdomain_with_hyphens() {
        let handler = DnsHandler::new();
        let result = handler.parse_subdomain("hello-world");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "hello-world");
    }

    #[test]
    fn test_parse_subdomain_with_hyphens_multiple_labels() {
        let handler = DnsHandler::new();
        let result = handler.parse_subdomain("hello-world foo-bar");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "hello-world foo-bar");
    }

    #[test]
    fn test_parse_subdomain_empty() {
        let handler = DnsHandler::new();
        let result = handler.parse_subdomain("");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Empty query"));
    }

    #[test]
    fn test_parse_subdomain_invalid_domain() {
        let handler = DnsHandler::new();
        // Any text is valid now - this test checks trailing dot removal
        let result = handler.parse_subdomain("hello example.");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "hello example");
    }

    #[test]
    fn test_parse_subdomain_case_insensitive() {
        let handler = DnsHandler::new();
        let result = handler.parse_subdomain("HELLO IS RUST");
        assert!(result.is_ok());
        // Case is preserved now
        assert_eq!(result.unwrap(), "HELLO IS RUST");
    }

    #[test]
    fn test_parse_subdomain_with_whitespace() {
        let handler = DnsHandler::new();
        let result = handler.parse_subdomain("  hello world  ");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "hello world");
    }

    #[test]
    fn test_build_txt_record_short() {
        let handler = DnsHandler::new();
        let chunks = vec!["Hello world".to_string()];
        let result = handler.build_txt_record(chunks);
        assert_eq!(result, b"Hello world");
    }

    #[test]
    fn test_build_txt_record_chunked() {
        let handler = DnsHandler::new();
        let chunks = vec!["Hello ".to_string(), "world".to_string()];
        let result = handler.build_txt_record(chunks);
        assert_eq!(result, b"Hello world");
    }

    #[test]
    fn test_build_txt_record_multiple_chunks() {
        let handler = DnsHandler::new();
        let chunks = vec![
            "The ".to_string(),
            "quick ".to_string(),
            "brown ".to_string(),
            "fox".to_string(),
        ];
        let result = handler.build_txt_record(chunks);
        assert_eq!(result, b"The quick brown fox");
    }

    #[test]
    fn test_build_txt_record_empty() {
        let handler = DnsHandler::new();
        let chunks: Vec<String> = vec![];
        let result = handler.build_txt_record(chunks);
        assert_eq!(result, b"");
    }

    #[test]
    fn test_build_txt_record_special_chars() {
        let handler = DnsHandler::new();
        let chunks = vec!["Hello!@#$%^&*()[]{}".to_string()];
        let result = handler.build_txt_record(chunks);
        assert_eq!(result, b"Hello!@#$%^&*()[]{}");
    }

    #[test]
    fn test_is_valid_txt_query_txt_type() {
        let handler = DnsHandler::new();
        assert!(handler.is_valid_txt_query(16)); // TXT record type
    }

    #[test]
    fn test_is_valid_txt_query_a_record() {
        let handler = DnsHandler::new();
        assert!(!handler.is_valid_txt_query(1)); // A record type
    }

    #[test]
    fn test_is_valid_txt_query_aaaa_record() {
        let handler = DnsHandler::new();
        assert!(!handler.is_valid_txt_query(28)); // AAAA record type
    }

    #[test]
    fn test_is_valid_txt_query_cname_record() {
        let handler = DnsHandler::new();
        assert!(!handler.is_valid_txt_query(5)); // CNAME record type
    }

    #[test]
    fn test_dns_handler_default() {
        let handler = DnsHandler;
        let result = handler.parse_subdomain("test query");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test query");
    }

    #[test]
    fn test_dns_handler_clone() {
        let handler1 = DnsHandler::new();
        let handler2 = handler1.clone();

        let result1 = handler1.parse_subdomain("test query").unwrap();
        let result2 = handler2.parse_subdomain("test query").unwrap();

        assert_eq!(result1, result2);
    }

    #[test]
    fn test_parse_subdomain_long_labels() {
        let handler = DnsHandler::new();
        let long_query = "a".repeat(100); // Long text query
        let result = handler.parse_subdomain(&long_query);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), long_query);
    }

    #[test]
    fn test_parse_subdomain_many_labels() {
        let handler = DnsHandler::new();
        let query = "a b c d e f g h i j";
        let result = handler.parse_subdomain(query);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "a b c d e f g h i j");
    }

    #[test]
    fn test_build_txt_record_unicode() {
        let handler = DnsHandler::new();
        let chunks = vec!["Hello 世界 🌍".to_string()];
        let result = handler.build_txt_record(chunks);
        let expected = "Hello 世界 🌍".as_bytes();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_subdomain_numbers() {
        let handler = DnsHandler::new();
        let result = handler.parse_subdomain("what is 123");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "what is 123");
    }

    #[test]
    fn test_parse_subdomain_with_trailing_dot() {
        let handler = DnsHandler::new();
        // DNS queries often have trailing dot
        let result = handler.parse_subdomain("hello world.");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "hello world");
    }

    #[test]
    fn test_parse_subdomain_only_dot() {
        let handler = DnsHandler::new();
        let result = handler.parse_subdomain(".");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Empty query"));
    }

    #[test]
    fn test_dns_handler_equality() {
        let handler1 = DnsHandler::new();
        let handler2 = DnsHandler;
        assert_eq!(handler1, handler2);
    }
}
