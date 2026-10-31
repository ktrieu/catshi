# catshi

Cargo workspace: `api` (axum HTTP API), `bot` (Discord bot), `common` (shared
store, currency, trade and portfolio logic), plus a `frontend`.

## Testing authenticated API endpoints locally

Real login goes through a Discord-issued single-use token, which is awkward for
automated testing. In the dev stack (`docker-compose.dev.yml`) you can instead
act as any user by sending an `X-Dev-User-Id` header with the user's integer
`users.id`:

```bash
curl -H "X-Dev-User-Id: 1" http://localhost:$API_PORT/auth/me/
curl -H "X-Dev-User-Id: 1" http://localhost:$API_PORT/portfolio/
```

No cookie or session is needed, and it works for every endpoint that uses the
`CurrentUser` extractor. A non-integer header value returns 400; an unknown user
id returns 500 (user lookup fails).

How it's kept out of production (see `api/src/auth.rs`):

- The code is behind `#[cfg(debug_assertions)]`, so release builds don't contain
  it. The prod image builds with `cargo build --release` (`api/Dockerfile`);
  keep it that way.
- It is also inert unless the `DEV_AUTH` env var is set, which only
  `docker-compose.dev.yml` does. Never add `DEV_AUTH` to the base or prod
  compose files.

The API logs a warning at startup when the bypass is active. If a request with
the header gets 403 instead, the API isn't running with `DEV_AUTH` (restart the
dev container after changing compose env).

## Code style

Don't write code comments unless explicitly asked to.
