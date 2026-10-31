use sqlx::{query, query_as};
use trait_variant::make;

use crate::{
    currency::Currency,
    store::{DbExecutor, instrument::Instrument, user::DbUser},
};

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
#[allow(dead_code)]
pub struct Position {
    pub id: i32,
    pub quantity: i64,
    pub cost_basis: Currency,
    pub instrument_id: i32,
    pub owner_id: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatePosition {
    pub quantity: i64,
    pub cost_basis: Currency,
    pub instrument_id: i32,
    pub owner_id: i32,
}

#[derive(Debug, PartialEq, Eq)]
pub struct PositionWithUser {
    pub position: Position,
    pub user: DbUser,
}

#[derive(Debug, PartialEq, Eq)]
pub struct PositionWithMarketId {
    pub position: Position,
    pub market_id: i32,
}

#[make(Send)]
pub trait PositionStore {
    async fn get_user_position(
        &self,
        db: &mut impl DbExecutor,
        instrument: &Instrument,
        owner: &DbUser,
    ) -> anyhow::Result<Option<Position>>;

    async fn upsert_position(
        &self,
        db: &mut impl DbExecutor,
        c: &CreatePosition,
    ) -> anyhow::Result<Position>;

    async fn delete_position(
        &self,
        db: &mut impl DbExecutor,
        instrument_id: i32,
        owner_id: i32,
    ) -> anyhow::Result<()>;

    async fn get_all_market_positions(
        &self,
        db: &mut impl DbExecutor,
        market_id: i32,
    ) -> anyhow::Result<Vec<PositionWithUser>>;

    async fn get_all_positions_with_market_id(
        &self,
        db: &mut impl DbExecutor,
    ) -> anyhow::Result<Vec<PositionWithMarketId>>;
}

#[derive(Debug, Clone)]
pub struct DbPositionStore {}

impl PositionStore for DbPositionStore {
    async fn get_user_position(
        &self,
        db: &mut impl DbExecutor,
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
            owner.id,
        )
        .fetch_optional(db.psql())
        .await?;

        Ok(position)
    }

    async fn upsert_position(
        &self,
        db: &mut impl DbExecutor,
        c: &CreatePosition,
    ) -> anyhow::Result<Position> {
        // We have a unique index on (instrument_id, owner_id). Use a CONFLICT clause.
        let position = query_as!(
            Position,
            r#"
            INSERT INTO positions (
                quantity,
                cost_basis,
                instrument_id,
                owner_id
            )
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (instrument_id, owner_id) DO UPDATE
            SET quantity = excluded.quantity, cost_basis = excluded.cost_basis
            RETURNING
                id,
                quantity,
                cost_basis,
                instrument_id,
                owner_id
            "#,
            c.quantity,
            i64::from(c.cost_basis),
            c.instrument_id,
            c.owner_id,
        )
        .fetch_one(db.psql())
        .await?;

        Ok(position)
    }

    async fn delete_position(
        &self,
        db: &mut impl DbExecutor,
        instrument_id: i32,
        owner_id: i32,
    ) -> anyhow::Result<()> {
        query!(
            "DELETE FROM positions WHERE instrument_id = $1 AND owner_id = $2",
            instrument_id,
            owner_id,
        )
        .execute(db.psql())
        .await?;

        Ok(())
    }

    async fn get_all_market_positions(
        &self,
        db: &mut impl DbExecutor,
        market_id: i32,
    ) -> anyhow::Result<Vec<PositionWithUser>> {
        let rows = query_as!(
            PgPositionWithUserRow,
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
                instruments ON instruments.id = positions.instrument_id
            JOIN
                users on users.id = positions.owner_id
            WHERE
                instruments.market_id = $1
            "#,
            market_id,
        )
        .fetch_all(db.psql())
        .await?;

        let positions = rows
            .into_iter()
            .map(PgPositionWithUserRow::into_position)
            .collect();

        Ok(positions)
    }

    async fn get_all_positions_with_market_id(
        &self,
        db: &mut impl DbExecutor,
    ) -> anyhow::Result<Vec<PositionWithMarketId>> {
        let rows = query_as!(
            PgPositionWithMarketIdRow,
            r#"
            SELECT
                positions.id,
                positions.quantity,
                positions.cost_basis,
                positions.instrument_id,
                positions.owner_id,
                instruments.market_id
            FROM positions
            JOIN
                instruments ON instruments.id = positions.instrument_id
            "#,
        )
        .fetch_all(db.psql())
        .await?;

        let positions = rows
            .into_iter()
            .map(PgPositionWithMarketIdRow::into_position)
            .collect();

        Ok(positions)
    }
}

#[derive(sqlx::FromRow)]
struct PgPositionWithUserRow {
    id: i32,
    quantity: i64,
    cost_basis: Currency,
    instrument_id: i32,
    owner_id: i32,
    users_id: i32,
    users_name: String,
    users_discord_id: String,
    users_cash_balance: Currency,
}

impl PgPositionWithUserRow {
    fn into_position(self) -> PositionWithUser {
        PositionWithUser {
            position: Position {
                id: self.id,
                quantity: self.quantity,
                cost_basis: self.cost_basis,
                instrument_id: self.instrument_id,
                owner_id: self.owner_id,
            },
            user: DbUser {
                id: self.users_id,
                discord_id: self.users_discord_id,
                name: self.users_name,
                cash_balance: self.users_cash_balance,
            },
        }
    }
}

#[derive(sqlx::FromRow)]
struct PgPositionWithMarketIdRow {
    id: i32,
    quantity: i64,
    cost_basis: Currency,
    instrument_id: i32,
    owner_id: i32,
    market_id: i32,
}

impl PgPositionWithMarketIdRow {
    fn into_position(self) -> PositionWithMarketId {
        PositionWithMarketId {
            position: Position {
                id: self.id,
                quantity: self.quantity,
                cost_basis: self.cost_basis,
                instrument_id: self.instrument_id,
                owner_id: self.owner_id,
            },
            market_id: self.market_id,
        }
    }
}
