use std::collections::HashMap;

use crate::{
    currency::Currency,
    store::{
        instrument::InstrumentWithShares, position::PositionWithMarketId, transfer::TransferSource,
        user::DbUser,
    },
    trade::{self, calc_sell_prices},
};
use anyhow::anyhow;

pub struct PortfolioValue {
    user: DbUser,
    net_deposits: Currency,
    trades_profit: Currency,
    fees_profit: Currency,
    net_user_transfers: Currency,
    positions_value: Currency,
}

impl PortfolioValue {
    pub fn new(
        user: DbUser,
        net_transfers: &HashMap<(i64, TransferSource), Currency>,
        positions: &Vec<PositionWithMarketId>,
        market_shares: &HashMap<i64, Vec<InstrumentWithShares>>,
    ) -> anyhow::Result<Self> {
        let net_deposits = *net_transfers
            .get(&(user.id, TransferSource::Deposit))
            .unwrap_or(&Currency::from(0));

        let trades_profit = *net_transfers
            .get(&(user.id, TransferSource::Order))
            .unwrap_or(&Currency::from(0));

        let fees_profit = *net_transfers
            .get(&(user.id, TransferSource::TradeFee))
            .unwrap_or(&Currency::from(0));

        let net_user_transfers = *net_transfers
            .get(&(user.id, TransferSource::UserInitiated))
            .unwrap_or(&Currency::from(0));

        let net_position_value: anyhow::Result<Currency> = positions
            .iter()
            .map(|p| -> anyhow::Result<Currency> {
                let market_id = &p.market_id;
                let instruments = market_shares.get(&p.market_id).ok_or(anyhow!(
                    "could not find shares for market {market_id} when calculating portfolio value"
                ))?;

                let price = calc_sell_prices(
                    p.position.quantity,
                    p.position.instrument_id,
                    instruments.iter(),
                    trade::MARKET_B,
                );

                Ok(price.total())
            })
            .sum();

        Ok(Self {
            user,
            net_deposits,
            trades_profit,
            fees_profit,
            net_user_transfers,
            positions_value: net_position_value?,
        })
    }

    pub fn net_profit(&self) -> Currency {
        self.trades_profit + self.fees_profit + self.positions_value
    }

    pub fn deposits(&self) -> Currency {
        self.net_deposits + self.net_user_transfers
    }

    pub fn table_header() -> [String; 5] {
        [
            "User".to_string(),
            "Balance".to_string(),
            "Deposits".to_string(),
            "Positions".to_string(),
            "Profit".to_string(),
        ]
    }

    pub fn to_table_row(&self) -> [String; 5] {
        [
            self.user.name.clone(),
            self.user.cash_balance.to_string(),
            self.deposits().to_string(),
            self.positions_value.to_string(),
            self.net_profit().to_string(),
        ]
    }
}
