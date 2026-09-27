use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tracing::{debug, error};

/// Per-request timeout applied to a single model attempt.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Budget for a whole [`LlmClient::query`] call, i.e. for the entire fallback
/// chain rather than for one request.
///
/// The per-request timeout bounds a single attempt, but models are tried
/// sequentially, so without an overall budget a call can take
/// `models.len() * 30s` (90s with the default 3-model list) while its caller
/// holds a concurrency permit for the whole window. Two request timeouts leaves
/// room for a first attempt plus a fallback.
const FALLBACK_TIMEOUT: Duration = Duration::from_secs(60);

/// Upper bound on how much of an error response body is copied into an error
/// message. A provider can answer a 4xx with a multi-megabyte HTML page, and
/// one bad response must not become one huge log line.
const MAX_ERROR_BODY_BYTES: usize = 512;

/// Message in the OpenRouter API request
#[derive(Debug, Clone, Serialize)]
struct Message {
    role: String,
    content: String,
}

/// Request sent to OpenRouter API
#[derive(Debug, Clone, Serialize)]
struct OpenRouterRequest {
    model: String,
    messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_k: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    frequency_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    presence_penalty: Option<f32>,
}

/// Message in OpenRouter response
#[derive(Debug, Clone, Deserialize)]
struct ResponseMessage {
    /// `null` is the documented shape for tool-call and reasoning-only
    /// completions, so it has to deserialize instead of failing the whole
    /// response.
    content: Option<String>,
}

/// Choice in OpenRouter response
#[derive(Debug, Clone, Deserialize)]
struct Choice {
    message: ResponseMessage,
}

/// Response from OpenRouter API
#[derive(Debug, Clone, Deserialize)]
struct OpenRouterResponse {
    choices: Vec<Choice>,
}

/// LLM client for querying the OpenRouter API with automatic model fallback
#[derive(Debug, Clone)]
pub struct LlmClient {
    api_key: String,
    models: Vec<String>,
    system_prompt: String,
    http_client: Client,
    base_url: String,
    fallback_timeout: Duration,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
    top_p: Option<f32>,
    top_k: Option<u32>,
    frequency_penalty: Option<f32>,
    presence_penalty: Option<f32>,
}

