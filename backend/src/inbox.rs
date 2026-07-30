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

#[async_trait]
pub trait InboxRepository: Send + Sync {
    /// Every Inbox, suggestion, and plan operation must accept `owner_uid` from
    /// verified authentication and scope through its parent Inbox item.
    async fn create_text(&self, owner_uid: &str, text: &str) -> anyhow::Result<InboxItem>;
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

        Ok(InboxItem {
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
