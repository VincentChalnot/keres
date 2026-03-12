# Build stage

# Build stage
FROM debian:stable-slim AS builder

# Install build dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        build-essential \
        musl-tools \
        curl \
        ca-certificates && \
    curl https://sh.rustup.rs -sSf | sh -s -- -y && \
    . $HOME/.cargo/env && \
    rustup target add x86_64-unknown-linux-musl && \
    apt-get clean && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy the Cargo files
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY ./src ./src


# Build the server binary for musl (static linking)
RUN . $HOME/.cargo/env && \
    CC=musl-gcc cargo build --release --target x86_64-unknown-linux-musl


# Runtime stage: use scratch for a true distroless image
FROM scratch

# Copy the statically linked server binary from builder
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/server /server

# Set entrypoint to the server binary
ENTRYPOINT ["/server"]