impl LlmClient {
    /// Create a new LLM client with multiple models for automatic fallback
    ///
    /// # Arguments
    /// * `api_key` - OpenRouter API key
    /// * `models` - List of model identifiers for automatic fallback
    /// * `system_prompt` - System prompt to guide LLM responses
    /// * `temperature` - Optional temperature for sampling (0.0-2.0)
    /// * `max_tokens` - Optional maximum response length in tokens
    /// * `top_p` - Optional nucleus sampling parameter (0.0-1.0)
    /// * `top_k` - Optional top-k sampling parameter
    /// * `frequency_penalty` - Optional frequency penalty (0.0-2.0)
    /// * `presence_penalty` - Optional presence penalty (0.0-2.0)
    ///
    /// # Returns
    /// * `Result<Self>` - Instance of LlmClient or error
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        api_key: String,
        models: Vec<String>,
        system_prompt: String,
        temperature: Option<f32>,
        max_tokens: Option<u32>,
        top_p: Option<f32>,
        top_k: Option<u32>,
        frequency_penalty: Option<f32>,
        presence_penalty: Option<f32>,
    ) -> Result<Self> {
        if api_key.is_empty() {
            return Err(anyhow!("API key cannot be empty"));
        }

        if models.is_empty() {
            return Err(anyhow!("Models list cannot be empty"));
        }

        let http_client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .context("Failed to build HTTP client")?;

        Ok(Self {
            api_key,
            models,
            system_prompt,
            http_client,
            base_url: "https://openrouter.ai/api/v1/chat/completions".to_string(),
            fallback_timeout: FALLBACK_TIMEOUT,
            temperature,
            max_tokens,
            top_p,
            top_k,
            frequency_penalty,
            presence_penalty,
        })
    }

    /// Set the base URL for testing purposes
    ///
    /// # Arguments
    /// * `url` - The base URL to use for API requests
    pub fn with_base_url(mut self, url: String) -> Self {
        self.base_url = url;
        self
    }

    /// Set the wall-clock budget for one `query()` call
    ///
    /// Bounds the whole model fallback chain, not a single request. Without it
    /// a query can occupy its caller's concurrency permit for
    /// `models.len() * REQUEST_TIMEOUT`.
    ///
    /// # Arguments
    /// * `timeout` - Budget for all attempts combined
    pub fn with_fallback_timeout(mut self, timeout: Duration) -> Self {
        self.fallback_timeout = timeout;
        self
    }

    /// Query the LLM with a prompt using automatic model fallback
    ///
    /// Tries each configured model in order until one succeeds. If a model fails
    /// due to rate limiting, data policy restrictions, or other errors, the next
    /// model in the list is tried automatically.
    ///
    /// The whole chain is bounded by `fallback_timeout`: the caller holds a
    /// concurrency permit until this call returns, so an unbounded chain lets a
    /// few slow upstreams shed all DNS traffic.
    ///
    /// # Arguments
    /// * `prompt` - The user prompt to send to the LLM
    ///
    /// # Returns
    /// * `Result<String>` - The LLM response, or an error whose message carries
    ///   the cause of every failed attempt
    pub async fn query(&self, prompt: &str) -> Result<String> {
        if prompt.is_empty() {
            return Err(anyhow!("Prompt cannot be empty"));
        }

        debug!("Querying LLM with prompt: {}", prompt);
        debug!("Available models for fallback: {:?}", self.models);

        let deadline = Instant::now() + self.fallback_timeout;
        let mut failures: Vec<String> = Vec::with_capacity(self.models.len());
        let mut attempted = 0;

        // Try each model in order
        for (index, model) in self.models.iter().enumerate() {
            debug!(
                "Attempting model {}/{}: {}",
                index + 1,
                self.models.len(),
                model
            );

            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                error!(
                    budget = ?self.fallback_timeout,
                    attempted = attempted,
                    models = self.models.len(),
                    "Fallback budget exhausted before attempting the next model"
                );
                failures.push(format!(
                    "fallback budget of {:?} exhausted before the next model",
                    self.fallback_timeout
                ));
                break;
            }
            attempted += 1;

            match tokio::time::timeout(remaining, self.query_single_model(prompt, model)).await {
                Ok(Ok(response)) => {
                    debug!("Successfully received response from model: {}", model);
                    return Ok(response);
                }
                Ok(Err(e)) => {
                    // `{}` renders only the outermost context, which would make a
                    // connect timeout, a TLS failure and a serde mismatch
                    // indistinguishable here, so log the full cause chain.
                    error!(
                        model = %model,
                        attempt = attempted,
                        models = self.models.len(),
                        error = ?e,
                        "Model failed in fallback chain"
                    );
                    failures.push(format!("{}: {:#}", model, e));

                    // If there are more models to try, continue
                    if index < self.models.len() - 1 {
                        debug!("Trying next model in fallback chain");
                    } else {
                        error!("All models exhausted");
                    }
                }
                Err(_elapsed) => {
                    let e = anyhow!(
                        "no response within {:?} (fallback budget of {:?} exhausted)",
                        remaining,
                        self.fallback_timeout
                    );
                    error!(
                        model = %model,
                        attempt = attempted,
                        models = self.models.len(),
                        error = ?e,
                        "Model attempt exceeded the fallback budget"
                    );
                    failures.push(format!("{}: {:#}", model, e));
                    break;
                }
            }
        }

        // All models failed. The causes are rendered into the message rather
        // than attached as context, because a downstream `{}` log would print
        // the outermost context only and drop every cause again.
        if failures.is_empty() {
            return Err(anyhow!("All models failed without specific error"));
        }

        Err(anyhow!(
            "All models failed ({}/{} attempted, budget {:?}): {}",
            attempted,
            self.models.len(),
            self.fallback_timeout,
            failures.join("; ")
        ))
    }

    /// Query a single specific model
    ///
    /// # Arguments
    /// * `prompt` - The user prompt to send to the LLM
    /// * `model` - The specific model to query
    ///
    /// # Returns
    /// * `Result<String>` - The LLM response or error
    async fn query_single_model(&self, prompt: &str, model: &str) -> Result<String> {
        let request = OpenRouterRequest {
            model: model.to_string(),
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: self.system_prompt.clone(),
                },
                Message {
                    role: "user".to_string(),
                    content: prompt.to_string(),
                },
            ],
            temperature: self.temperature,
            max_tokens: self.max_tokens,
            top_p: self.top_p,
            top_k: self.top_k,
            frequency_penalty: self.frequency_penalty,
            presence_penalty: self.presence_penalty,
        };

        // App-attribution headers so the request is identified in the provider's
        // dashboard instead of falling back to a raw User-Agent fingerprint.
        // `HTTP-Referer`/`X-Title` are the OpenRouter convention; the
        // `X-AnyRouter-*` triplet is read by AnyRouter. Both providers ignore
        // headers they don't recognize, so we send all of them unconditionally.
        let response = self
            .http_client
            .post(&self.base_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", env!("CARGO_PKG_REPOSITORY"))
            .header("X-Title", "LLM over DNS")
            .header("X-AnyRouter-Title", "LLM over DNS")
            .header("X-AnyRouter-Source", env!("CARGO_PKG_NAME"))
            .header("X-AnyRouter-Version", env!("CARGO_PKG_VERSION"))
            .json(&request)
            .send()
            .await
            .context("Failed to send request to OpenRouter API")?;

        let status = response.status();
        debug!("OpenRouter API response status for {}: {}", model, status);

        match status {
            reqwest::StatusCode::OK => {
                let body = response
                    .json::<OpenRouterResponse>()
                    .await
                    .context("Failed to parse OpenRouter API response")?;

                if body.choices.is_empty() {
                    return Err(anyhow!("No choices in API response"));
                }

                // A `200` with no text is a failed completion, not a successful
                // one: providers return blank content for refusals and
                // content-filtered prompts. Treating it as success would answer
                // the DNS query with NOERROR and no records - indistinguishable
                // from "no such record" - and cache that empty answer for its
                // full TTL, so every retry for minutes would skip the LLM call.
                let content = body.choices[0]
                    .message
                    .content
                    .as_deref()
                    .unwrap_or_default();
                if content.trim().is_empty() {
                    return Err(anyhow!(
                        "Empty content in API response (provider refused, content-filtered, \
                         or returned a tool call without text)"
                    ));
                }

                Ok(content.to_string())
            }
            reqwest::StatusCode::TOO_MANY_REQUESTS => Err(anyhow!("Rate limit exceeded (429)")),
            reqwest::StatusCode::NOT_FOUND => {
                Err(anyhow!("Model not found or data policy restriction (404)"))
            }
            reqwest::StatusCode::INTERNAL_SERVER_ERROR => {
                Err(anyhow!("OpenRouter API server error (500)"))
            }
            reqwest::StatusCode::UNAUTHORIZED => {
                Err(anyhow!("Unauthorized: Invalid API key (401)"))
            }
            reqwest::StatusCode::BAD_REQUEST => {
                // A failed body read keeps its cause instead of collapsing to
                // an empty string after the colon.
                let body = read_error_body(response)
                    .await
                    .with_context(|| format!("Failed to read error body for {}", status))?;
                Err(anyhow!("Bad request (400): {}", body))
            }
            _ => {
                let body = read_error_body(response)
                    .await
                    .with_context(|| format!("Failed to read error body for {}", status))?;
                Err(anyhow!("Unexpected status code {}: {}", status, body))
            }
        }
    }
}

