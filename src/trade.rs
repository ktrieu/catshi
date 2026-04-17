use anyhow::anyhow;
use sqlx::{Sqlite, Transaction};

use crate::{
    currency::Currency,
    store::{
        self, instrument::Instrument, instrument::InstrumentWithShares, market::Market,
        order::OrderDirection, position::Position, position::PositionWithUser,
        transfer::CreateTransfer, user::DbUser,
    },
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
    store::transfer::insert_transfer(&mut **tx, create).await?;

    // 2. Credit the receiving account.
    store::user::increment_balance(&mut **tx, receiver, amount).await?;

    // 3. Debit the sending account.
    store::user::increment_balance(&mut **tx, sender, -amount).await?;

    Ok(())
}

pub async fn system_credit_user(
    tx: &mut Transaction<'_, Sqlite>,
    user: &DbUser,
    system_user: &DbUser,
    amount: Currency,
    memo: &str,
) -> anyhow::Result<()> {
    transfer_cash(tx, system_user, user, amount, memo).await?;

    Ok(())
}

pub async fn system_debit_user(
    tx: &mut Transaction<'_, Sqlite>,
    user: &DbUser,
    system_user: &DbUser,
    amount: Currency,
    memo: &str,
) -> anyhow::Result<()> {
    transfer_cash(tx, user, system_user, amount, memo).await?;

    Ok(())
}

pub const MARKET_B: f32 = 70.0f32;

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
) -> (i64, OrderPrices) {
    let price = calc_price_raw(instrument_id, shares.clone(), b);
    let inv_price = 1f32 - price;

    let inner = ((budget.as_instrument_price() / b).exp() - inv_price) / price;

    let raw_max = b * inner.ln();

    // Round down to calculate the max buy.
    let max_shares = raw_max.floor() as i64;

    (
        max_shares,
        calc_buy_prices(max_shares, instrument_id, shares, b),
    )
}

pub fn calc_fees(shares_price: Currency) -> Currency {
    // Flat two percent.
    shares_price * 0.02f32
}

pub struct OrderPrices {
    pub shares_price: Currency,
    pub fees: Currency,
}

impl OrderPrices {
    pub fn total(&self, direction: OrderDirection) -> Currency {
        match direction {
            OrderDirection::Buy => self.shares_price + self.fees,
            OrderDirection::Sell => self.shares_price - self.fees,
        }
    }
}

pub fn calc_buy_prices<'s>(
    quantity: i64,
    instrument_id: i64,
    shares: impl Iterator<Item = &'s InstrumentWithShares> + Clone,
    b: f32,
) -> OrderPrices {
    let shares_price = calc_cost_delta(quantity, instrument_id, shares, b);

    OrderPrices {
        shares_price,
        fees: calc_fees(shares_price),
    }
}

