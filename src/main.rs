//! LLM over DNS - Binary entry point
//!
//! Simple DNS server that sends queries directly to LLM.

use anyhow::{Context, Result};
use std::sync::Arc;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use llm_over_dns::{Config, Server};

/// Mask an API key for secure logging (shows first 8 characters)
fn mask_api_key(key: &str) -> String {
    let visible_chars = 8;
    if key.len() <= visible_chars {
        "*".repeat(key.len())
    } else {
        key.chars().take(visible_chars).collect()
    }
}

/// Main async entry point
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("llm_over_dns=debug".parse()?)
                .add_directive("info".parse()?),
        )
        .init();

    info!("Starting LLM over DNS server...");
    info!("");

    // Load configuration from environment variables
    let config = Config::from_env().context("Failed to load configuration")?;

    // Display configuration with masked API key
    info!("=== Configuration ===");
    info!("Provider: OpenRouter API (https://openrouter.ai)");
    info!(
        "API Key: {}...*** (masked)",
        mask_api_key(&config.openrouter_api_key)
    );
    info!("Models (with fallback): {:?}", config.openrouter_models);
    info!("DNS Server: {}:{}", config.dns_address, config.dns_port);
    info!("");

    // Create DNS server with all components
    let server = Arc::new(Server::new(config.clone()).context("Failed to create DNS server")?);

    info!("=== Components Initialized ===");
    info!("✓ LLM client ready");
    info!("✓ Chunker ready (max chunk: 250 bytes, max total: 4096 bytes)");
    info!("✓ DNS handler ready");
    info!("✓ DNS server ready");
    info!("");

    info!("=== Example Queries ===");
    info!(
        "  dig @{} -p {} 'hello world' TXT +time=30",
        config.dns_address, config.dns_port
    );
    info!(
        "  dig @{} -p {} 'what is rust' TXT +time=30",
        config.dns_address, config.dns_port
    );
    info!(
        "  dig @{} -p {} 'explain quantum computing' TXT +time=30",
        config.dns_address, config.dns_port
    );
    info!("");
    info!("Note: DNS queries are sent directly to the LLM (no domain parsing)");
    info!("Tip: Use +time=30 to increase dig timeout (LLM calls can take 5-15 seconds)");
    info!("Tip: Add +short to show only TXT record content without DNS metadata");
    info!("");

    info!("=== Server Ready ===");
    info!("Press Ctrl+C to stop");
    info!("");

    // Spawn server task
    let server_clone = server.clone();
    let server_task = tokio::spawn(async move {
        info!("Server task starting...");
        match server_clone.start().await {
            Ok(_) => {
                info!("Server task completed successfully");
                Ok(())
            }
            Err(e) => {
                error!("Server task failed: {:?}", e);
                Err(e)
            }
        }
    });

    // Wait for either Ctrl+C or server task to complete/fail
    tokio::select! {
        result = server_task => {
            match result {
                Ok(Ok(_)) => {
                    info!("Server stopped normally");
                }
                Ok(Err(e)) => {
                    error!("Server error: {:?}", e);
                    return Err(e);
                }
                Err(e) => {
                    error!("Server task panicked: {:?}", e);
                    return Err(anyhow::anyhow!("Server task panicked: {}", e));
                }
            }
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Received shutdown signal (Ctrl+C)");
            // Trigger graceful shutdown
            server.shutdown()?;
            info!("Server shutdown complete");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_api_key_normal() {
        let key = "sk-or-v1-1234567890abcdef";
        let masked = mask_api_key(key);
        assert_eq!(masked, "sk-or-v1");
        assert_eq!(masked.len(), 8);
    }

    #[test]
    fn test_mask_api_key_short() {
        let key = "short";
        let masked = mask_api_key(key);
        assert_eq!(masked, "*****");
        assert_eq!(masked.len(), key.len());
    }

    #[test]
    fn test_mask_api_key_exactly_8() {
        let key = "12345678";
        let masked = mask_api_key(key);
        assert_eq!(masked, "********");
        assert_eq!(masked.len(), 8);
    }

    #[test]
    fn test_mask_api_key_empty() {
        let key = "";
        let masked = mask_api_key(key);
        assert_eq!(masked, "");
    }
}
