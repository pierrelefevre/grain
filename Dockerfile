# Build stage - using bookworm (Debian 12) which is more current
# Note: The builder image may show vulnerabilities, but these are not present in the final runtime image
# since we use a multi-stage build with distroless runtime (only 79.6MB, minimal attack surface)
FROM rust:1.91.1-trixie AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests first to cache dependencies layer
COPY Cargo.toml ./

# Create a dummy main.rs to build dependencies
RUN mkdir -p src/bin && \
    echo "fn main() {}" > src/main.rs && \
    echo "fn main() {}" > src/bin/grainctl.rs && \
    cargo build --release && \
    rm -rf src target/release/grain target/release/grainctl

# Copy source code
COPY src ./src

# Build release binary (both grain and grainctl)
# This layer will only rebuild if source code changes
RUN cargo build --release

# Runtime stage - use Google's distroless image for minimal attack surface
FROM gcr.io/distroless/cc-debian12:nonroot

WORKDIR /app

# Copy binaries from builder
COPY --from=builder /app/target/release/grain /app/grain
COPY --from=builder /app/target/release/grainctl /app/grainctl

# Expose registry port
EXPOSE 8888

# Set default environment variables
ENV RUST_LOG=info

# Default command (distroless already runs as nonroot user)
CMD ["/app/grain", "--host", "0.0.0.0:8888", "--users-file", "/data/users.json"]
