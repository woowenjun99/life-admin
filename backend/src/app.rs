use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tower_http::trace::TraceLayer;
use uuid::Uuid;

use crate::{
    auth::{AuthenticatedUser, TokenVerifier},
    domain::{CaptureSourceType, InboxStatus},
    inbox::{InboxItem, InboxRepository},
};

const MAX_TEXT_CAPTURE_CHARACTERS: usize = 10_000;

#[derive(Clone)]
pub struct AppState {
    pub database: PgPool,
    pub inbox_repository: Arc<dyn InboxRepository>,
    pub token_verifier: Arc<dyn TokenVerifier>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/ready", get(readiness))
        .route("/api/v1/me", get(current_user))
        .route("/api/v1/inbox-items", post(create_text_capture))
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
    let Ok(user) = authenticated_user(&headers, state.token_verifier.as_ref()).await else {
        return unauthenticated_response();
    };

    (StatusCode::OK, Json(json!({ "user": user }))).into_response()
}

async fn create_text_capture(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Result<Json<CreateTextCaptureRequest>, JsonRejection>,
) -> Response {
    let Ok(user) = authenticated_user(&headers, state.token_verifier.as_ref()).await else {
        return unauthenticated_response();
    };
    let Ok(Json(request)) = request else {
        return validation_error_response();
    };

    if !is_valid_text_capture(&request.text) {
        return validation_error_response();
    }

    match state
        .inbox_repository
        .create_text(&user.uid, &request.text)
        .await
    {
        Ok(item) => (
            StatusCode::CREATED,
            Json(CreateTextCaptureResponse {
                inbox_item: InboxItemResponse::from(item),
            }),
        )
            .into_response(),
        Err(error) => {
            tracing::error!(%error, "could not create inbox item");
            api_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "Could not save Inbox item.",
            )
        }
    }
}

async fn authenticated_user(
    headers: &HeaderMap,
    verifier: &dyn TokenVerifier,
) -> Result<AuthenticatedUser, ()> {
    let token = bearer_token(headers).ok_or(())?;

    verifier.verify(token).await
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
    api_error_response(
        StatusCode::UNAUTHORIZED,
        "UNAUTHENTICATED",
        "Authentication required.",
    )
}

fn validation_error_response() -> Response {
    api_error_response(
        StatusCode::BAD_REQUEST,
        "VALIDATION_ERROR",
        "Text must be a non-empty value of at most 10,000 characters.",
    )
}

fn api_error_response(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(json!({
            "error": {
                "code": code,
                "message": message
            }
        })),
    )
        .into_response()
}

