# syntax=docker/dockerfile:1

# Build context is the repo root (this is a Cargo workspace: api/bot/common
# share one Cargo.lock, so the workspace manifest needs all three members).
FROM rust:1-bookworm AS builder
WORKDIR /build
COPY . .
# The sqlx query!/query_as! macros normally verify queries against a live
# DB at compile time; SQLX_OFFLINE makes them read the checked-in .sqlx
# cache instead, since there's no database in the build context.
ENV SQLX_OFFLINE=true

# sharing=locked: api and bot share these cache paths and build concurrently,
# so this serializes access instead of racing on cargo's own lock files.
RUN --mount=type=cache,target=/build/target,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    cargo build --release -p api && cp target/release/api /api

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /api /usr/local/bin/api

# Working directory doubles as where dotenvy looks for .env and where the
# relative sqlite:catshi.sqlite DATABASE_URL resolves - both are mounted in
# via docker-compose.
WORKDIR /app
CMD ["api"]
