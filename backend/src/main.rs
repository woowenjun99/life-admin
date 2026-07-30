mod app;
mod config;
mod db;
mod firebase;

use std::sync::Arc;

use anyhow::{Context, Result};
use app::{AppState, router};
use config::Config;

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
    let firebase_auth = Arc::new(firebase::build_auth_client(
        &config.firebase_project_id,
        config.firebase_service_account_json.as_deref(),
    )?);

    let app = router(AppState {
        database,
        firebase_auth,
    });
    let listener = tokio::net::TcpListener::bind(config.bind_addr)
        .await
        .context("could not bind API listener")?;

    tracing::info!(address = %config.bind_addr, "API listening");
    axum::serve(listener, app)
        .await
        .context("API server stopped unexpectedly")
}
