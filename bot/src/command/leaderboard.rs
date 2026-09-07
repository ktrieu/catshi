use std::str::FromStr;
use std::{cmp::Reverse, collections::HashMap};

use common::store::transfer::TransferDirection;
use serenity::all::{
    CommandInteraction, CommandOptionType, Context, CreateCommand, CreateCommandOption,
};

use common::currency::Currency;
use common::store::{
    instrument::{InstrumentStore, InstrumentWithShares},
    position::{PositionStore, PositionWithMarketId},
    transfer::{TransferSource, TransferStore},
    user::DbUser,
};
use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter, EnumString};

use crate::ui;
use crate::{
    bot::Handler,
    portfolio::PortfolioValue,
    ui::{code_block, tabulate},
    utils,
};

pub const NAME: &'static str = "leaderboard";
pub const FIELD_OPT_NAME: &'static str = "field";

#[derive(Debug, PartialEq, Eq, EnumIter, Display, EnumString, Clone, Copy)]
#[strum(serialize_all = "snake_case")]
pub enum LeaderboardField {
    NetWorth,
    CashBalance,
    TradeProfits,
    TipsReceived,
    TipsSent,
    Gambling,
}

pub fn create() -> CreateCommand<'static> {
    let mut field_opt = CreateCommandOption::new(
        CommandOptionType::String,
        FIELD_OPT_NAME,
        "the field to rank by",
    );

    for f in LeaderboardField::iter() {
        field_opt = field_opt.add_string_choice(f.to_string(), f.to_string());
    }

    CreateCommand::new(NAME)
        .description("view rank of all users")
        .add_option(field_opt)
}

fn get_pv_field(p: &PortfolioValue, field: LeaderboardField) -> Currency {
    match field {
        LeaderboardField::NetWorth => p.net_worth(),
        LeaderboardField::CashBalance => p.user.cash_balance,
        LeaderboardField::TradeProfits => p.net_profit(),
        LeaderboardField::TipsReceived => p.tips_received,
        LeaderboardField::TipsSent => p.tips_sent,
        LeaderboardField::Gambling => p.gambling_winnings,
    }
}

pub async fn run(
    ctx: &Context,
    handler: &Handler,
    command: &CommandInteraction,
) -> anyhow::Result<()> {
    let selected_field = ui::get_command_option_str(command, FIELD_OPT_NAME)
        .map(|s| LeaderboardField::from_str(s))
        .unwrap_or(Ok(LeaderboardField::NetWorth))?;

    let mut tx = handler.db.begin().await?;

    let transfers = handler
        .transfer_store
        .get_user_transfer_by_source_and_direction(&mut tx)
        .await?;

    // Get our list of users by going through all unique users from the transfers.
    let mut users: HashMap<i32, DbUser> = HashMap::new();
    for t in transfers.iter() {
        users.entry(t.user.id).or_insert(t.user.clone());
    }

    // Process transfers into a HashMap by user and transaction type
    let transfers: HashMap<(i32, TransferSource, TransferDirection), Currency> = transfers
        .into_iter()
        .map(|t| ((t.user.id, t.source, t.direction), t.amount))
        .collect();

    let positions = handler
        .position_store
        .get_all_positions_with_market_id(&mut tx)
        .await?;
    let mut positions_by_user: HashMap<i64, Vec<PositionWithMarketId>> = HashMap::new();

    // Process positions into a HashMap of lists per user.
    for p in positions {
        positions_by_user
            .entry(p.position.owner_id)
            .or_insert_with(Vec::new)
            .push(p);
    }

    let instruments = handler
        .instrument_store
        .get_all_open_instruments_with_share_counts(&mut tx)
        .await?;
    let mut instruments_by_market: HashMap<i32, Vec<InstrumentWithShares>> = HashMap::new();
    for i in instruments {
        instruments_by_market
            .entry(i.0.market_id)
            .or_insert_with(Vec::new)
            .push(i);
    }

    let empty = Vec::new();
    let mut portfolio_values = Vec::new();

    for user in users.into_values() {
        let positions = positions_by_user
            .get(&i64::from(user.id))
            .unwrap_or(&empty);
        portfolio_values.push(PortfolioValue::new(
            user,
            &transfers,
            positions,
            &instruments_by_market,
        )?);
    }

    let mut sorted_fields: Vec<(&String, Currency)> = portfolio_values
        .iter()
        .map(|p| (&p.user.name, get_pv_field(p, selected_field)))
        .collect();

    sorted_fields.sort_by_key(|(_, v)| Reverse(*v));

    let mut rows: Vec<[String; 2]> = Vec::with_capacity(portfolio_values.len() + 1);
    rows.push(["User".to_string(), selected_field.to_string()]);

    for (name, value) in sorted_fields {
        rows.push([ui::user_shortname(&name), value.to_string()]);
    }

    let msg = code_block(&tabulate(rows));

    command
        .create_response(&ctx.http, utils::text_interaction_response(&msg, false))
        .await?;

    Ok(())
}
