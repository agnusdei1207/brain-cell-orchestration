FROM rust:1.90-bookworm AS builder

WORKDIR /workspace

COPY Cargo.toml ./
COPY apps ./apps
COPY crates ./crates

RUN cargo build --release --bin bco

FROM debian:bookworm-slim

RUN useradd --create-home --shell /bin/bash app

WORKDIR /app

COPY --from=builder /workspace/target/release/bco /usr/local/bin/bco

USER app

ENTRYPOINT ["bco"]

