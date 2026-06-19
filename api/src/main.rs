use std::env;

use axum::{Router, routing::get};
use common::store;
use sqlx::SqlitePool;

use crate::state::AppState;

mod catfishing;
mod error;
mod state;

async fn init_db(url: &str) -> anyhow::Result<SqlitePool> {
    let pool = SqlitePool::connect(url).await?;
    store::run_migrations(&pool).await?;

    Ok(pool)
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().expect(".env loading should succeed");

    let api_port = env::var("API_PORT").expect("API_PORT should be set");
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL should be set");

    let pool = init_db(&database_url)
        .await
        .expect("database initialization should succeed");

    // build our application with a single route
    let app = Router::new().route("/catfishing", get(catfishing::list_games));

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{api_port}"))
        .await
        .unwrap();
    axum::serve(listener, app.with_state(AppState::new(pool)))
        .await
        .unwrap();
}
