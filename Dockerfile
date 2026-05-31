# Stage 1: Builder
# rust:alpine uses musl as default target — produces a fully static binary with zero glibc deps.
# No need to download additional targets or musl-tools.
FROM docker.io/library/rust:alpine AS builder

# Install build dependencies (musl-dev is pre-installed, need openssl dev headers)
RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static pkgconfig perl make

WORKDIR /build

# Copy manifest files
COPY Cargo.toml Cargo.lock ./

# Create dummy main to cache dependencies separately from source
RUN mkdir -p src && \
    echo "fn main() {}" > src/main.rs && \
    OPENSSL_STATIC=1 cargo build --release 2>&1 | grep -v "warning" || true && \
    rm -rf src

# Copy actual source code
COPY src ./src

# Build release binary — statically linked against musl + openssl, zero glibc deps
RUN OPENSSL_STATIC=1 cargo build --release

# Copy binary to well-known location
RUN cp /build/target/release/llm-over-dns /build/llm-over-dns-binary


# Stage 2: Runtime
# Alpine is ~5MB and ships up-to-date CA certificates
FROM docker.io/library/alpine:3

# Install CA certificates for HTTPS requests to LLM APIs
RUN apk add --no-cache ca-certificates

# Create non-root user for security
RUN adduser -D -u 1000 -s /sbin/nologin llm

WORKDIR /app

# Copy static binary from builder stage
COPY --from=builder /build/llm-over-dns-binary /app/llm-over-dns

# Set proper permissions
RUN chmod +x /app/llm-over-dns && \
    chown llm:llm /app/llm-over-dns

# Create directory for runtime data
RUN mkdir -p /app/data && \
    chown llm:llm /app/data

# Expose DNS port (non-privileged; host iptables maps 53->5353)
EXPOSE 5353/udp

# Run as non-root
USER llm

ENTRYPOINT ["/app/llm-over-dns"]
