use anyhow::{anyhow, bail};
use serenity::all::UserId;
use sqlx::{query, query_as};
use trait_variant::make;

use crate::{currency::Currency, store::DbExecutor};

#[derive(Debug, sqlx::FromRow, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub struct DbUser {
    pub id: i32,
    pub discord_id: String,
    pub name: String,
    pub cash_balance: Currency,
}

pub struct CreateDbUser {
    pub name: String,
    pub discord_id: String,
    pub initial_balance: Currency,
}

#[make(Send)]
pub trait UserStore {
    async fn create_if_not_exists(
        &self,
        db: &mut impl DbExecutor,
        c: CreateDbUser,
    ) -> anyhow::Result<DbUser>;
    async fn get_by_discord_id(
        &self,
        db: &mut impl DbExecutor,
        discord_id: &UserId,
    ) -> anyhow::Result<Option<DbUser>>;
    async fn get_by_id(&self, db: &mut impl DbExecutor, id: i32) -> anyhow::Result<DbUser>;
    async fn get_system_user(&self, db: &mut impl DbExecutor) -> anyhow::Result<DbUser>;
    async fn increment_balance_by_id(
        &self,
        db: &mut impl DbExecutor,
        id: i32,
        amount: Currency,
    ) -> anyhow::Result<()>;
}

pub struct DbUserStore {}

impl UserStore for DbUserStore {
    async fn create_if_not_exists(
        &self,
        db: &mut impl DbExecutor,
        c: CreateDbUser,
    ) -> anyhow::Result<DbUser> {
        let user = query_as(
            r#"
            INSERT INTO users (
                discord_id,
                name,
                cash_balance
            )
            VALUES ($1, $2, $3) ON CONFLICT (discord_id) DO NOTHING
            RETURNING
                id,
                discord_id,
                name,
                cash_balance
            "#,
        )
        .bind(c.discord_id)
        .bind(c.name)
        .bind(c.initial_balance)
        .fetch_one(db.psql())
        .await?;

        Ok(user)
    }

    async fn get_by_discord_id(
        &self,
        db: &mut impl DbExecutor,
        discord_id: &UserId,
    ) -> anyhow::Result<Option<DbUser>> {
        let discord_id = discord_id.to_string();

        let user = query_as(
            r#"
            SELECT
                id,
                name,
                discord_id,
                cash_balance
            FROM users
            WHERE
                discord_id = $1
            "#,
        )
        .bind(discord_id)
        .fetch_optional(db.psql())
        .await?;

        Ok(user)
    }

    async fn get_by_id(&self, db: &mut impl DbExecutor, id: i32) -> anyhow::Result<DbUser> {
        let user = query_as(
            r#"
            SELECT
                id,
                name,
                discord_id,
                cash_balance
            FROM users
            WHERE
                id = $1
            "#,
        )
        .bind(id)
        .fetch_one(db.psql())
        .await?;

        Ok(user)
    }

    async fn get_system_user(&self, db: &mut impl DbExecutor) -> anyhow::Result<DbUser> {
        self.get_by_discord_id(db, &UserId::new(0))
            .await?
            .ok_or(anyhow!("system user not found"))
    }

    async fn increment_balance_by_id(
        &self,
        db: &mut impl DbExecutor,
        id: i32,
        amount: Currency,
    ) -> anyhow::Result<()> {
        let result = query(r#"UPDATE users SET cash_balance = cash_balance + $1 WHERE id = $2"#)
            .bind(amount)
            .bind(id)
            .execute(db.psql())
            .await?;

        if result.rows_affected() != 1 {
            bail!("no rows written for increment_balance_by_id")
        }

        Ok(())
    }
}
