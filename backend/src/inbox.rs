use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use time::{Date, OffsetDateTime};
use uuid::Uuid;

use crate::domain::{
    CaptureSourceType, InboxStatus, PlanStatus, PlanStepState, derived_plan_status,
    highlighted_next_action,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InboxItem {
    pub id: Uuid,
    pub plan_id: Option<Uuid>,
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
    pub suggestions: Vec<Suggestion>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileReference {
    pub storage_key: String,
    pub content_type: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileCapture {
    pub source_type: CaptureSourceType,
    pub original_filename: String,
    pub content_type: String,
    pub storage_key: String,
    pub byte_size: i64,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionKind {
    Task,
    Date,
    Person,
    Context,
    Question,
}

impl SuggestionKind {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "task" => Some(Self::Task),
            "date" => Some(Self::Date),
            "person" => Some(Self::Person),
            "context" => Some(Self::Context),
            "question" => Some(Self::Question),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Task => "task",
            Self::Date => "date",
            Self::Person => "person",
            Self::Context => "context",
            Self::Question => "question",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewSuggestion {
    pub kind: SuggestionKind,
    pub content: String,
    pub due_on: Option<Date>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suggestion {
    pub id: Uuid,
    pub kind: SuggestionKind,
    pub content: String,
    pub due_on: Option<Date>,
    pub position: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewPlanStep {
    pub title: String,
    pub rationale: String,
    pub status: PlanStatus,
    pub due_on: Option<Date>,
    pub waiting_on: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewPlan {
    pub summary: String,
    pub steps: Vec<NewPlanStep>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanStep {
    pub id: Uuid,
    pub position: i32,
    pub title: String,
    pub rationale: String,
    pub status: PlanStatus,
    pub due_on: Option<Date>,
    pub waiting_on: Option<String>,
    pub is_next_action: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    pub id: Uuid,
    pub inbox_item_id: Uuid,
    pub summary: String,
    pub status: PlanStatus,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub steps: Vec<PlanStep>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanStepUpdate {
    pub status: PlanStatus,
    pub waiting_on: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdatePlanStepResult {
    Updated(Plan),
    NotFound,
    InvalidState,
}

#[async_trait]
pub trait InboxRepository: Send + Sync {
    async fn create_text(&self, owner_uid: &str, text: &str) -> anyhow::Result<InboxItem>;
    async fn create_file(
        &self,
        owner_uid: &str,
        capture: &FileCapture,
    ) -> anyhow::Result<InboxItem>;
    async fn list(&self, owner_uid: &str) -> anyhow::Result<Vec<InboxItem>>;
    async fn get(&self, owner_uid: &str, item_id: Uuid) -> anyhow::Result<Option<InboxItemDetail>>;

    async fn get_file(
        &self,
        _owner_uid: &str,
        _item_id: Uuid,
    ) -> anyhow::Result<Option<FileReference>> {
        anyhow::bail!("file retrieval is not implemented")
    }

    async fn save_extraction(
        &self,
        _owner_uid: &str,
        _item_id: Uuid,
        _suggestions: &[NewSuggestion],
    ) -> anyhow::Result<Option<InboxItemDetail>> {
        anyhow::bail!("extraction persistence is not implemented")
    }

    async fn replace_suggestions(
        &self,
        _owner_uid: &str,
        _item_id: Uuid,
        _suggestions: &[NewSuggestion],
    ) -> anyhow::Result<Option<InboxItemDetail>> {
        anyhow::bail!("suggestion updates are not implemented")
    }

    async fn create_plan(
        &self,
        _owner_uid: &str,
        _item_id: Uuid,
        _plan: &NewPlan,
    ) -> anyhow::Result<Option<Plan>> {
        anyhow::bail!("plan persistence is not implemented")
    }

    async fn get_plan(&self, _owner_uid: &str, _plan_id: Uuid) -> anyhow::Result<Option<Plan>> {
        anyhow::bail!("plan retrieval is not implemented")
    }

    async fn update_plan_step(
        &self,
        _owner_uid: &str,
        _plan_id: Uuid,
        _step_id: Uuid,
        _update: &PlanStepUpdate,
    ) -> anyhow::Result<UpdatePlanStepResult> {
        anyhow::bail!("plan step updates are not implemented")
    }
}

#[derive(Clone)]
pub struct SqlxInboxRepository {
    database: PgPool,
}

impl SqlxInboxRepository {
    pub fn new(database: PgPool) -> Self {
        Self { database }
    }

    async fn suggestions(&self, owner_uid: &str, item_id: Uuid) -> anyhow::Result<Vec<Suggestion>> {
        let rows = sqlx::query_as::<_, SuggestionRow>(
            r#"
            SELECT s.id, s.kind, s.content, s.due_on, s.position
            FROM extraction_suggestions s
            INNER JOIN inbox_items i ON i.id = s.inbox_item_id
            WHERE s.inbox_item_id = $1 AND i.owner_uid = $2
            ORDER BY s.position ASC
            "#,
        )
        .bind(item_id)
        .bind(owner_uid)
        .fetch_all(&self.database)
        .await?;

        rows.into_iter().map(TryInto::try_into).collect()
    }

    async fn replace_suggestions_in_transaction(
        &self,
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        item_id: Uuid,
        suggestions: &[NewSuggestion],
    ) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM extraction_suggestions WHERE inbox_item_id = $1")
            .bind(item_id)
            .execute(&mut **transaction)
            .await?;
        for (position, suggestion) in suggestions.iter().enumerate() {
            sqlx::query(
                r#"
                INSERT INTO extraction_suggestions (inbox_item_id, kind, content, due_on, position)
                VALUES ($1, $2, $3, $4, $5)
                "#,
            )
            .bind(item_id)
            .bind(suggestion.kind.as_str())
            .bind(&suggestion.content)
            .bind(suggestion.due_on)
            .bind(position as i32)
            .execute(&mut **transaction)
            .await?;
        }
        Ok(())
    }
}

#[derive(FromRow)]
struct InboxItemRow {
    id: Uuid,
    plan_id: Option<Uuid>,
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
            plan_id: row.plan_id,
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
    plan_id: Option<Uuid>,
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
            plan_id: row.plan_id,
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
            suggestions: Vec::new(),
        })
    }
}

#[derive(FromRow)]
struct SuggestionRow {
    id: Uuid,
    kind: String,
    content: String,
    due_on: Option<Date>,
    position: i32,
}

impl TryFrom<SuggestionRow> for Suggestion {
    type Error = anyhow::Error;

    fn try_from(row: SuggestionRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            kind: SuggestionKind::parse(&row.kind)
                .ok_or_else(|| anyhow::anyhow!("suggestion has an invalid kind"))?,
            content: row.content,
            due_on: row.due_on,
            position: row.position,
        })
    }
}

#[derive(FromRow)]
struct PlanRow {
    id: Uuid,
    inbox_item_id: Uuid,
    summary: String,
    status: String,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

#[derive(FromRow)]
struct PlanStepRow {
    id: Uuid,
    position: i32,
    title: String,
    rationale: String,
    status: String,
    due_on: Option<Date>,
    waiting_on: Option<String>,
    is_next_action: bool,
}

#[derive(FromRow)]
struct LockedPlanStepRow {
    id: Uuid,
    position: i32,
    status: String,
}

impl TryFrom<PlanStepRow> for PlanStep {
    type Error = anyhow::Error;

    fn try_from(row: PlanStepRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            position: row.position,
            title: row.title,
            rationale: row.rationale,
            status: PlanStatus::parse(&row.status)
                .ok_or_else(|| anyhow::anyhow!("plan step has an invalid status"))?,
            due_on: row.due_on,
            waiting_on: row.waiting_on,
            is_next_action: row.is_next_action,
        })
    }
}

#[async_trait]
impl InboxRepository for SqlxInboxRepository {
    async fn create_text(&self, owner_uid: &str, text: &str) -> anyhow::Result<InboxItem> {
        let row = sqlx::query_as::<_, InboxItemRow>(
            "INSERT INTO inbox_items (owner_uid, source_type, original_text, status) VALUES ($1, 'text', $2, 'captured') RETURNING id, NULL::UUID AS plan_id, source_type, status, created_at, updated_at",
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
            "INSERT INTO inbox_items (owner_uid, source_type, original_filename, content_type, storage_key, byte_size, status) VALUES ($1, $2, $3, $4, $5, $6, 'captured') RETURNING id, NULL::UUID AS plan_id, source_type, status, created_at, updated_at",
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
            "SELECT i.id, p.id AS plan_id, i.source_type, i.status, i.created_at, i.updated_at FROM inbox_items i LEFT JOIN plans p ON p.inbox_item_id = i.id WHERE i.owner_uid = $1 ORDER BY i.created_at DESC, i.id DESC",
        )
        .bind(owner_uid)
        .fetch_all(&self.database)
        .await?;
        rows.into_iter().map(TryInto::try_into).collect()
    }

    async fn get(&self, owner_uid: &str, item_id: Uuid) -> anyhow::Result<Option<InboxItemDetail>> {
        let row = sqlx::query_as::<_, InboxItemDetailRow>(
            "SELECT i.id, p.id AS plan_id, i.source_type, i.status, i.original_text, i.original_filename, i.content_type, i.byte_size, i.created_at, i.updated_at FROM inbox_items i LEFT JOIN plans p ON p.inbox_item_id = i.id WHERE i.id = $1 AND i.owner_uid = $2",
        )
        .bind(item_id)
        .bind(owner_uid)
        .fetch_optional(&self.database)
        .await?;
        let Some(row) = row else { return Ok(None) };
        let mut detail: InboxItemDetail = row.try_into()?;
        detail.suggestions = self.suggestions(owner_uid, item_id).await?;
        Ok(Some(detail))
    }

    async fn get_file(
        &self,
        owner_uid: &str,
        item_id: Uuid,
    ) -> anyhow::Result<Option<FileReference>> {
        let row = sqlx::query_as::<_, (Option<String>, Option<String>)>(
            "SELECT storage_key, content_type FROM inbox_items WHERE id = $1 AND owner_uid = $2 AND source_type = 'pdf'",
        )
        .bind(item_id)
        .bind(owner_uid)
        .fetch_optional(&self.database)
        .await?;
        row.map(
            |(storage_key, content_type)| match (storage_key, content_type) {
                (Some(storage_key), Some(content_type)) => Ok(FileReference {
                    storage_key,
                    content_type,
                }),
                _ => Err(anyhow::anyhow!(
                    "PDF Inbox item is missing private storage metadata"
                )),
            },
        )
        .transpose()
    }

    async fn save_extraction(
        &self,
        owner_uid: &str,
        item_id: Uuid,
        suggestions: &[NewSuggestion],
    ) -> anyhow::Result<Option<InboxItemDetail>> {
        let mut transaction = self.database.begin().await?;
        let status = sqlx::query_scalar::<_, String>(
            "SELECT status FROM inbox_items WHERE id = $1 AND owner_uid = $2 FOR UPDATE",
        )
        .bind(item_id)
        .bind(owner_uid)
        .fetch_optional(&mut *transaction)
        .await?;
        if status.as_deref() != Some("captured") {
            transaction.rollback().await?;
            return Ok(None);
        }
        self.replace_suggestions_in_transaction(&mut transaction, item_id, suggestions)
            .await?;
        sqlx::query(
            "UPDATE inbox_items SET status = 'reviewing', updated_at = now() WHERE id = $1",
        )
        .bind(item_id)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        self.get(owner_uid, item_id).await
    }

    async fn replace_suggestions(
        &self,
        owner_uid: &str,
        item_id: Uuid,
        suggestions: &[NewSuggestion],
    ) -> anyhow::Result<Option<InboxItemDetail>> {
        let mut transaction = self.database.begin().await?;
        let status = sqlx::query_scalar::<_, String>(
            "SELECT status FROM inbox_items WHERE id = $1 AND owner_uid = $2 FOR UPDATE",
        )
        .bind(item_id)
        .bind(owner_uid)
        .fetch_optional(&mut *transaction)
        .await?;
        if status.as_deref() != Some("reviewing") {
            transaction.rollback().await?;
            return Ok(None);
        }
        self.replace_suggestions_in_transaction(&mut transaction, item_id, suggestions)
            .await?;
        sqlx::query("UPDATE inbox_items SET updated_at = now() WHERE id = $1")
            .bind(item_id)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        self.get(owner_uid, item_id).await
    }

    async fn create_plan(
        &self,
        owner_uid: &str,
        item_id: Uuid,
        plan: &NewPlan,
    ) -> anyhow::Result<Option<Plan>> {
        let mut transaction = self.database.begin().await?;
        let status = sqlx::query_scalar::<_, String>(
            "SELECT status FROM inbox_items WHERE id = $1 AND owner_uid = $2 FOR UPDATE",
        )
        .bind(item_id)
        .bind(owner_uid)
        .fetch_optional(&mut *transaction)
        .await?;
        if status.as_deref() != Some("reviewing") {
            transaction.rollback().await?;
            return Ok(None);
        }
        let plan_id = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO plans (inbox_item_id, summary, status) VALUES ($1, $2, 'ready') RETURNING id",
        )
        .bind(item_id)
        .bind(&plan.summary)
        .fetch_one(&mut *transaction)
        .await?;
        for (position, step) in plan.steps.iter().enumerate() {
            let status = match step.status {
                PlanStatus::Ready => "ready",
                PlanStatus::Waiting => "waiting",
                PlanStatus::Complete => anyhow::bail!("a new plan cannot contain a complete step"),
            };
            sqlx::query(
                "INSERT INTO plan_steps (plan_id, position, title, rationale, status, due_on, waiting_on, is_next_action) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            )
            .bind(plan_id)
            .bind(position as i32)
            .bind(&step.title)
            .bind(&step.rationale)
            .bind(status)
            .bind(step.due_on)
            .bind(&step.waiting_on)
            .bind(position == 0)
            .execute(&mut *transaction)
            .await?;
        }
        sqlx::query("UPDATE inbox_items SET status = 'planned', updated_at = now() WHERE id = $1")
            .bind(item_id)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        self.get_plan(owner_uid, plan_id).await
    }

    async fn get_plan(&self, owner_uid: &str, plan_id: Uuid) -> anyhow::Result<Option<Plan>> {
        let row = sqlx::query_as::<_, PlanRow>(
            "SELECT p.id, p.inbox_item_id, p.summary, p.status, p.created_at, p.updated_at FROM plans p INNER JOIN inbox_items i ON i.id = p.inbox_item_id WHERE p.id = $1 AND i.owner_uid = $2",
        )
        .bind(plan_id)
        .bind(owner_uid)
        .fetch_optional(&self.database)
        .await?;
        let Some(row) = row else { return Ok(None) };
        let steps = sqlx::query_as::<_, PlanStepRow>(
            "SELECT id, position, title, rationale, status, due_on, waiting_on, is_next_action FROM plan_steps WHERE plan_id = $1 ORDER BY position ASC",
        )
        .bind(plan_id)
        .fetch_all(&self.database)
        .await?
        .into_iter()
        .map(TryInto::try_into)
        .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(Some(Plan {
            id: row.id,
            inbox_item_id: row.inbox_item_id,
            summary: row.summary,
            status: PlanStatus::parse(&row.status)
                .ok_or_else(|| anyhow::anyhow!("plan has an invalid status"))?,
            created_at: row.created_at,
            updated_at: row.updated_at,
            steps,
        }))
    }

    async fn update_plan_step(
        &self,
        owner_uid: &str,
        plan_id: Uuid,
        step_id: Uuid,
        update: &PlanStepUpdate,
    ) -> anyhow::Result<UpdatePlanStepResult> {
        let mut transaction = self.database.begin().await?;
        let owned_plan_id = sqlx::query_scalar::<_, Uuid>(
            "SELECT p.id FROM plans p INNER JOIN inbox_items i ON i.id = p.inbox_item_id WHERE p.id = $1 AND i.owner_uid = $2 FOR UPDATE",
        )
        .bind(plan_id)
        .bind(owner_uid)
        .fetch_optional(&mut *transaction)
        .await?;
        if owned_plan_id.is_none() {
            transaction.rollback().await?;
            return Ok(UpdatePlanStepResult::NotFound);
        }

        let steps = sqlx::query_as::<_, LockedPlanStepRow>(
            "SELECT id, position, status FROM plan_steps WHERE plan_id = $1 ORDER BY position ASC FOR UPDATE",
        )
        .bind(plan_id)
        .fetch_all(&mut *transaction)
        .await?;
        let Some(target) = steps.iter().find(|step| step.id == step_id) else {
            transaction.rollback().await?;
            return Ok(UpdatePlanStepResult::NotFound);
        };
        let current_status = PlanStatus::parse(&target.status)
            .ok_or_else(|| anyhow::anyhow!("plan step has an invalid status"))?;
        if !current_status.can_transition_to(update.status) {
            transaction.rollback().await?;
            return Ok(UpdatePlanStepResult::InvalidState);
        }

        let states = steps
            .iter()
            .map(|step| {
                Ok(PlanStepState {
                    position: u32::try_from(step.position)
                        .map_err(|_| anyhow::anyhow!("plan step has an invalid position"))?,
                    status: if step.id == step_id {
                        update.status
                    } else {
                        PlanStatus::parse(&step.status)
                            .ok_or_else(|| anyhow::anyhow!("plan step has an invalid status"))?
                    },
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        let plan_status = derived_plan_status(&states)
            .ok_or_else(|| anyhow::anyhow!("plan must contain at least one step"))?;
        let next_position = highlighted_next_action(&states);

        sqlx::query(
            "UPDATE plan_steps SET is_next_action = false WHERE plan_id = $1 AND is_next_action",
        )
        .bind(plan_id)
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "UPDATE plan_steps SET status = $1, waiting_on = $2 WHERE id = $3 AND plan_id = $4",
        )
        .bind(update.status.as_str())
        .bind(&update.waiting_on)
        .bind(step_id)
        .bind(plan_id)
        .execute(&mut *transaction)
        .await?;
        if let Some(position) = next_position {
            sqlx::query(
                "UPDATE plan_steps SET is_next_action = true WHERE plan_id = $1 AND position = $2",
            )
            .bind(plan_id)
            .bind(
                i32::try_from(position)
                    .map_err(|_| anyhow::anyhow!("invalid plan step position"))?,
            )
            .execute(&mut *transaction)
            .await?;
        }
        sqlx::query("UPDATE plans SET status = $1, updated_at = now() WHERE id = $2")
            .bind(plan_status.as_str())
            .bind(plan_id)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;

        match self.get_plan(owner_uid, plan_id).await? {
            Some(plan) => Ok(UpdatePlanStepResult::Updated(plan)),
            None => Err(anyhow::anyhow!("updated plan could not be reloaded")),
        }
    }
}
