# Multi-stage build for Numo Engine Arbitrage Bot
# Stage 1: Build
FROM rust:1.75-slim as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./
COPY bin ./bin
COPY crates ./crates

# Build for release
RUN cargo build --release -p numo

# Stage 2: Runtime
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 -s /bin/bash numo

# Create app directory
WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/numo /usr/local/bin/numo

# Change ownership to non-root user
RUN chown -R numo:numo /app

# Switch to non-root user
USER numo

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD pgrep numo || exit 1

# Set environment variables
ENV RUST_LOG=info
ENV RUST_BACKTRACE=1

# Run the bot
ENTRYPOINT ["numo"]
CMD ["--help"]
