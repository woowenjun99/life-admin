use std::{env, net::SocketAddr};

use anyhow::{Context, Result, bail};

#[derive(Clone)]
pub struct Config {
    pub bind_addr: SocketAddr,
    pub database_url: String,
    pub database_max_connections: u32,
    pub firebase_project_id: String,
    pub firebase_service_account_json: Option<String>,
    pub firebase_storage_bucket: String,
    pub clamav_address: SocketAddr,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();

        let bind_addr = env::var("BIND_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:3001".to_owned())
            .parse()
            .context("BIND_ADDR must be a socket address, such as 127.0.0.1:3001")?;
        let database_max_connections = env::var("DATABASE_MAX_CONNECTIONS")
            .unwrap_or_else(|_| "5".to_owned())
            .parse()
            .context("DATABASE_MAX_CONNECTIONS must be an unsigned integer")?;

        if database_max_connections == 0 {
            bail!("DATABASE_MAX_CONNECTIONS must be greater than zero");
        }

        Ok(Self {
            bind_addr,
            database_url: required("DATABASE_URL")?,
            database_max_connections,
            firebase_project_id: required("FIREBASE_PROJECT_ID")?,
            firebase_service_account_json: optional("FIREBASE_SERVICE_ACCOUNT_JSON"),
            firebase_storage_bucket: required("FIREBASE_STORAGE_BUCKET")?,
            clamav_address: env::var("CLAMAV_ADDRESS")
                .unwrap_or_else(|_| "127.0.0.1:3310".to_owned())
                .parse()
                .context("CLAMAV_ADDRESS must be a socket address, such as 127.0.0.1:3310")?,
        })
    }
}

fn optional(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}

fn required(name: &str) -> Result<String> {
    let value = env::var(name).with_context(|| format!("{name} must be set"))?;

    if value.trim().is_empty() {
        bail!("{name} must not be empty");
    }

    Ok(value)
}
