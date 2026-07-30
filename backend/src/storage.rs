use std::{borrow::Cow, env};

use async_trait::async_trait;
use gcloud_storage::{
    client::{Client, ClientConfig, google_cloud_auth::credentials::CredentialsFile},
    http::objects::{
        delete::DeleteObjectRequest,
        upload::{Media, UploadObjectRequest, UploadType},
    },
};

#[async_trait]
pub trait PrivateObjectStore: Send + Sync {
    async fn upload(
        &self,
        object_key: &str,
        content_type: &str,
        content: &[u8],
    ) -> anyhow::Result<()>;
    async fn delete(&self, object_key: &str) -> anyhow::Result<()>;
}

#[derive(Clone)]
pub struct FirebaseStorage {
    client: Client,
    bucket: String,
}

impl FirebaseStorage {
    #[allow(dead_code)] // Used by the binary; integration tests import this module directly.
    pub async fn from_environment(
        bucket: String,
        service_account_json: Option<&str>,
    ) -> anyhow::Result<Self> {
        let emulator_host = env::var("FIREBASE_STORAGE_EMULATOR_HOST")
            .ok()
            .filter(|value| !value.trim().is_empty());

        let config = if let Some(host) = emulator_host {
            ClientConfig {
                storage_endpoint: format!("http://{host}"),
                ..Default::default()
            }
            .anonymous()
        } else {
            let credentials_json = service_account_json.ok_or_else(|| {
                anyhow::anyhow!(
                    "FIREBASE_SERVICE_ACCOUNT_JSON must be set outside the Storage Emulator"
                )
            })?;
            let credentials = CredentialsFile::new_from_str(credentials_json)
                .await
                .map_err(anyhow::Error::from)?;
            ClientConfig::default()
                .with_credentials(credentials)
                .await
                .map_err(anyhow::Error::from)?
        };

        Ok(Self {
            client: Client::new(config),
            bucket,
        })
    }

    #[cfg(test)]
    fn emulator_endpoint(host: &str) -> String {
        format!("http://{host}")
    }
}

#[async_trait]
impl PrivateObjectStore for FirebaseStorage {
    async fn upload(
        &self,
        object_key: &str,
        content_type: &str,
        content: &[u8],
    ) -> anyhow::Result<()> {
        let upload = UploadType::Simple(Media {
            name: Cow::Owned(object_key.to_owned()),
            content_type: Cow::Owned(content_type.to_owned()),
            content_length: Some(content.len() as u64),
        });
        self.client
            .upload_object(
                &UploadObjectRequest {
                    bucket: self.bucket.clone(),
                    if_generation_match: Some(0),
                    ..Default::default()
                },
                content.to_vec(),
                &upload,
            )
            .await
            .map(|_| ())
            .map_err(anyhow::Error::from)
    }

    async fn delete(&self, object_key: &str) -> anyhow::Result<()> {
        self.client
            .delete_object(&DeleteObjectRequest {
                bucket: self.bucket.clone(),
                object: object_key.to_owned(),
                ..Default::default()
            })
            .await
            .map_err(anyhow::Error::from)
    }
}

#[cfg(test)]
mod tests {
    use super::FirebaseStorage;

    #[test]
    fn points_emulator_requests_at_its_json_api_host() {
        assert_eq!(
            FirebaseStorage::emulator_endpoint("127.0.0.1:9199"),
            "http://127.0.0.1:9199"
        );
    }
}