fn is_valid_text_capture(text: &str) -> bool {
    !text.trim().is_empty() && text.chars().count() <= MAX_TEXT_CAPTURE_CHARACTERS
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateTextCaptureRequest {
    text: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateTextCaptureResponse {
    inbox_item: InboxItemResponse,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InboxItemResponse {
    id: Uuid,
    source_type: CaptureSourceType,
    status: InboxStatus,
    created_at: String,
    updated_at: String,
}

impl From<InboxItem> for InboxItemResponse {
    fn from(item: InboxItem) -> Self {
        Self {
            id: item.id,
            source_type: item.source_type,
            status: item.status,
            created_at: format_timestamp(item.created_at),
            updated_at: format_timestamp(item.updated_at),
        }
    }
}

fn format_timestamp(timestamp: OffsetDateTime) -> String {
    timestamp
        .format(&Rfc3339)
        .expect("database timestamps must be representable as RFC 3339")
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, Mutex},
        time::Duration,
    };

    use async_trait::async_trait;
    use axum::{
        Router,
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use serde_json::{Value, json};
    use sqlx::postgres::PgPoolOptions;
    use time::OffsetDateTime;
    use tower::ServiceExt;
    use uuid::Uuid;

    use super::{AppState, is_valid_text_capture, router};
    use crate::auth::{AuthenticatedUser, TokenVerifier};
    use crate::{
        domain::{CaptureSourceType, InboxStatus},
        inbox::{InboxItem, InboxRepository},
    };

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

    #[derive(Default)]
    struct TestInboxRepository {
        created: Mutex<Vec<(String, String)>>,
        should_fail: bool,
    }

    #[async_trait]
    impl InboxRepository for TestInboxRepository {
        async fn create_text(&self, owner_uid: &str, text: &str) -> anyhow::Result<InboxItem> {
            if self.should_fail {
                anyhow::bail!("database write failed")
            }

            self.created
                .lock()
                .expect("test repository lock should not be poisoned")
                .push((owner_uid.to_owned(), text.to_owned()));

            Ok(InboxItem {
                id: Uuid::nil(),
                source_type: CaptureSourceType::Text,
                status: InboxStatus::Captured,
                created_at: OffsetDateTime::UNIX_EPOCH,
                updated_at: OffsetDateTime::UNIX_EPOCH,
            })
        }
    }

    fn test_app() -> Router {
        test_app_with(Arc::new(TestInboxRepository::default()))
    }

    fn test_app_with(inbox_repository: Arc<dyn InboxRepository>) -> Router {
        let database = PgPoolOptions::new()
            .acquire_timeout(Duration::from_millis(10))
            .connect_lazy("postgres://app:app@127.0.0.1:1/app")
            .expect("a lazy database pool should be constructible");

        router(AppState {
            database,
            inbox_repository,
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

    #[tokio::test]
    async fn text_capture_persists_the_verified_owner_and_returns_safe_metadata() {
        let repository = Arc::new(TestInboxRepository::default());
        let response = test_app_with(repository.clone())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/inbox-items")
                    .header("authorization", "Bearer valid-token")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"text":"  Renew passport  "}"#))
                    .expect("capture request should be valid"),
            )
            .await
            .expect("router should respond");

        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(
            response_json(response).await,
            json!({
                "inboxItem": {
                    "id": "00000000-0000-0000-0000-000000000000",
                    "sourceType": "text",
                    "status": "captured",
                    "createdAt": "1970-01-01T00:00:00Z",
                    "updatedAt": "1970-01-01T00:00:00Z"
                }
            })
        );
        assert_eq!(
            *repository
                .created
                .lock()
                .expect("test repository lock should not be poisoned"),
            vec![("user-123".to_owned(), "  Renew passport  ".to_owned())]
        );
    }

    #[tokio::test]
    async fn text_capture_rejects_oversized_text_without_persisting() {
        let repository = Arc::new(TestInboxRepository::default());
        let response = test_app_with(repository.clone())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/inbox-items")
                    .header("authorization", "Bearer valid-token")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({ "text": "a".repeat(10_001) }).to_string(),
                    ))
                    .expect("capture request should be valid"),
            )
            .await
            .expect("router should respond");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response_json(response).await,
            json!({
                "error": {
                    "code": "VALIDATION_ERROR",
                    "message": "Text must be a non-empty value of at most 10,000 characters."
                }
            })
        );
        assert!(
            repository
                .created
                .lock()
                .expect("test repository lock should not be poisoned")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn text_capture_rejects_malformed_json_without_persisting() {
        let repository = Arc::new(TestInboxRepository::default());
        let response = test_app_with(repository.clone())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/inbox-items")
                    .header("authorization", "Bearer valid-token")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"text":true}"#))
                    .expect("capture request should be valid"),
            )
            .await
            .expect("router should respond");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response_json(response).await,
            json!({
                "error": {
                    "code": "VALIDATION_ERROR",
                    "message": "Text must be a non-empty value of at most 10,000 characters."
                }
            })
        );
        assert!(
            repository
                .created
                .lock()
                .expect("test repository lock should not be poisoned")
                .is_empty()
        );
    }

    #[test]
    fn text_capture_counts_unicode_characters_and_rejects_blank_text() {
        assert!(is_valid_text_capture(&"🪴".repeat(10_000)));
        assert!(!is_valid_text_capture(&"🪴".repeat(10_001)));
        assert!(!is_valid_text_capture(" \n\t "));
    }

    #[tokio::test]
    async fn text_capture_rejects_owner_uid_supplied_by_the_client() {
        let repository = Arc::new(TestInboxRepository::default());
        let response = test_app_with(repository.clone())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/inbox-items")
                    .header("authorization", "Bearer valid-token")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({ "text": "Renew passport", "ownerUid": "attacker" }).to_string(),
                    ))
                    .expect("capture request should be valid"),
            )
            .await
            .expect("router should respond");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(
            repository
                .created
                .lock()
                .expect("test repository lock should not be poisoned")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn text_capture_requires_authentication() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/inbox-items")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"text":"Renew passport"}"#))
                    .expect("capture request should be valid"),
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
    async fn text_capture_hides_repository_errors() {
        let repository = Arc::new(TestInboxRepository {
            created: Mutex::new(Vec::new()),
            should_fail: true,
        });
        let response = test_app_with(repository)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/inbox-items")
                    .header("authorization", "Bearer valid-token")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"text":"Renew passport"}"#))
                    .expect("capture request should be valid"),
            )
            .await
            .expect("router should respond");

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            response_json(response).await,
            json!({
                "error": {
                    "code": "INTERNAL_ERROR",
                    "message": "Could not save Inbox item."
                }
            })
        );
    }
}
