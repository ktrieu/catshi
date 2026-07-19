# syntax=docker/dockerfile:1

# Build context is the repo root (this is a Cargo workspace: api/bot/common
# share one Cargo.lock, so the workspace manifest needs all three members).
# WORKDIR is /app throughout so it matches where docker-compose mounts the
# source (dev) or the sqlite/.env files (prod), and where dotenvy and the
# relative sqlite:catshi.sqlite DATABASE_URL both resolve at runtime.
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
CMD ["cargo", "watch", "--poll", "-x", "run -p bot"]

FROM base AS builder
COPY . .
# The sqlx query!/query_as! macros normally verify queries against a live
# DB at compile time; SQLX_OFFLINE makes them read the checked-in .sqlx
# cache instead, since there's no database in the build context.
ENV SQLX_OFFLINE=true

# sharing=locked: api and bot build concurrently (e.g. in CI) and share
# these cache paths, so this serializes access instead of racing on
# cargo's own lock files.
RUN --mount=type=cache,target=/app/target,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    cargo build --release -p bot && cp target/release/bot /bot

FROM debian:bookworm-slim AS prod
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /bot /usr/local/bin/bot
WORKDIR /app
CMD ["bot"]
