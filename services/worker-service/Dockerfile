# Factor V: Build, Release, Run - Strict separation of stages
# Stage 1: Builder
FROM rust:1.93-bookworm AS builder

WORKDIR /app

# Copy workspace manifests
COPY Cargo.toml Cargo.lock ./
COPY crates/platform/Cargo.toml crates/platform/
COPY services/api-service/Cargo.toml services/api-service/
COPY services/worker-service/Cargo.toml services/worker-service/

# Create dummy source trees for dependency caching
RUN mkdir -p crates/platform/src services/api-service/src services/worker-service/src && \
    echo "pub fn dummy() {}" > crates/platform/src/lib.rs && \
    echo "fn main() {}" > services/api-service/src/main.rs && \
    echo "fn main() {}" > services/worker-service/src/main.rs

RUN cargo build --release -p worker-service

# Copy actual source code
COPY crates/ crates/
COPY services/worker-service/ services/worker-service/
COPY migrations/ migrations/

# Touch main files to invalidate the dummy build cache
RUN touch crates/platform/src/lib.rs services/worker-service/src/main.rs && \
    cargo build --release -p worker-service

# Stage 2: Runtime Image (Minimal footprint, non-root user, immutable)
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Run as non-root user
RUN groupadd -g 10001 appgroup && \
    useradd -u 10001 -g appgroup -s /bin/sh appuser

WORKDIR /home/appuser

COPY --from=builder /app/target/release/worker-service /usr/local/bin/worker-service

USER appuser

ENV WORKER_HEALTH_PORT=8081 \
    LOG_FORMAT=json

EXPOSE 8081

ENTRYPOINT ["/usr/local/bin/worker-service"]
