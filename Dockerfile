# Build Stage
FROM rust:1.83-slim-bookworm AS builder

WORKDIR /usr/src/app
COPY . .

# Install build dependencies
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Build optimized binary
RUN cargo build --release

# Runtime Stage
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies (OpenSSL)
RUN apt-get update && apt-get install -y libssl3 ca-certificates && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /usr/src/app/target/release/solana-epoch-roll-rs /usr/local/bin/solana-epoch-roll

# Run the monitor
CMD ["solana-epoch-roll"]
