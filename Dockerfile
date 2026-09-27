# Stage 1: Builder
# rust:alpine uses musl as default target — produces a fully static binary with zero glibc deps.
# No need to download additional targets or musl-tools.
FROM docker.io/library/rust:alpine AS builder

# Install build dependencies (musl-dev is pre-installed, need openssl dev headers)
RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static pkgconfig perl make

WORKDIR /build

# Copy manifest files
COPY Cargo.toml Cargo.lock ./

# Create dummy sources to cache dependencies separately from source.
# The crate declares BOTH a lib (src/lib.rs) and a bin (src/main.rs) target, so
# a dummy bin alone makes cargo abort before compiling anything. The build is
# deliberately not piped through `grep ... || true`: a silent failure here means
# an empty cache layer and a full dependency rebuild on every image build.
RUN mkdir -p src && \
    echo "fn main() {}" > src/main.rs && \
    echo "" > src/lib.rs && \
    OPENSSL_STATIC=1 cargo build --release && \
    rm -rf src

# Copy actual source code
COPY src ./src

# Cargo keys its rebuild decision on file mtimes. The dummy sources above were
# created during this build, so their mtimes are newer than the ones COPY
# restored from the build context — cargo would consider the crate unchanged,
# keep the dummy artifacts, and ship a stub binary that starts and exits at
# once. Touching the crate roots marks the real sources as newer.
RUN find src -name '*.rs' -exec touch {} +

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
