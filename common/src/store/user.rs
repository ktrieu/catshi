use anyhow::{anyhow, bail};
use serenity::all::UserId;
use sqlx::{SqliteConnection, query, query_as};
use trait_variant::make;

use crate::{
    currency::Currency,
    store::{DbExecutor, log_pg_compare_result, log_pg_write_err},
};

#[derive(Debug, sqlx::FromRow, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub struct DbUser {
    pub id: i64,
    pub discord_id: String,
    pub name: String,
    pub cash_balance: Currency,
}

pub async fn increment_balance_by_user_id(
    conn: &mut SqliteConnection,
    id: i64,
    amount: Currency,
) -> anyhow::Result<()> {
    let result = query!(
        r#"UPDATE users SET cash_balance = cash_balance + $1 WHERE id = $2"#,
        amount,
        id,
    )
    .execute(conn)
    .await?;

    if result.rows_affected() != 1 {
        bail!("no rows written for increment_balance_by_user_id")
    }

    Ok(())
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
    async fn get_by_id(&self, db: &mut impl DbExecutor, id: i64) -> anyhow::Result<DbUser>;
    async fn get_system_user(&self, db: &mut impl DbExecutor) -> anyhow::Result<DbUser>;
    async fn increment_balance_by_id(
        &self,
        db: &mut impl DbExecutor,
        id: i64,
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
        let user = query_as!(
            DbUser,
            r#"INSERT INTO users(
            discord_id,
            name,
            cash_balance
        ) VALUES ($1, $2, $3) ON CONFLICT (discord_id) DO NOTHING 
        RETURNING
            id,
            discord_id,
            name,
            cash_balance as "cash_balance: Currency"
        "#,
            c.discord_id,
            c.name,
            c.initial_balance,
        )
        .fetch_one(db.sqlite())
        .await?;

        let pg_result: sqlx::Result<DbUser> = query_as(
            r#"
            INSERT INTO users (
                id,
                discord_id,
                name,
                cash_balance
            ) 
            OVERRIDING SYSTEM VALUE
            VALUES ($1, $2, $3, $4) ON CONFLICT (discord_id) DO NOTHING 
            RETURNING
                CAST(id AS BIGINT) as id,
                discord_id,
                name,
                cash_balance
            "#,
        )
        .bind(user.id)
        .bind(c.discord_id)
        .bind(c.name)
        .bind(c.initial_balance)
        .fetch_one(db.psql())
        .await;

        log_pg_write_err(pg_result, "user create_if_not_exists");

        Ok(user)
    }

    async fn get_by_discord_id(
        &self,
        db: &mut impl DbExecutor,
        discord_id: &UserId,
    ) -> anyhow::Result<Option<DbUser>> {
        let discord_id = discord_id.to_string();

        let sqlite_user = query_as!(
            DbUser,
            r#"
            SELECT
                id,
                name,
                discord_id,
                cash_balance as "cash_balance: Currency"
            FROM users 
            WHERE 
                discord_id = $1
            "#,
            discord_id,
        )
        .fetch_optional(db.sqlite())
        .await?;

        let pg_user: sqlx::Result<Option<DbUser>> = query_as(
            r#"
            SELECT
                CAST(id AS BIGINT) as id,
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
        .await;

        log_pg_compare_result(pg_user, &sqlite_user, "get_by_discord_id");

        Ok(sqlite_user)
    }

    async fn get_by_id(&self, db: &mut impl DbExecutor, id: i64) -> anyhow::Result<DbUser> {
        let sqlite_user = query_as!(
            DbUser,
            r#"
            SELECT
                id,
                name,
                discord_id,
                cash_balance as "cash_balance: Currency"
            FROM users 
            WHERE 
                id = $1
            "#,
            id,
        )
        .fetch_one(db.sqlite())
        .await?;

        let pg_user = query_as(
            r#"
            SELECT
                CAST(id as BIGINT) as id,
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
        .await;

        log_pg_compare_result(pg_user, &sqlite_user, "get_by_id");

        Ok(sqlite_user)
    }

    async fn get_system_user(&self, db: &mut impl DbExecutor) -> anyhow::Result<DbUser> {
        self.get_by_discord_id(db, &UserId::new(0))
            .await?
            .ok_or(anyhow!("system user not found"))
    }

    async fn increment_balance_by_id(
        &self,
        db: &mut impl DbExecutor,
        id: i64,
        amount: Currency,
    ) -> anyhow::Result<()> {
        let result = query!(
            r#"UPDATE users SET cash_balance = cash_balance + $1 WHERE id = $2"#,
            amount,
            id,
        )
        .execute(db.sqlite())
        .await?;

        if result.rows_affected() != 1 {
            bail!("no rows written for increment_balance_by_user_id")
        }

        let pg_result = query(r#"UPDATE users SET cash_balance = cash_balance + $1 WHERE id = $2"#)
            .bind(amount)
            .bind(id)
            .execute(db.psql())
            .await;

        log_pg_write_err(pg_result, "increment_balance_by_id");

        Ok(())
    }
}
