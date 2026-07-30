use std::sync::Arc;

use axum::{Router, extract::State, http::StatusCode, response::IntoResponse, routing::get};
use firebase_admin::auth::AuthClient;
use serde_json::json;
use sqlx::PgPool;
use tower_http::trace::TraceLayer;

#[derive(Clone)]
pub struct AppState {
    pub database: PgPool,
    pub firebase_auth: Arc<AuthClient>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/ready", get(readiness))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn health() -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn readiness(State(state): State<AppState>) -> impl IntoResponse {
    // The Firebase Admin client is constructed during startup and shared by
    // authenticated routes that are added to this router.
    let _firebase_auth = &state.firebase_auth;

    match sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.database)
        .await
    {
        Ok(_) => (StatusCode::OK, axum::Json(json!({ "status": "ok" }))),
        Err(error) => {
            tracing::error!(%error, "PostgreSQL readiness check failed");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                axum::Json(json!({ "status": "database_unavailable" })),
            )
        }
    }
}
