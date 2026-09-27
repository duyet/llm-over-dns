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
//! - `MAX_CONCURRENT_LLM_REQUESTS` (optional): Global ceiling on in-flight LLM
//!   calls, defaults to 32. Set to 0 to disable. Queries arriving over the limit
//!   are shed with SERVFAIL rather than queued.
//! - `CACHE_MAX_ENTRIES` (optional): Ceiling on cached responses, defaults to
//!   10000. Set to 0 for an unbounded cache.
//!
//! # Validation
//!
//! Every value is validated at startup. A variable that is present but cannot be
//! parsed, falls outside its documented range, or holds an unusable API key is a
//! hard error rather than a silent fallback to a default. A config that loads is
//! a config the server can actually serve; the alternative is a server that
//! binds, logs a healthy startup, and then answers nothing.
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
use std::str::FromStr;

/// Stand-in values shipped in `.env.example`. Either one means the file was
/// copied but never edited, which must not be mistaken for a real credential.
const API_KEY_PLACEHOLDERS: &[&str] = &["your_api_key_here", "sk-ar-REDACTED"];

/// Reads the first of `names` that is present in the environment.
fn first_env_var(names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| env::var(name).ok())
}

/// Parses a numeric environment variable, applying `default` only when every
/// name in `names` is absent.
///
/// A present-but-unparseable value is a startup error, not a silent fallback.
/// `MAX_CONCURRENT_LLM_REQUESTS=1O` (letter O, not a zero) previously became 32,
/// so an operator who believed they had capped concurrent spend was running
/// three times over, with nothing in the logs to say the cap was never applied.
fn parse_env<T>(names: &[&str], default: T) -> Result<T>
where
    T: FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    match first_env_var(names) {
        Some(raw) => raw
            .parse()
            .with_context(|| format!("Invalid {} value: {raw:?}", names.join("/"))),
        None => Ok(default),
    }
}

/// Parses an optional float environment variable and enforces its documented
/// range.
///
/// `Ok(None)` means the variable is absent, which means "use the model default".
/// A present value that does not parse, or that falls outside `min..=max`, is a
/// startup error: `TEMPERATURE=2.5` makes the provider reject every request, so
/// the server SERVFAILs queries it could otherwise have answered.
fn parse_optional_f32_in_range(name: &str, min: f32, max: f32) -> Result<Option<f32>> {
    let Some(raw) = env::var(name).ok() else {
        return Ok(None);
    };

    let value: f32 = raw
        .parse()
        .with_context(|| format!("Invalid {name} value: {raw:?}"))?;

    // The range test alone also rejects `NaN` and the infinities, since every
    // comparison against a non-finite value is false.
    anyhow::ensure!(
        (min..=max).contains(&value),
        "{name} must be between {min} and {max} (got {value})"
    );

    Ok(Some(value))
}

/// Parses an optional unsigned environment variable.
///
/// `Ok(None)` means the variable is absent. A present value that does not parse
/// is a startup error rather than a silent "use the model default".
fn parse_optional_u32(name: &str) -> Result<Option<u32>> {
    let Some(raw) = env::var(name).ok() else {
        return Ok(None);
    };

    raw.parse()
        .with_context(|| format!("Invalid {name} value: {raw:?}"))
        .map(Some)
}

/// As [`parse_optional_u32`], but rejects values below `min`.
///
/// `MAX_TOKENS=0` asks the provider for a response that cannot contain any
/// tokens at all, which it rejects outright rather than truncating. That is a
/// total outage rather than a tuning knob, so it fails at startup instead of
/// being forwarded on every request.
fn parse_optional_u32_min(name: &str, min: u32) -> Result<Option<u32>> {
    let Some(value) = parse_optional_u32(name)? else {
        return Ok(None);
    };

    anyhow::ensure!(value >= min, "{name} must be at least {min} (got {value})");

    Ok(Some(value))
}

