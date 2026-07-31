mod ai;
mod app;
mod auth;
mod config;
mod db;
mod domain;
mod firebase;
mod inbox;
mod jwt;
mod storage;

use std::sync::Arc;

use ai::{
    AiProvider, DisabledAiProvider, OpenAiProvider, ensure_cleanup_queue_is_serviceable,
    spawn_cleanup_worker,
};
use anyhow::{Context, Result};
use app::{AppState, router};
use auth::FirebaseTokenVerifier;
use config::Config;
use inbox::SqlxInboxRepository;
use storage::FirebaseStorage;

#[tokio::main]
async fn main() -> Result<()> {
    jwt::install_default_provider()?;

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
    let ai_provider: Arc<dyn AiProvider> = match config.openai_api_key {
        Some(api_key) => Arc::new(OpenAiProvider::new(
            api_key,
            config.openai_model,
            config.openai_base_url,
            config.openai_api_mode,
        )?),
        None => Arc::new(DisabledAiProvider),
    };
    ensure_cleanup_queue_is_serviceable(&database, ai_provider.as_ref()).await?;
    spawn_cleanup_worker(database.clone(), ai_provider.clone());

    let app = router(AppState {
        inbox_repository: Arc::new(SqlxInboxRepository::new(database.clone())),
        object_store,
        ai_provider,
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
