# =============================================================================
# Builder stage - compiles the bco binary
# =============================================================================
FROM rust:1.94-bookworm AS builder

WORKDIR /workspace

COPY Cargo.toml ./
COPY Cargo.lock ./
COPY apps ./apps
COPY crates ./crates

RUN cargo build --release --bin bco

# =============================================================================
# Runtime stage - uses the pentesting-style Kali base
# =============================================================================
FROM agnusdei1207/pentesting-base:latest

# Copy binary from builder
COPY --from=builder /workspace/target/release/bco /usr/local/bin/bco

WORKDIR /root
VOLUME /tmp/.bco

ENTRYPOINT ["docker-entrypoint.sh", "bco"]
