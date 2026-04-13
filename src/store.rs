use anyhow::anyhow;
use serenity::all::{GenericChannelId, MessageId, UserId};
use sqlx::{Executor, QueryBuilder, Sqlite, query, query_as};

use crate::currency::Currency;

#[derive(Debug, sqlx::FromRow, Clone)]
#[allow(dead_code)]
pub struct DbUser {
    pub id: i64,
    pub discord_id: String,
    pub name: String,
    pub cash_balance: Currency,
}

pub async fn insert_user_if_not_exists(
    exec: impl Executor<'_, Database = Sqlite>,
    discord_id: &str,
    name: &str,
    initial_balance: Currency,
) -> anyhow::Result<Option<DbUser>> {
    let user = query_as!(
        DbUser,
        "INSERT INTO users(discord_id, name, cash_balance) VALUES ($1, $2, $3) ON CONFLICT (discord_id) DO NOTHING RETURNING *",
        discord_id,
        name,
        initial_balance,
    )
    .fetch_optional(exec)
    .await?;

    Ok(user)
}

pub async fn get_user_by_discord_id(
    exec: impl Executor<'_, Database = Sqlite>,
    discord_id: &UserId,
) -> anyhow::Result<Option<DbUser>> {
    let discord_id = discord_id.to_string();

    let user = query_as!(
        DbUser,
        r#"SELECT
            id,
            name,
            discord_id,
            cash_balance as "cash_balance: Currency"
        FROM users 
        WHERE 
            discord_id = $1"#,
        discord_id,
    )
    .fetch_optional(exec)
    .await?;

    Ok(user)
}

pub async fn get_user_by_id(
    exec: impl Executor<'_, Database = Sqlite>,
    id: i64,
) -> anyhow::Result<Option<DbUser>> {
    let user = query_as!(
        DbUser,
        r#"SELECT
            id,
            name,
            discord_id,
            cash_balance as "cash_balance: Currency"
        FROM users 
        WHERE 
            id = $1"#,
        id,
    )
    .fetch_optional(exec)
    .await?;

    Ok(user)
}

pub async fn get_system_user(exec: impl Executor<'_, Database = Sqlite>) -> anyhow::Result<DbUser> {
    // By convention the system user has a discord ID of 0, see the migrations.
    get_user_by_discord_id(exec, &UserId::new(0))
        .await?
        .ok_or(anyhow!("system user not found"))
}

pub async fn increment_balance(
    exec: impl Executor<'_, Database = Sqlite>,
    user: &DbUser,
    amount: Currency,
) -> anyhow::Result<()> {
    query!(
        r#"UPDATE users SET cash_balance = cash_balance + $1 WHERE id = $2"#,
        amount,
        user.id,
    )
    .execute(exec)
    .await?;

    Ok(())
}

pub async fn get_users_with_positions_in_market(
    exec: impl Executor<'_, Database = Sqlite>,
    market_id: i64,
) -> anyhow::Result<Vec<DbUser>> {
    let users = query_as!(
        DbUser,
        r#"SELECT
            DISTINCT users.id,
            users.name,
            users.discord_id,
            users.cash_balance as "cash_balance: Currency"
        FROM users 
        JOIN positions ON positions.owner_id = users.id
        JOIN instruments ON instruments.id = positions.instrument_id
        WHERE 
            instruments.market_id = $1"#,
        market_id,
    )
    .fetch_all(exec)
    .await?;

    Ok(users)
}

#[derive(Debug, sqlx::Type, Clone, Copy, PartialEq, Eq)]
#[sqlx(rename_all = "lowercase")]
pub enum MarketState {
    Open,
    Closed,
}

#[derive(Debug, sqlx::FromRow)]
#[allow(dead_code)]
pub struct Market {
    pub id: i64,
    pub description: String,
    pub state: MarketState,
    pub owner_id: i64,
    pub message_id: Option<String>,
    pub channel_id: Option<String>,
}

