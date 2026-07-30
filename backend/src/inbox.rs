use async_trait::async_trait;
use sqlx::{FromRow, PgPool};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::domain::{CaptureSourceType, InboxStatus};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InboxItem {
    pub id: Uuid,
    pub source_type: CaptureSourceType,
    pub status: InboxStatus,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InboxItemDetail {
    pub item: InboxItem,
    pub original_text: Option<String>,
    pub original_filename: Option<String>,
    pub content_type: Option<String>,
    pub byte_size: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileCapture {
    pub source_type: CaptureSourceType,
    pub original_filename: String,
    pub content_type: String,
    pub storage_key: String,
    pub byte_size: i64,
}

#[async_trait]
pub trait InboxRepository: Send + Sync {
    /// Every Inbox, suggestion, and plan operation must accept `owner_uid` from
    /// verified authentication and scope through its parent Inbox item.
    async fn create_text(&self, owner_uid: &str, text: &str) -> anyhow::Result<InboxItem>;
    async fn create_file(
        &self,
        owner_uid: &str,
        capture: &FileCapture,
    ) -> anyhow::Result<InboxItem>;
    async fn list(&self, owner_uid: &str) -> anyhow::Result<Vec<InboxItem>>;
    async fn get(&self, owner_uid: &str, item_id: Uuid) -> anyhow::Result<Option<InboxItemDetail>>;
}

#[derive(Clone)]
pub struct SqlxInboxRepository {
    database: PgPool,
}

impl SqlxInboxRepository {
    pub fn new(database: PgPool) -> Self {
        Self { database }
    }
}

#[derive(FromRow)]
struct InboxItemRow {
    id: Uuid,
    source_type: String,
    status: String,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

impl TryFrom<InboxItemRow> for InboxItem {
    type Error = anyhow::Error;

    fn try_from(row: InboxItemRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            source_type: CaptureSourceType::parse(&row.source_type)
                .ok_or_else(|| anyhow::anyhow!("inbox item has an invalid source type"))?,
            status: InboxStatus::parse(&row.status)
                .ok_or_else(|| anyhow::anyhow!("inbox item has an invalid status"))?,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

#[derive(FromRow)]
struct InboxItemDetailRow {
    id: Uuid,
    source_type: String,
    status: String,
    original_text: Option<String>,
    original_filename: Option<String>,
    content_type: Option<String>,
    byte_size: Option<i64>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

impl TryFrom<InboxItemDetailRow> for InboxItemDetail {
    type Error = anyhow::Error;

    fn try_from(row: InboxItemDetailRow) -> Result<Self, Self::Error> {
        let item = InboxItemRow {
            id: row.id,
            source_type: row.source_type,
            status: row.status,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
        .try_into()?;

        Ok(Self {
            item,
            original_text: row.original_text,
            original_filename: row.original_filename,
            content_type: row.content_type,
            byte_size: row.byte_size,
        })
    }
}

#[async_trait]
impl InboxRepository for SqlxInboxRepository {
    async fn create_text(&self, owner_uid: &str, text: &str) -> anyhow::Result<InboxItem> {
        let row = sqlx::query_as::<_, InboxItemRow>(
            r#"
            INSERT INTO inbox_items (owner_uid, source_type, original_text, status)
            VALUES ($1, 'text', $2, 'captured')
            RETURNING id, source_type, status, created_at, updated_at
            "#,
        )
        .bind(owner_uid)
        .bind(text)
        .fetch_one(&self.database)
        .await?;

        row.try_into()
    }

    async fn create_file(
        &self,
        owner_uid: &str,
        capture: &FileCapture,
    ) -> anyhow::Result<InboxItem> {
        let source_type = match capture.source_type {
            CaptureSourceType::Image => "image",
            CaptureSourceType::Pdf => "pdf",
            CaptureSourceType::Text => {
                anyhow::bail!("file captures cannot use the text source type")
            }
        };
        let row = sqlx::query_as::<_, InboxItemRow>(
            r#"
            INSERT INTO inbox_items (
                owner_uid, source_type, original_filename, content_type, storage_key, byte_size, status
            )
            VALUES ($1, $2, $3, $4, $5, $6, 'captured')
            RETURNING id, source_type, status, created_at, updated_at
            "#,
        )
        .bind(owner_uid)
        .bind(source_type)
        .bind(&capture.original_filename)
        .bind(&capture.content_type)
        .bind(&capture.storage_key)
        .bind(capture.byte_size)
        .fetch_one(&self.database)
        .await?;

        row.try_into()
    }

    async fn list(&self, owner_uid: &str) -> anyhow::Result<Vec<InboxItem>> {
        let rows = sqlx::query_as::<_, InboxItemRow>(
            r#"
            SELECT id, source_type, status, created_at, updated_at
            FROM inbox_items
            WHERE owner_uid = $1
            ORDER BY created_at DESC, id DESC
            "#,
        )
        .bind(owner_uid)
        .fetch_all(&self.database)
        .await?;

        rows.into_iter().map(TryInto::try_into).collect()
    }

    async fn get(&self, owner_uid: &str, item_id: Uuid) -> anyhow::Result<Option<InboxItemDetail>> {
        let row = sqlx::query_as::<_, InboxItemDetailRow>(
            r#"
            SELECT
                id,
                source_type,
                status,
                original_text,
                original_filename,
                content_type,
                byte_size,
                created_at,
                updated_at
            FROM inbox_items
            WHERE id = $1 AND owner_uid = $2
            "#,
        )
        .bind(item_id)
        .bind(owner_uid)
        .fetch_optional(&self.database)
        .await?;

        row.map(TryInto::try_into).transpose()
    }
}
