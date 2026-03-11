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
RUN . $HOME/.cargo/env && cargo build --bin server --release

# Runtime stage

# Use distroless/cc for minimal runtime with glibc
FROM gcr.io/distroless/cc

# Copy the server binary from builder
COPY --from=builder /app/target/release/server /server

# Set entrypoint to the server binary
ENTRYPOINT ["/server"]
