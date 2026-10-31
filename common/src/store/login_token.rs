use std::time::Duration;

use sqlx::{query, query_as, types::Uuid};
use trait_variant::make;

use crate::store::{DbExecutor, user::DbUser};

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
#[allow(dead_code)]
pub struct LoginToken {
    pub token: Uuid,
    pub user_id: i32,
    pub expires_at: i64,
    pub used_at: Option<i64>,
    pub created_at: i64,
}

#[make(Send)]
pub trait LoginTokenStore {
    async fn create(
        &self,
        db: &mut impl DbExecutor,
        user: &DbUser,
        expires_in: Duration,
    ) -> anyhow::Result<LoginToken>;

    // Marks the token as used and returns it, or None if the token doesn't
    // exist, has expired, or was already used. This is a single statement so
    // concurrent redemptions of the same token can't both succeed.
    async fn redeem(
        &self,
        db: &mut impl DbExecutor,
        token: Uuid,
    ) -> anyhow::Result<Option<LoginToken>>;

    async fn delete_expired(&self, db: &mut impl DbExecutor) -> anyhow::Result<u64>;
}

#[derive(Debug, Clone)]
pub struct DbLoginTokenStore {}

impl LoginTokenStore for DbLoginTokenStore {
    async fn create(
        &self,
        db: &mut impl DbExecutor,
        user: &DbUser,
        expires_in: Duration,
    ) -> anyhow::Result<LoginToken> {
        let expires_in_secs = i64::try_from(expires_in.as_secs())?;

        let token = query_as!(
            LoginToken,
            r#"
            INSERT INTO login_tokens (
                user_id,
                expires_at
            )
            VALUES ($1, CURRENT_TIMESTAMP + $2::BIGINT * INTERVAL '1 second')
            RETURNING
                token,
                user_id,
                EXTRACT(EPOCH FROM expires_at)::BIGINT AS "expires_at!",
                EXTRACT(EPOCH FROM used_at)::BIGINT AS used_at,
                EXTRACT(EPOCH FROM created_at)::BIGINT AS "created_at!"
            "#,
            user.id,
            expires_in_secs,
        )
        .fetch_one(db.psql())
        .await?;

        Ok(token)
    }

    async fn redeem(
        &self,
        db: &mut impl DbExecutor,
        token: Uuid,
    ) -> anyhow::Result<Option<LoginToken>> {
        let token = query_as!(
            LoginToken,
            r#"
            UPDATE login_tokens
            SET used_at = CURRENT_TIMESTAMP
            WHERE
                token = $1
                AND used_at IS NULL
                AND expires_at > CURRENT_TIMESTAMP
            RETURNING
                token,
                user_id,
                EXTRACT(EPOCH FROM expires_at)::BIGINT AS "expires_at!",
                EXTRACT(EPOCH FROM used_at)::BIGINT AS used_at,
                EXTRACT(EPOCH FROM created_at)::BIGINT AS "created_at!"
            "#,
            token,
        )
        .fetch_optional(db.psql())
        .await?;

        Ok(token)
    }

    async fn delete_expired(&self, db: &mut impl DbExecutor) -> anyhow::Result<u64> {
        let result = query!(r#"DELETE FROM login_tokens WHERE expires_at <= CURRENT_TIMESTAMP"#)
            .execute(db.psql())
            .await?;

        Ok(result.rows_affected())
    }
}
