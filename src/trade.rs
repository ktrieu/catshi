use anyhow::{anyhow, bail};
use sqlx::{Sqlite, Transaction};

use crate::{
    currency::Currency,
    store::{self, CreateTransfer, DbUser, Instrument, InstrumentWithShares, Market, Position},
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
    shares: impl Iterator<Item = &'s InstrumentWithShares> + Clone,
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

pub fn calc_price_raw<'s>(
    instrument_id: i64,
    shares: impl Iterator<Item = &'s InstrumentWithShares> + Clone,
    b: f32,
) -> f32 {
    let all_sum: f32 = shares
        .clone()
        .map(|(_, count)| (*count as f32 / b).exp())
        .sum();

    let selected = shares
        .clone()
        .find(|(instrument, _)| instrument.id == instrument_id)
        .expect("instrument should be in instruments list");

    let selected_price_exp = (selected.1 as f32 / b).exp();

    selected_price_exp / all_sum
}

pub fn calc_price<'s>(
    instrument_id: i64,
    shares: impl Iterator<Item = &'s InstrumentWithShares> + Clone,
    b: f32,
) -> Currency {
    Currency::from_instrument_price(calc_price_raw(instrument_id, shares, b))
}

pub fn get_max_buy_shares<'s>(
    budget: Currency,
    instrument_id: i64,
    shares: impl Iterator<Item = &'s InstrumentWithShares> + Clone,
    b: f32,
) -> (i64, Currency) {
    let price = calc_price_raw(instrument_id, shares.clone(), b);
    let inv_price = 1f32 - price;

    let inner = ((budget.as_instrument_price() / b).exp() - inv_price) / price;

    let raw_max = b * inner.ln();

    // Round down to calculate the max buy.
    let max_shares = raw_max.floor() as i64;

    // Calculate the cost we would have spent as well.
    let cost = calc_cost_delta(max_shares, instrument_id, shares.clone(), b);

    (max_shares, cost)
}

pub fn calc_fees(shares_price: Currency) -> Currency {
    // Flat two percent.
    shares_price * 0.02f32
}
pub struct TradeInput {
    pub quantity: i64,
    pub position: Option<Position>,
    pub user: DbUser,
    pub market: Market,
    pub market_instruments: Vec<InstrumentWithShares>,
    pub traded_instrument: Instrument,
    pub market_owner: DbUser,
}

impl TradeInput {
    pub async fn new(
        tx: &mut Transaction<'_, Sqlite>,
        instrument_id: i64,
        quantity: i64,
        user: DbUser,
    ) -> anyhow::Result<Self> {
        let traded_instrument = store::get_instrument_by_id(&mut **tx, instrument_id)
            .await?
            .ok_or(anyhow!("instrument {instrument_id} not found"))?;

        let market = store::get_market_by_id(&mut **tx, traded_instrument.market_id)
            .await?
            .ok_or(anyhow!("market {} not found", traded_instrument.market_id))?;

        let market_owner = store::get_user_by_id(&mut **tx, market.owner_id)
            .await?
            .ok_or(anyhow!("user {} not found", market.owner_id))?;

        let position = store::get_user_position(&mut **tx, &traded_instrument, &user).await?;

        let market_instruments =
            store::get_instruments_with_share_counts_for_market(&mut **tx, market.id).await?;

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

pub struct BuyResult<'a> {
    pub shares_price: Currency,
    pub fees: Currency,
    pub input: &'a TradeInput,
}

impl<'a> BuyResult<'a> {
    pub fn total(&self) -> Currency {
        self.shares_price + self.fees
    }

