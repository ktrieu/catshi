use sqlx::query_as;
use trait_variant::make;

use crate::{
    currency::Currency,
    store::{DbExecutor, log_pg_compare_result},
};

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
        .fetch_one(db.sqlite())
        .await?;

        // Mirror the insert to postgres, forcing `id` to match the sqlite row in case a
        // future table ends up referencing `orders(id)` by foreign key. `created_at` is
        // cast back to a unix-epoch bigint (postgres stores it as TIMESTAMPTZ, defaulted
        // independently by `CURRENT_TIMESTAMP` rather than passed in) so it decodes into
        // the same `Order` shape as the sqlite row for comparison; the two timestamps can
        // occasionally differ by a second since they're set by separate statements, not
        // copied from one write to the other.
        let pg_result: sqlx::Result<Order> = query_as(
            r#"
            INSERT INTO orders (
                id,
                direction,
                quantity,
                shares_price,
                fees,
                cost_basis,
                instrument_id,
                owner_id
            )
            OVERRIDING SYSTEM VALUE
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
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
        .bind(order.id)
        .bind(c.direction.clone())
        .bind(c.quantity)
        .bind(c.shares_price)
        .bind(c.fees)
        .bind(c.cost_basis)
        .bind(c.instrument_id)
        .bind(c.owner_id)
        .fetch_one(db.psql())
        .await;

        log_pg_compare_result(pg_result, &order, "order create_order");

        Ok(order)
    }
}
