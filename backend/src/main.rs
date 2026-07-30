mod app;
mod auth;
mod config;
mod db;
mod domain;
mod firebase;
mod inbox;
mod storage;

use std::sync::Arc;

use anyhow::{Context, Result};
use app::{AppState, router};
use auth::FirebaseTokenVerifier;
use config::Config;
use inbox::SqlxInboxRepository;
use storage::FirebaseStorage;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "backend=debug,tower_http=info".into()),
        )
        .init();

    let config = Config::from_env()?;
    let database = db::connect(&config.database_url, config.database_max_connections).await?;
    let auth_emulator_host = firebase::auth_emulator_host();
    let firebase_auth = Arc::new(firebase::build_auth_client(
        &config.firebase_project_id,
        config.firebase_service_account_json.as_deref(),
        auth_emulator_host.as_deref(),
    )?);
    let object_store = Arc::new(
        FirebaseStorage::from_environment(
            config.firebase_storage_bucket.clone(),
            config.firebase_service_account_json.as_deref(),
        )
        .await
        .context("could not initialize Firebase Storage")?,
    );

    let app = router(AppState {
        inbox_repository: Arc::new(SqlxInboxRepository::new(database.clone())),
        object_store,
        database,
        token_verifier: Arc::new(FirebaseTokenVerifier::new(
            firebase_auth,
            config.firebase_project_id,
            auth_emulator_host.is_some(),
        )),
    });
    let listener = tokio::net::TcpListener::bind(config.bind_addr)
        .await
        .context("could not bind API listener")?;

    tracing::info!(address = %config.bind_addr, "API listening");
    axum::serve(listener, app)
        .await
        .context("API server stopped unexpectedly")
}
