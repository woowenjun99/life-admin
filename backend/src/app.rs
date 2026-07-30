use std::sync::Arc;

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use serde_json::json;
use sqlx::PgPool;
use tower_http::trace::TraceLayer;

use crate::auth::TokenVerifier;

#[derive(Clone)]
pub struct AppState {
    pub database: PgPool,
    pub token_verifier: Arc<dyn TokenVerifier>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/ready", get(readiness))
        .route("/api/v1/me", get(current_user))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn health() -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn readiness(State(state): State<AppState>) -> impl IntoResponse {
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

async fn current_user(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(token) = bearer_token(&headers) else {
        return unauthenticated_response();
    };

    match state.token_verifier.verify(token).await {
        Ok(user) => (StatusCode::OK, Json(json!({ "user": user }))).into_response(),
        Err(()) => unauthenticated_response(),
    }
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let token = value.strip_prefix("Bearer ")?;

    if token.is_empty() || token.chars().any(char::is_whitespace) {
        return None;
    }

    Some(token)
}

fn unauthenticated_response() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({
            "error": {
                "code": "UNAUTHENTICATED",
                "message": "Authentication required."
            }
        })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use async_trait::async_trait;
    use axum::{
        Router,
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use serde_json::{Value, json};
    use sqlx::postgres::PgPoolOptions;
    use tower::ServiceExt;

    use super::{AppState, router};
    use crate::auth::{AuthenticatedUser, TokenVerifier};

    struct TestTokenVerifier;

    #[async_trait]
    impl TokenVerifier for TestTokenVerifier {
        async fn verify(&self, token: &str) -> Result<AuthenticatedUser, ()> {
            if token == "valid-token" {
                return Ok(AuthenticatedUser {
                    uid: "user-123".to_owned(),
                    email: "member@example.com".to_owned(),
                });
            }

            Err(())
        }
    }

    fn test_app() -> Router {
        let database = PgPoolOptions::new()
            .acquire_timeout(Duration::from_millis(10))
            .connect_lazy("postgres://app:app@127.0.0.1:1/app")
            .expect("a lazy database pool should be constructible");

        router(AppState {
            database,
            token_verifier: Arc::new(TestTokenVerifier),
        })
    }

    async fn response_json(response: axum::response::Response) -> Value {
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body should be readable");

        serde_json::from_slice(&bytes).expect("response should contain JSON")
    }

    #[tokio::test]
    async fn health_is_public() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .expect("health request should be valid"),
            )
            .await
            .expect("router should respond");

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn readiness_is_public() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/ready")
                    .body(Body::empty())
                    .expect("readiness request should be valid"),
            )
            .await
            .expect("router should respond");

        assert_ne!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn current_user_rejects_missing_credentials() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/me")
                    .body(Body::empty())
                    .expect("identity request should be valid"),
            )
            .await
            .expect("router should respond");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            response_json(response).await,
            json!({
                "error": {
                    "code": "UNAUTHENTICATED",
                    "message": "Authentication required."
                }
            })
        );
    }

    #[tokio::test]
    async fn current_user_rejects_malformed_credentials() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/me")
                    .header("authorization", "Basic not-a-bearer-token")
                    .body(Body::empty())
                    .expect("identity request should be valid"),
            )
            .await
            .expect("router should respond");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn current_user_rejects_invalid_tokens() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/me")
                    .header("authorization", "Bearer rejected-token")
                    .body(Body::empty())
                    .expect("identity request should be valid"),
            )
            .await
            .expect("router should respond");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn current_user_returns_a_verified_identity() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/me")
                    .header("authorization", "Bearer valid-token")
                    .body(Body::empty())
                    .expect("identity request should be valid"),
            )
            .await
            .expect("router should respond");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response_json(response).await,
            json!({
                "user": {
                    "uid": "user-123",
                    "email": "member@example.com"
                }
            })
        );
    }
}
