use std::str::FromStr;

use serenity::all::{
    ComponentInteraction, Context, CreateInteractionResponse, CreateMessage, EditMessage,
    ModalInteraction, ThreadId, UserId,
};

use crate::{
    bot::Handler,
    trade::{self, TradeResult, calc_buy_prices, calc_sell_prices},
    ui::{
        self, create_market_thread, instrument_display_text,
        market_message::{render_details_message, render_market_message},
        trade_flow::{create_trade_modal, extract_quantity_from_trade_modal},
    },
    utils,
};
use common::currency::Currency;
use common::discord_ui::render_trade_thread_message;
pub use common::place_trade::TradeAction;
use common::place_trade::{PlaceTradeError, PlacedTrade, TradeRequest, place_trade};
use common::store::{
    instrument::{Instrument, InstrumentStore},
    market::MarketStore,
    position::PositionStore,
    user::DbUser,
};

pub fn parse_trade_button_id(id: &str) -> Option<(TradeAction, i32)> {
    let components: Vec<&str> = id.split('|').collect();

    if components.len() != 3 {
        return None;
    }

    if components[0] != "trade_button" {
        return None;
    }

    let action = TradeAction::from_str(components[1]).ok()?;
    let id = components[2].parse::<i32>().ok()?;

    Some((action, id))
}

async fn calc_max_buy_shares(
    handler: &Handler,
    balance: Currency,
    market_id: i32,
    instrument_id: i32,
) -> anyhow::Result<i64> {
    let mut conn = handler.db.conn().await?;
    let shares = handler
        .instrument_store
        .get_instruments_with_share_counts_for_market(&mut conn, market_id)
        .await?;

    let (mut max_shares, prices) =
        trade::get_max_buy_shares(balance, instrument_id, shares.iter(), trade::MARKET_B);

    // If adding fees puts us over the top subtract our max_shares by 1.
    if prices.total() > balance && max_shares > 0 {
        max_shares -= 1;
    };

    Ok(max_shares)
}

async fn calc_max_sell_shares(
    handler: &Handler,
    instrument: &Instrument,
    user: &DbUser,
) -> anyhow::Result<i64> {
    let mut conn = handler.db.conn().await?;
    let position = handler
        .position_store
        .get_user_position(&mut conn, instrument, user)
        .await?;

    match position {
        Some(position) => Ok(position.quantity),
        // No position = no sell
        None => Ok(0),
    }
}

fn get_prefilled_quantity(
    quantity: i64,
    instrument_id: i32,
    instruments: &Vec<(Instrument, i64)>,
    action: TradeAction,
) -> (i64, Currency) {
    // TODO: we should really harmonize this.
    let total = match action {
        TradeAction::Buy => {
            calc_buy_prices(quantity, instrument_id, instruments.iter(), trade::MARKET_B).total()
        }
        TradeAction::Sell => {
            calc_sell_prices(quantity, instrument_id, instruments.iter(), trade::MARKET_B).total()
        }
    };

    (quantity, total)
}

const PREFILLED_QUANTITIES: [i64; 5] = [1, 2, 5, 10, 20];

pub async fn initiate_trade(
    ctx: &Context,
    handler: &Handler,
    user: &DbUser,
    component: &ComponentInteraction,
    action: TradeAction,
    instrument_id: i32,
) -> anyhow::Result<()> {
    let mut conn = handler.db.conn().await?;
    let market = handler
        .market_store
        .get_market_by_instrument_id(&mut conn, instrument_id)
        .await?;
    let instruments = handler
        .instrument_store
        .get_instruments_with_share_counts_for_market(&mut conn, market.id)
        .await?;

    let instrument = handler
        .instrument_store
        .get_instrument_by_id(&mut conn, instrument_id)
        .await?;

    let max_shares = match action {
        TradeAction::Buy => {
            calc_max_buy_shares(handler, user.cash_balance, market.id, instrument_id).await?
        }
        TradeAction::Sell => calc_max_sell_shares(handler, &instrument, user).await?,
    };

    let mut prefilled: Vec<(i64, Currency)> = Vec::new();
    for q in PREFILLED_QUANTITIES {
        if q < max_shares {
            prefilled.push(get_prefilled_quantity(
                q,
                instrument_id,
                &instruments,
                action,
            ));
        }
    }

    prefilled.push(get_prefilled_quantity(
        max_shares,
        instrument_id,
        &instruments,
        action,
    ));

    let modal = create_trade_modal(
        action,
        &market,
        &instrument,
        max_shares,
        prefilled,
        user.cash_balance,
    );

    component
        .create_response(&ctx.http, CreateInteractionResponse::Modal(modal))
        .await?;

    Ok(())
}

