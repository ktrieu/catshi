use sqlx::{Executor, Sqlite, Transaction, query_as};

use crate::{currency::Currency, store::user};

#[derive(Debug, sqlx::FromRow)]
#[allow(dead_code)]
pub struct Transfer {
    pub id: i64,
    pub amount: Currency,
    pub sender: i64,
    pub receiver: i64,
    pub memo: String,
    pub created_at: i64,
}

#[derive(Debug, Clone)]
pub struct CreateTransfer {
    pub amount: Currency,
    pub sender: i64,
    pub receiver: i64,
    pub memo: String,
}

pub async fn insert_transfer(
    exec: impl Executor<'_, Database = Sqlite>,
    create: &CreateTransfer,
) -> anyhow::Result<Transfer> {
    let transfer = query_as!(
        Transfer,
        r#"
        INSERT INTO transfers (
            amount,
            sender,
            receiver,
            memo
        ) VALUES ($1, $2, $3, $4)
        RETURNING
            id,
            amount as "amount: Currency",
            sender,
            receiver,
            memo,
            created_at
        "#,
        create.amount,
        create.sender,
        create.receiver,
        create.memo,
    )
    .fetch_one(exec)
    .await?;

    Ok(transfer)
}

// Save a transaction row and make the necessary balance updates.
// Because it makes three queries that must occur together it takes a transaction and not a generic Connection.
pub async fn persist_transfer(
    tx: &mut Transaction<'_, Sqlite>,
    create: &CreateTransfer,
) -> anyhow::Result<Transfer> {
    let transfer = insert_transfer(&mut **tx, create).await?;

    // Credit the receiving account.
    user::increment_balance_by_user_id(&mut **tx, create.receiver, create.amount).await?;

    // Debit the sending account.
    user::increment_balance_by_user_id(&mut **tx, create.sender, -create.amount).await?;

    Ok(transfer)
}
