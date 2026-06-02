use std::env;

use axum::{Router, routing::get};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().expect(".env loading should succeed");

    let api_port = env::var("API_PORT").expect("API_PORT should be set");

    // build our application with a single route
    let app = Router::new().route("/", get(|| async { "Hello, World!" }));

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{api_port}"))
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}
