use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use firebase_admin::auth::AuthClient;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AuthenticatedUser {
    pub uid: String,
    pub email: String,
}

#[async_trait]
pub trait TokenVerifier: Send + Sync {
    async fn verify(&self, token: &str) -> Result<AuthenticatedUser, ()>;
}

pub struct FirebaseTokenVerifier {
    client: Arc<AuthClient>,
    project_id: String,
    uses_auth_emulator: bool,
}

impl FirebaseTokenVerifier {
    pub fn new(
        client: Arc<AuthClient>,
        project_id: impl Into<String>,
        uses_auth_emulator: bool,
    ) -> Self {
        Self {
            client,
            project_id: project_id.into(),
            uses_auth_emulator,
        }
    }
}

#[async_trait]
impl TokenVerifier for FirebaseTokenVerifier {
    async fn verify(&self, token: &str) -> Result<AuthenticatedUser, ()> {
        if self.uses_auth_emulator {
            return verify_emulator_token(token, &self.project_id);
        }

        let claims = self.client.verify_id_token(token).await.map_err(|_| ())?;
        let email = claims
            .email
            .filter(|email| !email.trim().is_empty())
            .ok_or(())?;

        Ok(AuthenticatedUser {
            uid: claims.sub,
            email,
        })
    }
}

#[derive(Deserialize)]
struct EmulatorTokenHeader {
    alg: String,
}

#[derive(Deserialize)]
struct EmulatorTokenClaims {
    iss: String,
    aud: String,
    iat: i64,
    exp: i64,
    auth_time: i64,
    sub: String,
    email: Option<String>,
}

fn verify_emulator_token(token: &str, project_id: &str) -> Result<AuthenticatedUser, ()> {
    let mut segments = token.split('.');
    let header = decode_segment::<EmulatorTokenHeader>(segments.next().ok_or(())?)?;
    let claims = decode_segment::<EmulatorTokenClaims>(segments.next().ok_or(())?)?;
    let signature = segments.next().ok_or(())?;

    if segments.next().is_some() || header.alg != "none" || !signature.is_empty() {
        return Err(());
    }

    let expected_issuer = format!("https://securetoken.google.com/{project_id}");
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ())?
        .as_secs() as i64;
    let email = claims
        .email
        .filter(|email| !email.trim().is_empty())
        .ok_or(())?;

    if claims.iss != expected_issuer
        || claims.aud != project_id
        || claims.sub.trim().is_empty()
        || claims.iat > now
        || claims.auth_time > now
        || claims.exp <= now
    {
        return Err(());
    }

    Ok(AuthenticatedUser {
        uid: claims.sub,
        email,
    })
}

fn decode_segment<T: DeserializeOwned>(segment: &str) -> Result<T, ()> {
    let decoded = URL_SAFE_NO_PAD.decode(segment).map_err(|_| ())?;
    serde_json::from_slice(&decoded).map_err(|_| ())
}

#[cfg(test)]
mod tests {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    use serde_json::json;

    use super::verify_emulator_token;

    fn unsigned_token(claims: serde_json::Value) -> String {
        let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"none","typ":"JWT"}"#);
        let claims = URL_SAFE_NO_PAD.encode(claims.to_string());

        format!("{header}.{claims}.")
    }

    fn current_time() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after the Unix epoch")
            .as_secs() as i64
    }

    #[test]
    fn accepts_a_current_emulator_token_for_the_expected_project() {
        let now = current_time();
        let token = unsigned_token(json!({
            "iss": "https://securetoken.google.com/demo-life-inbox",
            "aud": "demo-life-inbox",
            "iat": now - 10,
            "exp": now + 3600,
            "auth_time": now - 10,
            "sub": "user-123",
            "email": "member@example.com"
        }));

        let user = verify_emulator_token(&token, "demo-life-inbox")
            .expect("a current token for this emulator project should be accepted");

        assert_eq!(user.uid, "user-123");
        assert_eq!(user.email, "member@example.com");
    }

    #[test]
    fn rejects_an_emulator_token_for_another_project() {
        let now = current_time();
        let token = unsigned_token(json!({
            "iss": "https://securetoken.google.com/other-project",
            "aud": "other-project",
            "iat": now - 10,
            "exp": now + 3600,
            "auth_time": now - 10,
            "sub": "user-123",
            "email": "member@example.com"
        }));

        assert!(verify_emulator_token(&token, "demo-life-inbox").is_err());
    }
}
