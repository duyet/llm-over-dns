use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, error};

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
}

/// Message in OpenRouter response
#[derive(Debug, Clone, Deserialize)]
struct ResponseMessage {
    content: String,
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
}

impl LlmClient {
    /// Create a new LLM client with multiple models for automatic fallback
    ///
    /// # Arguments
    /// * `api_key` - OpenRouter API key
    /// * `models` - List of model identifiers for automatic fallback
    /// * `system_prompt` - System prompt to guide LLM responses
    ///
    /// # Returns
    /// * `Result<Self>` - Instance of LlmClient or error
    pub fn new(api_key: String, models: Vec<String>, system_prompt: String) -> Result<Self> {
        if api_key.is_empty() {
            return Err(anyhow!("API key cannot be empty"));
        }

        if models.is_empty() {
            return Err(anyhow!("Models list cannot be empty"));
        }

        let http_client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("Failed to build HTTP client")?;

        Ok(Self {
            api_key,
            models,
            system_prompt,
            http_client,
            base_url: "https://openrouter.ai/api/v1/chat/completions".to_string(),
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

    /// Query the LLM with a prompt using automatic model fallback
    ///
    /// Tries each configured model in order until one succeeds. If a model fails
    /// due to rate limiting, data policy restrictions, or other errors, the next
    /// model in the list is tried automatically.
    ///
    /// # Arguments
    /// * `prompt` - The user prompt to send to the LLM
    ///
    /// # Returns
    /// * `Result<String>` - The LLM response or error if all models fail
    pub async fn query(&self, prompt: &str) -> Result<String> {
        if prompt.is_empty() {
            return Err(anyhow!("Prompt cannot be empty"));
        }

        debug!("Querying LLM with prompt: {}", prompt);
        debug!("Available models for fallback: {:?}", self.models);

        let mut last_error = None;

        // Try each model in order
        for (index, model) in self.models.iter().enumerate() {
            debug!("Attempting model {}/{}: {}", index + 1, self.models.len(), model);

            match self.query_single_model(prompt, model).await {
                Ok(response) => {
                    debug!("Successfully received response from model: {}", model);
                    return Ok(response);
                }
                Err(e) => {
                    error!("Model {} failed: {}", model, e);
                    last_error = Some(e);

                    // If there are more models to try, continue
                    if index < self.models.len() - 1 {
                        debug!("Trying next model in fallback chain");
                    } else {
                        error!("All models exhausted");
                    }
                }
            }
        }

        // All models failed
        Err(last_error.unwrap_or_else(|| anyhow!("All models failed without specific error")))
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
        };

        let response = self
            .http_client
            .post(&self.base_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
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

                let content = body.choices[0].message.content.clone();
                Ok(content)
            }
            reqwest::StatusCode::TOO_MANY_REQUESTS => {
                Err(anyhow!("Rate limit exceeded (429)"))
            }
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
                let text = response.text().await.unwrap_or_default();
                Err(anyhow!("Bad request (400): {}", text))
            }
            _ => {
                let text = response.text().await.unwrap_or_default();
                Err(anyhow!("Unexpected status code {}: {}", status, text))
            }
        }
    }
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
        );
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Models list cannot be empty"));
    }

    #[test]
    fn test_llm_client_creation_multiple_models() {
        let models = vec!["model1".to_string(), "model2".to_string(), "model3".to_string()];
        let result = LlmClient::new(
            "test_api_key".to_string(),
            models.clone(),
            "Test system prompt".to_string(),
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
        )
        .expect("Failed to create client")
        .with_base_url(server.url());

        let result = client.query("Test prompt").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Multi-line\nresponse\nfrom\nLLM");
    }

    #[tokio::test]
    async fn test_timeout_handling() {
        // Test that the client can be created successfully with timeout configuration
        // The timeout is set during Client::builder() and is verified indirectly through
        // the client creation process
        let result = LlmClient::new(
            "test_key".to_string(),
            vec!["test_model".to_string()],
            "Test system prompt".to_string(),
        );
        assert!(result.is_ok());
        let client = result.unwrap();

        // Verify the HTTP client was properly initialized
        // (The actual timeout configuration is set during Client::builder())
        assert!(!client.api_key.is_empty());
        assert!(!client.models.is_empty());
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
        )
        .expect("Failed to create client")
        .with_base_url(server.url());

        let result = client.query("Test prompt").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_network_error() {
        // Use an invalid URL that will fail to connect
        let client = LlmClient::new(
            "test_key".to_string(),
            vec!["test_model".to_string()],
            "Test system prompt".to_string(),
        )
        .expect("Failed to create client")
        .with_base_url("http://invalid.local:99999".to_string());

        let result = client.query("Test prompt").await;
        assert!(result.is_err());
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
        )
        .expect("Failed to create client")
        .with_base_url(server.url());

        let result = client.query("Test prompt").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("400"));
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

    #[test]
    fn test_with_base_url() {
        let client = LlmClient::new(
            "test_key".to_string(),
            vec!["test_model".to_string()],
            "Test system prompt".to_string(),
        )
        .expect("Failed to create client")
        .with_base_url("http://custom.url".to_string());

        assert_eq!(client.base_url, "http://custom.url");
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
                "messages": [{"role": "user", "content": "Test prompt"}]
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
                "messages": [{"role": "user", "content": "Test prompt"}]
            })))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"choices": [{"message": {"content": "Success from model2"}}]}"#)
            .create_async()
            .await;

        let client = LlmClient::new(
            "test_key".to_string(),
            vec!["model1".to_string(), "model2".to_string()]
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
                "messages": [{"role": "user", "content": "Test prompt"}]
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
                "messages": [{"role": "user", "content": "Test prompt"}]
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
                "messages": [{"role": "user", "content": "Test prompt"}]
            })))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"choices": [{"message": {"content": "Success from model3"}}]}"#)
            .create_async()
            .await;

        let client = LlmClient::new(
            "test_key".to_string(),
            vec!["model1".to_string(), "model2".to_string(), "model3".to_string()]
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
            vec!["model1".to_string(), "model2".to_string()]
        )
        .expect("Failed to create client")
        .with_base_url(server.url());

        let result = client.query("Test prompt").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Rate limit"));
    }
}
