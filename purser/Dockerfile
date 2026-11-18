# syntax=docker/dockerfile:1

FROM rust:1-bookworm AS base
WORKDIR /app

FROM base AS dev
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    cargo install cargo-watch
CMD ["cargo", "watch", "--poll", \
     "-w", "purser", "-w", "common", "-w", "Cargo.toml", "-w", "Cargo.lock", "-w", ".sqlx", \
     "-x", "run -p purser"]

FROM base AS chef
ARG TARGETARCH
RUN set -eux; \
    case "$TARGETARCH" in \
      amd64) arch=x86_64; sha=70ef940ef90d04d122f0176fdb8d6c39069191b484a1eaa29b327370c2e1c3c0 ;; \
      arm64) arch=aarch64; sha=a47e13fba89c2895f5a5c3d0844acd2a5fd416eceb3a6f9dfb26e28155099f4e ;; \
      *) echo "unsupported arch: $TARGETARCH" >&2; exit 1 ;; \
    esac; \
    curl -fsSL -o /tmp/cargo-chef.tar.xz \
      "https://github.com/LukeMathWalker/cargo-chef/releases/download/v0.1.78/cargo-chef-${arch}-unknown-linux-gnu.tar.xz"; \
    echo "${sha}  /tmp/cargo-chef.tar.xz" | sha256sum -c -; \
    tar -xJf /tmp/cargo-chef.tar.xz -C /usr/local/cargo/bin --strip-components=1 \
      "cargo-chef-${arch}-unknown-linux-gnu/cargo-chef"; \
    rm /tmp/cargo-chef.tar.xz

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
ENV SQLX_OFFLINE=true
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release -p purser --recipe-path recipe.json

COPY . .
RUN cargo build --release -p purser && cp target/release/purser /purser

FROM debian:bookworm-slim AS prod
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /purser /usr/local/bin/purser
WORKDIR /app
CMD ["purser"]