pub fn calc_sell_prices<'s>(
    quantity: i64,
    instrument_id: i64,
    shares: impl Iterator<Item = &'s InstrumentWithShares> + Clone,
    b: f32,
) -> OrderPrices {
    // A sell would be decreasing the amount of shares, so negate quantity. We receive the corresponding decrease in price
    // so also negate the result.
    let shares_price = -calc_cost_delta(-quantity, instrument_id, shares, b);

    OrderPrices {
        shares_price,
        fees: calc_fees(shares_price),
    }
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
        let traded_instrument =
            store::instrument::get_instrument_by_id(&mut **tx, instrument_id).await?;

        let market =
            store::market::get_market_by_id(&mut **tx, traded_instrument.market_id).await?;

        let market_owner = store::user::get_user_by_id(&mut **tx, market.owner_id).await?;
        let position =
            store::position::get_user_position(&mut **tx, &traded_instrument, &user).await?;

        let market_instruments =
            store::instrument::get_instruments_with_share_counts_for_market(&mut **tx, market.id)
                .await?;

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
            store::position::create_new_position(
                &mut **tx,
                self.input.quantity,
                self.total(),
                &self.input.traded_instrument,
                &self.input.user,
            )
            .await?;
        } else {
            store::position::increase_position(
                &mut **tx,
                self.input.quantity,
                self.total(),
                &self.input.traded_instrument,
                &self.input.user,
            )
            .await?;
        }

        store::order::create_order(
            &mut **tx,
            store::order::OrderDirection::Buy,
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

#[derive(Debug)]
pub enum BuyError {
    InsufficientFunds(Currency),
}

pub async fn buy<'i>(input: &'i TradeInput) -> Result<BuyResult<'i>, BuyError> {
    let OrderPrices { shares_price, fees } = calc_buy_prices(
        input.quantity,
        input.traded_instrument.id,
        input.market_instruments.iter(),
        MARKET_B,
    );

    // Check that we have enough money to actually purchase these shares.
    // To be generous, (and avoid annoying fractional YPs lying around) we'll let people go 1 YP into overdraft.
    let overdraft = input.user.cash_balance - (shares_price + fees);
    if overdraft < Currency::new_yp(-1) {
        return Err(BuyError::InsufficientFunds(shares_price + fees));
    }

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

        store::position::decrease_position(
            &mut **tx,
            self.input.quantity,
            new_position_cost_basis,
            &self.input.traded_instrument,
            &self.input.user,
        )
        .await?;

        store::order::create_order(
            &mut **tx,
            OrderDirection::Sell,
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

pub enum SellError {
    InsufficientShares,
}

pub async fn sell<'i>(input: &'i TradeInput) -> Result<SellResult<'i>, SellError> {
    let OrderPrices { shares_price, fees } = calc_sell_prices(
        input.quantity,
        input.traded_instrument.id,
        input.market_instruments.iter(),
        MARKET_B,
    );

    let position = match &input.position {
        Some(position) => position,
        None => {
            return Err(SellError::InsufficientShares);
        }
    };

    // Important! Check and make sure we have enough shares to sell!
    if position.quantity < input.quantity {
        return Err(SellError::InsufficientShares);
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

#[derive(Debug)]
pub struct ResolveInput {
    pub market: Market,
    pub market_owner: DbUser,
    pub market_instruments: Vec<InstrumentWithShares>,
    pub winner: Instrument,
    pub all_positions: Vec<PositionWithUser>,
}

impl ResolveInput {
    pub async fn new(
        tx: &mut Transaction<'_, Sqlite>,
        market_id: i64,
        winning_instrument_id: i64,
    ) -> anyhow::Result<Self> {
        let market = store::market::get_market_by_id(&mut **tx, market_id).await?;
        let market_owner = store::user::get_user_by_id(&mut **tx, market.owner_id).await?;

        let market_instruments =
            store::instrument::get_instruments_with_share_counts_for_market(&mut **tx, market_id)
                .await?;

        let winner = market_instruments
            .iter()
            .find_map(|(i, _)| {
                if i.id == winning_instrument_id {
                    Some(i)
                } else {
                    None
                }
            })
            .ok_or(anyhow!(
                "winning instrument {winning_instrument_id} does not exist"
            ))?
            .clone();

        let all_positions = store::position::get_all_market_positions(&mut **tx, market_id).await?;

        Ok(Self {
            market,
            market_owner,
            market_instruments,
            winner,
            all_positions,
        })
    }
}

#[derive(Debug)]
pub struct ResolveResult<'i> {
    pub quantity: i64,
    pub shares_price: Currency,
    pub fees: Currency,
    pub instrument: &'i Instrument,
    pub user: &'i DbUser,
    pub market_owner: &'i DbUser,
    pub cost_basis: Currency,
}

impl<'i> ResolveResult<'i> {
    pub async fn persist(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        system_user: &DbUser,
    ) -> anyhow::Result<()> {
        // Close out the position.
        store::position::decrease_position(
            &mut **tx,
            self.quantity,
            Currency::from(0),
            self.instrument,
            self.user,
        )
        .await?;

        // Create the sell order.
        store::order::create_order(
            &mut **tx,
            OrderDirection::Sell,
            self.quantity,
            self.shares_price,
            self.fees,
            self.cost_basis,
            self.instrument,
            self.user,
        )
        .await?;

        // Only transfer money if it actually changes hands
        if self.shares_price != Currency::from(0) {
            // Give the user their proceeds.
            system_credit_user(
                tx,
                &self.user,
                system_user,
                self.shares_price,
                format!("SELL {} shares {}", self.quantity, self.instrument.name).as_str(),
            )
            .await?;
        };

        if self.fees != Currency::from(0) {
            // And pay the market owner their fees.
            transfer_cash(
                tx,
                &self.user,
                &self.market_owner,
                self.fees,
                format!(
                    "SELL {} shares {} fees",
                    self.quantity, self.instrument.name
                )
                .as_str(),
            )
            .await?;
        }

        Ok(())
    }

    pub fn profit(&self) -> Currency {
        self.shares_price - self.fees - self.cost_basis
    }
}

pub async fn resolve<'i>(input: &'i ResolveInput) -> anyhow::Result<Vec<ResolveResult<'i>>> {
    let mut results: Vec<ResolveResult> = Vec::new();

    for p in &input.all_positions {
        let position = &p.position;
        let user = &p.user;

        // We're closing out the whole position.
        let quantity = position.quantity;
        let cost_basis = position.cost_basis;
        let instrument = &input
            .market_instruments
            .iter()
            .find(|(i, _)| i.id == position.instrument_id)
            .ok_or(anyhow!(
                "instrument {} not in instruments list",
                position.instrument_id
            ))?
            .0;

        let (share_price, fees) = if position.instrument_id == input.winner.id {
            let share_price = Currency::from_instrument_price(1.0) * position.quantity;
            let fees = calc_fees(share_price);

            (share_price, fees)
        } else {
            // You get nothing. Sorry.
            let share_price = Currency::from_instrument_price(0.0);
            // Well you know we might change the fee calculation later to charge money on zero prices.
            let fees = calc_fees(share_price);

            (share_price, fees)
        };

        results.push(ResolveResult {
            shares_price: share_price,
            cost_basis,
            quantity,
            fees,
            instrument: &instrument,
            user,
            market_owner: &input.market_owner,
        });
    }

    Ok(results)
}
