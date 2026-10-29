-- Single-use tokens minted by the bot's login command and embedded in a
-- magic link. The frontend exchanges a token for a session, setting used_at.
-- Tokens are short-lived, so they're stored as-is rather than hashed.
CREATE TABLE login_tokens (
    token UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id INTEGER REFERENCES users(id) ON DELETE CASCADE NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX login_tokens_user_id_idx ON login_tokens(user_id);
