use std::collections::HashMap;

use serenity::all::{
    CacheHttp, CommandInteraction, CommandOptionType, Context, CreateCommand, CreateCommandOption,
};

use common::currency::Currency;
use common::store::transfer::TransferDirection;
use common::store::{
    instrument::{InstrumentStore, InstrumentWithShares},
    position::{PositionStore, PositionWithMarketId},
    transfer::{TransferSource, TransferStore},
    user::{DbUser, UserStore},
};

use crate::{
    bot::Handler,
    portfolio::PortfolioValue,
    ui::{self, code_block, tabulate},
    utils,
};

pub const NAME: &'static str = "portfolio";
pub const USER_OPT_NAME: &'static str = "user";

pub fn create() -> CreateCommand<'static> {
    let user_opt = CreateCommandOption::new(
        CommandOptionType::User,
        USER_OPT_NAME,
        "the user whose portfolio to view (defaults to you)",
    );

    CreateCommand::new(NAME)
        .description("view a breakdown of a user's portfolio")
        .add_option(user_opt)
}

pub async fn run(
    ctx: &Context,
    handler: &Handler,
    user: &DbUser,
    command: &CommandInteraction,
) -> anyhow::Result<()> {
    let mut tx = handler.db.begin().await?;

    let target = match ui::get_command_option_user(command, USER_OPT_NAME) {
        Some(user_id) => {
            handler
                .user_store
                .get_by_discord_id(&mut tx, &user_id)
                .await?
        }
        None => Some(user.clone()),
    };

    let target = match target {
        Some(t) => t,
        None => {
            command
                .create_response(
                    ctx.http(),
                    utils::text_interaction_response("User not found", true),
                )
                .await?;
            return Ok(());
        }
    };

    // Aggregate transfers by source and direction, keyed by user id.
    let transfers: HashMap<(i32, TransferSource, TransferDirection), Currency> = handler
        .transfer_store
        .get_user_transfer_by_source_and_direction(&mut tx)
        .await?
        .into_iter()
        .map(|t| ((t.user.id, t.source, t.direction), t.amount))
        .collect();

    // Collect the target user's open positions.
    let positions: Vec<PositionWithMarketId> = handler
        .position_store
        .get_all_positions_with_market_id(&mut tx)
        .await?
        .into_iter()
        .filter(|p| p.position.owner_id == target.id)
        .collect();

    // Group open instruments by market so positions can be priced.
    let mut instruments_by_market: HashMap<i32, Vec<InstrumentWithShares>> = HashMap::new();
    for i in handler
        .instrument_store
        .get_all_open_instruments_with_share_counts(&mut tx)
        .await?
    {
        instruments_by_market
            .entry(i.0.market_id)
            .or_insert_with(Vec::new)
            .push(i);
    }

    let pv = PortfolioValue::new(target, &transfers, &positions, &instruments_by_market)?;

    let rows: Vec<[String; 2]> = vec![
        ["Trades".to_string(), pv.trades_profit.to_string()],
        ["Fees".to_string(), pv.fees_profit.to_string()],
        ["Tips (in)".to_string(), pv.tips_received.to_string()],
        ["Tips (out)".to_string(), pv.tips_sent.to_string()],
        [
            "Ctransfers (in)".to_string(),
            pv.transfers_received.to_string(),
        ],
        [
            "Ctransfers (out)".to_string(),
            pv.transfers_sent.to_string(),
        ],
        ["Gambling".to_string(), pv.gambling_winnings.to_string()],
        ["Deposits".to_string(), pv.net_deposits.to_string()],
        ["".to_string(), "".to_string()],
        ["Cash".to_string(), pv.user.cash_balance.to_string()],
        ["Positions".to_string(), pv.positions_value.to_string()],
        ["".to_string(), "".to_string()],
        ["Net worth".to_string(), pv.net_worth().to_string()],
    ];

    let msg = format!("## {}\n{}", pv.user.name, code_block(&tabulate(rows)));

    command
        .create_response(&ctx.http, utils::text_interaction_response(&msg, false))
        .await?;

    Ok(())
}
