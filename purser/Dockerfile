# syntax=docker/dockerfile:1

FROM rust:1-bookworm AS base
WORKDIR /app

FROM base AS dev
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    cargo install cargo-watch
CMD ["cargo", "watch", "--poll", \
     "-w", "purser", "-w", "common", "-w", "Cargo.toml", "-w", "Cargo.lock", "-w", ".sqlx", \
     "-x", "run -p purser"]

FROM base AS builder
COPY . .
ENV SQLX_OFFLINE=true

RUN --mount=type=cache,target=/app/target,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    cargo build --release -p purser && cp target/release/purser /purser

FROM debian:bookworm-slim AS prod
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /purser /usr/local/bin/purser
WORKDIR /app
CMD ["purser"]
