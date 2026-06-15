mod handler;
mod redis_pool;

use axum::{routing::{get, post}, Router};
use std::sync::Arc;

pub struct AppState {
    pub redis_client: redis::Client,
}

#[tokio::main]
async fn main() {
    let redis_url = "redis://127.0.0.1:6379";
    let state = Arc::new(AppState {
        redis_client: redis_pool::create_client(redis_url),
    });

    let app = Router::new()
        .route("/health", get(handler::health_handler))
        .route("/login", post(handler::login_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Gatekeeper en écoute sur http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}


