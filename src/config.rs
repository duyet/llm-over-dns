//! Configuration management for LLM over DNS server.
//!
//! This module handles loading and validating configuration from environment variables.
//! Configuration includes API credentials, model selection, and DNS server settings.
//!
//! # Environment Variables
//!
//! - `OPENROUTER_API_KEY` (required): Your OpenRouter API key
//! - `OPENROUTER_MODEL` (optional): Comma-separated list of models for automatic fallback.
//!   Defaults to `nvidia/nemotron-nano-12b-v2-vl:free`
//! - `PORT` or `DNS_PORT` (optional): Port to listen on, defaults to 53. `PORT` takes precedence.
//! - `HOST` or `DNS_ADDRESS` (optional): Address to bind to, defaults to 0.0.0.0. `HOST` takes precedence.
//!
//! # Examples
//!
//! ```no_run
//! use llm_over_dns::Config;
//!
//! # async fn example() -> anyhow::Result<()> {
//! // Load configuration from environment variables and .env file
//! let config = Config::from_env()?;
//!
//! println!("DNS Server: {}:{}", config.dns_address, config.dns_port);
//! println!("Models: {:?}", config.openrouter_models);
//! # Ok(())
//! # }
//! ```

use anyhow::{Context, Result};
use std::env;

/// Configuration for the LLM over DNS server.
///
/// Contains all necessary configuration for starting the DNS server
/// and making requests to the LLM API.
///
/// # Fields
///
/// * `openrouter_api_key` - Authentication key for OpenRouter API
/// * `openrouter_models` - List of LLM model identifiers for automatic fallback (e.g., ["nvidia/nemotron-nano-12b-v2-vl:free"])
/// * `dns_port` - Port to listen for DNS queries (default: 53)
/// * `dns_address` - Address to bind DNS server to (default: 0.0.0.0)
/// * `system_prompt` - System prompt for LLM (default: "You are a helpful assistant. Keep responses concise and under 200 words.")
/// * `temperature` - Temperature for LLM sampling (0.0-2.0, controls randomness)
/// * `max_tokens` - Maximum response length in tokens
/// * `top_p` - Top-p nucleus sampling (0.0-1.0)
/// * `top_k` - Top-k sampling
/// * `frequency_penalty` - Frequency penalty (0.0-2.0, reduces repetition)
/// * `presence_penalty` - Presence penalty (0.0-2.0, encourages new topics)
#[derive(Debug, Clone)]
pub struct Config {
    /// OpenRouter API key for authentication
    pub openrouter_api_key: String,
    /// List of model identifiers for LLM inference with automatic fallback
    pub openrouter_models: Vec<String>,
    /// System prompt to guide LLM responses
    pub system_prompt: String,
    /// DNS server listening port
    pub dns_port: u16,
    /// DNS server listening address
    pub dns_address: String,
    /// Temperature for LLM sampling (0.0-2.0, controls randomness)
    pub temperature: Option<f32>,
    /// Maximum response length in tokens
    pub max_tokens: Option<u32>,
    /// Top-p nucleus sampling (0.0-1.0)
    pub top_p: Option<f32>,
    /// Top-k sampling
    pub top_k: Option<u32>,
    /// Frequency penalty (0.0-2.0, reduces repetition)
    pub frequency_penalty: Option<f32>,
    /// Presence penalty (0.0-2.0, encourages new topics)
    pub presence_penalty: Option<f32>,
}

