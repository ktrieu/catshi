use crate::{
    currency::Currency,
    store::{
        instrument::{Instrument, InstrumentWithShares},
        market::FullMarket,
        order::{CreateOrder, OrderDirection},
        position::{CreatePosition, Position, PositionWithUser},
        transfer::CreateTransfer,
        user::DbUser,
    },
    ui::instrument_display_text,
};

fn create_transfer(
    sender: &DbUser,
    receiver: &DbUser,
    amount: Currency,
    memo: &str,
) -> CreateTransfer {
    CreateTransfer {
        amount,
        sender: sender.id,
        receiver: receiver.id,
        memo: memo.to_owned(),
    }
}

fn create_system_debit(
    user: &DbUser,
    system_user: &DbUser,
    amount: Currency,
    memo: &str,
) -> CreateTransfer {
    create_transfer(user, system_user, amount, memo)
}

pub fn create_system_credit(
    user: &DbUser,
    system_user: &DbUser,
    amount: Currency,
    memo: &str,
) -> CreateTransfer {
    create_transfer(system_user, user, amount, memo)
}

pub const MARKET_B: f32 = 70.0f32;

fn cost(share_counts: impl IntoIterator<Item = i64>, b: f32) -> f32 {
    let summed: f32 = share_counts.into_iter().map(|s| (s as f32 / b).exp()).sum();

    b * summed.ln()
}

fn calc_cost_delta<'s>(
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

fn calc_price_raw<'s>(
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

pub struct TradeResult {
    pub order: CreateOrder,
    pub transfers: Vec<CreateTransfer>,
    pub position: CreatePosition,
    pub shares_price: Currency,
    pub fees: Currency,
    pub quantity: i64,
    pub direction: OrderDirection,
}

impl TradeResult {
    pub fn total(&self) -> Currency {
        match self.direction {
            OrderDirection::Buy => self.shares_price + self.fees,
            OrderDirection::Sell => self.shares_price - self.fees,
        }
    }
}

#[derive(Debug)]
pub enum TradeError {
    InsufficientFunds(Currency),
    InsufficientShares,
}

pub fn buy(
    quantity: i64,
    instrument: &Instrument,
    market: &FullMarket,
    existing_position: Option<&Position>,
    user: &DbUser,
    system_user: &DbUser,
) -> Result<TradeResult, TradeError> {
    let prices = calc_buy_prices(quantity, instrument.id, market.instruments.iter(), MARKET_B);
    let total = prices.total(OrderDirection::Buy);

    // Check that we have enough money to actually purchase these shares.
    // To be generous, (and avoid annoying fractional YPs lying around) we'll let people go 1 YP into overdraft.
    let overdraft = user.cash_balance - (total);
    if overdraft < Currency::new_yp(-1) {
        return Err(TradeError::InsufficientFunds(total));
    }

    let order = CreateOrder {
        direction: OrderDirection::Buy,
        quantity,
        cost_basis: total,
        shares_price: prices.shares_price,
        fees: prices.fees,
        instrument_id: instrument.id,
        owner_id: user.id,
    };

    let shares_memo = format!(
        "BUY {} shares {}",
        quantity,
        instrument_display_text(instrument, &market.row)
    );
    let shares_transfer = create_system_debit(user, system_user, prices.shares_price, &shares_memo);

    let fees_memo = format!(
        "BUY FEES {} shares {}",
        quantity,
        instrument_display_text(instrument, &market.row)
    );
    let fees_transfer = create_transfer(user, &market.owner, prices.fees, &fees_memo);

    let existing_cost_basis = existing_position
        .map(|p| p.cost_basis)
        .unwrap_or(Currency::from(0));
    let held_shares = existing_position.map(|p| p.quantity).unwrap_or(0);

    let position = CreatePosition {
        quantity: held_shares + quantity,
        cost_basis: existing_cost_basis + prices.total(OrderDirection::Buy),
        instrument_id: instrument.id,
        owner_id: user.id,
    };

    Ok(TradeResult {
        order,
        transfers: vec![shares_transfer, fees_transfer],
        position,
        shares_price: prices.shares_price,
        fees: prices.fees,
        quantity,
        direction: OrderDirection::Buy,
    })
}

