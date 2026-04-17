use sqlx::{SqliteConnection, query_as};

use crate::currency::Currency;

#[derive(Debug, sqlx::Type, Clone)]
#[sqlx(rename_all = "lowercase")]
pub enum OrderDirection {
    Buy,
    Sell,
}

#[derive(Debug, sqlx::FromRow)]
#[allow(dead_code)]
pub struct Order {
    pub id: i64,
    pub direction: OrderDirection,
    pub quantity: i64,
    pub shares_price: Currency,
    pub fees: Currency,
    // Same as shares_price + fees for buys but based on position cost basis for sells.
    // Allows us to calculate the profit on a sell.
    pub cost_basis: Currency,
    pub instrument_id: i64,
    pub owner_id: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone)]
pub struct CreateOrder {
    pub direction: OrderDirection,
    pub quantity: i64,
    pub shares_price: Currency,
    pub fees: Currency,
    pub cost_basis: Currency,
    pub instrument_id: i64,
    pub owner_id: i64,
}

pub async fn create_order(exec: &mut SqliteConnection, c: &CreateOrder) -> anyhow::Result<Order> {
    let order = query_as!(
        Order,
        r#"
            INSERT INTO orders (
                direction,
                quantity,
                shares_price,
                fees,
                cost_basis,
                instrument_id,
                owner_id
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING
                id,
                direction as "direction: OrderDirection",
                quantity,
                shares_price as "shares_price: Currency",
                fees as "fees: Currency",
                cost_basis as "cost_basis: Currency",
                instrument_id,
                owner_id,
                created_at
        "#,
        c.direction,
        c.quantity,
        c.shares_price,
        c.fees,
        c.cost_basis,
        c.instrument_id,
        c.owner_id
    )
    .fetch_one(exec)
    .await?;

    Ok(order)
}