impl Config {
    /// Load configuration from environment variables.
    ///
    /// Loads `.env` file if it exists, then reads configuration from environment.
    /// The `OPENROUTER_API_KEY` is required; other values have sensible defaults.
    ///
    /// # Environment Variables
    ///
    /// - `OPENROUTER_API_KEY` - **Required**. Your OpenRouter API key
    /// - `OPENROUTER_MODEL` - Optional. Comma-separated list of models for automatic fallback.
    ///   Defaults to: `nvidia/nemotron-nano-9b-v2:free,meituan/longcat-flash-chat:free,minimax/minimax-m2:free`
    ///   (fastest free models optimized for speed)
    /// - `SYSTEM_PROMPT` - Optional. System prompt to guide LLM responses.
    ///   Defaults to: "You are a helpful assistant. Keep responses concise and under 200 words."
    /// - `PORT` or `DNS_PORT` - Optional. Defaults to `53`. `PORT` takes precedence.
    /// - `HOST` or `DNS_ADDRESS` - Optional. Defaults to `0.0.0.0`. `HOST` takes precedence.
    /// - `TEMPERATURE` - Optional. Controls randomness (0.0-2.0). Uses model default if not set.
    /// - `MAX_TOKENS` - Optional. Maximum response length in tokens. Uses model default if not set.
    /// - `TOP_P` - Optional. Nucleus sampling parameter (0.0-1.0). Uses model default if not set.
    /// - `TOP_K` - Optional. Top-k sampling parameter. Uses model default if not set.
    /// - `FREQUENCY_PENALTY` - Optional. Reduces repetition (0.0-2.0). Defaults to 0 if not set.
    /// - `PRESENCE_PENALTY` - Optional. Encourages new topics (0.0-2.0). Defaults to 0 if not set.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `OPENROUTER_API_KEY` is not set
    /// - `PORT` or `DNS_PORT` is not a valid u16
    /// - `OPENROUTER_MODEL` list is empty after parsing
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use llm_over_dns::Config;
    ///
    /// # async fn example() -> anyhow::Result<()> {
    /// let config = Config::from_env()?;
    /// assert!(!config.openrouter_api_key.is_empty());
    /// assert!(config.dns_port > 0);
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_env() -> Result<Self> {
        // Load .env files in order of precedence:
        // 1. .env.local (highest priority, gitignored for local overrides)
        // 2. .env (standard config file)
        // Skip loading .env files during tests to avoid interference
        #[cfg(not(test))]
        {
            dotenvy::from_filename(".env.local").ok();
            dotenvy::dotenv().ok();
        }

        let openrouter_api_key = env::var("OPENROUTER_API_KEY")
            .context("OPENROUTER_API_KEY environment variable not set")?;

        // Default to fastest free models if not configured
        let default_models = "nvidia/nemotron-nano-9b-v2:free,meituan/longcat-flash-chat:free,minimax/minimax-m2:free";

        let openrouter_model_str = env::var("OPENROUTER_MODEL")
            .unwrap_or_else(|_| default_models.to_string());

        // Parse comma-separated models, trim whitespace, and filter out empty strings
        let openrouter_models: Vec<String> = openrouter_model_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        if openrouter_models.is_empty() {
            return Err(anyhow::anyhow!("OPENROUTER_MODEL list cannot be empty"));
        }

        // Load system prompt with sensible default
        let system_prompt = env::var("SYSTEM_PROMPT")
            .unwrap_or_else(|_| "You are a helpful assistant. Keep responses concise and under 200 words.".to_string());

        // Support both PORT/DNS_PORT and HOST/DNS_ADDRESS environment variables
        // Priority: PORT > DNS_PORT, HOST > DNS_ADDRESS
        let dns_port = env::var("PORT")
            .or_else(|_| env::var("DNS_PORT"))
            .unwrap_or_else(|_| "53".to_string())
            .parse()
            .context("Invalid PORT/DNS_PORT value")?;

        let dns_address = env::var("HOST")
            .or_else(|_| env::var("DNS_ADDRESS"))
            .unwrap_or_else(|_| "0.0.0.0".to_string());

        // Load optional OpenRouter model parameters
        let temperature = env::var("TEMPERATURE").ok().and_then(|s| s.parse().ok());
        let max_tokens = env::var("MAX_TOKENS").ok().and_then(|s| s.parse().ok());
        let top_p = env::var("TOP_P").ok().and_then(|s| s.parse().ok());
        let top_k = env::var("TOP_K").ok().and_then(|s| s.parse().ok());
        let frequency_penalty = env::var("FREQUENCY_PENALTY").ok().and_then(|s| s.parse().ok());
        let presence_penalty = env::var("PRESENCE_PENALTY").ok().and_then(|s| s.parse().ok());

        Ok(Self {
            openrouter_api_key,
            openrouter_models,
            system_prompt,
            dns_port,
            dns_address,
            temperature,
            max_tokens,
            top_p,
            top_k,
            frequency_penalty,
            presence_penalty,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::env;

    #[test]
    #[serial]
    fn test_config_from_env_with_api_key() {
        // Setup environment
        env::set_var("OPENROUTER_API_KEY", "test_key");
        env::set_var("OPENROUTER_MODEL", "test_model");
        env::set_var("SYSTEM_PROMPT", "Test system prompt");
        env::set_var("DNS_PORT", "5353");
        env::set_var("DNS_ADDRESS", "127.0.0.1");

        // Test
        let config = Config::from_env().expect("Failed to load config");

        assert_eq!(config.openrouter_api_key, "test_key");
        assert_eq!(config.openrouter_models, vec!["test_model".to_string()]);
        assert_eq!(config.system_prompt, "Test system prompt");
        assert_eq!(config.dns_port, 5353);
        assert_eq!(config.dns_address, "127.0.0.1");
        // Optional parameters should be None when not set
        assert_eq!(config.temperature, None);
        assert_eq!(config.max_tokens, None);
        assert_eq!(config.top_p, None);
        assert_eq!(config.top_k, None);
        assert_eq!(config.frequency_penalty, None);
        assert_eq!(config.presence_penalty, None);

        // Cleanup
        env::remove_var("OPENROUTER_API_KEY");
        env::remove_var("OPENROUTER_MODEL");
        env::remove_var("SYSTEM_PROMPT");
        env::remove_var("DNS_PORT");
        env::remove_var("DNS_ADDRESS");
    }

    #[test]
    #[serial]
    fn test_config_default_values() {
        // Clean all environment variables first
        env::remove_var("OPENROUTER_API_KEY");
        env::remove_var("OPENROUTER_MODEL");
        env::remove_var("SYSTEM_PROMPT");
        env::remove_var("DNS_PORT");
        env::remove_var("DNS_ADDRESS");
        env::remove_var("TEMPERATURE");
        env::remove_var("MAX_TOKENS");
        env::remove_var("TOP_P");
        env::remove_var("TOP_K");
        env::remove_var("FREQUENCY_PENALTY");
        env::remove_var("PRESENCE_PENALTY");

        // Now set only the required one
        env::set_var("OPENROUTER_API_KEY", "test_key");

        let config = Config::from_env().expect("Failed to load config");

        assert_eq!(config.openrouter_api_key, "test_key");
        assert_eq!(
            config.openrouter_models,
            vec![
                "nvidia/nemotron-nano-9b-v2:free".to_string(),
                "meituan/longcat-flash-chat:free".to_string(),
                "minimax/minimax-m2:free".to_string()
            ]
        );
        assert_eq!(config.system_prompt, "You are a helpful assistant. Keep responses concise and under 200 words.");
        assert_eq!(config.dns_port, 53);
        assert_eq!(config.dns_address, "0.0.0.0");
        // Optional parameters should be None when not set
        assert_eq!(config.temperature, None);
        assert_eq!(config.max_tokens, None);
        assert_eq!(config.top_p, None);
        assert_eq!(config.top_k, None);
        assert_eq!(config.frequency_penalty, None);
        assert_eq!(config.presence_penalty, None);

        env::remove_var("OPENROUTER_API_KEY");
    }

    #[test]
    #[serial]
    fn test_config_missing_api_key() {
        // Clean all environment variables to ensure test isolation
        env::remove_var("OPENROUTER_API_KEY");
        env::remove_var("OPENROUTER_MODEL");
        env::remove_var("DNS_PORT");
        env::remove_var("DNS_ADDRESS");

        let result = Config::from_env();

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("OPENROUTER_API_KEY"));
    }

    #[test]
    #[serial]
    fn test_config_invalid_port() {
        env::set_var("OPENROUTER_API_KEY", "test_key");
        env::set_var("DNS_PORT", "invalid_port");

        let result = Config::from_env();

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid PORT/DNS_PORT"));

        env::remove_var("OPENROUTER_API_KEY");
        env::remove_var("DNS_PORT");
    }

    #[test]
    #[serial]
    fn test_config_multiple_models() {
        env::set_var("OPENROUTER_API_KEY", "test_key");
        env::set_var(
            "OPENROUTER_MODEL",
            "model1,model2,model3"
        );

        let config = Config::from_env().expect("Failed to load config");

        assert_eq!(config.openrouter_models, vec![
            "model1".to_string(),
            "model2".to_string(),
            "model3".to_string()
        ]);

        env::remove_var("OPENROUTER_API_KEY");
        env::remove_var("OPENROUTER_MODEL");
    }

    #[test]
    #[serial]
    fn test_config_multiple_models_with_spaces() {
        env::set_var("OPENROUTER_API_KEY", "test_key");
        env::set_var(
            "OPENROUTER_MODEL",
            "model1 , model2 ,  model3  "
        );

        let config = Config::from_env().expect("Failed to load config");

        assert_eq!(config.openrouter_models, vec![
            "model1".to_string(),
            "model2".to_string(),
            "model3".to_string()
        ]);

        env::remove_var("OPENROUTER_API_KEY");
        env::remove_var("OPENROUTER_MODEL");
    }

    #[test]
    #[serial]
    fn test_config_empty_model_string() {
        env::set_var("OPENROUTER_API_KEY", "test_key");
        env::set_var("OPENROUTER_MODEL", "");

        let result = Config::from_env();

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));

        env::remove_var("OPENROUTER_API_KEY");
        env::remove_var("OPENROUTER_MODEL");
    }

