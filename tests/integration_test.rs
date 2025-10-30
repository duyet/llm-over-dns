mod common;

use anyhow::Result;
use futures::future;
use llm_over_dns::{Chunker, Config, DnsHandler, LlmClient};
use std::env;

/// Test LLM client creation
#[test]
fn test_llm_client_creation() -> Result<()> {
    let _client = LlmClient::new(
        "test_key".to_string(),
        vec!["test_model".to_string()],
        "Test system prompt".to_string(),
        None,
        None,
        None,
        None,
        None,
        None,
    )?;
    Ok(())
}

/// Test config loading from environment
/// Note: This test may occasionally fail in parallel execution due to environment variable races
#[test]
#[ignore]
fn test_config_from_environment() -> Result<()> {
    // Clean environment first to avoid contamination
    env::remove_var("OPENROUTER_MODEL");
    env::remove_var("DNS_PORT");
    env::remove_var("DNS_ADDRESS");

    // Set environment variables
    env::set_var("OPENROUTER_API_KEY", "test_key_from_env");
    env::set_var("OPENROUTER_MODEL", "test_model_from_env");
    env::set_var("DNS_PORT", "5353");
    env::set_var("DNS_ADDRESS", "127.0.0.1");

    // Load config
    let config = Config::from_env()?;

    // Verify config values
    assert_eq!(config.openrouter_api_key, "test_key_from_env");
    assert_eq!(
        config.openrouter_models,
        vec!["test_model_from_env".to_string()]
    );
    assert_eq!(config.dns_port, 5353);
    assert_eq!(config.dns_address, "127.0.0.1");

    // Cleanup
    env::remove_var("OPENROUTER_API_KEY");
    env::remove_var("OPENROUTER_MODEL");
    env::remove_var("DNS_PORT");
    env::remove_var("DNS_ADDRESS");

    Ok(())
}

/// Test config with defaults
/// Note: This test may occasionally fail in parallel execution due to environment variable races
#[test]
#[ignore]
fn test_config_with_defaults() -> Result<()> {
    // Clean environment first
    env::remove_var("OPENROUTER_MODEL");
    env::remove_var("DNS_PORT");
    env::remove_var("DNS_ADDRESS");

    env::set_var("OPENROUTER_API_KEY", "test_key_defaults");

    let config = Config::from_env()?;

    assert_eq!(config.openrouter_api_key, "test_key_defaults");
    assert_eq!(
        config.openrouter_models,
        vec!["nvidia/nemotron-nano-12b-v2-vl:free".to_string()]
    );
    assert_eq!(config.dns_port, 53);
    assert_eq!(config.dns_address, "0.0.0.0");

    env::remove_var("OPENROUTER_API_KEY");

    Ok(())
}

/// Test DNS handler creation and basic functionality
#[test]
fn test_dns_handler_creation() -> Result<()> {
    let handler = DnsHandler::new();
    assert_eq!(handler, DnsHandler::new());
    Ok(())
}

/// Test DNS handler treats query text directly as prompts
#[test]
fn test_dns_handler_parse_subdomain() -> Result<()> {
    let handler = DnsHandler::new();

    // Test simple text
    let result = handler.parse_subdomain("hello world")?;
    assert_eq!(result, "hello world");

    // Test question
    let result = handler.parse_subdomain("what is rust")?;
    assert_eq!(result, "what is rust");

    // Test with hyphens and special characters
    let result = handler.parse_subdomain("hello-world")?;
    assert_eq!(result, "hello-world");

    Ok(())
}

/// Test DNS handler validates TXT query type
#[test]
fn test_dns_handler_validates_txt_queries() -> Result<()> {
    let handler = DnsHandler::new();

    // TXT record type is 16
    assert!(handler.is_valid_txt_query(16));

    // Other record types should be invalid
    assert!(!handler.is_valid_txt_query(1)); // A record
    assert!(!handler.is_valid_txt_query(28)); // AAAA record
    assert!(!handler.is_valid_txt_query(5)); // CNAME record

    Ok(())
}

/// Test chunker with short response
#[test]
fn test_chunker_short_response() -> Result<()> {
    let chunker = Chunker::new();

    let short_response = "Hello World";
    let chunks = chunker.chunk_text(short_response);

    assert_eq!(chunks.len(), 1, "Short response should fit in single chunk");
    assert_eq!(chunks[0], short_response);

    Ok(())
}

