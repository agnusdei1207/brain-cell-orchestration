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
# Runtime stage - Kali-based for CTF/offensive tooling compatibility
# =============================================================================
FROM kalilinux/kali-rolling

# ── Terminal color & Unicode support ──
ENV TERM=xterm-256color \
    COLORTERM=truecolor \
    FORCE_COLOR=3 \
    LANG=en_US.UTF-8 \
    LC_ALL=en_US.UTF-8

# Copy binary from builder
COPY --from=builder /workspace/target/release/bco /usr/local/bin/bco

WORKDIR /app

ENTRYPOINT ["bco"]

