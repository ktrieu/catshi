use sqlx::{Executor, Sqlite, Transaction, query, query_as};

use crate::{
    currency::Currency,
    store::user::{self, DbUser},
};

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]
pub enum TransferSource {
    Unknown,
    Deposit,
    UserInitiated,
    Order,
    TradeFee,
}

#[derive(Debug, sqlx::FromRow)]
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
    pub sender: i64,
    pub receiver: i64,
    pub memo: String,
    pub source: TransferSource,
}

// Save a transaction row and make the necessary balance updates.
// Because it makes three queries that must occur together it takes a transaction and not a generic Connection.
pub async fn persist_transfer(
    tx: &mut Transaction<'_, Sqlite>,
    create: &CreateTransfer,
) -> anyhow::Result<Transfer> {
    let transfer = query_as!(
        Transfer,
        r#"
        INSERT INTO transfers (
            amount,
            sender,
            receiver,
            memo,
            source
        ) VALUES ($1, $2, $3, $4, $5)
        RETURNING
            id,
            amount as "amount: Currency",
            sender,
            receiver,
            memo,
            created_at,
            source as "source: TransferSource"
        "#,
        create.amount,
        create.sender,
        create.receiver,
        create.memo,
        create.source,
    )
    .fetch_one(&mut **tx)
    .await?;

    // Credit the receiving account.
    user::increment_balance_by_user_id(&mut **tx, create.receiver, create.amount).await?;

    // Debit the sending account.
    user::increment_balance_by_user_id(&mut **tx, create.sender, -create.amount).await?;

    Ok(transfer)
}

pub struct UserTransfersBySource {
    pub user: DbUser,
    pub source: TransferSource,
    pub net: Currency,
}

pub async fn get_net_user_transfers_by_source(
    exec: impl Executor<'_, Database = Sqlite>,
) -> anyhow::Result<Vec<UserTransfersBySource>> {
    let rows = query!(
        r#"
        SELECT
            users.id,
            users.name,
            users.discord_id,
            users.cash_balance as "cash_balance: Currency",
            source as "source: TransferSource",
            SUM(net_amount) AS "net: Currency"
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
        JOIN users ON users.id = user_id
        GROUP BY user_id, source
        "#,
    )
    .fetch_all(exec)
    .await?;

    let sums = rows
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