/// Test chunker with long response
#[test]
fn test_chunker_long_response() -> Result<()> {
    let chunker = Chunker::new();

    // Create a long response (> 255 bytes for DNS TXT limit)
    let long_response = "This is a very long response that exceeds the DNS TXT record limit of 255 bytes. \
        It should be split into multiple chunks. Lorem ipsum dolor sit amet, consectetur adipiscing elit. \
        Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, \
        quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.";

    let chunks = chunker.chunk_text(long_response);

    // Assert that long responses are chunked
    assert!(
        chunks.len() > 1,
        "Long response should be split into multiple chunks"
    );

    // Verify each chunk is within the limit
    for chunk in &chunks {
        assert!(
            chunk.len() <= 255,
            "Each chunk must be <= 255 bytes, got {}",
            chunk.len()
        );
    }

    // Verify content is preserved
    let joined = chunks.join("");
    assert!(
        joined.contains("Lorem ipsum"),
        "Content should be preserved in chunks"
    );

    Ok(())
}

/// Test chunker with empty response
#[test]
fn test_chunker_empty_response() -> Result<()> {
    let chunker = Chunker::new();

    let chunks = chunker.chunk_text("");

    assert_eq!(
        chunks.len(),
        0,
        "Empty response should chunk to empty vector"
    );

    Ok(())
}

/// Test chunker with exact boundary
#[test]
fn test_chunker_exact_boundary() -> Result<()> {
    let chunker = Chunker::new();

    // Default max_chunk_size is 250 for Chunker::new()
    let response = "a".repeat(250);

    let chunks = chunker.chunk_text(&response);

    assert_eq!(
        chunks.len(),
        1,
        "250-byte response should fit in single chunk with default settings"
    );
    assert_eq!(chunks[0].len(), 250);

    Ok(())
}

/// Test chunker with just over boundary
#[test]
fn test_chunker_over_boundary() -> Result<()> {
    let chunker = Chunker::new();

    // Default max_chunk_size is 250, so 251 should split
    let response = "a".repeat(251);

    let chunks = chunker.chunk_text(&response);

    assert!(
        chunks.len() > 1,
        "251-byte response should split into multiple chunks"
    );

    // Verify all chunks are within limit
    for chunk in &chunks {
        assert!(
            chunk.len() <= 250,
            "Each chunk must be <= 250 bytes with default settings"
        );
    }

    Ok(())
}

/// Test chunker with custom sizes
#[test]
fn test_chunker_custom_size() -> Result<()> {
    let chunker = Chunker::with_sizes(10, 100);

    let response = "This is a test string";

    let chunks = chunker.chunk_text(response);

    // With size limit of 10, should be chunked
    for chunk in &chunks {
        assert!(chunk.len() <= 10, "Custom chunk size should be respected");
    }

    Ok(())
}

/// Test chunker maximum total size limit
#[test]
fn test_chunker_max_total_size() -> Result<()> {
    let chunker = Chunker::with_sizes(255, 1000);

    // Create a response larger than max_total_size
    let response = "a".repeat(2000);

    let chunks = chunker.chunk_text(&response);

    // Should be truncated to max_total_size
    let total = chunks.iter().map(|c| c.len()).sum::<usize>();
    assert!(total <= 1000, "Total size should not exceed max_total_size");

    Ok(())
}

/// Test basic flow: parse query -> chunk response
#[test]
fn test_basic_integration_flow() -> Result<()> {
    // Create handler
    let handler = DnsHandler::new();

    // Parse a query (direct text, no domain parsing)
    let query_str = "what is rust";
    let parsed = handler.parse_subdomain(query_str)?;
    assert_eq!(parsed, "what is rust");

    // Create chunker
    let chunker = Chunker::new();

    // Simulate chunking a response
    let response = "Rust is a systems programming language that runs blazingly fast.";
    let chunks = chunker.chunk_text(response);

    // Verify chunks are valid
    assert!(!chunks.is_empty());
    for chunk in &chunks {
        assert!(chunk.len() <= 250); // Default max_chunk_size
    }

    Ok(())
}

