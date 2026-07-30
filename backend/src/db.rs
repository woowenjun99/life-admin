use anyhow::{Context, Result};
use sqlx::{PgPool, postgres::PgPoolOptions};

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

pub async fn connect(database_url: &str, max_connections: u32) -> Result<PgPool> {
    let database = PgPoolOptions::new()
        .max_connections(max_connections)
        .connect(database_url)
        .await
        .context("could not connect to PostgreSQL")?;

    MIGRATOR
        .run(&database)
        .await
        .context("could not apply database migrations")?;

    Ok(database)
}
