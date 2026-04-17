use sqlx::{Executor, Sqlite, query_as};

use crate::currency::Currency;

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

pub struct CreateTransfer {
    pub amount: Currency,
    pub sender: i64,
    pub receiver: i64,
    pub memo: String,
}

pub async fn insert_transfer(
    exec: impl Executor<'_, Database = Sqlite>,
    create: CreateTransfer,
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
