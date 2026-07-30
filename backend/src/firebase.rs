use std::env;

use anyhow::{Context, Result, bail};
use firebase_admin::{auth::AuthClient, core::ServiceAccountKey};

pub fn auth_emulator_host() -> Option<String> {
    env::var("FIREBASE_AUTH_EMULATOR_HOST")
        .ok()
        .filter(|host| !host.trim().is_empty())
}

pub fn build_auth_client(
    project_id: &str,
    service_account_json: Option<&str>,
    auth_emulator_host: Option<&str>,
) -> Result<AuthClient> {
    let builder = AuthClient::builder(project_id);
    match auth_emulator_host {
        Some(host) => builder
            .use_emulator(host.to_owned())
            .build()
            .context("could not initialize Firebase Admin Auth"),
        _ => build_live_auth_client(builder, project_id, service_account_json),
    }
}

fn build_live_auth_client(
    builder: firebase_admin::auth::AuthClientBuilder,
    project_id: &str,
    service_account_json: Option<&str>,
) -> Result<AuthClient> {
    let service_account_json = service_account_json
        .context("FIREBASE_SERVICE_ACCOUNT_JSON must be set outside the Firebase emulator")?;
    let service_account = ServiceAccountKey::from_json(service_account_json).context(
        "FIREBASE_SERVICE_ACCOUNT_JSON must contain a valid service-account JSON object",
    )?;

    if service_account.project_id != project_id {
        bail!("FIREBASE_PROJECT_ID must match the project_id in FIREBASE_SERVICE_ACCOUNT_JSON");
    }

    builder
        .service_account_key(service_account)
        .build()
        .context("could not initialize Firebase Admin Auth")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SERVICE_ACCOUNT_JSON: &str = r#"{
        "client_email": "test@demo-backend.iam.gserviceaccount.com",
        "private_key": "not-a-real-key",
        "project_id": "demo-backend",
        "private_key_id": "test-key-id"
    }"#;

    #[test]
    fn direct_service_account_json_initializes_live_auth_client() {
        let client = build_live_auth_client(
            AuthClient::builder("demo-backend"),
            "demo-backend",
            Some(SERVICE_ACCOUNT_JSON),
        )
        .expect("valid service-account JSON should initialize the Admin client");

        assert_eq!(client.project_id().as_str(), "demo-backend");
    }

    #[test]
    fn direct_service_account_json_must_match_the_configured_project() {
        let error = build_live_auth_client(
            AuthClient::builder("other-project"),
            "other-project",
            Some(SERVICE_ACCOUNT_JSON),
        )
        .err()
        .expect("a different Firebase project must be rejected");

        assert!(error.to_string().contains("FIREBASE_PROJECT_ID must match"));
    }
}