pub async fn trade(
    ctx: &Context,
    handler: &Handler,
    user: &DbUser,
    modal: &ModalInteraction,
    action: TradeAction,
    instrument_id: i32,
) -> anyhow::Result<()> {
    let quantity = match extract_quantity_from_trade_modal(modal) {
        Some(quantity) => quantity,
        None => {
            modal
                .create_response(
                    &ctx.http,
                    utils::text_interaction_response("Quantity was not a valid number.", true),
                )
                .await?;
            return Ok(());
        }
    };

    let mut tx = handler.db.begin().await?;

    let request = TradeRequest {
        market_id: None,
        instrument_id,
        action,
        quantity,
    };

    let placed = place_trade(
        &mut tx,
        &handler.market_store,
        &handler.instrument_store,
        &handler.user_store,
        &handler.position_store,
        &handler.order_store,
        &handler.transfer_store,
        user.id,
        request,
    )
    .await;

    let PlacedTrade {
        market,
        instrument: traded_instrument,
        result,
        ..
    } = match placed {
        Ok(placed) => placed,
        Err(PlaceTradeError::Internal(e)) => return Err(e),
        Err(e) => {
            let message = e.user_message().unwrap_or_default();
            modal
                .create_response(
                    &ctx.http,
                    utils::text_interaction_response(message.as_str(), true),
                )
                .await?;
            return Ok(());
        }
    };
    let traded_instrument = &traded_instrument;

    let verb = match action {
        TradeAction::Buy => "Bought",
        TradeAction::Sell => "Sold",
    };
    let fee_sign = match action {
        TradeAction::Buy => "+",
        TradeAction::Sell => "-",
    };

    let TradeResult {
        quantity, prices, ..
    } = result;
    let total = prices.total();
    let disp = instrument_display_text(traded_instrument, &market.row);
    let shares_price = prices.shares_price;
    let fees = prices.fees;

    let prices_msg = format!("Total: {total} ({shares_price} {fee_sign} {fees} fees)");

    let msg = format!("{verb} {quantity} shares of {disp}. {prices_msg}",);
    modal
        .create_response(&ctx.http, utils::text_interaction_response(&msg, true))
        .await?;

    if market.row.thread_id.is_some() {
        let thread_msg = render_trade_thread_message(
            &user.name,
            action,
            quantity,
            &traded_instrument.name,
            &prices,
        );
        // This function doesn't return an Option because ideally
        // all market should have a thread. We will backfill later to make this true.
        let thread_id = ui::get_market_thread_id(&market.row)?;
        thread_id
            .send_message(&ctx.http, CreateMessage::new().content(thread_msg))
            .await?;

        let user_id = user.discord_id.parse::<u64>()?;
        // Add the user to the thread on trade.
        ctx.http
            .add_thread_channel_member(ThreadId::from(thread_id.get()), UserId::new(user_id))
            .await?;
    }

    tx.commit().await?;

    let mut conn = handler.db.conn().await?;

    // Refetch the instruments and positions after the trade is complete to update the market.
    let instruments = handler
        .instrument_store
        .get_instruments_with_share_counts_for_market(&mut conn, market.row.id)
        .await?;

    let all_positions = handler
        .position_store
        .get_all_market_positions(&mut conn, market.row.id)
        .await?;

    let new_market_message = render_market_message(&market.row, &market.owner, instruments.iter());
    let mut market_message = ui::get_market_message(&market.row, ctx).await?;
    market_message
        .edit(&ctx.http, EditMessage::new().components(new_market_message))
        .await?;

    let new_details_message = render_details_message(&instruments, &all_positions);

    // Backfill thread if it doesn't exist - we're rolling this out to potential existing market messages.
    if market.row.thread_id.is_none() {
        let thread = create_market_thread(
            &market.row,
            market_message.channel_id.expect_channel(),
            market_message.id,
            &ctx.http,
        )
        .await?;

        let details_msg = thread
            .send_message(&ctx.http, CreateMessage::new().content(new_details_message))
            .await?;

        handler
            .market_store
            .set_market_message_id(
                &mut conn,
                market.row.id,
                market_message.id,
                market_message.channel_id,
                thread.id,
                details_msg.id,
            )
            .await?;
    } else {
        let mut details_message = ui::get_details_msg(&market.row, ctx).await?;

        details_message
            .edit(&ctx.http, EditMessage::new().content(new_details_message))
            .await?;
    }

    Ok(())
}
