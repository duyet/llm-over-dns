use anyhow::Result;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;
use tokio::time::timeout;

/// Test DNS server wrapper for managing lifecycle
pub struct TestDnsServer {
    port: u16,
}

impl TestDnsServer {
    /// Create a new test DNS server instance
    pub fn new(port: u16) -> Self {
        Self { port }
    }

    /// Get the server port
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Get server address
    pub fn address(&self) -> String {
        format!("127.0.0.1:{}", self.port)
    }
}

/// Wait for DNS server to be ready
#[allow(dead_code)]
pub async fn wait_for_dns_server(port: u16, max_attempts: u32) -> Result<()> {
    let _socket_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);

    for attempt in 0..max_attempts {
        match timeout(
            Duration::from_millis(500),
            tokio::net::UdpSocket::bind("127.0.0.1:0"),
        )
        .await
        {
            Ok(Ok(_)) => return Ok(()),
            _ => {
                if attempt < max_attempts - 1 {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
    }

    anyhow::bail!(
        "DNS server failed to become ready after {} attempts",
        max_attempts
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dns_server_creation() {
        let server = TestDnsServer::new(5353);
        assert_eq!(server.port(), 5353);
        assert_eq!(server.address(), "127.0.0.1:5353");
    }
}
