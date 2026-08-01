use std::collections::HashMap;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{FromRow, PgPool, types::Json};
use time::{Date, Duration, OffsetDateTime};
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
    pub updated_at: OffsetDateTime,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    pub id: Uuid,
    pub inbox_item_id: Uuid,
    pub summary: String,
    pub status: PlanStatus,
    pub revision: i32,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub steps: Vec<PlanStep>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanStepUpdate {
    pub expected_revision: i32,
    pub status: PlanStatus,
    pub waiting_on: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanDraftStep {
    pub id: Option<Uuid>,
    pub title: String,
    pub rationale: String,
    pub status: PlanStatus,
    pub due_on: Option<Date>,
    pub waiting_on: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanDraft {
    pub summary: String,
    pub steps: Vec<PlanDraftStep>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanUpdate {
    pub expected_revision: i32,
    pub draft: PlanDraft,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanRevisionSource {
    Manual,
    StepStatus,
    Assistant,
}

impl PlanRevisionSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::StepStatus => "step_status",
            Self::Assistant => "assistant",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdatePlanResult {
    Updated(Plan),
    NotFound,
    Conflict,
    Invalid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanMessageRole {
    User,
    Assistant,
}

impl PlanMessageRole {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "user" => Some(Self::User),
            "assistant" => Some(Self::Assistant),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanMessage {
    pub id: Uuid,
    pub role: PlanMessageRole,
    pub content: String,
    pub proposal: Option<PlanDraft>,
    pub base_revision: Option<i32>,
    pub applied_revision: Option<i32>,
    pub created_at: OffsetDateTime,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanDiscussionReply {
    pub content: String,
    pub proposal: Option<PlanDraft>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanMessagesPage {
    pub messages: Vec<PlanMessage>,
    pub has_more: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplyPlanProposalResult {
    Updated(Plan),
    NotFound,
    Conflict,
    Invalid,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdatePlanStepResult {
    Updated(Plan),
    NotFound,
    Conflict,
    InvalidState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArchivePlanResult {
    Updated,
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

    async fn list_plans(&self, _owner_uid: &str, _archived: bool) -> anyhow::Result<Vec<Plan>> {
        anyhow::bail!("plan listing is not implemented")
    }

    async fn archive_plan(
        &self,
        _owner_uid: &str,
        _plan_id: Uuid,
    ) -> anyhow::Result<ArchivePlanResult> {
        anyhow::bail!("Plan archiving is not implemented")
    }

    async fn restore_plan(
        &self,
        _owner_uid: &str,
        _plan_id: Uuid,
    ) -> anyhow::Result<ArchivePlanResult> {
        anyhow::bail!("Plan restoration is not implemented")
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

    async fn update_plan(
        &self,
        _owner_uid: &str,
        _plan_id: Uuid,
        _update: &PlanUpdate,
        _source: PlanRevisionSource,
    ) -> anyhow::Result<UpdatePlanResult> {
        anyhow::bail!("plan updates are not implemented")
    }

    async fn list_plan_messages(
        &self,
        _owner_uid: &str,
        _plan_id: Uuid,
        _before: Option<OffsetDateTime>,
        _limit: i64,
    ) -> anyhow::Result<Option<PlanMessagesPage>> {
        anyhow::bail!("plan discussions are not implemented")
    }

    async fn add_plan_discussion(
        &self,
        _owner_uid: &str,
        _plan_id: Uuid,
        _base_revision: i32,
        _user_content: &str,
        _reply: &PlanDiscussionReply,
    ) -> anyhow::Result<Option<(PlanMessage, PlanMessage)>> {
        anyhow::bail!("plan discussions are not implemented")
    }

    async fn apply_plan_proposal(
        &self,
        _owner_uid: &str,
        _plan_id: Uuid,
        _message_id: Uuid,
        _expected_revision: i32,
    ) -> anyhow::Result<ApplyPlanProposalResult> {
        anyhow::bail!("plan proposal application is not implemented")
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

    async fn plan_steps(&self, plan_ids: &[Uuid]) -> anyhow::Result<HashMap<Uuid, Vec<PlanStep>>> {
        if plan_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let rows = sqlx::query_as::<_, PlanStepRow>(
            "SELECT plan_id, id, position, title, rationale, status, due_on, waiting_on, is_next_action, updated_at FROM plan_steps WHERE plan_id = ANY($1) ORDER BY plan_id ASC, position ASC",
        )
        .bind(plan_ids)
        .fetch_all(&self.database)
        .await?;

        let mut steps_by_plan = HashMap::new();
        for row in rows {
            let plan_id = row.plan_id;
            steps_by_plan
                .entry(plan_id)
                .or_insert_with(Vec::new)
                .push(row.try_into()?);
        }
        Ok(steps_by_plan)
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

    async fn record_plan_revision(
        &self,
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        plan_id: Uuid,
        revision: i32,
        source: &str,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO plan_revisions (plan_id, revision, source, snapshot)
            SELECT
                p.id,
                $2,
                $3,
                jsonb_build_object(
                    'summary', p.summary,
                    'status', p.status,
                    'steps', COALESCE(
                        (
                            SELECT jsonb_agg(
                                jsonb_build_object(
                                    'id', s.id,
                                    'position', s.position,
                                    'title', s.title,
                                    'rationale', s.rationale,
                                    'status', s.status,
                                    'dueOn', s.due_on,
                                    'waitingOn', s.waiting_on,
                                    'isNextAction', s.is_next_action
                                ) ORDER BY s.position
                            )
                            FROM plan_steps s
                            WHERE s.plan_id = p.id
                        ),
                        '[]'::jsonb
                    )
                )
            FROM plans p
            WHERE p.id = $1
            "#,
        )
        .bind(plan_id)
        .bind(revision)
        .bind(source)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    async fn owned_plan_revision(
        &self,
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        owner_uid: &str,
        plan_id: Uuid,
    ) -> anyhow::Result<Option<i32>> {
        sqlx::query_scalar::<_, i32>(
            "SELECT p.revision FROM plans p INNER JOIN inbox_items i ON i.id = p.inbox_item_id WHERE p.id = $1 AND i.owner_uid = $2 AND i.status = 'planned' FOR UPDATE",
        )
        .bind(plan_id)
        .bind(owner_uid)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(Into::into)
    }

    async fn replace_plan_in_transaction(
        &self,
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        owner_uid: &str,
        plan_id: Uuid,
        update: &PlanUpdate,
        source: PlanRevisionSource,
    ) -> anyhow::Result<UpdatePlanResult> {
        let Some(current_revision) = self
            .owned_plan_revision(transaction, owner_uid, plan_id)
            .await?
        else {
            return Ok(UpdatePlanResult::NotFound);
        };
        if current_revision != update.expected_revision {
            return Ok(UpdatePlanResult::Conflict);
        }

        let existing = sqlx::query_as::<_, LockedPlanStepRow>(
            "SELECT id, position, status FROM plan_steps WHERE plan_id = $1 ORDER BY position ASC FOR UPDATE",
        )
        .bind(plan_id)
        .fetch_all(&mut **transaction)
        .await?;
        let existing_ids = existing.iter().map(|step| step.id).collect::<Vec<_>>();
        let mut retained_ids = Vec::new();
        for step in &update.draft.steps {
            if let Some(id) = step.id {
                if !existing_ids.contains(&id) || retained_ids.contains(&id) {
                    return Ok(UpdatePlanResult::Invalid);
                }
                retained_ids.push(id);
            }
        }
        for existing_step in &existing {
            let current_status = PlanStatus::parse(&existing_step.status)
                .ok_or_else(|| anyhow::anyhow!("plan step has an invalid status"))?;
            let Some(next_status) = update
                .draft
                .steps
                .iter()
                .find(|step| step.id == Some(existing_step.id))
                .map(|step| step.status)
            else {
                continue;
            };
            if current_status != next_status && !current_status.can_transition_to(next_status) {
                return Ok(UpdatePlanResult::Invalid);
            }
        }

        const TEMPORARY_PLAN_STEP_POSITION_OFFSET: i32 = 1_000_000;
        sqlx::query(
            "UPDATE plan_steps SET position = position + $2, is_next_action = false WHERE plan_id = $1",
        )
            .bind(plan_id)
            .bind(TEMPORARY_PLAN_STEP_POSITION_OFFSET)
            .execute(&mut **transaction)
            .await?;
        for (position, step) in update.draft.steps.iter().enumerate() {
            if let Some(id) = step.id {
                sqlx::query(
                    "UPDATE plan_steps SET position = $1, title = $2, rationale = $3, status = $4, due_on = $5, waiting_on = $6 WHERE id = $7 AND plan_id = $8",
                )
                .bind(position as i32)
                .bind(&step.title)
                .bind(&step.rationale)
                .bind(step.status.as_str())
                .bind(step.due_on)
                .bind(&step.waiting_on)
                .bind(id)
                .bind(plan_id)
                .execute(&mut **transaction)
                .await?;
            } else {
                sqlx::query(
                    "INSERT INTO plan_steps (plan_id, position, title, rationale, status, due_on, waiting_on, is_next_action) VALUES ($1, $2, $3, $4, $5, $6, $7, false)",
                )
                .bind(plan_id)
                .bind(position as i32)
                .bind(&step.title)
                .bind(&step.rationale)
                .bind(step.status.as_str())
                .bind(step.due_on)
                .bind(&step.waiting_on)
                .execute(&mut **transaction)
                .await?;
            }
        }
        sqlx::query("DELETE FROM plan_steps WHERE plan_id = $1 AND position >= $2")
            .bind(plan_id)
            .bind(TEMPORARY_PLAN_STEP_POSITION_OFFSET)
            .execute(&mut **transaction)
            .await?;

        let states = update
            .draft
            .steps
            .iter()
            .enumerate()
            .map(|(position, step)| PlanStepState {
                position: position as u32,
                status: step.status,
            })
            .collect::<Vec<_>>();
        let plan_status = derived_plan_status(&states)
            .ok_or_else(|| anyhow::anyhow!("plan must contain at least one step"))?;
        if let Some(position) = highlighted_next_action(&states) {
            sqlx::query(
                "UPDATE plan_steps SET is_next_action = true WHERE plan_id = $1 AND position = $2",
            )
            .bind(plan_id)
            .bind(position as i32)
            .execute(&mut **transaction)
            .await?;
        }

        let next_revision = current_revision + 1;
        sqlx::query("UPDATE plans SET summary = $1, status = $2, revision = $3, updated_at = now() WHERE id = $4")
            .bind(&update.draft.summary)
            .bind(plan_status.as_str())
            .bind(next_revision)
            .bind(plan_id)
            .execute(&mut **transaction)
            .await?;
        self.record_plan_revision(transaction, plan_id, next_revision, source.as_str())
            .await?;
        Ok(UpdatePlanResult::Updated(Plan {
            id: plan_id,
            inbox_item_id: Uuid::nil(),
            summary: String::new(),
            status: plan_status,
            revision: next_revision,
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
            steps: Vec::new(),
        }))
    }

    async fn transition_plan_archive_state(
        &self,
        owner_uid: &str,
        plan_id: Uuid,
        expected: InboxStatus,
        next: InboxStatus,
    ) -> anyhow::Result<ArchivePlanResult> {
        let mut transaction = self.database.begin().await?;
        let source = sqlx::query_as::<_, (Uuid, String)>(
            "SELECT i.id, i.status FROM inbox_items i INNER JOIN plans p ON p.inbox_item_id = i.id WHERE p.id = $1 AND i.owner_uid = $2 FOR UPDATE OF i",
        )
        .bind(plan_id)
        .bind(owner_uid)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some((inbox_item_id, status)) = source else {
            transaction.rollback().await?;
            return Ok(ArchivePlanResult::NotFound);
        };
        let current = InboxStatus::parse(&status)
            .ok_or_else(|| anyhow::anyhow!("Inbox item has an invalid status"))?;
        if current != expected || !current.can_transition_to(next) {
            transaction.rollback().await?;
            return Ok(ArchivePlanResult::InvalidState);
        }

        sqlx::query("UPDATE inbox_items SET status = $1 WHERE id = $2")
            .bind(match next {
                InboxStatus::Planned => "planned",
                InboxStatus::Archived => "archived",
                InboxStatus::Captured | InboxStatus::Reviewing => {
                    anyhow::bail!("Plans cannot transition to this Inbox status")
                }
            })
            .bind(inbox_item_id)
            .execute(&mut *transaction)
            .await?;
        sqlx::query("UPDATE plans SET updated_at = now() WHERE id = $1")
            .bind(plan_id)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(ArchivePlanResult::Updated)
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
    revision: i32,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

#[derive(FromRow)]
struct PlanStepRow {
    plan_id: Uuid,
    id: Uuid,
    position: i32,
    title: String,
    rationale: String,
    status: String,
    due_on: Option<Date>,
    waiting_on: Option<String>,
    is_next_action: bool,
    updated_at: OffsetDateTime,
}

#[derive(FromRow)]
struct LockedPlanStepRow {
    id: Uuid,
    position: i32,
    status: String,
}

#[derive(FromRow)]
struct PlanMessageRow {
    id: Uuid,
    role: String,
    content: String,
    proposal: Option<Json<Value>>,
    base_revision: Option<i32>,
    applied_revision: Option<i32>,
    created_at: OffsetDateTime,
}

impl TryFrom<PlanMessageRow> for PlanMessage {
    type Error = anyhow::Error;

    fn try_from(row: PlanMessageRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            role: PlanMessageRole::parse(&row.role)
                .ok_or_else(|| anyhow::anyhow!("plan message has an invalid role"))?,
            content: row.content,
            proposal: row
                .proposal
                .map(|value| serde_json::from_value(value.0))
                .transpose()
                .map_err(|_| anyhow::anyhow!("plan message has an invalid proposal"))?,
            base_revision: row.base_revision,
            applied_revision: row.applied_revision,
            created_at: row.created_at,
        })
    }
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
            updated_at: row.updated_at,
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
            "SELECT i.id, p.id AS plan_id, i.source_type, i.status, i.created_at, i.updated_at FROM inbox_items i LEFT JOIN plans p ON p.inbox_item_id = i.id WHERE i.owner_uid = $1 AND i.status <> 'archived' ORDER BY i.created_at DESC, i.id DESC",
        )
        .bind(owner_uid)
        .fetch_all(&self.database)
        .await?;
        rows.into_iter().map(TryInto::try_into).collect()
    }

    async fn get(&self, owner_uid: &str, item_id: Uuid) -> anyhow::Result<Option<InboxItemDetail>> {
        let row = sqlx::query_as::<_, InboxItemDetailRow>(
            "SELECT i.id, p.id AS plan_id, i.source_type, i.status, i.original_text, i.original_filename, i.content_type, i.byte_size, i.created_at, i.updated_at FROM inbox_items i LEFT JOIN plans p ON p.inbox_item_id = i.id WHERE i.id = $1 AND i.owner_uid = $2 AND i.status <> 'archived'",
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
            "SELECT storage_key, content_type FROM inbox_items WHERE id = $1 AND owner_uid = $2 AND source_type = 'pdf' AND status <> 'archived'",
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
        self.record_plan_revision(&mut transaction, plan_id, 1, "initial")
            .await?;
        transaction.commit().await?;
        self.get_plan(owner_uid, plan_id).await
    }

    async fn get_plan(&self, owner_uid: &str, plan_id: Uuid) -> anyhow::Result<Option<Plan>> {
        let row = sqlx::query_as::<_, PlanRow>(
            "SELECT p.id, p.inbox_item_id, p.summary, p.status, p.revision, p.created_at, p.updated_at FROM plans p INNER JOIN inbox_items i ON i.id = p.inbox_item_id WHERE p.id = $1 AND i.owner_uid = $2 AND i.status = 'planned'",
        )
        .bind(plan_id)
        .bind(owner_uid)
        .fetch_optional(&self.database)
        .await?;
        let Some(row) = row else { return Ok(None) };
        let steps = self
            .plan_steps(&[plan_id])
            .await?
            .remove(&plan_id)
            .unwrap_or_default();
        Ok(Some(Plan {
            id: row.id,
            inbox_item_id: row.inbox_item_id,
            summary: row.summary,
            status: PlanStatus::parse(&row.status)
                .ok_or_else(|| anyhow::anyhow!("plan has an invalid status"))?,
            revision: row.revision,
            created_at: row.created_at,
            updated_at: row.updated_at,
            steps,
        }))
    }

    async fn list_plans(&self, owner_uid: &str, archived: bool) -> anyhow::Result<Vec<Plan>> {
        let rows = sqlx::query_as::<_, PlanRow>(
            "SELECT p.id, p.inbox_item_id, p.summary, p.status, p.revision, p.created_at, p.updated_at FROM plans p INNER JOIN inbox_items i ON i.id = p.inbox_item_id WHERE i.owner_uid = $1 AND i.status = $2 ORDER BY p.updated_at DESC, p.id DESC",
        )
        .bind(owner_uid)
        .bind(if archived { "archived" } else { "planned" })
        .fetch_all(&self.database)
        .await?;
        let plan_ids = rows.iter().map(|row| row.id).collect::<Vec<_>>();
        let mut steps_by_plan = self.plan_steps(&plan_ids).await?;

        rows.into_iter()
            .map(|row| {
                Ok(Plan {
                    id: row.id,
                    inbox_item_id: row.inbox_item_id,
                    summary: row.summary,
                    status: PlanStatus::parse(&row.status)
                        .ok_or_else(|| anyhow::anyhow!("plan has an invalid status"))?,
                    revision: row.revision,
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                    steps: steps_by_plan.remove(&row.id).unwrap_or_default(),
                })
            })
            .collect()
    }

    async fn archive_plan(
        &self,
        owner_uid: &str,
        plan_id: Uuid,
    ) -> anyhow::Result<ArchivePlanResult> {
        self.transition_plan_archive_state(
            owner_uid,
            plan_id,
            InboxStatus::Planned,
            InboxStatus::Archived,
        )
        .await
    }

    async fn restore_plan(
        &self,
        owner_uid: &str,
        plan_id: Uuid,
    ) -> anyhow::Result<ArchivePlanResult> {
        self.transition_plan_archive_state(
            owner_uid,
            plan_id,
            InboxStatus::Archived,
            InboxStatus::Planned,
        )
        .await
    }

    async fn update_plan_step(
        &self,
        owner_uid: &str,
        plan_id: Uuid,
        step_id: Uuid,
        update: &PlanStepUpdate,
    ) -> anyhow::Result<UpdatePlanStepResult> {
        let mut transaction = self.database.begin().await?;
        let owned_plan = sqlx::query_as::<_, (Uuid, i32)>(
            "SELECT p.id, p.revision FROM plans p INNER JOIN inbox_items i ON i.id = p.inbox_item_id WHERE p.id = $1 AND i.owner_uid = $2 AND i.status = 'planned' FOR UPDATE",
        )
        .bind(plan_id)
        .bind(owner_uid)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some((_owned_plan_id, current_revision)) = owned_plan else {
            transaction.rollback().await?;
            return Ok(UpdatePlanStepResult::NotFound);
        };
        if current_revision != update.expected_revision {
            transaction.rollback().await?;
            return Ok(UpdatePlanStepResult::Conflict);
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
        if current_status != update.status && !current_status.can_transition_to(update.status) {
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
        let next_revision = current_revision + 1;
        sqlx::query(
            "UPDATE plans SET status = $1, revision = $2, updated_at = now() WHERE id = $3",
        )
        .bind(plan_status.as_str())
        .bind(next_revision)
        .bind(plan_id)
        .execute(&mut *transaction)
        .await?;
        self.record_plan_revision(
            &mut transaction,
            plan_id,
            next_revision,
            PlanRevisionSource::StepStatus.as_str(),
        )
        .await?;
        transaction.commit().await?;

        match self.get_plan(owner_uid, plan_id).await? {
            Some(plan) => Ok(UpdatePlanStepResult::Updated(plan)),
            None => Err(anyhow::anyhow!("updated plan could not be reloaded")),
        }
    }

    async fn update_plan(
        &self,
        owner_uid: &str,
        plan_id: Uuid,
        update: &PlanUpdate,
        source: PlanRevisionSource,
    ) -> anyhow::Result<UpdatePlanResult> {
        let mut transaction = self.database.begin().await?;
        let result = self
            .replace_plan_in_transaction(&mut transaction, owner_uid, plan_id, update, source)
            .await?;
        match result {
            UpdatePlanResult::Updated(_) => {
                transaction.commit().await?;
                self.get_plan(owner_uid, plan_id)
                    .await?
                    .map(UpdatePlanResult::Updated)
                    .ok_or_else(|| anyhow::anyhow!("updated plan could not be reloaded"))
            }
            other => {
                transaction.rollback().await?;
                Ok(other)
            }
        }
    }

    async fn list_plan_messages(
        &self,
        owner_uid: &str,
        plan_id: Uuid,
        before: Option<OffsetDateTime>,
        limit: i64,
    ) -> anyhow::Result<Option<PlanMessagesPage>> {
        if self.get_plan(owner_uid, plan_id).await?.is_none() {
            return Ok(None);
        }
        let rows = sqlx::query_as::<_, PlanMessageRow>(
            r#"
            SELECT id, role, content, proposal, base_revision, applied_revision, created_at
            FROM plan_messages
            WHERE plan_id = $1 AND ($2::timestamptz IS NULL OR created_at < $2)
            ORDER BY created_at DESC, id DESC
            LIMIT $3
            "#,
        )
        .bind(plan_id)
        .bind(before)
        .bind(limit + 1)
        .fetch_all(&self.database)
        .await?;
        let has_more = rows.len() as i64 > limit;
        let messages = rows
            .into_iter()
            .take(limit as usize)
            .map(TryInto::try_into)
            .collect::<anyhow::Result<Vec<PlanMessage>>>()?
            .into_iter()
            .rev()
            .collect();
        Ok(Some(PlanMessagesPage { messages, has_more }))
    }

    async fn add_plan_discussion(
        &self,
        owner_uid: &str,
        plan_id: Uuid,
        base_revision: i32,
        user_content: &str,
        reply: &PlanDiscussionReply,
    ) -> anyhow::Result<Option<(PlanMessage, PlanMessage)>> {
        let mut transaction = self.database.begin().await?;
        if self
            .owned_plan_revision(&mut transaction, owner_uid, plan_id)
            .await?
            .is_none()
        {
            transaction.rollback().await?;
            return Ok(None);
        }
        let user = sqlx::query_as::<_, PlanMessageRow>(
            "INSERT INTO plan_messages (plan_id, role, content) VALUES ($1, $2, $3) RETURNING id, role, content, proposal, base_revision, applied_revision, created_at",
        )
        .bind(plan_id)
        .bind(PlanMessageRole::User.as_str())
        .bind(user_content)
        .fetch_one(&mut *transaction)
        .await?;
        let assistant_created_at = user.created_at + Duration::microseconds(1);
        let proposal = reply
            .proposal
            .as_ref()
            .map(serde_json::to_value)
            .transpose()?;
        let assistant = sqlx::query_as::<_, PlanMessageRow>(
            "INSERT INTO plan_messages (plan_id, role, content, proposal, base_revision, created_at) VALUES ($1, $2, $3, $4, $5, $6) RETURNING id, role, content, proposal, base_revision, applied_revision, created_at",
        )
        .bind(plan_id)
        .bind(PlanMessageRole::Assistant.as_str())
        .bind(&reply.content)
        .bind(proposal.map(Json))
        .bind(reply.proposal.as_ref().map(|_| base_revision))
        .bind(assistant_created_at)
        .fetch_one(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(Some((user.try_into()?, assistant.try_into()?)))
    }

    async fn apply_plan_proposal(
        &self,
        owner_uid: &str,
        plan_id: Uuid,
        message_id: Uuid,
        expected_revision: i32,
    ) -> anyhow::Result<ApplyPlanProposalResult> {
        let mut transaction = self.database.begin().await?;
        let Some(current_revision) = self
            .owned_plan_revision(&mut transaction, owner_uid, plan_id)
            .await?
        else {
            transaction.rollback().await?;
            return Ok(ApplyPlanProposalResult::NotFound);
        };
        let message = sqlx::query_as::<_, PlanMessageRow>(
            "SELECT id, role, content, proposal, base_revision, applied_revision, created_at FROM plan_messages WHERE id = $1 AND plan_id = $2 FOR UPDATE",
        )
        .bind(message_id)
        .bind(plan_id)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some(message) = message else {
            transaction.rollback().await?;
            return Ok(ApplyPlanProposalResult::NotFound);
        };
        let message: PlanMessage = message.try_into()?;
        let Some(draft) = message.proposal else {
            transaction.rollback().await?;
            return Ok(ApplyPlanProposalResult::Invalid);
        };
        let Some(base_revision) = message.base_revision else {
            transaction.rollback().await?;
            return Ok(ApplyPlanProposalResult::Invalid);
        };
        if message.applied_revision.is_some() {
            transaction.rollback().await?;
            return self
                .get_plan(owner_uid, plan_id)
                .await?
                .map(ApplyPlanProposalResult::Updated)
                .ok_or_else(|| anyhow::anyhow!("applied plan could not be reloaded"));
        }
        if base_revision != expected_revision || current_revision != expected_revision {
            transaction.rollback().await?;
            return Ok(ApplyPlanProposalResult::Conflict);
        }
        let result = self
            .replace_plan_in_transaction(
                &mut transaction,
                owner_uid,
                plan_id,
                &PlanUpdate {
                    expected_revision,
                    draft,
                },
                PlanRevisionSource::Assistant,
            )
            .await?;
        match result {
            UpdatePlanResult::Updated(plan) => {
                sqlx::query("UPDATE plan_messages SET applied_revision = $1 WHERE id = $2")
                    .bind(plan.revision)
                    .bind(message_id)
                    .execute(&mut *transaction)
                    .await?;
                transaction.commit().await?;
                self.get_plan(owner_uid, plan_id)
                    .await?
                    .map(ApplyPlanProposalResult::Updated)
                    .ok_or_else(|| anyhow::anyhow!("applied plan could not be reloaded"))
            }
            UpdatePlanResult::NotFound => {
                transaction.rollback().await?;
                Ok(ApplyPlanProposalResult::NotFound)
            }
            UpdatePlanResult::Conflict => {
                transaction.rollback().await?;
                Ok(ApplyPlanProposalResult::Conflict)
            }
            UpdatePlanResult::Invalid => {
                transaction.rollback().await?;
                Ok(ApplyPlanProposalResult::Invalid)
            }
        }
    }
}