pub fn sell(
    quantity: i64,
    instrument: &Instrument,
    market: &FullMarket,
    existing_position: Option<&Position>,
    user: &DbUser,
    system_user: &DbUser,
) -> Result<TradeResult, TradeError> {
    let prices = calc_sell_prices(quantity, instrument.id, market.instruments.iter(), MARKET_B);

    let position = match existing_position {
        Some(position) => position,
        None => {
            return Err(TradeError::InsufficientShares);
        }
    };

    // Important! Check and make sure we have enough shares to sell!
    if position.quantity < quantity {
        return Err(TradeError::InsufficientShares);
    }

    let sold_ratio = quantity as f32 / position.quantity as f32;
    let new_cost_basis = position.cost_basis * (1.0f32 - sold_ratio);
    let order_cost_basis = position.cost_basis - new_cost_basis;

    let order = CreateOrder {
        direction: OrderDirection::Sell,
        quantity,
        cost_basis: order_cost_basis,
        shares_price: prices.shares_price,
        fees: prices.fees,
        instrument_id: instrument.id,
        owner_id: user.id,
    };

    let shares_memo = format!(
        "SELL {} shares {}",
        quantity,
        instrument_display_text(instrument, &market.row)
    );
    let shares_transfer =
        create_system_credit(user, system_user, prices.shares_price, &shares_memo);

    let fees_memo = format!(
        "SELL FEES {} shares {}",
        quantity,
        instrument_display_text(instrument, &market.row)
    );
    let fees_transfer = create_transfer(user, &market.owner, prices.fees, &fees_memo);

    let position = CreatePosition {
        quantity: position.quantity - quantity,
        cost_basis: new_cost_basis,
        instrument_id: instrument.id,
        owner_id: user.id,
    };

    Ok(TradeResult {
        order,
        position,
        transfers: vec![shares_transfer, fees_transfer],
        shares_price: prices.shares_price,
        fees: prices.fees,
        quantity,
        direction: OrderDirection::Sell,
    })
}

#[derive(Debug)]
pub struct ResolveResult {
    pub order: CreateOrder,
    pub transfers: Vec<CreateTransfer>,
    pub position: CreatePosition,
    pub shares_price: Currency,
    pub fees: Currency,
    pub cost_basis: Currency,
}

impl ResolveResult {
    pub fn profit(&self) -> Currency {
        self.shares_price - self.fees - self.cost_basis
    }
}

pub fn resolve(
    market: &FullMarket,
    winner: &Instrument,
    positions: &Vec<PositionWithUser>,
    system_user: &DbUser,
) -> anyhow::Result<Vec<ResolveResult>> {
    let mut results: Vec<ResolveResult> = Vec::new();

    for p in positions {
        let position = &p.position;
        let user = &p.user;

        // We're closing out the whole position.
        let quantity = position.quantity;
        let cost_basis = position.cost_basis;
        let is_winning_position = position.instrument_id == winner.id;

        let resolve_price = if is_winning_position {
            Currency::from_instrument_price(1.0)
        } else {
            Currency::from_instrument_price(0.0)
        };

        let shares_price = resolve_price * position.quantity;
        let prices = OrderPrices {
            shares_price,
            fees: calc_fees(shares_price),
        };

        let order = CreateOrder {
            direction: OrderDirection::Sell,
            quantity,
            shares_price,
            fees: prices.fees,
            cost_basis,
            instrument_id: winner.id,
            owner_id: user.id,
        };

        let mut transfers = Vec::new();

        if is_winning_position {
            let shares_memo = format!(
                "RESOLVE {quantity} shares {}",
                instrument_display_text(winner, &market.row)
            );
            let shares_transfer =
                create_system_credit(user, system_user, shares_price, &shares_memo);

            let fees_memo = format!(
                "RESOLVE FEES {quantity} shares {}",
                instrument_display_text(winner, &market.row)
            );
            let fees_transfer = create_transfer(user, &market.owner, prices.fees, &fees_memo);

            transfers.push(shares_transfer);
            transfers.push(fees_transfer);
        }

        // Create a new closed out position.
        let position = CreatePosition {
            quantity: 0,
            cost_basis: Currency::from(0),
            instrument_id: position.instrument_id,
            owner_id: user.id,
        };

        results.push(ResolveResult {
            order,
            transfers,
            position,
            shares_price,
            fees: prices.fees,
            cost_basis,
        });
    }

    Ok(results)
}