/// Test E2E flow: DNS query -> parse -> LLM -> chunk -> DNS response
#[tokio::test]
async fn test_e2e_dns_to_llm_to_response() -> Result<()> {
    use std::sync::Arc;

    // Setup mock LLM server
    let mut server = mockito::Server::new_async().await;
    let mock_response = r#"{
        "choices": [{
            "message": {
                "content": "Rust is a systems programming language."
            }
        }]
    }"#;

    let _mock = server
        .mock("POST", mockito::Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(mock_response)
        .create_async()
        .await;

    // Create components
    let llm_client = Arc::new(
        LlmClient::new(
            "test_key".to_string(),
            vec!["test_model".to_string()],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )?
        .with_base_url(server.url()),
    );
    let chunker = Arc::new(Chunker::new());
    let dns_handler = Arc::new(DnsHandler::new());

    // Simulate E2E flow (direct text query, no domain parsing)
    let query = "what is rust";
    let prompt = dns_handler.parse_subdomain(query)?;
    assert_eq!(prompt, "what is rust");

    let llm_response = llm_client.query(&prompt).await?;
    assert!(!llm_response.is_empty());

    let chunks = chunker.chunk_text(&llm_response);
    assert!(!chunks.is_empty());

    // Verify each chunk is within DNS TXT limits
    for chunk in chunks {
        assert!(chunk.len() <= 255);
    }

    Ok(())
}