pub async fn create_new_market(
    exec: impl Executor<'_, Database = Sqlite>,
    description: &str,
    owner: &DbUser,
) -> anyhow::Result<Market> {
    let result = query_as!(
        Market,
        r#"
            INSERT INTO markets(
                description,
                state,
                owner_id
            ) 
            VALUES ($1, $2, $3) 
            RETURNING 
                id, 
                description, 
                state as "state: MarketState", 
                owner_id, 
                message_id,
                channel_id
        "#,
        description,
        MarketState::Open,
        owner.id
    )
    .fetch_one(exec)
    .await?;

    Ok(result)
}

pub async fn set_market_message_id(
    exec: impl Executor<'_, Database = Sqlite>,
    market_id: i64,
    message_id: MessageId,
    channel_id: GenericChannelId,
) -> anyhow::Result<()> {
    let message_id = message_id.to_string();
    let channel_id = channel_id.to_string();

    query!(
        "UPDATE markets SET message_id = $1, channel_id = $2 WHERE id = $3",
        message_id,
        channel_id,
        market_id,
    )
    .execute(exec)
    .await?;

    Ok(())
}

pub async fn get_market_by_id(
    exec: impl Executor<'_, Database = Sqlite>,
    id: i64,
) -> anyhow::Result<Option<Market>> {
    let market = query_as!(
        Market,
        r#"
        SELECT
             id, 
            description, 
            state as "state: MarketState", 
            owner_id, 
            message_id,
            channel_id
        FROM
            markets
        WHERE
            id = $1
        "#,
        id
    )
    .fetch_optional(exec)
    .await?;

    Ok(market)
}

pub async fn get_market_by_instrument_id(
    exec: impl Executor<'_, Database = Sqlite>,
    instrument_id: i64,
) -> anyhow::Result<Option<Market>> {
    let market = query_as!(
        Market,
        r#"
        SELECT
            markets.id, 
            markets.description, 
            markets.state as "state: MarketState", 
            markets.owner_id, 
            markets.message_id,
            markets.channel_id
        FROM
            markets
        JOIN
            instruments ON instruments.market_id = markets.id
        WHERE
            instruments.id = $1
        "#,
        instrument_id
    )
    .fetch_optional(exec)
    .await?;

    Ok(market)
}

pub async fn set_market_state(
    exec: impl Executor<'_, Database = Sqlite>,
    market: &Market,
    state: MarketState,
) -> anyhow::Result<()> {
    query!(
        r#"
        UPDATE
            markets
        SET
            state = $1
        WHERE
            id = $2
        "#,
        state,
        market.id,
    )
    .execute(exec)
    .await?;

    Ok(())
}

#[derive(Debug, sqlx::Type, Clone, Copy, PartialEq, Eq)]
#[sqlx(rename_all = "lowercase")]
pub enum InstrumentState {
    Open,
    Winner,
    Loser,
}

#[derive(Debug, sqlx::FromRow, Clone)]
#[allow(dead_code)]
pub struct Instrument {
    pub id: i64,
    pub name: String,
    pub state: InstrumentState,
    pub market_id: i64,
}

pub async fn insert_market_instruments(
    exec: impl Executor<'_, Database = Sqlite>,
    market: &Market,
    names: &[&str],
) -> anyhow::Result<Vec<Instrument>> {
    let mut builder = QueryBuilder::new("INSERT INTO instruments (name, state, market_id) ");

    builder.push_values(names.iter(), |mut b, name| {
        b.push_bind(name);
        b.push_bind(InstrumentState::Open);
        b.push_bind(market.id);
    });

    builder.push(" RETURNING *");

    let query = builder.build_query_as::<Instrument>();

    let rows = query.fetch_all(exec).await?;

    Ok(rows)
}

pub async fn get_instrument_by_id(
    exec: impl Executor<'_, Database = Sqlite>,
    id: i64,
) -> anyhow::Result<Option<Instrument>> {
    let instrument = query_as!(
        Instrument,
        r#"
            SELECT
                id,
                name,
                state as "state: InstrumentState",
                market_id
            FROM 
                instruments 
            WHERE id = $1
        "#,
        id
    )
    .fetch_optional(exec)
    .await?;

    Ok(instrument)
}

