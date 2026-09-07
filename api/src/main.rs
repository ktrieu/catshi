use std::{env, sync::Arc};

use axum::{Router, routing::get};
use common::store::CatshiDb;

use crate::state::AppState;

mod catfishing;
mod error;
mod state;

#[tokio::main]
async fn main() {
    // Optional: absent when env vars are supplied another way (e.g. docker
    // compose's `environment:` block instead of a mounted .env file).
    dotenvy::dotenv().ok();

    let api_port = env::var("API_PORT").expect("API_PORT should be set");
    let pg_url = env::var("POSTGRES_URL").expect("POSTGRES_URL should be set");

    let db = CatshiDb::new(&pg_url)
        .await
        .expect("database initialization should succeed");

    // build our application with a single route
    let app = Router::new().route("/catfishing", get(catfishing::list_games));

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{api_port}"))
        .await
        .unwrap();
    axum::serve(listener, app.with_state(AppState::new(Arc::new(db))))
        .await
        .unwrap();
}