/// Test E2E flow with long LLM response requiring chunking
#[tokio::test]
async fn test_e2e_long_response_chunking() -> Result<()> {
    use std::sync::Arc;

    // Setup mock LLM server with long response
    let mut server = mockito::Server::new_async().await;
    let long_content = "a".repeat(600); // Will require multiple chunks
    let mock_response = format!(
        r#"{{"choices": [{{"message": {{"content": "{}"}}}}]}}"#,
        long_content
    );

    let _mock = server
        .mock("POST", mockito::Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(mock_response)
        .create_async()
        .await;

    // Create components
    let llm_client = Arc::new(
        LlmClient::new(
            "test_key".to_string(),
            vec!["test_model".to_string()],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )?
        .with_base_url(server.url()),
    );
    let chunker = Arc::new(Chunker::new());
    let dns_handler = Arc::new(DnsHandler::new());

    // E2E flow (direct text query)
    let query = "tell me about rust";
    let prompt = dns_handler.parse_subdomain(query)?;

    let llm_response = llm_client.query(&prompt).await?;
    let chunks = chunker.chunk_text(&llm_response);

    // Verify multiple chunks
    assert!(chunks.len() > 1);

    // Verify each chunk is valid
    for chunk in &chunks {
        assert!(chunk.len() <= 250);
        assert!(!chunk.is_empty());
    }

    // Verify total doesn't exceed DNS UDP limit
    let total_size: usize = chunks.iter().map(|c| c.len()).sum();
    assert!(total_size <= 4096);

    Ok(())
}

/// Test E2E error handling: LLM returns 429 rate limit
#[tokio::test]
async fn test_e2e_llm_rate_limit_error() -> Result<()> {
    use std::sync::Arc;

    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("POST", mockito::Matcher::Any)
        .with_status(429)
        .with_body(r#"{"error": "rate_limit_exceeded"}"#)
        .create_async()
        .await;

    let llm_client = Arc::new(
        LlmClient::new(
            "test_key".to_string(),
            vec!["test_model".to_string()],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )?
        .with_base_url(server.url()),
    );
    let dns_handler = Arc::new(DnsHandler::new());

    let query = "test query";
    let prompt = dns_handler.parse_subdomain(query)?;

    let result = llm_client.query(&prompt).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Rate limit"));

    Ok(())
}

/// Test E2E error handling: LLM returns 500 server error
#[tokio::test]
async fn test_e2e_llm_server_error() -> Result<()> {
    use std::sync::Arc;

    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("POST", mockito::Matcher::Any)
        .with_status(500)
        .with_body(r#"{"error": "internal_server_error"}"#)
        .create_async()
        .await;

    let llm_client = Arc::new(
        LlmClient::new(
            "test_key".to_string(),
            vec!["test_model".to_string()],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )?
        .with_base_url(server.url()),
    );
    let dns_handler = Arc::new(DnsHandler::new());

    let query = "test question";
    let prompt = dns_handler.parse_subdomain(query)?;

    let result = llm_client.query(&prompt).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("server error"));

    Ok(())
}

/// Test E2E error handling: LLM returns 401 unauthorized
#[tokio::test]
async fn test_e2e_llm_unauthorized() -> Result<()> {
    use std::sync::Arc;

    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("POST", mockito::Matcher::Any)
        .with_status(401)
        .with_body(r#"{"error": "unauthorized"}"#)
        .create_async()
        .await;

    let llm_client = Arc::new(
        LlmClient::new(
            "invalid_key".to_string(),
            vec!["test_model".to_string()],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )?
        .with_base_url(server.url()),
    );
    let dns_handler = Arc::new(DnsHandler::new());

    let query = "test question";
    let prompt = dns_handler.parse_subdomain(query)?;

    let result = llm_client.query(&prompt).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Unauthorized"));

    Ok(())
}

/// Test E2E error handling: LLM returns invalid JSON
#[tokio::test]
async fn test_e2e_llm_invalid_json() -> Result<()> {
    use std::sync::Arc;

    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("POST", mockito::Matcher::Any)
        .with_status(200)
        .with_body(r#"{"invalid": "response structure"}"#)
        .create_async()
        .await;

    let llm_client = Arc::new(
        LlmClient::new(
            "test_key".to_string(),
            vec!["test_model".to_string()],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )?
        .with_base_url(server.url()),
    );
    let dns_handler = Arc::new(DnsHandler::new());

    let query = "test question";
    let prompt = dns_handler.parse_subdomain(query)?;

    let result = llm_client.query(&prompt).await;
    assert!(result.is_err());

    Ok(())
}

/// Test E2E error handling: malformed DNS query (any valid text is accepted now)
#[test]
fn test_e2e_malformed_dns_query_invalid_domain() -> Result<()> {
    let dns_handler = DnsHandler::new();

    // With simplified parsing, any text is valid (no domain validation)
    // This used to be an error but now it's valid
    let result = dns_handler.parse_subdomain("invalid.example.com");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "invalid.example.com");

    Ok(())
}

/// Test E2E error handling: empty query text
#[test]
fn test_e2e_malformed_dns_query_empty_subdomain() -> Result<()> {
    let dns_handler = DnsHandler::new();

    // Empty query text should still be an error
    let result = dns_handler.parse_subdomain("");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Empty query"));

    Ok(())
}

/// Test E2E network timeout scenario
#[tokio::test]
async fn test_e2e_network_timeout() -> Result<()> {
    use std::sync::Arc;

    // Use invalid URL that will fail to connect
    let llm_client = Arc::new(
        LlmClient::new(
            "test_key".to_string(),
            vec!["test_model".to_string()],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )?
        .with_base_url("http://invalid.local:99999".to_string()),
    );
    let dns_handler = Arc::new(DnsHandler::new());

    let query = "test question";
    let prompt = dns_handler.parse_subdomain(query)?;

    let result = llm_client.query(&prompt).await;
    assert!(result.is_err());

    Ok(())
}

/// Test concurrent request handling
#[tokio::test]
async fn test_concurrent_requests() -> Result<()> {
    use std::sync::Arc;
    use tokio::task;

    // Setup mock LLM server
    let mut server = mockito::Server::new_async().await;
    let mock_response = r#"{
        "choices": [{
            "message": {
                "content": "Response"
            }
        }]
    }"#;

    let _mock = server
        .mock("POST", mockito::Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(mock_response)
        .expect_at_least(5) // Expect at least 5 concurrent requests
        .create_async()
        .await;

    // Create shared components
    let llm_client = Arc::new(
        LlmClient::new(
            "test_key".to_string(),
            vec!["test_model".to_string()],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )?
        .with_base_url(server.url()),
    );
    let dns_handler = Arc::new(DnsHandler::new());

    // Spawn concurrent tasks
    let mut handles = vec![];
    for i in 0..5 {
        let client = llm_client.clone();
        let handler = dns_handler.clone();

        let handle = task::spawn(async move {
            let query = format!("test query {}", i);
            let prompt = handler.parse_subdomain(&query)?;
            client.query(&prompt).await
        });

        handles.push(handle);
    }

    // Wait for all tasks to complete
    let results: Result<Vec<_>> = future::try_join_all(handles)
        .await
        .map_err(|e| anyhow::anyhow!("Task join error: {}", e))?
        .into_iter()
        .collect();

    // Verify all succeeded
    let responses = results?;
    assert_eq!(responses.len(), 5);
    for response in responses {
        assert!(!response.is_empty());
    }

    Ok(())
}

/// Test performance baseline: single request latency
#[tokio::test]
async fn test_performance_single_request() -> Result<()> {
    use std::sync::Arc;
    use std::time::Instant;

    let mut server = mockito::Server::new_async().await;
    let mock_response = r#"{
        "choices": [{
            "message": {
                "content": "Fast response"
            }
        }]
    }"#;

    let _mock = server
        .mock("POST", mockito::Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(mock_response)
        .create_async()
        .await;

    let llm_client = Arc::new(
        LlmClient::new(
            "test_key".to_string(),
            vec!["test_model".to_string()],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )?
        .with_base_url(server.url()),
    );
    let dns_handler = Arc::new(DnsHandler::new());
    let chunker = Arc::new(Chunker::new());

    // Measure E2E latency
    let start = Instant::now();

    let query = "test question";
    let prompt = dns_handler.parse_subdomain(query)?;
    let response = llm_client.query(&prompt).await?;
    let chunks = chunker.chunk_text(&response);

    let duration = start.elapsed();

    // Verify completion
    assert!(!chunks.is_empty());

    // Performance baseline: should complete in reasonable time
    // Note: This is a mock, so should be very fast (<100ms)
    assert!(
        duration.as_millis() < 100,
        "E2E flow took {}ms, expected <100ms",
        duration.as_millis()
    );

    Ok(())
}

/// Test edge case: empty LLM response
#[tokio::test]
async fn test_e2e_empty_llm_response() -> Result<()> {
    use std::sync::Arc;

    let mut server = mockito::Server::new_async().await;
    let mock_response = r#"{
        "choices": [{
            "message": {
                "content": ""
            }
        }]
    }"#;

    let _mock = server
        .mock("POST", mockito::Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(mock_response)
        .create_async()
        .await;

    let llm_client = Arc::new(
        LlmClient::new(
            "test_key".to_string(),
            vec!["test_model".to_string()],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )?
        .with_base_url(server.url()),
    );
    let chunker = Arc::new(Chunker::new());
    let dns_handler = Arc::new(DnsHandler::new());

    let query = "test question";
    let prompt = dns_handler.parse_subdomain(query)?;
    let response = llm_client.query(&prompt).await?;
    let chunks = chunker.chunk_text(&response);

    // Empty response should result in empty chunks
    assert_eq!(chunks.len(), 0);

    Ok(())
}

/// Test edge case: Unicode in queries and responses
#[tokio::test]
async fn test_e2e_unicode_handling() -> Result<()> {
    use std::sync::Arc;

    let mut server = mockito::Server::new_async().await;
    let unicode_response = "Hello 世界 🌍 مرحبا";
    let mock_response = format!(
        r#"{{"choices": [{{"message": {{"content": "{}"}}}}]}}"#,
        unicode_response
    );

    let _mock = server
        .mock("POST", mockito::Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(mock_response)
        .create_async()
        .await;

    let llm_client = Arc::new(
        LlmClient::new(
            "test_key".to_string(),
            vec!["test_model".to_string()],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )?
        .with_base_url(server.url()),
    );
    let chunker = Arc::new(Chunker::new());
    let dns_handler = Arc::new(DnsHandler::new());

    let query = "test question";
    let prompt = dns_handler.parse_subdomain(query)?;
    let response = llm_client.query(&prompt).await?;
    let chunks = chunker.chunk_text(&response);

    // Verify chunks are valid UTF-8
    for chunk in &chunks {
        // This will panic if not valid UTF-8
        let _ = chunk.chars().count();
    }

    // Verify content is preserved
    let reassembled = chunks.join("");
    assert_eq!(reassembled, unicode_response);

    Ok(())
}

/// Test edge case: maximum DNS response size (4096 bytes)
#[tokio::test]
async fn test_e2e_maximum_dns_response_size() -> Result<()> {
    use std::sync::Arc;

    let mut server = mockito::Server::new_async().await;
    // Create response larger than DNS UDP limit
    let large_response = "a".repeat(5000);
    let mock_response = format!(
        r#"{{"choices": [{{"message": {{"content": "{}"}}}}]}}"#,
        large_response
    );

    let _mock = server
        .mock("POST", mockito::Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(mock_response)
        .create_async()
        .await;

    let llm_client = Arc::new(
        LlmClient::new(
            "test_key".to_string(),
            vec!["test_model".to_string()],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )?
        .with_base_url(server.url()),
    );
    let chunker = Arc::new(Chunker::new());
    let dns_handler = Arc::new(DnsHandler::new());

    let query = "test question";
    let prompt = dns_handler.parse_subdomain(query)?;
    let response = llm_client.query(&prompt).await?;
    let chunks = chunker.chunk_text(&response);

    // Verify total size doesn't exceed DNS UDP limit
    let total_size: usize = chunks.iter().map(|c| c.len()).sum();
    assert!(total_size <= 4096);

    // Verify each chunk is within TXT record limit
    for chunk in chunks {
        assert!(chunk.len() <= 250);
    }

    Ok(())
}