    #[test]
    #[serial]
    fn test_config_only_commas() {
        env::set_var("OPENROUTER_API_KEY", "test_key");
        env::set_var("OPENROUTER_MODEL", ",,, ,");

        let result = Config::from_env();

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));

        env::remove_var("OPENROUTER_API_KEY");
        env::remove_var("OPENROUTER_MODEL");
    }

    #[test]
    #[serial]
    fn test_config_with_model_parameters() {
        // Setup environment with all parameters
        env::set_var("OPENROUTER_API_KEY", "test_key");
        env::set_var("OPENROUTER_MODEL", "test_model");
        env::set_var("TEMPERATURE", "0.7");
        env::set_var("MAX_TOKENS", "500");
        env::set_var("TOP_P", "0.9");
        env::set_var("TOP_K", "40");
        env::set_var("FREQUENCY_PENALTY", "0.5");
        env::set_var("PRESENCE_PENALTY", "0.5");

        let config = Config::from_env().expect("Failed to load config");

        assert_eq!(config.temperature, Some(0.7));
        assert_eq!(config.max_tokens, Some(500));
        assert_eq!(config.top_p, Some(0.9));
        assert_eq!(config.top_k, Some(40));
        assert_eq!(config.frequency_penalty, Some(0.5));
        assert_eq!(config.presence_penalty, Some(0.5));

        // Cleanup
        env::remove_var("OPENROUTER_API_KEY");
        env::remove_var("OPENROUTER_MODEL");
        env::remove_var("TEMPERATURE");
        env::remove_var("MAX_TOKENS");
        env::remove_var("TOP_P");
        env::remove_var("TOP_K");
        env::remove_var("FREQUENCY_PENALTY");
        env::remove_var("PRESENCE_PENALTY");
    }
}
