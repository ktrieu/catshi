use std::{env, sync::Arc};

use axum::{
    Router,
    routing::{get, post},
};
use common::store::CatshiDb;
use time::Duration;
use tower_sessions::{Expiry, SessionManagerLayer};

use crate::{session_store::PgSessionStore, state::AppState};

mod auth;
mod catfishing;
mod discord;
mod error;
mod markets;
mod portfolio;
mod session_store;
mod state;
mod trade;

#[tokio::main]
async fn main() {
    // Optional: absent when env vars are supplied another way (e.g. docker
    // compose's `environment:` block instead of a mounted .env file).
    dotenvy::dotenv().ok();

    let api_port = env::var("API_PORT").expect("API_PORT should be set");
    let pg_url = env::var("POSTGRES_URL").expect("POSTGRES_URL should be set");
    let guild_id = env::var("GUILD_ID").expect("GUILD_ID should be set");

    #[cfg(debug_assertions)]
    if auth::dev_auth_enabled() {
        eprintln!("WARNING: DEV_AUTH is set; X-Dev-User-Id header bypasses authentication");
    }

    let db = CatshiDb::new(&pg_url)
        .await
        .expect("database initialization should succeed");

    let db = Arc::new(db);

    let session_layer = SessionManagerLayer::new(PgSessionStore::new(db.clone()))
        .with_expiry(Expiry::OnInactivity(Duration::days(30)));

    let app = Router::new()
        .route("/auth/me/", get(auth::me))
        .route("/auth/redeem/{token}/", post(auth::redeem))
        .route("/portfolio/", get(portfolio::get_portfolio))
        .route("/markets/", get(markets::list_markets))
        .route("/elxn26/markets/", get(markets::get_elxn26_markets))
        .route("/catfishing/", get(catfishing::list_games))
        .route("/trade/", post(trade::post_trade))
        .route("/trade/quote/", get(trade::get_quote))
        .layer(session_layer);

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{api_port}"))
        .await
        .unwrap();
    axum::serve(listener, app.with_state(AppState::new(db, guild_id, Arc::new(discord::new_http()))))
        .await
        .unwrap();
}