pub async fn set_instrument_state(
    exec: impl Executor<'_, Database = Sqlite>,
    instrument: &Instrument,
    state: InstrumentState,
) -> anyhow::Result<()> {
    query!(
        r#"
        UPDATE
            instruments
        SET 
            state = $1
        WHERE
            id = $2
        "#,
        state,
        instrument.id
    )
    .execute(exec)
    .await?;

    Ok(())
}

pub type InstrumentWithShares = (Instrument, i64);

pub async fn get_instruments_with_share_counts_for_market(
    exec: impl Executor<'_, Database = Sqlite>,
    market_id: i64,
) -> anyhow::Result<Vec<InstrumentWithShares>> {
    // Maybe one day we'll cache this data on the instrument but it seems fine for now?
    let rows = query!(
        r#"
            SELECT
                instruments.id,
                instruments.name,
                instruments.state as "state: InstrumentState",
                instruments.market_id,
                COALESCE(SUM(quantity), 0) as shares
            FROM
                instruments
            LEFT JOIN
                positions ON instruments.id = positions.instrument_id
            WHERE
                instruments.market_id = $1
            GROUP BY instruments.id
        "#,
        market_id,
    )
    .fetch_all(exec)
    .await?;

    Ok(rows
        .iter()
        .map(|r| {
            (
                Instrument {
                    id: r.id,
                    name: r.name.clone(),
                    state: r.state,
                    market_id: r.market_id,
                },
                r.shares,
            )
        })
        .collect())
}

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

pub async fn create_new_position(
    exec: impl Executor<'_, Database = Sqlite>,
    quantity: i64,
    cost_basis: Currency,
    instrument: &Instrument,
    owner: &DbUser,
) -> anyhow::Result<Position> {
    let position = query_as!(
        Position,
        r#"
            INSERT INTO positions (
                quantity,
                cost_basis,
                instrument_id,
                owner_id
            ) VALUES ($1, $2, $3, $4)
            RETURNING *
        "#,
        quantity,
        cost_basis,
        instrument.id,
        owner.id
    )
    .fetch_one(exec)
    .await?;

    Ok(position)
}

pub async fn increase_position(
    exec: impl Executor<'_, Database = Sqlite>,
    quantity: i64,
    price_paid: Currency,
    instrument: &Instrument,
    owner: &DbUser,
) -> anyhow::Result<Position> {
    let position = query_as!(
        Position,
        r#"
            UPDATE positions
            SET
                quantity = quantity + $1,
                cost_basis = cost_basis + $2
            WHERE
                instrument_id = $3 AND owner_id = $4
            RETURNING *
        "#,
        quantity,
        price_paid,
        instrument.id,
        owner.id
    )
    .fetch_one(exec)
    .await?;

    Ok(position)
}

// Similar to increase position but we take the new_cost_basis directly instead of
// blindly adding it, since we need to do some weighted adjustment.
pub async fn decrease_position(
    exec: impl Executor<'_, Database = Sqlite>,
    quantity: i64,
    new_cost_basis: Currency,
    instrument: &Instrument,
    owner: &DbUser,
) -> anyhow::Result<Position> {
    let position = query_as!(
        Position,
        r#"
            UPDATE positions
            SET
                quantity = quantity - $1,
                cost_basis = $2
            WHERE
                instrument_id = $3 AND owner_id = $4
            RETURNING *
        "#,
        quantity,
        new_cost_basis,
        instrument.id,
        owner.id
    )
    .fetch_one(exec)
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

#[derive(Debug, sqlx::Type)]
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

pub async fn create_order(
    exec: impl Executor<'_, Database = Sqlite>,
    direction: OrderDirection,
    quantity: i64,
    shares_price: Currency,
    fees: Currency,
    cost_basis: Currency,
    instrument: &Instrument,
    owner: &DbUser,
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
        direction,
        quantity,
        shares_price,
        fees,
        cost_basis,
        instrument.id,
        owner.id
    )
    .fetch_one(exec)
    .await?;

    Ok(order)
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
