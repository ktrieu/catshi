# syntax=docker/dockerfile:1

# Build context is the repo root (this is a Cargo workspace: api/bot/common
# share one Cargo.lock, so the workspace manifest needs all three members).
# WORKDIR is /app throughout so it matches where docker-compose.dev.yml
# bind-mounts the workspace source at runtime. Env vars (POSTGRES_URL,
# GUILD_ID, DISCORD_TOKEN) come from docker-compose's `environment:` block
# rather than a mounted .env file - dotenvy::dotenv() is a no-op when none
# is found.
FROM rust:1-bookworm AS base
WORKDIR /app

# dev: no source baked in - docker-compose.dev.yml bind-mounts api/bot/common
# and the workspace manifests here at runtime; cargo-watch recompiles on
# change, using whatever's already in this container's own writable layer
# for target/registry/git (BuildKit --mount=type=cache above only applies
# during `docker build`, not to code invoked from CMD at container runtime -
# so those caches don't carry over between container recreations here).
# --poll: bind-mounted files edited from a Windows host often don't emit
# the inotify events notify/cargo-watch rely on by default, so file changes
# get silently missed without forcing polling instead.
FROM base AS dev
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    cargo install cargo-watch
CMD ["cargo", "watch", "--poll", \
     "-w", "api", "-w", "bot", "-w", "common", "-w", "Cargo.toml", "-w", "Cargo.lock", "-w", ".sqlx", \
     "-x", "run -p bot"]

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
# The sqlx query!/query_as! macros normally verify queries against a live
# DB at compile time; SQLX_OFFLINE makes them read the checked-in .sqlx
# cache instead, since there's no database in the build context.
ENV SQLX_OFFLINE=true
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release -p bot --recipe-path recipe.json

COPY . .
RUN cargo build --release -p bot && cp target/release/bot /bot

FROM debian:bookworm-slim AS prod
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /bot /usr/local/bin/bot
WORKDIR /app
CMD ["bot"]
