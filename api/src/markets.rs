use std::collections::HashMap;

use axum::{
    Json,
    extract::{Query, State},
};
use common::{
    currency::Currency,
    trade::{MARKET_B, calc_price},
};
use common::store::{
    instrument::{InstrumentState, InstrumentWithShares},
    market::{FullMarketWithPositions, MarketState, MarketStore},
    position::PositionWithUser,
};
use serde::{Deserialize, Serialize};

use crate::{auth::CurrentUser, error::HandlerResult, state::AppState};

const ELXN26_TAGS: &[&str] = &[
    "elxn26-senate-margin",
    "elxn26-senate-nc",
    "elxn26-senate-me",
    "elxn26-senate-mi",
    "elxn26-senate-oh",
    "elxn26-senate-ia",
    "elxn26-senate-tx",
    "elxn26-senate-ak",
    "elxn26-senate-sc",
    "elxn26-senate-ne",
    "elxn26-senate-ks",
    "elxn26-senate-ga",
    "elxn26-senate-nh",
    "elxn26-house-nj7",
];

#[derive(Serialize)]
pub struct PositionResponse {
    id: i32,
    owner_id: i32,
    owner_name: String,
    quantity: i64,
    cost_basis: i64,
}

impl From<PositionWithUser> for PositionResponse {
    fn from(p: PositionWithUser) -> Self {
        Self {
            id: p.position.id,
            owner_id: p.user.id,
            owner_name: p.user.name,
            quantity: p.position.quantity,
            cost_basis: p.position.cost_basis.into(),
        }
    }
}

#[derive(Serialize)]
pub struct InstrumentResponse {
    id: i32,
    name: String,
    state: &'static str,
    shares: i64,
    price: i64,
    positions: Vec<PositionResponse>,
}

impl InstrumentResponse {
    fn new(
        (instrument, shares): InstrumentWithShares,
        price: Currency,
        positions: Vec<PositionResponse>,
    ) -> Self {
        Self {
            id: instrument.id,
            name: instrument.name,
            state: match instrument.state {
                InstrumentState::Open => "open",
                InstrumentState::Winner => "winner",
                InstrumentState::Loser => "loser",
            },
            shares,
            price: price.into(),
            positions,
        }
    }
}

#[derive(Serialize)]
pub struct MarketResponse {
    id: i32,
    description: String,
    state: &'static str,
    owner_id: i32,
    owner_name: String,
    thread_url: Option<String>,
    instruments: Vec<InstrumentResponse>,
}

impl MarketResponse {
    fn new(full: FullMarketWithPositions, guild_id: &str) -> Self {
        let market = full.market;
        let thread_url = market
            .row
            .thread_id
            .as_ref()
            .map(|thread_id| format!("https://discord.com/channels/{guild_id}/{thread_id}"));
        let mut positions: HashMap<i32, Vec<PositionResponse>> = HashMap::new();
        for p in full.positions {
            positions
                .entry(p.position.instrument_id)
                .or_default()
                .push(p.into());
        }

        let prices: Vec<Currency> = market
            .instruments
            .iter()
            .map(|(i, _)| match i.state {
                InstrumentState::Open => calc_price(i.id, market.instruments.iter(), MARKET_B),
                InstrumentState::Winner => Currency::new_yp(1),
                InstrumentState::Loser => Currency::new_yp(0),
            })
            .collect();

        Self {
            id: market.row.id,
            description: market.row.description,
            state: match market.row.state {
                MarketState::Open => "open",
                MarketState::Closed => "closed",
            },
            owner_id: market.owner.id,
            owner_name: market.owner.name,
            thread_url,
            instruments: market
                .instruments
                .into_iter()
                .zip(prices)
                .map(|(iws, price)| {
                    let instrument_positions = positions.remove(&iws.0.id).unwrap_or_default();
                    InstrumentResponse::new(iws, price, instrument_positions)
                })
                .collect(),
        }
    }
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum StatusFilter {
    Open,
    Closed,
}

impl From<StatusFilter> for MarketState {
    fn from(s: StatusFilter) -> Self {
        match s {
            StatusFilter::Open => MarketState::Open,
            StatusFilter::Closed => MarketState::Closed,
        }
    }
}

#[derive(Deserialize)]
pub struct ListMarketsParams {
    status: Option<StatusFilter>,
    #[serde(default)]
    staked: bool,
}

pub async fn list_markets(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(params): Query<ListMarketsParams>,
) -> HandlerResult<Json<Vec<MarketResponse>>> {
    let mut conn = state.db.conn().await?;

    let rows = state
        .market_store
        .list_markets(
            &mut conn,
            params.status.map(Into::into),
            params.staked.then_some(user.id),
        )
        .await?;

    let markets = FullMarketWithPositions::new_from_rows(
        &mut conn,
        &state.instrument_store,
        &state.user_store,
        &state.position_store,
        rows,
    )
    .await?;

    Ok(Json(
        markets
            .into_iter()
            .map(|m| MarketResponse::new(m, &state.guild_id))
            .collect(),
    ))
}

pub async fn get_elxn26_markets(
    State(state): State<AppState>,
    _user: CurrentUser,
) -> HandlerResult<Json<HashMap<String, MarketResponse>>> {
    let tags: Vec<String> = ELXN26_TAGS.iter().map(|t| (*t).to_owned()).collect();

    let mut conn = state.db.conn().await?;

    let markets = FullMarketWithPositions::new_from_tags(
        &mut conn,
        &state.market_store,
        &state.instrument_store,
        &state.user_store,
        &state.position_store,
        &tags,
    )
    .await?;

    let response = markets
        .into_iter()
        .filter_map(|m| m.market.row.tag.clone().map(|tag| (tag, MarketResponse::new(m, &state.guild_id))))
        .collect();

    Ok(Json(response))
}
