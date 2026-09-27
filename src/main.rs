//! LLM over DNS - Binary entry point
//!
//! Simple DNS server that sends queries directly to LLM.

use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::task::{JoinError, JoinHandle};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use llm_over_dns::{Config, Server};

/// Number of leading characters of an API key that are safe to log.
const VISIBLE_KEY_CHARS: usize = 8;

/// Outcome of the spawned server task: the task's own result, or the join error
/// if the task panicked or was cancelled.
type ServerTaskResult = std::result::Result<Result<()>, JoinError>;

/// Mask an API key for secure logging (shows first 8 characters)
///
/// Measured in characters, not bytes: a key of non-ASCII characters is longer in
/// bytes than it is long, and a byte count let a short multibyte key past the
/// full-mask branch to be logged in full.
fn mask_api_key(key: &str) -> String {
    let key_chars = key.chars().count();
    if key_chars <= VISIBLE_KEY_CHARS {
        "*".repeat(key_chars)
    } else {
        key.chars().take(VISIBLE_KEY_CHARS).collect()
    }
}

/// Reports how the server task ended, turning a failure into a fatal error.
///
/// Both exits from the wait loop go through here: a task that ended badly is a
/// real failure whether it happened during normal operation or while unwinding
/// from a shutdown signal.
fn report_server_outcome(outcome: ServerTaskResult) -> Result<()> {
    match outcome {
        Ok(Ok(_)) => {
            info!("Server stopped normally");
            Ok(())
        }
        Ok(Err(e)) => {
            error!("Server error: {:?}", e);
            Err(e)
        }
        Err(e) => {
            error!("Server task panicked: {:?}", e);
            Err(anyhow::anyhow!("Server task panicked: {}", e))
        }
    }
}

/// Waits for the server task to unwind after a shutdown signal has been sent.
///
/// Awaiting the handle is what makes shutdown graceful. Returning from `main`
/// drops the runtime, which aborts whatever the task still has in flight
/// instead of letting it finish.
async fn await_shutdown(server_task: &mut JoinHandle<Result<()>>) -> Result<()> {
    report_server_outcome(server_task.await)
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
    let provider_name = if config.llm_base_url.contains("anyrouter.dev") {
        "AnyRouter API (https://anyrouter.dev)"
    } else {
        "OpenRouter API (https://openrouter.ai)"
    };
    info!("Provider: {}", provider_name);
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
    let mut server_task = tokio::spawn(async move {
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

    // Wait for either Ctrl+C or server task to complete/fail. The handle is
    // borrowed rather than moved into the select, so the signal arm can await
    // it instead of detaching the task.
    tokio::select! {
        result = &mut server_task => report_server_outcome(result)?,
        _ = tokio::signal::ctrl_c() => {
            info!("Received shutdown signal (Ctrl+C)");
            // Signal, then await: the server task leaves its receive loop on the
            // signal, and awaiting it here is what gives in-flight LLM calls the
            // chance to finish that the graceful shutdown path promises.
            server.shutdown()?;
            info!("Server shutdown complete");
            await_shutdown(&mut server_task).await?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

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

    #[test]
    fn test_mask_api_key_short_multibyte_key_is_fully_masked() {
        // 7 characters but 10 bytes. A byte-length check calls this a long key
        // and logs the entire secret.
        let key = "kéy-ünï";
        assert_eq!(key.chars().count(), 7);
        assert!(key.len() > 8);

        let masked = mask_api_key(key);
        assert_eq!(masked, "*".repeat(7), "multibyte key was logged in full");
        assert_eq!(masked.chars().count(), 7);
    }

    #[test]
    fn test_mask_api_key_multibyte_key_at_visible_limit_is_fully_masked() {
        // 8 characters but 9 bytes: exactly at the limit by character count,
        // over it by byte count.
        let key = "1234567é";
        assert_eq!(key.chars().count(), 8);
        assert!(key.len() > 8);

        assert_eq!(mask_api_key(key), "*".repeat(8));
    }

    #[test]
    fn test_mask_api_key_long_multibyte_key_keeps_eight_characters() {
        // Past the limit, the visible prefix is measured in characters.
        let key = "kéy-ünïcode-abcdef";
        let masked = mask_api_key(key);
        assert_eq!(masked, "kéy-ünïc");
        assert_eq!(masked.chars().count(), 8);
    }

    #[tokio::test]
    async fn test_shutdown_waits_for_the_server_task() {
        // The shutdown path must await the task: `main` returning drops the
        // runtime, and a detached task has its in-flight LLM calls aborted
        // there rather than being allowed to finish.
        let finished = Arc::new(AtomicBool::new(false));
        let flag = finished.clone();
        let mut server_task = tokio::spawn(async move {
            // Yield once so the flag is only observable if the caller really
            // waited: a detached task has not been polled again by the time a
            // non-waiting caller checks.
            tokio::task::yield_now().await;
            flag.store(true, Ordering::SeqCst);
            Ok(())
        });

        await_shutdown(&mut server_task)
            .await
            .expect("a clean stop is not an error");

        assert!(
            finished.load(Ordering::SeqCst),
            "shutdown returned before the server task finished"
        );
    }

    #[tokio::test]
    async fn test_shutdown_surfaces_a_server_error() {
        let mut server_task = tokio::spawn(async { anyhow::bail!("socket bind failed") });

        let err = await_shutdown(&mut server_task)
            .await
            .expect_err("a server task that failed during shutdown must be reported");
        assert!(
            err.to_string().contains("socket bind failed"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn test_shutdown_reports_a_panicking_task() {
        // Awaiting the handle is also what surfaces a panic instead of leaving
        // it as an unobserved event on a detached task.
        let mut server_task = tokio::spawn(async { panic!("boom") });

        let err = await_shutdown(&mut server_task)
            .await
            .expect_err("a panicked server task must be reported");
        assert!(
            err.to_string().contains("panicked"),
            "unexpected error: {err}"
        );
    }
}
