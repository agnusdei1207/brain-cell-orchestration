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

# Avoid prompts during installation
ENV DEBIAN_FRONTEND=noninteractive

# ── Terminal color & Unicode support ──
ENV TERM=xterm-256color \
    COLORTERM=truecolor \
    FORCE_COLOR=3 \
    LANG=en_US.UTF-8 \
    LC_ALL=en_US.UTF-8

# Kali GPG keyring setup for authenticated package installation
RUN echo "deb [signed-by=/usr/share/keyrings/kali-archive-keyring.gpg] http://kali.download/kali kali-rolling main contrib non-free non-free-firmware" > /etc/apt/sources.list && \
    apt-get update -qq && \
    apt-get install -y --no-install-recommends ca-certificates curl gnupg dirmngr gpg-agent && \
    curl -fsSL https://archive.kali.org/archive-keyring.gpg | gpg --dearmor > /usr/share/keyrings/kali-archive-keyring.gpg && \
    apt-get update -qq

# Core runtime tools - minimal for bco orchestration
RUN apt-get install -y --no-install-recommends \
    bash \
    build-essential \
    curl \
    git \
    jq \
    libssl-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user for security
RUN useradd --create-home --shell /bin/bash app

WORKDIR /app

# Copy binary from builder
COPY --from=builder /workspace/target/release/bco /usr/local/bin/bco

USER app

ENTRYPOINT ["bco"]

