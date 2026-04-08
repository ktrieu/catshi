use anyhow::{anyhow, bail};
use sqlx::{Executor, Sqlite, SqlitePool, Transaction};

use crate::{
    currency::Currency,
    store::{self, CreateTransfer, DbUser, Instrument, Market, Position},
};

async fn transfer_cash(
    tx: &mut Transaction<'_, Sqlite>,
    sender: &DbUser,
    receiver: &DbUser,
    amount: Currency,
    memo: &str,
) -> anyhow::Result<()> {
    // 1. Create the transfer.
    let create = CreateTransfer {
        amount,
        sender: sender.id,
        receiver: receiver.id,
        memo: memo.to_owned(),
    };
    store::insert_transfer(&mut **tx, create).await?;

    // 2. Credit the receiving account.
    store::increment_balance(&mut **tx, receiver, amount).await?;

    // 3. Debit the sending account.
    store::increment_balance(&mut **tx, sender, -amount).await?;

    Ok(())
}

async fn system_credit_user(
    tx: &mut Transaction<'_, Sqlite>,
    user: &DbUser,
    system_user: &DbUser,
    amount: Currency,
    memo: &str,
) -> anyhow::Result<()> {
    transfer_cash(tx, system_user, user, amount, memo).await?;

    Ok(())
}

async fn system_debit_user(
    tx: &mut Transaction<'_, Sqlite>,
    user: &DbUser,
    system_user: &DbUser,
    amount: Currency,
    memo: &str,
) -> anyhow::Result<()> {
    transfer_cash(tx, user, system_user, amount, memo).await?;

    Ok(())
}

pub const MARKET_B: f32 = 10.0f32;

fn cost(share_counts: impl IntoIterator<Item = i64>, b: f32) -> f32 {
    let summed: f32 = share_counts.into_iter().map(|s| (s as f32 / b).exp()).sum();

    b * summed.ln()
}

pub fn calc_cost_delta<'s>(
    quantity: i64,
    instrument_id: i64,
    shares: impl Iterator<Item = &'s (Instrument, i64)> + Clone,
    b: f32,
) -> Currency {
    // Pre cost is the current state of the market.
    let pre_cost = cost(shares.clone().map(|(_, qty)| *qty), b);

    // Post cost is the market + the potentially bought shares.
    let post_shares = shares.clone().map(|(instrument, count)| {
        if instrument.id == instrument_id {
            count + quantity
        } else {
            *count
        }
    });

    let post_cost = cost(post_shares, b);

    let total_price = post_cost - pre_cost;

    Currency::from_instrument_price(total_price)
}

fn calc_fees(shares_price: Currency) -> Currency {
    // Flat two percent.
    shares_price * 0.02f32
}

pub struct BuyResult {
    pub shares_price: Currency,
    pub fees: Currency,
}

impl BuyResult {
    pub fn total(&self) -> Currency {
        self.shares_price + self.fees
    }
}

pub struct TradeInput {
    pub quantity: i64,
    pub position: Option<Position>,
    pub user: DbUser,
    pub market: Market,
    pub market_instruments: Vec<(Instrument, i64)>,
    pub traded_instrument: Instrument,
    pub market_owner: DbUser,
}

impl TradeInput {
    pub async fn new(
        exec: impl Executor<'_, Database = Sqlite> + Copy,
        instrument_id: i64,
        quantity: i64,
        user: DbUser,
    ) -> anyhow::Result<Self> {
        let traded_instrument = store::get_instrument_by_id(exec, instrument_id)
            .await?
            .ok_or(anyhow!("instrument {instrument_id} not found"))?;

        let market = store::get_market_by_id(exec, traded_instrument.market_id)
            .await?
            .ok_or(anyhow!("market {} not found", traded_instrument.market_id))?;

        let market_owner = store::get_user_by_id(exec, market.owner_id)
            .await?
            .ok_or(anyhow!("user {} not found", market.owner_id))?;

        let position = store::get_user_position(exec, &traded_instrument, &user).await?;

        let market_instruments =
            store::get_instruments_with_share_counts_for_market(exec, market.id).await?;

        Ok(Self {
            quantity,
            position,
            user,
            market,
            market_instruments,
            market_owner,
            traded_instrument,
        })
    }
}

