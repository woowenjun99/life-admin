#[path = "../src/app.rs"]
mod app;
#[path = "../src/auth.rs"]
mod auth;
#[path = "../src/domain.rs"]
mod domain;
#[path = "../src/inbox.rs"]
mod inbox;
#[path = "../src/scanner.rs"]
mod scanner;
#[path = "../src/storage.rs"]
mod storage;

use std::sync::Arc;

use app::{AppState, router};
use async_trait::async_trait;
use auth::FirebaseTokenVerifier;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use firebase_admin::auth::AuthClient;
use inbox::SqlxInboxRepository;
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;

struct HealthScanner;

#[async_trait]
impl scanner::FileScanner for HealthScanner {
    async fn scan(&self, _content: &[u8]) -> anyhow::Result<scanner::ScanResult> {
        Ok(scanner::ScanResult::Clean)
    }
}

struct HealthObjectStore;

#[async_trait]
impl storage::PrivateObjectStore for HealthObjectStore {
    async fn upload(
        &self,
        _object_key: &str,
        _content_type: &str,
        _content: &[u8],
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn delete(&self, _object_key: &str) -> anyhow::Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn health_endpoint_does_not_require_database_connectivity() {
    let database = PgPoolOptions::new()
        .connect_lazy("postgres://app:app@127.0.0.1:5432/app")
        .expect("valid PostgreSQL URL");
    let firebase_auth = AuthClient::builder("demo-backend")
        .use_emulator("127.0.0.1:9099")
        .build()
        .expect("Firebase emulator client should initialize");
    let app = router(AppState {
        inbox_repository: Arc::new(SqlxInboxRepository::new(database.clone())),
        database,
        object_store: Arc::new(HealthObjectStore),
        scanner: Arc::new(HealthScanner),
        token_verifier: Arc::new(FirebaseTokenVerifier::new(
            Arc::new(firebase_auth),
            "demo-backend",
            true,
        )),
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("router should respond");

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}