    pub async fn persist<'e>(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        system_user: &DbUser,
    ) -> anyhow::Result<()> {
        if self.input.position.is_none() {
            store::create_new_position(
                &mut **tx,
                self.input.quantity,
                self.total(),
                &self.input.traded_instrument,
                &self.input.user,
            )
            .await?;
        } else {
            store::increase_position(
                &mut **tx,
                self.input.quantity,
                self.total(),
                &self.input.traded_instrument,
                &self.input.user,
            )
            .await?;
        }

        store::create_order(
            &mut **tx,
            store::OrderDirection::Buy,
            self.input.quantity,
            self.shares_price,
            self.fees,
            self.total(), // cost basis is the same as shares_price for buys.
            &self.input.traded_instrument,
            &self.input.user,
        )
        .await?;

        // Make the user pay for their shares.
        system_debit_user(
            tx,
            &self.input.user,
            system_user,
            self.shares_price,
            format!(
                "BUY order {} shares {}",
                &self.input.quantity, self.input.traded_instrument.name
            )
            .as_str(),
        )
        .await?;

        // And transfer the market fees to the market ownwer.
        transfer_cash(
            tx,
            &self.input.user,
            &self.input.market_owner,
            self.fees,
            format!(
                "BUY order {} shares {} fees",
                &self.input.quantity, self.input.traded_instrument.name
            )
            .as_str(),
        )
        .await?;

        Ok(())
    }
}

pub async fn buy<'i>(input: &'i TradeInput) -> anyhow::Result<BuyResult<'i>> {
    // Simple MVP behaviour here: buy 1 share.
    let shares_price = calc_cost_delta(
        input.quantity,
        input.traded_instrument.id,
        input.market_instruments.iter(),
        MARKET_B,
    );

    let fees = calc_fees(shares_price);

    Ok(BuyResult {
        shares_price,
        fees,
        input,
    })
}

pub struct SellResult<'i> {
    pub shares_price: Currency,
    pub order_cost_basis: Currency,
    pub fees: Currency,
    pub input: &'i TradeInput,
}

impl<'i> SellResult<'i> {
    pub fn profit(&self) -> Currency {
        self.shares_price - self.fees - self.order_cost_basis
    }

    pub fn net(&self) -> Currency {
        self.shares_price - self.fees
    }

    pub async fn persist(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        system_user: &DbUser,
    ) -> anyhow::Result<()> {
        let position = self
            .input
            .position
            .as_ref()
            .ok_or(anyhow!("position should exist for sells"))?;

        let new_position_cost_basis = position.cost_basis - self.order_cost_basis;

        store::decrease_position(
            &mut **tx,
            self.input.quantity,
            new_position_cost_basis,
            &self.input.traded_instrument,
            &self.input.user,
        )
        .await?;

        store::create_order(
            &mut **tx,
            store::OrderDirection::Sell,
            self.input.quantity,
            self.shares_price,
            self.fees,
            self.order_cost_basis,
            &self.input.traded_instrument,
            &self.input.user,
        )
        .await?;

        // Give the user their proceeds.
        system_credit_user(
            tx,
            &self.input.user,
            system_user,
            self.shares_price,
            format!(
                "SELL {} shares {}",
                self.input.quantity, self.input.traded_instrument.name
            )
            .as_str(),
        )
        .await?;

        // And pay the market owner their fees.
        transfer_cash(
            tx,
            &self.input.user,
            &self.input.market_owner,
            self.fees,
            format!(
                "SELL {} shares {} fees",
                self.input.quantity, self.input.traded_instrument.name
            )
            .as_str(),
        )
        .await?;

        Ok(())
    }
}

pub async fn sell<'i>(input: &'i TradeInput) -> anyhow::Result<SellResult<'i>> {
    // A sell would be decreasing the amount of shares, so negate quantity. We receive the corresponding decrease in price
    // so also negate the result.
    let shares_price = -calc_cost_delta(
        -input.quantity,
        input.traded_instrument.id,
        input.market_instruments.iter(),
        MARKET_B,
    );

    let fees = calc_fees(shares_price);

    let position = match &input.position {
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

    let sold_ratio = input.quantity as f32 / position.quantity as f32;
    let order_cost_basis = position.cost_basis * sold_ratio;

    Ok(SellResult {
        shares_price,
        fees,
        order_cost_basis,
        input,
    })
}
