use sqlx::{Sqlite, Transaction, query_as};

use crate::{currency::Currency, store::user};

#[derive(Debug, PartialEq, Eq, Clone, Copy, sqlx::Type)]
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
