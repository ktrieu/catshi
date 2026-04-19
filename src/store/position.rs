use sqlx::{Executor, Sqlite, SqliteConnection, query, query_as};

use crate::{
    currency::Currency,
    store::{instrument::Instrument, user::DbUser},
};

#[derive(Debug, sqlx::FromRow)]
#[allow(dead_code)]
pub struct Position {
    pub id: i64,
    pub quantity: i64,
    pub cost_basis: Currency,
    pub instrument_id: i64,
    pub owner_id: i64,
}

pub async fn get_user_position(
    exec: impl Executor<'_, Database = Sqlite>,
    instrument: &Instrument,
    owner: &DbUser,
) -> anyhow::Result<Option<Position>> {
    let position = query_as!(
        Position,
        r#"
        SELECT
            id,
            quantity,
            cost_basis,
            instrument_id,
            owner_id
        FROM positions
        WHERE
            instrument_id = $1 AND owner_id = $2
        "#,
        instrument.id,
        owner.id
    )
    .fetch_optional(exec)
    .await?;

    Ok(position)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatePosition {
    pub quantity: i64,
    pub cost_basis: Currency,
    pub instrument_id: i64,
    pub owner_id: i64,
}

pub async fn upsert_position(
    conn: &mut SqliteConnection,
    c: &CreatePosition,
) -> anyhow::Result<Position> {
    // We have a unique index on instrument and owner_id. Use a CONFLICT clause.
    let position = query_as!(
        Position,
        r#"
        INSERT INTO positions (quantity, cost_basis, instrument_id, owner_id)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT DO UPDATE 
        SET quantity = excluded.quantity, cost_basis = excluded.cost_basis
        RETURNING *
        "#,
        c.quantity,
        c.cost_basis,
        c.instrument_id,
        c.owner_id,
    )
    .fetch_one(conn)
    .await?;

    Ok(position)
}

#[derive(Debug)]
pub struct PositionWithUser {
    pub position: Position,
    pub user: DbUser,
}

pub async fn get_all_market_positions(
    exec: impl Executor<'_, Database = Sqlite>,
    market_id: i64,
) -> anyhow::Result<Vec<PositionWithUser>> {
    let positions = query!(
        r#"
        SELECT
            positions.id,
            positions.quantity,
            positions.cost_basis,
            positions.instrument_id,
            positions.owner_id,
            users.id as users_id,
            users.name as users_name,
            users.discord_id as users_discord_id,
            users.cash_balance as users_cash_balance
        FROM positions
        JOIN
            instruments ON instruments.id = instrument_id
        JOIN
            users on users.id = owner_id
        WHERE
            instruments.market_id = $1
        "#,
        market_id
    )
    .fetch_all(exec)
    .await?
    .into_iter()
    .map(|r| PositionWithUser {
        position: Position {
            id: r.id,
            quantity: r.quantity,
            cost_basis: Currency::from(r.cost_basis),
            instrument_id: r.instrument_id,
            owner_id: r.owner_id,
        },
        user: DbUser {
            id: r.users_id,
            discord_id: r.users_discord_id,
            name: r.users_name,
            cash_balance: Currency::from(r.users_cash_balance),
        },
    })
    .collect();

    Ok(positions)
}