pub async fn buy(
    pool: &SqlitePool,
    input: &TradeInput,
    system_user: &DbUser,
) -> anyhow::Result<BuyResult> {
    let mut tx = pool.begin().await?;

    // Simple MVP behaviour here: buy 1 share.
    let shares_price = calc_cost_delta(
        input.quantity,
        input.traded_instrument.id,
        input.market_instruments.iter(),
        MARKET_B,
    );

    let fees = calc_fees(shares_price);
    let total_price = shares_price + fees;

    if input.position.is_none() {
        store::create_new_position(
            &mut *tx,
            input.quantity,
            total_price,
            &input.traded_instrument,
            &input.user,
        )
        .await?;
    } else {
        store::increase_position(
            &mut *tx,
            input.quantity,
            total_price,
            &input.traded_instrument,
            &input.user,
        )
        .await?;
    }

    store::create_order(
        &mut *tx,
        store::OrderDirection::Buy,
        input.quantity,
        shares_price,
        fees,        // no fees for now.
        total_price, // cost basis is the same as shares_price for buys.
        &input.traded_instrument,
        &input.user,
    )
    .await?;

    // Make the user pay for their shares.
    system_debit_user(
        &mut tx,
        &input.user,
        system_user,
        shares_price,
        format!(
            "BUY {} shares {}",
            input.quantity, input.traded_instrument.name
        )
        .as_str(),
    )
    .await?;

    // And transfer the market fees to the market ownwer.
    transfer_cash(
        &mut tx,
        &input.user,
        &input.market_owner,
        fees,
        format!(
            "BUY {} shares {} fees",
            input.quantity, input.traded_instrument.name
        )
        .as_str(),
    )
    .await?;

    tx.commit().await?;

    Ok(BuyResult { shares_price, fees })
}

pub struct SellResult {
    pub shares_price: Currency,
    pub order_cost_basis: Currency,
    pub fees: Currency,
}

impl SellResult {
    pub fn profit(&self) -> Currency {
        self.shares_price - self.fees - self.order_cost_basis
    }

    pub fn net(&self) -> Currency {
        self.shares_price - self.fees
    }
}

pub async fn sell(
    pool: &SqlitePool,
    input: &TradeInput,
    system_user: &DbUser,
) -> anyhow::Result<SellResult> {
    let mut tx = pool.begin().await?;

    // A sell would be decreasing the amount of shares, so negate quantity. We receive the corresponding decrease in price
    // so also negate the result.
    let shares_price = -calc_cost_delta(
        -input.quantity,
        input.traded_instrument.id,
        input.market_instruments.iter(),
        MARKET_B,
    );

    let fees = calc_fees(shares_price);

    let position =
        match store::get_user_position(&mut *tx, &input.traded_instrument, &input.user).await? {
            Some(position) => position,
            None => {
                // Can't sell if there's no position. Raise an error here. Caller should catch this
                // and display a more graceful message, however.
                bail!(
                    "no position to sell for user {}, instrument {}",
                    input.user.id,
                    input.traded_instrument.id
                );
            }
        };

    // Important! Check and make sure we have enough shares to sell!
    if position.quantity < input.quantity {
        bail!(
            "insufficient shares to sell for user {}, instrument {}. tried to sell {} but only had {}",
            input.user.id,
            input.traded_instrument.id,
            input.quantity,
            position.quantity
        )
    }

    // Scale down the cost-basis uniformly based on the proportion of shares we are selling.
    let new_position_cost_basis =
        position.cost_basis * (1f32 - (input.quantity as f32 / position.quantity as f32));

    store::decrease_position(
        &mut *tx,
        input.quantity,
        new_position_cost_basis,
        &input.traded_instrument,
        &input.user,
    )
    .await?;

    // The cost basis of the shares we sold is what's remaining from the original position's cost basis.
    let order_cost_basis = position.cost_basis - new_position_cost_basis;

    store::create_order(
        &mut *tx,
        store::OrderDirection::Sell,
        input.quantity,
        shares_price,
        fees,
        order_cost_basis,
        &input.traded_instrument,
        &input.user,
    )
    .await?;

    // Give the user their proceeds.
    system_credit_user(
        &mut tx,
        &input.user,
        system_user,
        shares_price,
        format!(
            "SELL {} shares {}",
            input.quantity, input.traded_instrument.name
        )
        .as_str(),
    )
    .await?;

    // And pay the market owner their fees.
    transfer_cash(
        &mut tx,
        &input.user,
        &input.market_owner,
        fees,
        format!(
            "SELL {} shares {} fees",
            input.quantity, input.traded_instrument.name
        )
        .as_str(),
    )
    .await?;

    tx.commit().await?;

    Ok(SellResult {
        shares_price,
        fees,
        order_cost_basis,
    })
}
