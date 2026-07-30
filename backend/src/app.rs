use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{
        DefaultBodyLimit, FromRequest, Multipart, Request, State, multipart::MultipartError,
        rejection::JsonRejection,
    },
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
    inbox::{FileCapture, InboxItem, InboxRepository},
    scanner::{FileScanner, ScanResult},
    storage::PrivateObjectStore,
};

const MAX_TEXT_CAPTURE_CHARACTERS: usize = 10_000;
const MAX_FILE_BYTES: usize = 10 * 1024 * 1024;
const MAX_MULTIPART_BODY_BYTES: usize = MAX_FILE_BYTES + 1024 * 1024;

#[derive(Clone)]
pub struct AppState {
    pub database: PgPool,
    pub inbox_repository: Arc<dyn InboxRepository>,
    pub token_verifier: Arc<dyn TokenVerifier>,
    pub object_store: Arc<dyn PrivateObjectStore>,
    pub scanner: Arc<dyn FileScanner>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/ready", get(readiness))
        .route("/api/v1/me", get(current_user))
        .route("/api/v1/inbox-items", post(create_text_capture))
        .route(
            "/api/v1/inbox-items/files",
            post(create_file_capture).layer(DefaultBodyLimit::max(MAX_MULTIPART_BODY_BYTES)),
        )
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

async fn create_file_capture(State(state): State<AppState>, request: Request) -> Response {
    // Keep authentication ahead of multipart parsing so unauthenticated requests never cause a
    // body read, scanner connection, or storage/database side effect.
    let Ok(user) = authenticated_user(request.headers(), state.token_verifier.as_ref()).await
    else {
        return unauthenticated_response();
    };
    if request
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<usize>().ok())
        .is_some_and(|length| length > MAX_MULTIPART_BODY_BYTES)
    {
        return file_too_large_response();
    }

    let mut multipart = match Multipart::from_request(request, &state).await {
        Ok(multipart) => multipart,
        Err(_) => return file_validation_error_response(),
    };
    let field = match multipart.next_field().await {
        Ok(Some(field)) => field,
        Ok(None) => return file_validation_error_response(),
        Err(error) => return multipart_error_response(&error),
    };
    let field_name = field.name().map(str::to_owned);
    let filename = field.file_name().map(str::to_owned);
    let content_type = field.content_type().map(str::to_owned);
    if field_name.as_deref() != Some("file") {
        return file_validation_error_response();
    }

    let content = match field.bytes().await {
        Ok(content) => content,
        Err(error) => return multipart_error_response(&error),
    };
    if content.len() > MAX_FILE_BYTES {
        return file_too_large_response();
    }
    match multipart.next_field().await {
        Ok(None) => {}
        Ok(Some(_)) => return file_validation_error_response(),
        Err(error) => return multipart_error_response(&error),
    }

    let Some(filename) = filename.filter(|value| is_safe_filename(value)) else {
        return file_validation_error_response();
    };
    let Some((source_type, content_type)) =
        validated_file_type(content_type.as_deref(), content.as_ref())
    else {
        return file_validation_error_response();
    };

    match state.scanner.scan(content.as_ref()).await {
        Ok(ScanResult::Clean) => {}
        Ok(ScanResult::Unsafe) => return unsafe_file_response(),
        Err(_) => return scanner_unavailable_response(),
    }

    let object_key = Uuid::new_v4().to_string();
    if state
        .object_store
        .upload(&object_key, content_type, content.as_ref())
        .await
        .is_err()
    {
        return storage_unavailable_response();
    }

