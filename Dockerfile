# Stage 1: Builder
# Uses latest Rust toolchain to compile the binary in release mode
FROM rust:latest AS builder

WORKDIR /build

# Copy manifest files
COPY Cargo.toml Cargo.lock ./

# Create dummy main to cache dependencies
# This layer is cached separately from actual source code
RUN mkdir -p src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release 2>&1 | grep -v "warning" || true && \
    rm -rf src

# Copy source code
COPY src ./src

# Build release binary
# Cargo will rebuild only what's necessary due to cached dependencies
RUN cargo build --release

# Extract binary name from Cargo.toml and create a symlink for easy access
RUN cp /build/target/release/llm-over-dns /build/llm-over-dns-binary


# Stage 2: Runtime
# Uses Debian slim image for smaller final image while maintaining glibc compatibility
FROM debian:bookworm-slim

# Install minimal runtime dependencies
# ca-certificates: For HTTPS requests to OpenRouter API
# libssl3: Required by reqwest for TLS connections
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 && \
    rm -rf /var/lib/apt/lists/*

# Create non-root user for security
# The application will run as 'llm' user (uid: 1000)
RUN useradd -m -u 1000 -s /sbin/nologin llm

WORKDIR /app

# Copy binary from builder stage
COPY --from=builder /build/llm-over-dns-binary /app/llm-over-dns

# Set proper permissions
RUN chmod +x /app/llm-over-dns && \
    chown llm:llm /app/llm-over-dns

# Create directory for any runtime data (logs, cache, etc.)
RUN mkdir -p /app/data && \
    chown llm:llm /app/data

# Expose DNS port (UDP)
EXPOSE 53/udp

# Health check that validates DNS server is responding
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD timeout 2 bash -c 'echo -e "\\x00\\x00\\x00\\x00\\x00\\x00\\x00\\x00" | nc -u localhost 53 > /dev/null 2>&1 && echo "OK" || exit 1' || exit 1

# Run the application
# Note: Port 53 requires elevated privileges, so we use the --cap-drop/--cap-add approach
# in docker-compose or keep this as root by removing the USER directive if needed
USER llm

ENTRYPOINT ["/app/llm-over-dns"]
