# Build stage
FROM debian:stable-slim AS builder

# Install build dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        build-essential \
        curl \
        ca-certificates && \
    curl https://sh.rustup.rs -sSf | sh -s -- -y && \
    . $HOME/.cargo/env && \
    apt-get clean && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy the Cargo files
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY ./src ./src

# Build the server binary
RUN . $HOME/.cargo/env && cargo build --bin server --bin keres --release

# Runtime stage
FROM debian:stable-slim

# Copy the binary from builder
COPY --from=builder /app/target/release/server /usr/local/bin/server
COPY --from=builder /app/target/release/keres /usr/local/bin/keres

# Expose the server port
EXPOSE 3000

# Run the server
CMD ["server"]
