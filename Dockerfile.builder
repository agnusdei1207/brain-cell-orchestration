FROM rust:1.94-bookworm

WORKDIR /workspace

# Install cargo-watch for dev mode
RUN cargo install cargo-watch

# Copy workspace files
COPY Cargo.toml ./
COPY Cargo.lock ./
COPY apps ./apps
COPY crates ./crates

# Pre-build dependencies
RUN cargo build --release --bin bco

ENTRYPOINT ["cargo", "watch", "-x", "build"]
