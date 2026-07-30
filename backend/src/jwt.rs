use anyhow::{Result, anyhow};

/// Select the process-wide provider shared by Firebase Auth and Cloud Storage.
pub fn install_default_provider() -> Result<()> {
    jsonwebtoken::crypto::aws_lc::DEFAULT_PROVIDER
        .install_default()
        .map_err(|_| anyhow!("JWT crypto provider was initialized before application startup"))
}

#[cfg(test)]
mod tests {
    use jsonwebtoken::{Algorithm, DecodingKey, crypto};

    use super::install_default_provider;

    #[test]
    fn selected_provider_verifies_jwt_signatures() {
        install_default_provider().expect("AWS-LC JWT provider should install");

        let valid = crypto::verify(
            "c0zGLzKEFWj0VxWuufTXiRMk5tlI5MbGDAYhzaxIYjo",
            b"hello world",
            &DecodingKey::from_secret(b"secret"),
            Algorithm::HS256,
        )
        .expect("selected JWT provider should verify HMAC signatures");

        assert!(valid);
    }
}
