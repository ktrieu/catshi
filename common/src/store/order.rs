use sqlx::query_as;
use trait_variant::make;

use crate::{currency::Currency, store::DbExecutor};

#[derive(Debug, sqlx::Type, Clone, PartialEq, Eq)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
pub enum OrderDirection {
    Buy,
    Sell,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateOrder {
    pub direction: OrderDirection,
    pub quantity: i64,
    pub shares_price: Currency,
    pub fees: Currency,
    pub cost_basis: Currency,
    pub instrument_id: i64,
    pub owner_id: i64,
}

#[make(Send)]
pub trait OrderStore {
    async fn create_order(
        &self,
        db: &mut impl DbExecutor,
        c: &CreateOrder,
    ) -> anyhow::Result<Order>;
}

pub struct DbOrderStore {}

impl OrderStore for DbOrderStore {
    async fn create_order(
        &self,
        db: &mut impl DbExecutor,
        c: &CreateOrder,
    ) -> anyhow::Result<Order> {
        // `created_at` is read back as a unix-epoch bigint (postgres stores it as
        // TIMESTAMPTZ, defaulted by `CURRENT_TIMESTAMP`) so it decodes into the
        // `Order` shape.
        let order = query_as(
            r#"
            INSERT INTO orders (
                direction,
                quantity,
                shares_price,
                fees,
                cost_basis,
                instrument_id,
                owner_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING
                CAST(id AS BIGINT) as id,
                direction,
                CAST(quantity AS BIGINT) as quantity,
                CAST(shares_price AS BIGINT) as shares_price,
                CAST(fees AS BIGINT) as fees,
                CAST(cost_basis AS BIGINT) as cost_basis,
                CAST(instrument_id AS BIGINT) as instrument_id,
                CAST(owner_id AS BIGINT) as owner_id,
                EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at
            "#,
        )
        .bind(c.direction.clone())
        .bind(c.quantity)
        .bind(c.shares_price)
        .bind(c.fees)
        .bind(c.cost_basis)
        .bind(c.instrument_id)
        .bind(c.owner_id)
        .fetch_one(db.psql())
        .await?;

        Ok(order)
    }
}
