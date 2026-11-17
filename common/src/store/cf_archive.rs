use serenity::all::MessageId;
use sqlx::{query, query_as, types::JsonValue};
use trait_variant::make;

use crate::store::{CatshiTx, DbExecutor, user::DbUser};

#[derive(Debug, Clone, PartialEq)]
pub struct CfArchiveEntry {
    pub id: i32,
    pub game_number: i32,
    pub message_id: String,
    pub results: JsonValue,
    pub posted_at: i64,
    pub user_ids: Vec<i32>,
}

#[make(Send)]
pub trait CfArchiveStore {
    async fn insert(
        &self,
        tx: &mut CatshiTx,
        game_number: i32,
        message_id: MessageId,
        results: &JsonValue,
        posted_at: i64,
        users: &[DbUser],
    ) -> anyhow::Result<CfArchiveEntry>;
}

#[derive(Debug, Clone)]
pub struct DbCfArchiveStore {}

struct ArchiveRow {
    id: i32,
    game_number: i32,
    message_id: String,
    results: JsonValue,
    posted_at: i64,
}

impl CfArchiveStore for DbCfArchiveStore {
    async fn insert(
        &self,
        tx: &mut CatshiTx,
        game_number: i32,
        message_id: MessageId,
        results: &JsonValue,
        posted_at: i64,
        users: &[DbUser],
    ) -> anyhow::Result<CfArchiveEntry> {
        let message_id = message_id.to_string();

        let row = query_as!(
            ArchiveRow,
            r#"
            INSERT INTO cf_archive (
                game_number,
                message_id,
                results,
                posted_at
            )
            VALUES ($1, $2, $3, to_timestamp($4))
            RETURNING
                id,
                game_number,
                message_id,
                results,
                EXTRACT(EPOCH FROM posted_at)::BIGINT AS "posted_at!"
            "#,
            game_number,
            message_id,
            results,
            posted_at as f64,
        )
        .fetch_one(tx.psql())
        .await?;

        let user_ids: Vec<i32> = users.iter().map(|u| u.id).collect();

        query!(
            r#"
            INSERT INTO cf_archive_users (archive_id, user_id)
            SELECT $1, user_id FROM UNNEST($2::INT[]) AS user_id
            "#,
            row.id,
            &user_ids,
        )
        .execute(tx.psql())
        .await?;

        Ok(CfArchiveEntry {
            id: row.id,
            game_number: row.game_number,
            message_id: row.message_id,
            results: row.results,
            posted_at: row.posted_at,
            user_ids,
        })
    }
}