    let capture = FileCapture {
        source_type,
        original_filename: filename,
        content_type: content_type.to_owned(),
        storage_key: object_key.clone(),
        byte_size: content.len() as i64,
    };
    match state
        .inbox_repository
        .create_file(&user.uid, &capture)
        .await
    {
        Ok(item) => (
            StatusCode::CREATED,
            Json(CreateTextCaptureResponse {
                inbox_item: InboxItemResponse::from(item),
            }),
        )
            .into_response(),
        Err(_) => {
            if state.object_store.delete(&object_key).await.is_err() {
                tracing::error!(
                    byte_size = content.len(),
                    source_type = ?source_type,
                    "could not remove object after Inbox database failure"
                );
            }
            tracing::error!(
                byte_size = content.len(),
                source_type = ?source_type,
                "could not create Inbox item for clean uploaded file"
            );
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

fn file_validation_error_response() -> Response {
    api_error_response(
        StatusCode::BAD_REQUEST,
        "VALIDATION_ERROR",
        "Upload exactly one PDF, JPEG, or PNG file with a safe filename.",
    )
}

fn multipart_error_response(error: &MultipartError) -> Response {
    if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
        file_too_large_response()
    } else {
        file_validation_error_response()
    }
}

fn file_too_large_response() -> Response {
    api_error_response(
        StatusCode::PAYLOAD_TOO_LARGE,
        "FILE_TOO_LARGE",
        "Files must not exceed 10 MiB.",
    )
}

fn unsafe_file_response() -> Response {
    api_error_response(
        StatusCode::UNPROCESSABLE_ENTITY,
        "UNSAFE_FILE",
        "The uploaded file could not be accepted.",
    )
}

fn scanner_unavailable_response() -> Response {
    api_error_response(
        StatusCode::SERVICE_UNAVAILABLE,
        "FILE_SCAN_UNAVAILABLE",
        "File scanning is temporarily unavailable. Please try again later.",
    )
}

fn storage_unavailable_response() -> Response {
    api_error_response(
        StatusCode::SERVICE_UNAVAILABLE,
        "STORAGE_UNAVAILABLE",
        "Private file storage is temporarily unavailable. Please try again later.",
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

fn is_safe_filename(filename: &str) -> bool {
    !filename.trim().is_empty()
        && filename.len() <= 255
        && filename != "."
        && filename != ".."
        && !filename
            .chars()
            .any(|character| character.is_control() || matches!(character, '/' | '\\'))
}

fn validated_file_type(
    content_type: Option<&str>,
    content: &[u8],
) -> Option<(CaptureSourceType, &'static str)> {
    match content_type? {
        "image/jpeg" if content.starts_with(&[0xFF, 0xD8, 0xFF]) => {
            Some((CaptureSourceType::Image, "image/jpeg"))
        }
        "image/png" if content.starts_with(b"\x89PNG\r\n\x1a\n") => {
            Some((CaptureSourceType::Image, "image/png"))
        }
        "application/pdf" if content.starts_with(b"%PDF-") => {
            Some((CaptureSourceType::Pdf, "application/pdf"))
        }
        _ => None,
    }
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

    use super::{
        AppState, MAX_FILE_BYTES, is_safe_filename, is_valid_text_capture, router,
        validated_file_type,
    };
    use crate::auth::{AuthenticatedUser, TokenVerifier};
    use crate::{
        domain::{CaptureSourceType, InboxStatus},
        inbox::{FileCapture, InboxItem, InboxRepository},
        scanner::{FileScanner, ScanResult},
        storage::PrivateObjectStore,
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
        files: Mutex<Vec<(String, FileCapture)>>,
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

        async fn create_file(
            &self,
            owner_uid: &str,
            capture: &FileCapture,
        ) -> anyhow::Result<InboxItem> {
            if self.should_fail {
                anyhow::bail!("database write failed")
            }

            self.files
                .lock()
                .expect("test repository lock should not be poisoned")
                .push((owner_uid.to_owned(), capture.clone()));

            Ok(InboxItem {
                id: Uuid::nil(),
                source_type: capture.source_type,
                status: InboxStatus::Captured,
                created_at: OffsetDateTime::UNIX_EPOCH,
                updated_at: OffsetDateTime::UNIX_EPOCH,
            })
        }
    }

    #[derive(Clone, Copy)]
    enum ScannerBehavior {
        Clean,
        Unsafe,
        Unavailable,
    }

    struct TestScanner {
        behavior: ScannerBehavior,
        scanned: Mutex<Vec<usize>>,
    }

    impl Default for TestScanner {
        fn default() -> Self {
            Self {
                behavior: ScannerBehavior::Clean,
                scanned: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl FileScanner for TestScanner {
        async fn scan(&self, content: &[u8]) -> anyhow::Result<ScanResult> {
            self.scanned
                .lock()
                .expect("test scanner lock should not be poisoned")
                .push(content.len());
            match self.behavior {
                ScannerBehavior::Clean => Ok(ScanResult::Clean),
                ScannerBehavior::Unsafe => Ok(ScanResult::Unsafe),
                ScannerBehavior::Unavailable => anyhow::bail!("scanner unavailable"),
            }
        }
    }

    #[derive(Default)]
    struct TestObjectStore {
        uploaded: Mutex<Vec<(String, String, usize)>>,
        deleted: Mutex<Vec<String>>,
        fail_upload: bool,
    }

    #[async_trait]
    impl PrivateObjectStore for TestObjectStore {
        async fn upload(
            &self,
            object_key: &str,
            content_type: &str,
            content: &[u8],
        ) -> anyhow::Result<()> {
            if self.fail_upload {
                anyhow::bail!("storage unavailable")
            }
            self.uploaded
                .lock()
                .expect("test object-store lock should not be poisoned")
                .push((
                    object_key.to_owned(),
                    content_type.to_owned(),
                    content.len(),
                ));
            Ok(())
        }

        async fn delete(&self, object_key: &str) -> anyhow::Result<()> {
            self.deleted
                .lock()
                .expect("test object-store lock should not be poisoned")
                .push(object_key.to_owned());
            Ok(())
        }
    }

    fn test_app() -> Router {
        test_app_with(Arc::new(TestInboxRepository::default()))
    }

    fn test_app_with(inbox_repository: Arc<dyn InboxRepository>) -> Router {
        test_app_with_dependencies(
            inbox_repository,
            Arc::new(TestScanner::default()),
            Arc::new(TestObjectStore::default()),
        )
    }

    fn test_app_with_dependencies(
        inbox_repository: Arc<dyn InboxRepository>,
        scanner: Arc<dyn FileScanner>,
        object_store: Arc<dyn PrivateObjectStore>,
    ) -> Router {
        let database = PgPoolOptions::new()
            .acquire_timeout(Duration::from_millis(10))
            .connect_lazy("postgres://app:app@127.0.0.1:1/app")
            .expect("a lazy database pool should be constructible");

        router(AppState {
            database,
            inbox_repository,
            token_verifier: Arc::new(TestTokenVerifier),
            scanner,
            object_store,
        })
    }

    async fn response_json(response: axum::response::Response) -> Value {
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body should be readable");

        serde_json::from_slice(&bytes).expect("response should contain JSON")
    }

    fn file_request(
        filename: &str,
        content_type: &str,
        content: &[u8],
        extra_field: bool,
    ) -> Request<Body> {
        let boundary = "inbox-upload-test-boundary";
        let mut body = Vec::new();
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\nContent-Type: {content_type}\r\n\r\n"
            )
            .as_bytes(),
        );
        body.extend_from_slice(content);
        body.extend_from_slice(b"\r\n");
        if extra_field {
            body.extend_from_slice(
                format!(
                    "--{boundary}\r\nContent-Disposition: form-data; name=\"extra\"\r\n\r\nnope\r\n"
                )
                .as_bytes(),
            );
        }
        body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

        Request::builder()
            .method("POST")
            .uri("/api/v1/inbox-items/files")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(body))
            .expect("multipart request should be valid")
    }

    fn with_valid_token(request: Request<Body>) -> Request<Body> {
        let (mut parts, body) = request.into_parts();
        parts.headers.insert(
            "authorization",
            "Bearer valid-token".parse().expect("valid header"),
        );
        Request::from_parts(parts, body)
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
            files: Mutex::new(Vec::new()),
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

    #[tokio::test]
    async fn file_capture_authenticates_before_reading_or_scanning_multipart_data() {
        let repository = Arc::new(TestInboxRepository::default());
        let scanner = Arc::new(TestScanner::default());
        let object_store = Arc::new(TestObjectStore::default());
        let response =
            test_app_with_dependencies(repository.clone(), scanner.clone(), object_store.clone())
                .oneshot(file_request(
                    "note.pdf",
                    "application/pdf",
                    b"%PDF-1.7 private content",
                    false,
                ))
                .await
                .expect("router should respond");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(
            scanner
                .scanned
                .lock()
                .expect("test scanner lock should not be poisoned")
                .is_empty()
        );
        assert!(
            object_store
                .uploaded
                .lock()
                .expect("test object-store lock should not be poisoned")
                .is_empty()
        );
        assert!(
            repository
                .files
                .lock()
                .expect("test repository lock should not be poisoned")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn file_capture_accepts_each_allowed_type_and_persists_verified_owner_only() {
        let cases: [(&str, &str, &[u8], &str); 3] = [
            ("letter.pdf", "application/pdf", b"%PDF-1.7 content", "pdf"),
            (
                "photo.jpg",
                "image/jpeg",
                b"\xff\xd8\xff\xe0JPEG content",
                "image",
            ),
            (
                "diagram.png",
                "image/png",
                b"\x89PNG\r\n\x1a\nPNG content",
                "image",
            ),
        ];

        for (filename, content_type, content, expected_source_type) in cases {
            let repository = Arc::new(TestInboxRepository::default());
            let scanner = Arc::new(TestScanner::default());
            let object_store = Arc::new(TestObjectStore::default());
            let response = test_app_with_dependencies(
                repository.clone(),
                scanner.clone(),
                object_store.clone(),
            )
            .oneshot(with_valid_token(file_request(
                filename,
                content_type,
                content,
                false,
            )))
            .await
            .expect("router should respond");

            assert_eq!(response.status(), StatusCode::CREATED);
            let response_body = response_json(response).await;
            assert_eq!(
                response_body["inboxItem"]["sourceType"],
                expected_source_type
            );
            assert!(response_body["inboxItem"].get("storageKey").is_none());
            assert!(response_body["inboxItem"].get("filename").is_none());

            let uploaded = object_store
                .uploaded
                .lock()
                .expect("test object-store lock should not be poisoned");
            assert_eq!(uploaded.len(), 1);
            assert_eq!(uploaded[0].1, content_type);
            assert!(Uuid::parse_str(&uploaded[0].0).is_ok());
            drop(uploaded);

            let files = repository
                .files
                .lock()
                .expect("test repository lock should not be poisoned");
            assert_eq!(files.len(), 1);
            assert_eq!(files[0].0, "user-123");
            assert_eq!(files[0].1.original_filename, filename);
            assert_eq!(files[0].1.content_type, content_type);
        }
    }

    #[tokio::test]
    async fn file_capture_rejects_mismatched_magic_bytes_unsafe_names_and_extra_fields() {
        let invalid_requests = [
            with_valid_token(file_request(
                "not-really-a-pdf.pdf",
                "application/pdf",
                b"plain text",
                false,
            )),
            with_valid_token(file_request(
                "../escape.pdf",
                "application/pdf",
                b"%PDF-1.7 content",
                false,
            )),
            with_valid_token(file_request(
                "letter.pdf",
                "application/pdf",
                b"%PDF-1.7 content",
                true,
            )),
        ];

        for request in invalid_requests {
            let repository = Arc::new(TestInboxRepository::default());
            let scanner = Arc::new(TestScanner::default());
            let object_store = Arc::new(TestObjectStore::default());
            let response = test_app_with_dependencies(
                repository.clone(),
                scanner.clone(),
                object_store.clone(),
            )
            .oneshot(request)
            .await
            .expect("router should respond");

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            assert_eq!(
                response_json(response).await["error"]["code"],
                "VALIDATION_ERROR"
            );
            assert!(
                scanner
                    .scanned
                    .lock()
                    .expect("test scanner lock should not be poisoned")
                    .is_empty()
            );
            assert!(
                object_store
                    .uploaded
                    .lock()
                    .expect("test object-store lock should not be poisoned")
                    .is_empty()
            );
            assert!(
                repository
                    .files
                    .lock()
                    .expect("test repository lock should not be poisoned")
                    .is_empty()
            );
        }
    }

    #[tokio::test]
    async fn file_capture_accepts_exact_limit_and_rejects_oversize_before_scanning() {
        let mut exact_limit = b"%PDF-1.7\n".to_vec();
        exact_limit.resize(MAX_FILE_BYTES, b'a');
        let repository = Arc::new(TestInboxRepository::default());
        let scanner = Arc::new(TestScanner::default());
        let object_store = Arc::new(TestObjectStore::default());
        let response =
            test_app_with_dependencies(repository.clone(), scanner.clone(), object_store.clone())
                .oneshot(with_valid_token(file_request(
                    "exact.pdf",
                    "application/pdf",
                    &exact_limit,
                    false,
                )))
                .await
                .expect("router should respond");
        assert_eq!(response.status(), StatusCode::CREATED);

        let mut oversized = b"%PDF-1.7\n".to_vec();
        oversized.resize(MAX_FILE_BYTES + 1, b'a');
        let scanner = Arc::new(TestScanner::default());
        let response = test_app_with_dependencies(
            Arc::new(TestInboxRepository::default()),
            scanner.clone(),
            Arc::new(TestObjectStore::default()),
        )
        .oneshot(with_valid_token(file_request(
            "oversized.pdf",
            "application/pdf",
            &oversized,
            false,
        )))
        .await
        .expect("router should respond");
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(
            response_json(response).await["error"]["code"],
            "FILE_TOO_LARGE"
        );
        assert!(
            scanner
                .scanned
                .lock()
                .expect("test scanner lock should not be poisoned")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn file_capture_fails_closed_for_malware_and_scanner_outages() {
        for behavior in [ScannerBehavior::Unsafe, ScannerBehavior::Unavailable] {
            let repository = Arc::new(TestInboxRepository::default());
            let scanner = Arc::new(TestScanner {
                behavior,
                scanned: Mutex::new(Vec::new()),
            });
            let object_store = Arc::new(TestObjectStore::default());
            let response =
                test_app_with_dependencies(repository.clone(), scanner, object_store.clone())
                    .oneshot(with_valid_token(file_request(
                        "letter.pdf",
                        "application/pdf",
                        b"%PDF-1.7 content",
                        false,
                    )))
                    .await
                    .expect("router should respond");

            let expected = match behavior {
                ScannerBehavior::Unsafe => (StatusCode::UNPROCESSABLE_ENTITY, "UNSAFE_FILE"),
                ScannerBehavior::Unavailable => {
                    (StatusCode::SERVICE_UNAVAILABLE, "FILE_SCAN_UNAVAILABLE")
                }
                ScannerBehavior::Clean => unreachable!("only failure modes are tested"),
            };
            assert_eq!(response.status(), expected.0);
            assert_eq!(response_json(response).await["error"]["code"], expected.1);
            assert!(
                object_store
                    .uploaded
                    .lock()
                    .expect("test object-store lock should not be poisoned")
                    .is_empty()
            );
            assert!(
                repository
                    .files
                    .lock()
                    .expect("test repository lock should not be poisoned")
                    .is_empty()
            );
        }
    }

    #[tokio::test]
    async fn file_capture_reports_storage_failure_and_deletes_uploaded_object_if_database_fails() {
        let storage_failure = Arc::new(TestObjectStore {
            uploaded: Mutex::new(Vec::new()),
            deleted: Mutex::new(Vec::new()),
            fail_upload: true,
        });
        let response = test_app_with_dependencies(
            Arc::new(TestInboxRepository::default()),
            Arc::new(TestScanner::default()),
            storage_failure.clone(),
        )
        .oneshot(with_valid_token(file_request(
            "letter.pdf",
            "application/pdf",
            b"%PDF-1.7 content",
            false,
        )))
        .await
        .expect("router should respond");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            response_json(response).await["error"]["code"],
            "STORAGE_UNAVAILABLE"
        );

        let object_store = Arc::new(TestObjectStore::default());
        let response = test_app_with_dependencies(
            Arc::new(TestInboxRepository {
                created: Mutex::new(Vec::new()),
                files: Mutex::new(Vec::new()),
                should_fail: true,
            }),
            Arc::new(TestScanner::default()),
            object_store.clone(),
        )
        .oneshot(with_valid_token(file_request(
            "letter.pdf",
            "application/pdf",
            b"%PDF-1.7 content",
            false,
        )))
        .await
        .expect("router should respond");
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let uploaded = object_store
            .uploaded
            .lock()
            .expect("test object-store lock should not be poisoned");
        let deleted = object_store
            .deleted
            .lock()
            .expect("test object-store lock should not be poisoned");
        assert_eq!(uploaded.len(), 1);
        assert_eq!(deleted.as_slice(), [uploaded[0].0.clone()]);
    }

    #[test]
    fn file_type_and_filename_validation_do_not_trust_extensions_or_paths() {
        assert!(is_safe_filename("intake photo.png"));
        assert!(!is_safe_filename("../private.pdf"));
        assert!(!is_safe_filename("dir\\private.pdf"));
        assert!(!is_safe_filename("bad\nname.pdf"));
        assert_eq!(validated_file_type(Some("image/png"), b"%PDF-1.7"), None);
    }
}