/// Read at most [`MAX_ERROR_BODY_BYTES`] of an error response body.
///
/// Reads one chunk past the cap so truncation can be reported, instead of
/// buffering whatever the provider decided to send.
async fn read_error_body(mut response: reqwest::Response) -> Result<String> {
    let mut captured: Vec<u8> = Vec::with_capacity(MAX_ERROR_BODY_BYTES + 1);

    while captured.len() <= MAX_ERROR_BODY_BYTES {
        match response
            .chunk()
            .await
            .context("Failed to read error response body")?
        {
            Some(chunk) => captured.extend_from_slice(&chunk),
            None => break,
        }
    }

    if captured.len() > MAX_ERROR_BODY_BYTES {
        captured.truncate(MAX_ERROR_BODY_BYTES);
        return Ok(format!(
            "{}... (truncated at {} bytes)",
            String::from_utf8_lossy(&captured).trim_end(),
            MAX_ERROR_BODY_BYTES
        ));
    }

    Ok(String::from_utf8_lossy(&captured).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_client_creation_success() {
        let result = LlmClient::new(
            "test_api_key".to_string(),
            vec!["test_model".to_string()],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert!(result.is_ok());
        let client = result.unwrap();
        assert_eq!(client.api_key, "test_api_key");
        assert_eq!(client.models, vec!["test_model".to_string()]);
        assert_eq!(client.system_prompt, "Test system prompt");
    }

    #[test]
    fn test_llm_client_creation_empty_api_key() {
        let result = LlmClient::new(
            String::new(),
            vec!["test_model".to_string()],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("API key cannot be empty"));
    }

    #[test]
    fn test_llm_client_creation_empty_models() {
        let result = LlmClient::new(
            "test_api_key".to_string(),
            vec![],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Models list cannot be empty"));
    }

    #[test]
    fn test_llm_client_creation_multiple_models() {
        let models = vec![
            "model1".to_string(),
            "model2".to_string(),
            "model3".to_string(),
        ];
        let result = LlmClient::new(
            "test_api_key".to_string(),
            models.clone(),
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert!(result.is_ok());
        let client = result.unwrap();
        assert_eq!(client.models, models);
    }

    #[tokio::test]
    async fn test_successful_api_call() {
        let mut server = mockito::Server::new_async().await;

        let mock_response = r#"{
            "choices": [{
                "message": {
                    "content": "This is a test response"
                }
            }]
        }"#;

        let _mock = server
            .mock("POST", mockito::Matcher::Regex(r"^/.*".to_string()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_response)
            .create_async()
            .await;

        let client = LlmClient::new(
            "test_key".to_string(),
            vec!["test_model".to_string()],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("Failed to create client")
        .with_base_url(server.url());

        let result = client.query("Test prompt").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "This is a test response");
    }

    #[tokio::test]
    async fn test_response_parsing() {
        let mut server = mockito::Server::new_async().await;

        let mock_response = r#"{
            "choices": [{
                "message": {
                    "content": "Multi-line\nresponse\nfrom\nLLM"
                }
            }]
        }"#;

        let _mock = server
            .mock("POST", mockito::Matcher::Regex(r"^/.*".to_string()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_response)
            .create_async()
            .await;

        let client = LlmClient::new(
            "test_key".to_string(),
            vec!["test_model".to_string()],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("Failed to create client")
        .with_base_url(server.url());

        let result = client.query("Test prompt").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Multi-line\nresponse\nfrom\nLLM");
    }

    #[tokio::test]
    async fn test_timeout_handling() {
        use std::time::{Duration, Instant};

        // A server that accepts the connection and never answers. The
        // per-request timeout is 30s, so only the fallback budget can end this
        // call in a test-friendly amount of time.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("Failed to bind listener");
        let addr = listener.local_addr().expect("Failed to read local address");
        tokio::spawn(async move {
            // Hold every accepted socket open: dropping one would close the
            // connection and surface as an I/O error instead of a timeout.
            let mut held = Vec::new();
            while let Ok((socket, _)) = listener.accept().await {
                held.push(socket);
            }
        });

        let client = LlmClient::new(
            "test_key".to_string(),
            vec!["test_model".to_string()],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("Failed to create client")
        .with_base_url(format!("http://{}", addr))
        .with_fallback_timeout(Duration::from_millis(200));

        let started = Instant::now();
        let result = client.query("Test prompt").await;
        let elapsed = started.elapsed();

        let error = result.expect_err("Expected the query to time out");
        let rendered = error.to_string();
        assert!(
            rendered.contains("fallback budget"),
            "Timeout error should name the exhausted budget, got: {}",
            rendered
        );
        assert!(
            elapsed < Duration::from_secs(5),
            "Query was not bounded by the fallback budget, took {:?}",
            elapsed
        );
    }

    #[tokio::test]
    async fn test_rate_limit_429() {
        let mut server = mockito::Server::new_async().await;

        let _mock = server
            .mock("POST", mockito::Matcher::Regex(r"^/.*".to_string()))
            .with_status(429)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error": "rate_limit_exceeded"}"#)
            .create_async()
            .await;

        let client = LlmClient::new(
            "test_key".to_string(),
            vec!["test_model".to_string()],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("Failed to create client")
        .with_base_url(server.url());

        let result = client.query("Test prompt").await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Rate limit exceeded"));
    }

    #[tokio::test]
    async fn test_server_error_500() {
        let mut server = mockito::Server::new_async().await;

        let _mock = server
            .mock("POST", mockito::Matcher::Regex(r"^/.*".to_string()))
            .with_status(500)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error": "internal_server_error"}"#)
            .create_async()
            .await;

        let client = LlmClient::new(
            "test_key".to_string(),
            vec!["test_model".to_string()],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("Failed to create client")
        .with_base_url(server.url());

        let result = client.query("Test prompt").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("server error"));
    }

    #[tokio::test]
    async fn test_invalid_json_response() {
        let mut server = mockito::Server::new_async().await;

        let _mock = server
            .mock("POST", mockito::Matcher::Regex(r"^/.*".to_string()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"invalid": "json structure"}"#)
            .create_async()
            .await;

        let client = LlmClient::new(
            "test_key".to_string(),
            vec!["test_model".to_string()],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("Failed to create client")
        .with_base_url(server.url());

        let result = client.query("Test prompt").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_network_error() {
        // Bind a port and release it again: the address is routable, so the
        // failure comes from connect() rather than from URL parsing.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("Failed to bind listener");
        let addr = listener.local_addr().expect("Failed to read local address");
        drop(listener);

        let client = LlmClient::new(
            "test_key".to_string(),
            vec!["test_model".to_string()],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("Failed to create client")
        .with_base_url(format!("http://{}", addr));

        let error = client
            .query("Test prompt")
            .await
            .expect_err("Expected a network error");
        // `{}` is how the DNS layer logs this error, so the cause has to be
        // part of the message rather than only part of the context chain.
        let rendered = error.to_string();
        assert!(
            rendered.contains("Failed to send request to OpenRouter API"),
            "Missing request context: {}",
            rendered
        );
        assert!(
            rendered.contains("error sending request"),
            "Transport cause was dropped from the error: {}",
            rendered
        );
    }

    #[tokio::test]
    async fn test_auth_header_format() {
        let mut server = mockito::Server::new_async().await;

        let mock_response = r#"{"choices": [{"message": {"content": "test"}}]}"#;

        let _mock = server
            .mock("POST", mockito::Matcher::Regex(r"^/.*".to_string()))
            .match_header(
                "Authorization",
                mockito::Matcher::Regex(r"^Bearer .+$".to_string()),
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_response)
            .create_async()
            .await;

        let client = LlmClient::new(
            "test_api_key_123".to_string(),
            vec!["test_model".to_string()],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("Failed to create client")
        .with_base_url(server.url());

        let result = client.query("Test prompt").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_empty_prompt() {
        let client = LlmClient::new(
            "test_key".to_string(),
            vec!["test_model".to_string()],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("Failed to create client");

        let result = client.query("").await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Prompt cannot be empty"));
    }

    #[tokio::test]
    async fn test_unauthorized_401() {
        let mut server = mockito::Server::new_async().await;

        let _mock = server
            .mock("POST", mockito::Matcher::Regex(r"^/.*".to_string()))
            .with_status(401)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error": "unauthorized"}"#)
            .create_async()
            .await;

        let client = LlmClient::new(
            "invalid_key".to_string(),
            vec!["test_model".to_string()],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("Failed to create client")
        .with_base_url(server.url());

        let result = client.query("Test prompt").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unauthorized"));
    }

    #[tokio::test]
    async fn test_bad_request_400() {
        let mut server = mockito::Server::new_async().await;

        let _mock = server
            .mock("POST", mockito::Matcher::Regex(r"^/.*".to_string()))
            .with_status(400)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error": "invalid_request"}"#)
            .create_async()
            .await;

        let client = LlmClient::new(
            "test_key".to_string(),
            vec!["test_model".to_string()],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("Failed to create client")
        .with_base_url(server.url());

        let result = client.query("Test prompt").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("400"));
    }

    #[tokio::test]
    async fn test_error_body_is_capped() {
        let mut server = mockito::Server::new_async().await;

        // A 4xx body larger than the capture cap: the tail must not be copied
        // into the error message.
        let huge_body = format!("START{}TAIL", "b".repeat(5000));

        let _mock = server
            .mock("POST", mockito::Matcher::Regex(r"^/.*".to_string()))
            .with_status(400)
            .with_body(huge_body)
            .create_async()
            .await;

        let client = LlmClient::new(
            "test_key".to_string(),
            vec!["test_model".to_string()],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("Failed to create client")
        .with_base_url(server.url());

        let error = client
            .query("Test prompt")
            .await
            .expect_err("Expected a bad request error");
        let rendered = error.to_string();

        assert!(
            rendered.contains("START"),
            "Start of the error body should be captured: {}",
            rendered
        );
        assert!(
            !rendered.contains("TAIL"),
            "Oversized error body was not capped: {} bytes in the message",
            rendered.len()
        );
        assert!(
            rendered.contains("truncated"),
            "Truncation should be visible: {}",
            rendered
        );
        assert!(
            rendered.len() < 1024,
            "Error message is {} bytes, expected a capped message",
            rendered.len()
        );
    }

    #[tokio::test]
    async fn test_error_body_read_failure_keeps_cause() {
        use tokio::io::AsyncWriteExt;

        // Announce more body than is sent, then close: reading the body fails
        // and the cause must survive into the error message.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("Failed to bind listener");
        let addr = listener.local_addr().expect("Failed to read local address");
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let _ = socket
                    .write_all(b"HTTP/1.1 400 Bad Request\r\ncontent-length: 100\r\n\r\nshort")
                    .await;
            }
        });

        let client = LlmClient::new(
            "test_key".to_string(),
            vec!["test_model".to_string()],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("Failed to create client")
        .with_base_url(format!("http://{}", addr));

        let error = client
            .query("Test prompt")
            .await
            .expect_err("Expected a bad request error");
        let rendered = error.to_string();

        assert!(
            rendered.contains("400"),
            "Status should be reported: {}",
            rendered
        );
        assert!(
            rendered.contains("Failed to read error body"),
            "Body read failure should be reported: {}",
            rendered
        );
        assert!(
            !rendered.ends_with(": "),
            "Read failure must not leave an empty body: {}",
            rendered
        );
    }

    #[tokio::test]
    async fn test_empty_choices_response() {
        let mut server = mockito::Server::new_async().await;

        let _mock = server
            .mock("POST", mockito::Matcher::Regex(r"^/.*".to_string()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"choices": []}"#)
            .create_async()
            .await;

        let client = LlmClient::new(
            "test_key".to_string(),
            vec!["test_model".to_string()],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("Failed to create client")
        .with_base_url(server.url());

        let result = client.query("Test prompt").await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No choices in API response"));
    }

    #[tokio::test]
    async fn test_empty_content_is_a_model_failure() {
        let mut server = mockito::Server::new_async().await;

        let _mock = server
            .mock("POST", mockito::Matcher::Regex(r"^/.*".to_string()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"choices": [{"message": {"content": ""}}]}"#)
            .create_async()
            .await;

        let client = LlmClient::new(
            "test_key".to_string(),
            vec!["test_model".to_string()],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("Failed to create client")
        .with_base_url(server.url());

        // A 200 with no text is a refused or filtered completion, not a
        // successful one: answering with it would produce NOERROR with zero TXT
        // records and cache that empty answer for the full TTL.
        let error = client
            .query("Test prompt")
            .await
            .expect_err("Empty content must not be reported as success");
        assert!(error.to_string().contains("Empty content in API response"));
    }

    #[tokio::test]
    async fn test_whitespace_only_content_is_a_model_failure() {
        let mut server = mockito::Server::new_async().await;

        let _mock = server
            .mock("POST", mockito::Matcher::Regex(r"^/.*".to_string()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"choices": [{"message": {"content": "  \n\t "}}]}"#)
            .create_async()
            .await;

        let client = LlmClient::new(
            "test_key".to_string(),
            vec!["test_model".to_string()],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("Failed to create client")
        .with_base_url(server.url());

        let error = client
            .query("Test prompt")
            .await
            .expect_err("Whitespace-only content must not be reported as success");
        assert!(error.to_string().contains("Empty content in API response"));
    }

    #[tokio::test]
    async fn test_null_content_is_a_model_failure() {
        let mut server = mockito::Server::new_async().await;

        // `null` content is the standard shape for tool-call completions and for
        // reasoning-only responses; it must deserialize, and then be reported as
        // an empty completion rather than as a parse failure.
        let _mock = server
            .mock("POST", mockito::Matcher::Regex(r"^/.*".to_string()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"choices": [{"message": {"content": null}}]}"#)
            .create_async()
            .await;

        let client = LlmClient::new(
            "test_key".to_string(),
            vec!["test_model".to_string()],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("Failed to create client")
        .with_base_url(server.url());

        let error = client
            .query("Test prompt")
            .await
            .expect_err("Null content must not be reported as success");
        let rendered = error.to_string();
        assert!(
            rendered.contains("Empty content in API response"),
            "Unexpected error: {}",
            rendered
        );
        assert!(
            !rendered.contains("Failed to parse"),
            "Null content must not look like a deserialization failure: {}",
            rendered
        );
    }

    #[tokio::test]
    async fn test_fallback_continues_past_empty_content() {
        let mut server = mockito::Server::new_async().await;

        // First model answers 200 with no text
        let _mock1 = server
            .mock("POST", mockito::Matcher::Regex(r"^/.*".to_string()))
            .match_body(mockito::Matcher::Json(serde_json::json!({
                "model": "model1",
                "messages": [
                    {"role": "system", "content": "Test system prompt"},
                    {"role": "user", "content": "Test prompt"}
                ]
            })))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"choices": [{"message": {"content": ""}}]}"#)
            .create_async()
            .await;

        // Second model succeeds
        let _mock2 = server
            .mock("POST", mockito::Matcher::Regex(r"^/.*".to_string()))
            .match_body(mockito::Matcher::Json(serde_json::json!({
                "model": "model2",
                "messages": [
                    {"role": "system", "content": "Test system prompt"},
                    {"role": "user", "content": "Test prompt"}
                ]
            })))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"choices": [{"message": {"content": "Success from model2"}}]}"#)
            .create_async()
            .await;

        let client = LlmClient::new(
            "test_key".to_string(),
            vec!["model1".to_string(), "model2".to_string()],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("Failed to create client")
        .with_base_url(server.url());

        let result = client.query("Test prompt").await;
        assert!(
            result.is_ok(),
            "Empty content should fall through: {:?}",
            result
        );
        assert_eq!(result.unwrap(), "Success from model2");
    }

    #[test]
    fn test_with_base_url() {
        let client = LlmClient::new(
            "test_key".to_string(),
            vec!["test_model".to_string()],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("Failed to create client")
        .with_base_url("http://custom.url".to_string());

        assert_eq!(client.base_url, "http://custom.url");
    }

    #[test]
    fn test_with_fallback_timeout() {
        let client = LlmClient::new(
            "test_key".to_string(),
            vec!["test_model".to_string()],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("Failed to create client")
        .with_fallback_timeout(Duration::from_millis(250));

        assert_eq!(client.fallback_timeout, Duration::from_millis(250));
    }

    #[test]
    fn test_default_fallback_timeout_covers_the_whole_chain() {
        let client = LlmClient::new(
            "test_key".to_string(),
            vec![
                "model1".to_string(),
                "model2".to_string(),
                "model3".to_string(),
            ],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("Failed to create client");

        // The default has to be longer than a single request timeout, otherwise
        // the first slow model would consume the whole budget and no fallback
        // would ever be attempted.
        assert!(
            client.fallback_timeout > REQUEST_TIMEOUT,
            "Fallback budget {:?} should exceed the per-request timeout {:?}",
            client.fallback_timeout,
            REQUEST_TIMEOUT
        );
    }

    #[tokio::test]
    async fn test_unexpected_status_code() {
        let mut server = mockito::Server::new_async().await;

        let _mock = server
            .mock("POST", mockito::Matcher::Regex(r"^/.*".to_string()))
            .with_status(503)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error": "service_unavailable"}"#)
            .create_async()
            .await;

        let client = LlmClient::new(
            "test_key".to_string(),
            vec!["test_model".to_string()],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("Failed to create client")
        .with_base_url(server.url());

        let result = client.query("Test prompt").await;
        assert!(result.is_err());
        let error = result.unwrap_err().to_string();
        assert!(error.contains("503"));
        assert!(error.contains("service_unavailable"));
    }

    #[tokio::test]
    async fn test_fallback_to_second_model() {
        let mut server = mockito::Server::new_async().await;

        // First model returns 429 (rate limit)
        let _mock1 = server
            .mock("POST", mockito::Matcher::Regex(r"^/.*".to_string()))
            .match_body(mockito::Matcher::Json(serde_json::json!({
                "model": "model1",
                "messages": [
                    {"role": "system", "content": "Test system prompt"},
                    {"role": "user", "content": "Test prompt"}
                ]
            })))
            .with_status(429)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error": "rate_limit_exceeded"}"#)
            .create_async()
            .await;

        // Second model succeeds
        let _mock2 = server
            .mock("POST", mockito::Matcher::Regex(r"^/.*".to_string()))
            .match_body(mockito::Matcher::Json(serde_json::json!({
                "model": "model2",
                "messages": [
                    {"role": "system", "content": "Test system prompt"},
                    {"role": "user", "content": "Test prompt"}
                ]
            })))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"choices": [{"message": {"content": "Success from model2"}}]}"#)
            .create_async()
            .await;

        let client = LlmClient::new(
            "test_key".to_string(),
            vec!["model1".to_string(), "model2".to_string()],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("Failed to create client")
        .with_base_url(server.url());

        let result = client.query("Test prompt").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Success from model2");
    }

    #[tokio::test]
    async fn test_fallback_to_third_model() {
        let mut server = mockito::Server::new_async().await;

        // First model returns 404 (not found)
        let _mock1 = server
            .mock("POST", mockito::Matcher::Regex(r"^/.*".to_string()))
            .match_body(mockito::Matcher::Json(serde_json::json!({
                "model": "model1",
                "messages": [
                    {"role": "system", "content": "Test system prompt"},
                    {"role": "user", "content": "Test prompt"}
                ]
            })))
            .with_status(404)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error": "not_found"}"#)
            .create_async()
            .await;

        // Second model returns 500 (server error)
        let _mock2 = server
            .mock("POST", mockito::Matcher::Regex(r"^/.*".to_string()))
            .match_body(mockito::Matcher::Json(serde_json::json!({
                "model": "model2",
                "messages": [
                    {"role": "system", "content": "Test system prompt"},
                    {"role": "user", "content": "Test prompt"}
                ]
            })))
            .with_status(500)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error": "internal_error"}"#)
            .create_async()
            .await;

        // Third model succeeds
        let _mock3 = server
            .mock("POST", mockito::Matcher::Regex(r"^/.*".to_string()))
            .match_body(mockito::Matcher::Json(serde_json::json!({
                "model": "model3",
                "messages": [
                    {"role": "system", "content": "Test system prompt"},
                    {"role": "user", "content": "Test prompt"}
                ]
            })))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"choices": [{"message": {"content": "Success from model3"}}]}"#)
            .create_async()
            .await;

        let client = LlmClient::new(
            "test_key".to_string(),
            vec![
                "model1".to_string(),
                "model2".to_string(),
                "model3".to_string(),
            ],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("Failed to create client")
        .with_base_url(server.url());

        let result = client.query("Test prompt").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Success from model3");
    }

    #[tokio::test]
    async fn test_all_models_fail() {
        let mut server = mockito::Server::new_async().await;

        // All models fail
        let _mock = server
            .mock("POST", mockito::Matcher::Regex(r"^/.*".to_string()))
            .with_status(429)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error": "rate_limit_exceeded"}"#)
            .expect_at_least(2)
            .create_async()
            .await;

        let client = LlmClient::new(
            "test_key".to_string(),
            vec!["model1".to_string(), "model2".to_string()],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("Failed to create client")
        .with_base_url(server.url());

        let result = client.query("Test prompt").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Rate limit"));
    }

    #[tokio::test]
    async fn test_aggregate_error_carries_every_cause() {
        let mut server = mockito::Server::new_async().await;

        // First model returns 429 (rate limit)
        let _mock1 = server
            .mock("POST", mockito::Matcher::Regex(r"^/.*".to_string()))
            .match_body(mockito::Matcher::Json(serde_json::json!({
                "model": "model1",
                "messages": [
                    {"role": "system", "content": "Test system prompt"},
                    {"role": "user", "content": "Test prompt"}
                ]
            })))
            .with_status(429)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error": "rate_limit_exceeded"}"#)
            .create_async()
            .await;

        // Second model returns 500 (server error)
        let _mock2 = server
            .mock("POST", mockito::Matcher::Regex(r"^/.*".to_string()))
            .match_body(mockito::Matcher::Json(serde_json::json!({
                "model": "model2",
                "messages": [
                    {"role": "system", "content": "Test system prompt"},
                    {"role": "user", "content": "Test prompt"}
                ]
            })))
            .with_status(500)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error": "internal_error"}"#)
            .create_async()
            .await;

        let client = LlmClient::new(
            "test_key".to_string(),
            vec!["model1".to_string(), "model2".to_string()],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("Failed to create client")
        .with_base_url(server.url());

        // The DNS layer logs this error with `{}`, which prints the outermost
        // context only, so every cause has to live in the message itself.
        let rendered = client
            .query("Test prompt")
            .await
            .expect_err("Both models should fail")
            .to_string();

        assert!(
            rendered.contains("model1"),
            "Missing first model: {}",
            rendered
        );
        assert!(
            rendered.contains("model2"),
            "Missing second model: {}",
            rendered
        );
        assert!(
            rendered.contains("Rate limit exceeded"),
            "Missing first cause: {}",
            rendered
        );
        assert!(
            rendered.contains("server error"),
            "Missing second cause: {}",
            rendered
        );
    }

    #[tokio::test]
    async fn test_error_cause_reaches_the_caller() {
        let mut server = mockito::Server::new_async().await;

        let _mock = server
            .mock("POST", mockito::Matcher::Regex(r"^/.*".to_string()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("not json at all")
            .create_async()
            .await;

        let client = LlmClient::new(
            "test_key".to_string(),
            vec!["test_model".to_string()],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("Failed to create client")
        .with_base_url(server.url());

        let rendered = client
            .query("Test prompt")
            .await
            .expect_err("Malformed body should fail")
            .to_string();

        assert!(
            rendered.contains("Failed to parse OpenRouter API response"),
            "Missing context: {}",
            rendered
        );
        assert!(
            rendered.contains("error decoding response body"),
            "Deserialization cause was dropped: {}",
            rendered
        );
    }

    #[tokio::test]
    async fn test_fallback_budget_bounds_the_chain() {
        use std::time::{Duration, Instant};

        // Three models, none of which ever answers: without a chain-wide budget
        // this call would run for `models.len() * 30s`.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("Failed to bind listener");
        let addr = listener.local_addr().expect("Failed to read local address");
        tokio::spawn(async move {
            let mut held = Vec::new();
            while let Ok((socket, _)) = listener.accept().await {
                held.push(socket);
            }
        });

        let client = LlmClient::new(
            "test_key".to_string(),
            vec![
                "model1".to_string(),
                "model2".to_string(),
                "model3".to_string(),
            ],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("Failed to create client")
        .with_base_url(format!("http://{}", addr))
        .with_fallback_timeout(Duration::from_millis(300));

        let started = Instant::now();
        let rendered = client
            .query("Test prompt")
            .await
            .expect_err("Expected the fallback budget to be exhausted")
            .to_string();
        let elapsed = started.elapsed();

        assert!(
            rendered.contains("model1"),
            "The in-flight model should be reported: {}",
            rendered
        );
        assert!(
            !rendered.contains("model3"),
            "The chain was not bounded, later models were attempted: {}",
            rendered
        );
        assert!(
            elapsed < Duration::from_secs(5),
            "Query was not bounded by the fallback budget, took {:?}",
            elapsed
        );
    }

    #[tokio::test]
    async fn test_exhausted_budget_skips_every_request() {
        use std::time::Duration;

        let mut server = mockito::Server::new_async().await;

        let mock = server
            .mock("POST", mockito::Matcher::Regex(r"^/.*".to_string()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"choices": [{"message": {"content": "unused"}}]}"#)
            .expect(0)
            .create_async()
            .await;

        let client = LlmClient::new(
            "test_key".to_string(),
            vec!["test_model".to_string()],
            "Test system prompt".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("Failed to create client")
        .with_base_url(server.url())
        .with_fallback_timeout(Duration::ZERO);

        let error = client
            .query("Test prompt")
            .await
            .expect_err("Expected an exhausted budget error");
        let rendered = error.to_string();
        assert!(
            rendered.contains("fallback budget of") && rendered.contains("before the next model"),
            "Unexpected error: {}",
            rendered
        );
        assert!(
            rendered.contains("0/1 attempted"),
            "Unexpected attempt count: {}",
            rendered
        );

        // No request may be sent once the budget is gone.
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_parameters_included_in_request() {
        let mut server = mockito::Server::new_async().await;

        let mock_response = r#"{"choices": [{"message": {"content": "test"}}]}"#;

        // Verify all parameters are included in the request body
        let _mock = server
            .mock("POST", mockito::Matcher::Regex(r"^/.*".to_string()))
            .match_body(mockito::Matcher::Json(serde_json::json!({
                "model": "test_model",
                "messages": [
                    {"role": "system", "content": "Test system prompt"},
                    {"role": "user", "content": "Test prompt"}
                ],
                "temperature": 0.7,
                "max_tokens": 500,
                "top_p": 0.9,
                "top_k": 40,
                "frequency_penalty": 0.5,
                "presence_penalty": 0.5
            })))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_response)
            .create_async()
            .await;

        let client = LlmClient::new(
            "test_key".to_string(),
            vec!["test_model".to_string()],
            "Test system prompt".to_string(),
            Some(0.7),
            Some(500),
            Some(0.9),
            Some(40),
            Some(0.5),
            Some(0.5),
        )
        .expect("Failed to create client")
        .with_base_url(server.url());

        let result = client.query("Test prompt").await;
        assert!(result.is_ok());
    }

    /// Live smoke test against AnyRouter.
    ///
    /// Ignored by default: it needs a real `ANYROUTER_API_KEY` and makes up to
    /// three billable calls to a third party. Opt in explicitly with
    /// `cargo test --lib -- --ignored`.
    #[tokio::test]
    #[ignore = "live smoke test: needs ANYROUTER_API_KEY and makes billable API calls"]
    async fn test_anyrouter_smoke() {
        let _ = tracing_subscriber::fmt()
            .with_env_filter("debug")
            .try_init();
        use std::env;
        let api_key = match env::var("ANYROUTER_API_KEY") {
            Ok(key) => key,
            Err(_) => {
                println!("Skipping AnyRouter smoke test (ANYROUTER_API_KEY not set)");
                return;
            }
        };

        // Ensure key starts with sk-ar-. The key is never interpolated into the
        // message: CI logs are world-readable, so a failing assertion must not
        // publish a live credential.
        assert!(
            api_key.starts_with("sk-ar-"),
            "ANYROUTER_API_KEY must start with 'sk-ar-'"
        );

        let models = vec![
            "google/gemini-2.5-flash-lite".to_string(),
            "openai/gpt-4o-mini".to_string(),
            "meta/llama-3.2-3b-instruct".to_string(),
        ];
        let system_prompt = "You are a helpful assistant.".to_string();

        let client = LlmClient::new(
            api_key,
            models,
            system_prompt,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("Failed to create client")
        .with_base_url("https://anyrouter.dev/api/v1/chat/completions".to_string());

        println!("Sending smoke test query to AnyRouter...");
        match client
            .query("Hello! Return exactly the word 'SUCCESS'")
            .await
        {
            Ok(reply) => {
                println!("Received AnyRouter reply: {}", reply);
                assert!(!reply.is_empty(), "Reply cannot be empty");
            }
            Err(e) => {
                panic!("AnyRouter query failed: {:#}", e);
            }
        }
    }
}
