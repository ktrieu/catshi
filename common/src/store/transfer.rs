use sqlx::query_as;
use trait_variant::make;

use crate::{
    currency::Currency,
    store::{
        CatshiTx, DbExecutor,
        user::{DbUser, UserStore},
    },
};

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
pub enum TransferSource {
    Unknown,
    Deposit,
    UserInitiated,
    Order,
    TradeFee,
    Gambling,
    MessageTip,
}

#[derive(Debug, PartialEq, Eq, sqlx::FromRow)]
#[allow(dead_code)]
pub struct Transfer {
    pub id: i64,
    pub amount: Currency,
    pub sender: i64,
    pub receiver: i64,
    pub memo: String,
    pub created_at: i64,
    pub source: TransferSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateTransfer {
    pub amount: Currency,
    pub sender: i32,
    pub receiver: i32,
    pub memo: String,
    pub source: TransferSource,
}

#[derive(Debug, PartialEq, Eq)]
pub struct UserTransfersBySource {
    pub user: DbUser,
    pub source: TransferSource,
    pub net: Currency,
}

#[derive(sqlx::Type, Debug, PartialEq, Eq, Hash)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
pub enum TransferDirection {
    Debit,
    Credit,
}

#[derive(Debug, PartialEq, Eq)]
pub struct UserTransfersBySourceAndDirection {
    pub user: DbUser,
    pub source: TransferSource,
    pub direction: TransferDirection,
    pub amount: Currency,
}

#[make(Send)]
pub trait TransferStore {
    // Because it makes three queries that must occur together it takes a transaction
    // and not a generic connection.
    async fn persist(
        &self,
        tx: &mut CatshiTx,
        user_store: &(impl UserStore + Sync),
        create: &CreateTransfer,
    ) -> anyhow::Result<Transfer>;

    async fn get_net_user_transfers_by_source(
        &self,
        db: &mut impl DbExecutor,
    ) -> anyhow::Result<Vec<UserTransfersBySource>>;

    async fn get_user_transfer_by_source_and_direction(
        &self,
        db: &mut impl DbExecutor,
    ) -> anyhow::Result<Vec<UserTransfersBySourceAndDirection>>;
}

pub struct DbTransferStore {}

impl TransferStore for DbTransferStore {
    async fn persist(
        &self,
        tx: &mut CatshiTx,
        user_store: &(impl UserStore + Sync),
        create: &CreateTransfer,
    ) -> anyhow::Result<Transfer> {
        // `created_at` is read back as a unix-epoch bigint (postgres stores it as
        // TIMESTAMPTZ, defaulted by `CURRENT_TIMESTAMP`) so it decodes into the
        // `Transfer` shape.
        let transfer: Transfer = query_as(
            r#"
            INSERT INTO transfers (
                amount,
                sender,
                receiver,
                memo,
                source
            )
            VALUES ($1, $2, $3, $4, $5)
            RETURNING
                CAST(id AS BIGINT) as id,
                CAST(amount AS BIGINT) as amount,
                CAST(sender AS BIGINT) as sender,
                CAST(receiver AS BIGINT) as receiver,
                memo,
                EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at,
                source
            "#,
        )
        .bind(create.amount)
        .bind(create.sender)
        .bind(create.receiver)
        .bind(&create.memo)
        .bind(create.source)
        .fetch_one(tx.psql())
        .await?;

        // Credit the receiving account.
        user_store
            .increment_balance_by_id(tx, create.receiver, create.amount)
            .await?;

        // Debit the sending account.
        user_store
            .increment_balance_by_id(tx, create.sender, -create.amount)
            .await?;

        Ok(transfer)
    }

    async fn get_net_user_transfers_by_source(
        &self,
        db: &mut impl DbExecutor,
    ) -> anyhow::Result<Vec<UserTransfersBySource>> {
        let rows = query_as::<_, PgUserTransfersBySourceRow>(
            r#"
            SELECT
                users.id as id,
                users.name,
                users.discord_id,
                users.cash_balance,
                source,
                SUM(net_amount) AS net
            FROM (
                SELECT
                    receiver AS user_id,
                    source,
                    amount AS net_amount
                FROM transfers
                UNION ALL
                SELECT
                    sender AS user_id,
                    source,
                    -amount AS net_amount
                FROM transfers
            ) t
            JOIN users ON users.id = t.user_id
            GROUP BY users.id, source
            ORDER BY users.id, source
            "#,
        )
        .fetch_all(db.psql())
        .await?;

        let sums: Vec<UserTransfersBySource> = rows
            .into_iter()
            .map(|r| UserTransfersBySource {
                user: DbUser {
                    id: r.id,
                    discord_id: r.discord_id,
                    name: r.name,
                    cash_balance: r.cash_balance,
                },
                source: r.source,
                net: r.net,
            })
            .collect();

        Ok(sums)
    }

    async fn get_user_transfer_by_source_and_direction(
        &self,
        db: &mut impl DbExecutor,
    ) -> anyhow::Result<Vec<UserTransfersBySourceAndDirection>> {
        let rows = query_as!(
            PgUserTransfersBySourceDirectionRow,
            r#"
            SELECT
                users.id as "id!",
                users.name,
                users.discord_id,
                users.cash_balance,
                source as "source!: TransferSource",
                direction as "direction!: TransferDirection",
                SUM(amount) as "amount!"
            FROM (
                SELECT
                    receiver AS user_id,
                    source,
                    amount,
                    'credit' as direction
                FROM transfers
                UNION ALL
                SELECT
                    sender AS user_id,
                    source,
                    -amount as amount,
                    'debit' as direction
                FROM transfers
            ) t
            JOIN users ON users.id = t.user_id
            GROUP BY users.id, source, direction
            ORDER BY users.id, source, direction
            "#,
        )
        .fetch_all(db.psql())
        .await?;

        let results = rows
            .into_iter()
            .map(|r| UserTransfersBySourceAndDirection {
                user: DbUser {
                    id: r.id,
                    discord_id: r.discord_id,
                    name: r.name,
                    cash_balance: r.cash_balance,
                },
                direction: r.direction,
                source: r.source,
                amount: r.amount,
            })
            .collect();

        Ok(results)
    }
}

#[derive(sqlx::FromRow)]
struct PgUserTransfersBySourceRow {
    id: i32,
    name: String,
    discord_id: String,
    cash_balance: Currency,
    source: TransferSource,
    net: Currency,
}

#[derive(sqlx::FromRow)]
struct PgUserTransfersBySourceDirectionRow {
    id: i32,
    name: String,
    discord_id: String,
    cash_balance: Currency,
    source: TransferSource,
    direction: TransferDirection,
    amount: Currency,
}
