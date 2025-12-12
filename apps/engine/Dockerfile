# Build stage
FROM debian:stable-slim AS builder

# Install build dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        build-essential \
        curl \
        ca-certificates \
        libvulkan1 \
        vulkan-tools \
        mesa-vulkan-drivers && \
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
RUN . $HOME/.cargo/env && cargo build --bin server

# Runtime stage
FROM debian:stable-slim

# Install Vulkan loader and tools for runtime GPU access
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        libvulkan1 \
        vulkan-tools \
        mesa-vulkan-drivers && \
    apt-get clean && rm -rf /var/lib/apt/lists/*

# Copy the binary from builder
COPY --from=builder /app/target/debug/server /usr/local/bin/server

# Expose the server port
EXPOSE 3000

# Run the server
CMD ["server"]
