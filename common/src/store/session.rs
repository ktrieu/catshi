use sqlx::{query, query_as, types::JsonValue};
use trait_variant::make;

use crate::store::DbExecutor;

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
#[allow(dead_code)]
pub struct Session {
    pub id: String,
    pub data: JsonValue,
    pub expires_at: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[make(Send)]
pub trait SessionStore {
    // Returns the session, or None if it doesn't exist or has expired.
    async fn get(&self, db: &mut impl DbExecutor, id: &str) -> anyhow::Result<Option<Session>>;

    // Inserts the session, overwriting the data and expiry of an existing
    // session with the same id. This matches tower-sessions' save semantics.
    async fn insert(
        &self,
        db: &mut impl DbExecutor,
        id: &str,
        data: &JsonValue,
        expires_at: i64,
    ) -> anyhow::Result<Session>;

    async fn delete(&self, db: &mut impl DbExecutor, id: &str) -> anyhow::Result<()>;
}

#[derive(Debug, Clone)]
pub struct DbSessionStore {}

impl SessionStore for DbSessionStore {
    async fn get(&self, db: &mut impl DbExecutor, id: &str) -> anyhow::Result<Option<Session>> {
        let session = query_as!(
            Session,
            r#"
            SELECT
                id,
                data,
                EXTRACT(EPOCH FROM expires_at)::BIGINT AS "expires_at!",
                EXTRACT(EPOCH FROM created_at)::BIGINT AS "created_at!",
                EXTRACT(EPOCH FROM updated_at)::BIGINT AS "updated_at!"
            FROM
                sessions
            WHERE
                id = $1 AND expires_at > CURRENT_TIMESTAMP
            "#,
            id,
        )
        .fetch_optional(db.psql())
        .await?;

        Ok(session)
    }

    async fn insert(
        &self,
        db: &mut impl DbExecutor,
        id: &str,
        data: &JsonValue,
        expires_at: i64,
    ) -> anyhow::Result<Session> {
        let session = query_as!(
            Session,
            r#"
            INSERT INTO sessions (
                id,
                data,
                expires_at
            )
            VALUES ($1, $2, TO_TIMESTAMP($3::BIGINT))
            ON CONFLICT (id) DO UPDATE SET
                data = EXCLUDED.data,
                expires_at = EXCLUDED.expires_at,
                updated_at = CURRENT_TIMESTAMP
            RETURNING
                id,
                data,
                EXTRACT(EPOCH FROM expires_at)::BIGINT AS "expires_at!",
                EXTRACT(EPOCH FROM created_at)::BIGINT AS "created_at!",
                EXTRACT(EPOCH FROM updated_at)::BIGINT AS "updated_at!"
            "#,
            id,
            data,
            expires_at,
        )
        .fetch_one(db.psql())
        .await?;

        Ok(session)
    }

    async fn delete(&self, db: &mut impl DbExecutor, id: &str) -> anyhow::Result<()> {
        query!(r#"DELETE FROM sessions WHERE id = $1"#, id)
            .execute(db.psql())
            .await?;

        Ok(())
    }
}
