use axum::{extract::State, http::StatusCode, Json};
use std::sync::Arc;
use crate::{AppState, redis_pool};
use shared::{LoginRequest, LoginResponse}; // Utilise tes types partagés
use uuid::Uuid;

pub async fn health_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

pub async fn login_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, StatusCode> {
    // 1. Authentification fictive
    if payload.username.is_empty() || payload.password != "1234" {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // 2. Connexion Redis
    let mut con = state.redis_client.get_async_connection().await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // 3. Recherche de serveur
    if let Some(server) = redis_pool::find_available_server(&mut con).await {
        Ok(Json(LoginResponse {
            player_id: Uuid::new_v4().to_string(),
            server,
        }))
    } else {
        Err(StatusCode::SERVICE_UNAVAILABLE) // Erreur 503
    }
}