/// Rejects rate-limiting values that are negative or non-finite.
///
/// `NaN` parses cleanly as an `f64`, and `IpRateLimiter` then behaves as though
/// limiting had been switched off entirely, so a typo silently removes the only
/// per-IP protection the server has.
fn require_non_negative_finite(name: &str, value: f64) -> Result<f64> {
    anyhow::ensure!(
        value.is_finite() && value >= 0.0,
        "{name} must be a finite value of 0 (which disables the limit) or greater (got {value})"
    );

    Ok(value)
}

/// Validates a provider API key before the server binds, returning it trimmed.
///
/// Presence alone is not enough to prove a key works. `.env.example` ships
/// `OPENROUTER_API_KEY=your_api_key_here`, so a user who copied it without
/// editing got a server that bound, logged "LLM client ready", and then
/// SERVFAILed every query with only a per-query `warn!` that reads like an
/// upstream outage. An interior-whitespace key (a half-pasted credential) or
/// an unwrapped placeholder fails the same way, so both are startup errors.
///
/// A key that is *entirely* blank is different: it carries no intent, and
/// `install.sh` and `docker-compose.yml` used to write an empty
/// `ANYROUTER_API_KEY=` into every deployment. Treating that as "AnyRouter
/// selected" is the original bug, so blank falls through to the other
/// provider rather than erroring — an existing deployment that has the empty
/// line keeps working, and stops 401ing.
fn validate_api_key(name: &str, key: String) -> Result<String> {
    let key = key.trim();

    anyhow::ensure!(
        !key.contains(char::is_whitespace),
        "{name} contains whitespace, which no provider key does; it looks half-pasted"
    );

    anyhow::ensure!(
        !API_KEY_PLACEHOLDERS.contains(&key),
        "{name} still holds the .env.example placeholder; replace it with a real {name}"
    );

    Ok(key.to_string())
}

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
    /// Base URL for the LLM API
    pub llm_base_url: String,
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
    /// Cache TTL in seconds (default: 300, set to 0 to disable)
    pub cache_ttl_seconds: u64,
    /// Rate limit requests per second per IP (default: 5.0, set to 0 to disable)
    pub rate_limit_rps: f64,
    /// Rate limit burst requests per IP (default: 10.0)
    pub rate_limit_burst: f64,
    /// Maximum LLM API calls in flight at once (default: 32, set to 0 to disable)
    ///
    /// Per-IP rate limiting cannot bound this: UDP source addresses are
    /// spoofable, so rotating sources yields unlimited per-IP allowance. This is
    /// the global ceiling on concurrent spend and in-flight tasks.
    pub max_concurrent_llm_requests: usize,
    /// Maximum cached responses retained (default: 10000, set to 0 for unbounded)
    pub cache_max_entries: usize,
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
    /// - `MAX_TOKENS` - Optional. Maximum response length in tokens, at least 1.
    ///   Uses model default if not set.
    /// - `TOP_P` - Optional. Nucleus sampling parameter (0.0-1.0). Uses model default if not set.
    /// - `TOP_K` - Optional. Top-k sampling parameter. Uses model default if not set.
    /// - `FREQUENCY_PENALTY` - Optional. Reduces repetition (0.0-2.0). Defaults to 0 if not set.
    /// - `PRESENCE_PENALTY` - Optional. Encourages new topics (0.0-2.0). Defaults to 0 if not set.
    /// - `RATE_LIMIT_RPS` - Optional. Requests per second per IP (default: 5.0, 0 disables).
    /// - `RATE_LIMIT_BURST` - Optional. Burst requests per IP (default: 10.0, 0 disables).
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Neither `ANYROUTER_API_KEY` nor `OPENROUTER_API_KEY` is set
    /// - The API key is blank, contains whitespace, or is still the `.env.example`
    ///   placeholder
    /// - `PORT` or `DNS_PORT` is not a valid u16
    /// - `OPENROUTER_MODEL` list is empty after parsing
    /// - A numeric variable is present but cannot be parsed, or falls outside the
    ///   range documented for it
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

        // Support ANYROUTER_API_KEY with fallback to OPENROUTER_API_KEY.
        // A blank ANYROUTER_API_KEY is treated as unset, not as a selection:
        // deploy tooling used to write an empty one, and honouring it picked
        // the AnyRouter base URL with no credential, so every call 401'd.
        let anyrouter_key = env::var("ANYROUTER_API_KEY")
            .ok()
            .filter(|key| !key.trim().is_empty());
        let (openrouter_api_key, is_anyrouter) = if let Some(key) = anyrouter_key {
            (validate_api_key("ANYROUTER_API_KEY", key)?, true)
        } else {
            let key = env::var("OPENROUTER_API_KEY").context(
                "Neither ANYROUTER_API_KEY nor OPENROUTER_API_KEY environment variable is set",
            )?;
            let key = validate_api_key("OPENROUTER_API_KEY", key)?;
            // AnyRouter keys are recognised by their `sk-ar-` prefix. Selecting
            // on the trimmed key keeps a pasted credential on the right provider.
            let is_ar = key.starts_with("sk-ar-");
            (key, is_ar)
        };

        // Determine base URL based on provider
        let llm_base_url = if is_anyrouter {
            "https://anyrouter.dev/api/v1/chat/completions".to_string()
        } else {
            "https://openrouter.ai/api/v1/chat/completions".to_string()
        };

        // Default models
        let default_models = if is_anyrouter {
            "google/gemini-2.5-flash-lite,meta/llama-3.2-3b-instruct"
        } else {
            "nvidia/nemotron-nano-9b-v2:free,meituan/longcat-flash-chat:free,minimax/minimax-m2:free"
        };

        // Load models from the variable belonging to the active provider. Reading
        // ANYROUTER_MODEL unconditionally meant an OpenRouter user with a leftover
        // ANYROUTER_MODEL sent AnyRouter-namespaced model ids to openrouter.ai,
        // where every model in the fallback chain 404s and every query SERVFAILs.
        let model_var = if is_anyrouter {
            "ANYROUTER_MODEL"
        } else {
            "OPENROUTER_MODEL"
        };
        let openrouter_model_str =
            env::var(model_var).unwrap_or_else(|_| default_models.to_string());

        // Parse comma-separated models, trim whitespace, and filter out empty strings
        let openrouter_models: Vec<String> = openrouter_model_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        if openrouter_models.is_empty() {
            return Err(anyhow::anyhow!("Model list cannot be empty"));
        }

        // Load system prompt with sensible default
        let system_prompt = env::var("SYSTEM_PROMPT").unwrap_or_else(|_| {
            "You are a helpful assistant. Keep responses concise and under 200 words.".to_string()
        });

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

        // Load optional OpenRouter model parameters. These used to be dropped
        // outright when they failed to parse, so a typo silently handed the model
        // default to the provider and a bad value silently reached it.
        let temperature = parse_optional_f32_in_range("TEMPERATURE", 0.0, 2.0)?;
        let max_tokens = parse_optional_u32_min("MAX_TOKENS", 1)?;
        let top_p = parse_optional_f32_in_range("TOP_P", 0.0, 1.0)?;
        // `TOP_K` keeps only the parse check: providers disagree on whether 0
        // disables top-k sampling, so it is not treated as a range violation.
        let top_k = parse_optional_u32("TOP_K")?;
        let frequency_penalty = parse_optional_f32_in_range("FREQUENCY_PENALTY", 0.0, 2.0)?;
        let presence_penalty = parse_optional_f32_in_range("PRESENCE_PENALTY", 0.0, 2.0)?;

        // Load caching and rate limiting parameters
        let cache_ttl_seconds = parse_env(&["CACHE_TTL_SEC", "DNS_CACHE_TTL"], 300u64)?;

        // 0 is the documented "disable" sentinel for both rate-limiting knobs.
        let rate_limit_rps = require_non_negative_finite(
            "RATE_LIMIT_RPS",
            parse_env(&["RATE_LIMIT_RPS", "DNS_RATE_LIMIT_RPS"], 5.0f64)?,
        )?;
        let rate_limit_burst = require_non_negative_finite(
            "RATE_LIMIT_BURST",
            parse_env(&["RATE_LIMIT_BURST", "DNS_RATE_LIMIT_BURST"], 10.0f64)?,
        )?;

        // A bucket sized below one whole token can never satisfy the limiter's
        // `tokens >= 1.0` check, so a sub-1.0 burst refuses every query forever
        // while the server still logs a healthy startup. 0 stays the "disable"
        // sentinel.
        anyhow::ensure!(
            rate_limit_burst == 0.0 || rate_limit_burst >= 1.0,
            "RATE_LIMIT_BURST must be 0 (disabled) or at least 1 (got {rate_limit_burst})"
        );

        // 0 keeps its documented meaning for both ceilings: no semaphore for
        // in-flight LLM calls, and an unbounded cache.
        let max_concurrent_llm_requests = parse_env(&["MAX_CONCURRENT_LLM_REQUESTS"], 32usize)?;
        let cache_max_entries = parse_env(&["CACHE_MAX_ENTRIES"], 10000usize)?;

        Ok(Self {
            openrouter_api_key,
            openrouter_models,
            llm_base_url,
            system_prompt,
            dns_port,
            dns_address,
            temperature,
            max_tokens,
            top_p,
            top_k,
            frequency_penalty,
            presence_penalty,
            cache_ttl_seconds,
            rate_limit_rps,
            rate_limit_burst,
            max_concurrent_llm_requests,
            cache_max_entries,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::env;

    /// Every variable `Config::from_env` reads, so a test starts from a known
    /// state instead of inheriting the developer's shell or a variable an
    /// earlier test leaked.
    const ALL_CONFIG_VARS: &[&str] = &[
        "ANYROUTER_API_KEY",
        "ANYROUTER_MODEL",
        "OPENROUTER_API_KEY",
        "OPENROUTER_MODEL",
        "SYSTEM_PROMPT",
        "PORT",
        "DNS_PORT",
        "HOST",
        "DNS_ADDRESS",
        "TEMPERATURE",
        "MAX_TOKENS",
        "TOP_P",
        "TOP_K",
        "FREQUENCY_PENALTY",
        "PRESENCE_PENALTY",
        "CACHE_TTL_SEC",
        "DNS_CACHE_TTL",
        "RATE_LIMIT_RPS",
        "DNS_RATE_LIMIT_RPS",
        "RATE_LIMIT_BURST",
        "DNS_RATE_LIMIT_BURST",
        "MAX_CONCURRENT_LLM_REQUESTS",
        "CACHE_MAX_ENTRIES",
    ];

    /// Applies environment mutations for one test and restores the previous
    /// values on drop.
    ///
    /// The in-repo pattern of `set_var` up front and `remove_var` at the end
    /// leaks the variable into every later `#[serial]` test whenever an
    /// assertion panics first, and destroys a developer's real environment for
    /// the rest of the run. Dropping restores either way.
    struct ScopedEnv(Vec<(&'static str, Option<String>)>);

    impl ScopedEnv {
        fn new() -> Self {
            Self(Vec::new())
        }

        fn set(&mut self, key: &'static str, value: &str) {
            self.remember(key);
            env::set_var(key, value);
        }

        fn remove(&mut self, key: &'static str) {
            self.remember(key);
            env::remove_var(key);
        }

        fn remember(&mut self, key: &'static str) {
            if !self.0.iter().any(|(seen, _)| *seen == key) {
                self.0.push((key, env::var(key).ok()));
            }
        }
    }

    impl Drop for ScopedEnv {
        fn drop(&mut self) {
            for (key, previous) in self.0.drain(..) {
                match previous {
                    Some(value) => env::set_var(key, value),
                    None => env::remove_var(key),
                }
            }
        }
    }

    /// A cleared environment holding only a usable API key, so a test only has
    /// to state the variable it is actually exercising.
    fn base_env() -> ScopedEnv {
        let mut vars = ScopedEnv::new();
        for key in ALL_CONFIG_VARS {
            vars.remove(key);
        }
        vars.set("OPENROUTER_API_KEY", "test_key");
        vars
    }

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
        env::remove_var("ANYROUTER_API_KEY");
        env::remove_var("ANYROUTER_MODEL");
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
        assert_eq!(
            config.system_prompt,
            "You are a helpful assistant. Keep responses concise and under 200 words."
        );
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
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid PORT/DNS_PORT"));

        env::remove_var("OPENROUTER_API_KEY");
        env::remove_var("DNS_PORT");
    }

    #[test]
    #[serial]
    fn test_openrouter_provider_ignores_anyrouter_model() {
        // A leftover ANYROUTER_MODEL used to win regardless of provider, so an
        // OpenRouter user sent AnyRouter model ids to openrouter.ai and every
        // query SERVFAILed. The active provider must pick its own variable.
        env::set_var("OPENROUTER_API_KEY", "test_key");
        env::set_var("ANYROUTER_MODEL", "google/gemini-2.5-flash-lite");
        env::remove_var("OPENROUTER_MODEL");

        let config = Config::from_env().expect("Failed to load config");

        assert!(
            !config
                .openrouter_models
                .contains(&"google/gemini-2.5-flash-lite".to_string()),
            "AnyRouter model leaked into an OpenRouter config: {:?}",
            config.openrouter_models
        );
        assert!(config.llm_base_url.contains("openrouter.ai"));

        env::remove_var("OPENROUTER_API_KEY");
        env::remove_var("ANYROUTER_MODEL");
    }

    #[test]
    #[serial]
    fn test_anyrouter_provider_uses_anyrouter_model() {
        env::remove_var("OPENROUTER_API_KEY");
        env::set_var("ANYROUTER_API_KEY", "sk-ar-test_key");
        env::set_var("ANYROUTER_MODEL", "vendor/model-a,vendor/model-b");

        let config = Config::from_env().expect("Failed to load config");

        assert_eq!(
            config.openrouter_models,
            vec!["vendor/model-a".to_string(), "vendor/model-b".to_string()]
        );
        assert!(config.llm_base_url.contains("anyrouter.dev"));

        env::remove_var("ANYROUTER_API_KEY");
        env::remove_var("ANYROUTER_MODEL");
    }

    #[test]
    #[serial]
    fn test_config_multiple_models() {
        env::set_var("OPENROUTER_API_KEY", "test_key");
        env::set_var("OPENROUTER_MODEL", "model1,model2,model3");

        let config = Config::from_env().expect("Failed to load config");

        assert_eq!(
            config.openrouter_models,
            vec![
                "model1".to_string(),
                "model2".to_string(),
                "model3".to_string()
            ]
        );

        env::remove_var("OPENROUTER_API_KEY");
        env::remove_var("OPENROUTER_MODEL");
    }

    #[test]
    #[serial]
    fn test_config_multiple_models_with_spaces() {
        env::set_var("OPENROUTER_API_KEY", "test_key");
        env::set_var("OPENROUTER_MODEL", "model1 , model2 ,  model3  ");

        let config = Config::from_env().expect("Failed to load config");

        assert_eq!(
            config.openrouter_models,
            vec![
                "model1".to_string(),
                "model2".to_string(),
                "model3".to_string()
            ]
        );

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

    #[test]
    #[serial]
    fn test_config_anyrouter() {
        env::set_var("ANYROUTER_API_KEY", "sk-ar-v1-testkey");
        env::set_var("ANYROUTER_MODEL", "meta/llama-3.2-3b-instruct");

        let config = Config::from_env().expect("Failed to load AnyRouter config");
        assert_eq!(config.openrouter_api_key, "sk-ar-v1-testkey");
        assert_eq!(
            config.openrouter_models,
            vec!["meta/llama-3.2-3b-instruct".to_string()]
        );
        assert_eq!(
            config.llm_base_url,
            "https://anyrouter.dev/api/v1/chat/completions"
        );

        env::remove_var("ANYROUTER_API_KEY");
        env::remove_var("ANYROUTER_MODEL");
    }

    #[test]
    #[serial]
    fn test_unparseable_numeric_var_is_a_startup_error() {
        // These three used to discard their ParseIntError and fall back to the
        // default with nothing logged, so `MAX_CONCURRENT_LLM_REQUESTS=1O` (letter
        // O) silently became 32 and the spend ceiling was never applied.
        let mut vars = base_env();
        vars.set("MAX_CONCURRENT_LLM_REQUESTS", "1O");
        let err = Config::from_env().unwrap_err().to_string();
        assert!(
            err.contains("MAX_CONCURRENT_LLM_REQUESTS"),
            "error should name the variable, got: {err}"
        );

        vars.set("MAX_CONCURRENT_LLM_REQUESTS", "32");
        vars.set("CACHE_MAX_ENTRIES", "ten-thousand");
        let err = Config::from_env().unwrap_err().to_string();
        assert!(
            err.contains("CACHE_MAX_ENTRIES"),
            "error should name the variable, got: {err}"
        );

        vars.set("CACHE_MAX_ENTRIES", "10000");
        vars.set("CACHE_TTL_SEC", "5m");
        let err = Config::from_env().unwrap_err().to_string();
        assert!(
            err.contains("CACHE_TTL_SEC"),
            "error should name the variable, got: {err}"
        );
    }

    #[test]
    #[serial]
    fn test_absent_numeric_var_uses_documented_default() {
        // The default belongs to an absent variable only: the test above proves
        // a present-but-unparseable value is an error, this one proves nothing
        // regressed into a default for a variable nobody set.
        let _vars = base_env();

        let config = Config::from_env().expect("Failed to load config");

        assert_eq!(config.cache_ttl_seconds, 300);
        assert_eq!(config.rate_limit_rps, 5.0);
        assert_eq!(config.rate_limit_burst, 10.0);
        assert_eq!(config.max_concurrent_llm_requests, 32);
        assert_eq!(config.cache_max_entries, 10000);
    }

    #[test]
    #[serial]
    fn test_fractional_rate_limit_burst_is_rejected() {
        // A bucket holding less than one whole token can never reach the
        // limiter's `tokens >= 1.0` check, so this config refuses 100% of queries
        // while the server still binds and logs a healthy startup.
        let mut vars = base_env();
        vars.set("RATE_LIMIT_BURST", "0.5");

        let err = Config::from_env().unwrap_err().to_string();
        assert!(
            err.contains("RATE_LIMIT_BURST"),
            "error should name the variable, got: {err}"
        );
    }

    #[test]
    #[serial]
    fn test_non_finite_rate_limit_rps_is_rejected() {
        // "NaN" parses as a valid f64, and the limiter then behaves as though
        // limiting were switched off, silently removing the per-IP protection.
        let mut vars = base_env();
        vars.set("RATE_LIMIT_RPS", "NaN");
        let err = Config::from_env().unwrap_err().to_string();
        assert!(
            err.contains("RATE_LIMIT_RPS"),
            "error should name the variable, got: {err}"
        );

        vars.set("RATE_LIMIT_RPS", "-1");
        let err = Config::from_env().unwrap_err().to_string();
        assert!(
            err.contains("RATE_LIMIT_RPS"),
            "error should name the variable, got: {err}"
        );
    }

    #[test]
    #[serial]
    fn test_out_of_range_model_parameter_is_rejected() {
        // Each of these reached the provider as-is and made it reject the whole
        // request, so every query SERVFAILed rather than degrading.
        let cases = [
            ("TEMPERATURE", "2.5"),
            ("TEMPERATURE", "warm"),
            ("TOP_P", "1.5"),
            ("FREQUENCY_PENALTY", "2.1"),
            ("PRESENCE_PENALTY", "-0.1"),
            ("MAX_TOKENS", "0"),
        ];

        for (name, value) in cases {
            let mut vars = base_env();
            vars.set(name, value);

            let err = Config::from_env()
                .map(|_| ())
                .expect_err(&format!("{name}={value} should be rejected"))
                .to_string();
            assert!(err.contains(name), "error should name {name}, got: {err}");
        }
    }

    #[test]
    #[serial]
    fn test_documented_sentinels_are_accepted() {
        // Zero is documented as "disable" or "unbounded" for these four. Range
        // validation must never turn a supported value into a startup error.
        let mut vars = base_env();
        vars.set("MAX_CONCURRENT_LLM_REQUESTS", "0");
        vars.set("CACHE_MAX_ENTRIES", "0");
        vars.set("RATE_LIMIT_RPS", "0");
        vars.set("RATE_LIMIT_BURST", "0");

        let config = Config::from_env().expect("documented zero sentinels must be accepted");

        assert_eq!(config.max_concurrent_llm_requests, 0);
        assert_eq!(config.cache_max_entries, 0);
        assert_eq!(config.rate_limit_rps, 0.0);
        assert_eq!(config.rate_limit_burst, 0.0);
    }

    #[test]
    #[serial]
    fn test_env_example_placeholder_api_key_is_rejected() {
        // `.env.example` ships these verbatim, so accepting them produced a
        // server that bound, logged "LLM client ready", and then SERVFAILed every
        // query with only a per-query warn that looked like an upstream outage.
        let mut vars = base_env();
        vars.set("OPENROUTER_API_KEY", "your_api_key_here");
        let err = Config::from_env().unwrap_err().to_string();
        assert!(
            err.contains("OPENROUTER_API_KEY"),
            "error should name the variable, got: {err}"
        );

        vars.set("OPENROUTER_API_KEY", "test_key");
        vars.set("ANYROUTER_API_KEY", "sk-ar-REDACTED");
        let err = Config::from_env().unwrap_err().to_string();
        assert!(
            err.contains("ANYROUTER_API_KEY"),
            "error should name the variable, got: {err}"
        );
    }

    #[test]
    #[serial]
    fn test_half_pasted_api_key_is_rejected() {
        // An interior-whitespace key is a half-pasted credential and
        // authenticates no better than the placeholder, so it must fail
        // before the server binds.
        let mut vars = base_env();
        vars.set("OPENROUTER_API_KEY", "sk-or-v1 abc def");

        let err = Config::from_env()
            .map(|_| ())
            .expect_err("a key with interior whitespace should be rejected")
            .to_string();
        assert!(
            err.contains("OPENROUTER_API_KEY"),
            "error should name the variable, got: {err}"
        );
    }

    #[test]
    #[serial]
    fn test_blank_anyrouter_api_key_falls_through_instead_of_selecting_anyrouter() {
        // `install.sh` and `docker-compose.yml` used to write an empty
        // `ANYROUTER_API_KEY=` into every deployment. Treating that as
        // "AnyRouter selected" pointed the client at the AnyRouter base URL
        // with no credential, so every query 401'd. Blank carries no intent,
        // so it must fall through to the OpenRouter key instead.
        let mut vars = base_env();
        vars.set("ANYROUTER_API_KEY", "   ");
        vars.set("OPENROUTER_API_KEY", "sk-or-TESTKEY");

        let config = Config::from_env().expect("blank ANYROUTER_API_KEY must not be fatal");
        assert_eq!(config.openrouter_api_key, "sk-or-TESTKEY");
        assert!(
            !config.llm_base_url.contains("anyrouter"),
            "a blank key must not select the AnyRouter endpoint, got: {}",
            config.llm_base_url
        );
    }

    #[test]
    #[serial]
    fn test_api_key_is_trimmed_without_breaking_provider_selection() {
        // Credentials are routinely pasted with surrounding whitespace. Trimming
        // must not change which provider a real key selects, in either direction.
        let mut vars = base_env();
        vars.set("OPENROUTER_API_KEY", "  sk-ar-TESTKEY  ");
        let config = Config::from_env().expect("Failed to load config");
        assert_eq!(config.openrouter_api_key, "sk-ar-TESTKEY");
        assert_eq!(
            config.llm_base_url,
            "https://anyrouter.dev/api/v1/chat/completions"
        );

        vars.set("OPENROUTER_API_KEY", "  sk-or-TESTKEY  ");
        let config = Config::from_env().expect("Failed to load config");
        assert_eq!(config.openrouter_api_key, "sk-or-TESTKEY");
        assert_eq!(
            config.llm_base_url,
            "https://openrouter.ai/api/v1/chat/completions"
        );

        vars.set("ANYROUTER_API_KEY", " sk-ar-TESTKEY ");
        vars.remove("OPENROUTER_API_KEY");
        let config = Config::from_env().expect("Failed to load config");
        assert_eq!(config.openrouter_api_key, "sk-ar-TESTKEY");
        assert_eq!(
            config.llm_base_url,
            "https://anyrouter.dev/api/v1/chat/completions"
        );
    }
}
