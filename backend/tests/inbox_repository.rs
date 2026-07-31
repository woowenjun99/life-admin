#![allow(dead_code)]

#[path = "../src/ai.rs"]
mod ai;
#[path = "../src/domain.rs"]
mod domain;
#[allow(dead_code)]
#[path = "../src/inbox.rs"]
mod inbox;

use std::{collections::VecDeque, env, sync::Mutex};

use ai::{AiCall, AiError, AiProvider, Extraction};
use async_trait::async_trait;
use domain::PlanStatus;
use inbox::{
    InboxRepository, NewPlan, PlanStepUpdate, SqlxInboxRepository, Suggestion, UpdatePlanStepResult,
};
use sqlx::{PgPool, postgres::PgPoolOptions};
use uuid::Uuid;

async fn test_database() -> PgPool {
    let database_url = env::var("TEST_DATABASE_URL")
        .expect("TEST_DATABASE_URL must point to an isolated PostgreSQL database");
    let database = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await
        .expect("test PostgreSQL database should be reachable");

    sqlx::migrate!("./migrations")
        .run(&database)
        .await
        .expect("inbox migrations should apply to the test database");

    database
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for an isolated PostgreSQL database"]
async fn sqlx_repository_scopes_reads_to_the_owner_and_orders_newest_first() {
    let database = test_database().await;
    let repository = SqlxInboxRepository::new(database.clone());
    let owner_uid = format!("inbox-repository-owner-{}", Uuid::new_v4());
    let other_owner_uid = format!("inbox-repository-other-{}", Uuid::new_v4());
    let older_id = Uuid::new_v4();
    let newer_id = Uuid::new_v4();
    let foreign_id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO inbox_items (id, owner_uid, source_type, original_text, status, created_at, updated_at)
        VALUES
            ($1, $2, 'text', 'Older private note', 'captured', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z'),
            ($3, $2, 'text', 'Newer private note', 'captured', '2026-01-02T00:00:00Z', '2026-01-02T00:00:00Z'),
            ($4, $5, 'text', 'Another users note', 'captured', '2026-01-03T00:00:00Z', '2026-01-03T00:00:00Z')
        "#,
    )
    .bind(older_id)
    .bind(&owner_uid)
    .bind(newer_id)
    .bind(foreign_id)
    .bind(&other_owner_uid)
    .execute(&database)
    .await
    .expect("test inbox items should insert");

    let listed = repository
        .list(&owner_uid)
        .await
        .expect("owner list should succeed");
    assert_eq!(
        listed.iter().map(|item| item.id).collect::<Vec<_>>(),
        [newer_id, older_id]
    );

    let owned_detail = repository
        .get(&owner_uid, newer_id)
        .await
        .expect("owned detail should load")
        .expect("owned item should be found");
    assert_eq!(
        owned_detail.original_text.as_deref(),
        Some("Newer private note")
    );
    assert!(
        repository
            .get(&owner_uid, foreign_id)
            .await
            .expect("foreign detail lookup should succeed")
            .is_none()
    );

    sqlx::query("DELETE FROM inbox_items WHERE owner_uid = $1 OR owner_uid = $2")
        .bind(&owner_uid)
        .bind(&other_owner_uid)
        .execute(&database)
        .await
        .expect("test inbox items should clean up");
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for an isolated PostgreSQL database"]
async fn sqlx_repository_updates_owned_plan_steps_and_derives_plan_state() {
    let database = test_database().await;
    let repository = SqlxInboxRepository::new(database.clone());
    let owner_uid = format!("plan-step-owner-{}", Uuid::new_v4());
    let other_owner_uid = format!("plan-step-other-{}", Uuid::new_v4());
    let inbox_item_id = Uuid::new_v4();
    let plan_id = Uuid::new_v4();
    let first_step_id = Uuid::new_v4();
    let second_step_id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO inbox_items (id, owner_uid, source_type, original_text, status)
        VALUES ($1, $2, 'text', 'Renew before travelling.', 'planned')
        "#,
    )
    .bind(inbox_item_id)
    .bind(&owner_uid)
    .execute(&database)
    .await
    .expect("owned Inbox item should insert");
    sqlx::query(
        r#"
        INSERT INTO plans (id, inbox_item_id, summary, status, created_at, updated_at)
        VALUES ($1, $2, 'Renew before travelling.', 'ready', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')
        "#,
    )
    .bind(plan_id)
    .bind(inbox_item_id)
    .execute(&database)
    .await
    .expect("owned Plan should insert");
    sqlx::query(
        r#"
        INSERT INTO plan_steps (id, plan_id, position, title, rationale, status, is_next_action)
        VALUES
            ($1, $2, 0, 'Check requirements', 'Confirms what is needed.', 'ready', true),
            ($3, $2, 1, 'Prepare documents', 'Makes the application ready.', 'ready', false)
        "#,
    )
    .bind(first_step_id)
    .bind(plan_id)
    .bind(second_step_id)
    .execute(&database)
    .await
    .expect("owned Plan steps should insert");

    let UpdatePlanStepResult::Updated(completed) = repository
        .update_plan_step(
            &owner_uid,
            plan_id,
            first_step_id,
            &PlanStepUpdate {
                status: PlanStatus::Complete,
                waiting_on: None,
            },
        )
        .await
        .expect("owned Plan step should update")
    else {
        panic!("owned Plan step should be found and transitionable");
    };
    assert_eq!(completed.status, PlanStatus::Ready);
    assert!(completed.updated_at > time::OffsetDateTime::UNIX_EPOCH);
    assert_eq!(
        completed
            .steps
            .iter()
            .find(|step| step.id == first_step_id)
            .expect("first step should remain in the Plan")
            .status,
        PlanStatus::Complete
    );
    assert!(
        completed
            .steps
            .iter()
            .find(|step| step.id == second_step_id)
            .expect("second step should remain in the Plan")
            .is_next_action
    );

    let UpdatePlanStepResult::Updated(waiting) = repository
        .update_plan_step(
            &owner_uid,
            plan_id,
            second_step_id,
            &PlanStepUpdate {
                status: PlanStatus::Waiting,
                waiting_on: Some("A reply from the agency".to_owned()),
            },
        )
        .await
        .expect("owned Plan step should become Waiting")
    else {
        panic!("owned Plan step should be found and transitionable");
    };
    assert_eq!(waiting.status, PlanStatus::Waiting);
    assert!(waiting.steps.iter().all(|step| !step.is_next_action));
    assert_eq!(
        waiting
            .steps
            .iter()
            .find(|step| step.id == second_step_id)
            .expect("second step should remain in the Plan")
            .waiting_on
            .as_deref(),
        Some("A reply from the agency")
    );

    let UpdatePlanStepResult::Updated(ready_again) = repository
        .update_plan_step(
            &owner_uid,
            plan_id,
            second_step_id,
            &PlanStepUpdate {
                status: PlanStatus::Ready,
                waiting_on: None,
            },
        )
        .await
        .expect("owned Waiting Plan step should become Ready")
    else {
        panic!("owned Waiting Plan step should be found and transitionable");
    };
    assert_eq!(ready_again.status, PlanStatus::Ready);
    let ready_second_step = ready_again
        .steps
        .iter()
        .find(|step| step.id == second_step_id)
        .expect("second step should remain in the Plan");
    assert_eq!(ready_second_step.waiting_on, None);
    assert!(ready_second_step.is_next_action);

    assert!(matches!(
        repository
            .update_plan_step(
                &other_owner_uid,
                plan_id,
                second_step_id,
                &PlanStepUpdate {
                    status: PlanStatus::Ready,
                    waiting_on: None,
                },
            )
            .await
            .expect("foreign lookup should succeed"),
        UpdatePlanStepResult::NotFound
    ));

    sqlx::query("DELETE FROM inbox_items WHERE id = $1")
        .bind(inbox_item_id)
        .execute(&database)
        .await
        .expect("owned fixture should clean up");
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for an isolated PostgreSQL database"]
async fn sqlx_repository_lists_only_owned_plans_with_ordered_steps() {
    let database = test_database().await;
    let repository = SqlxInboxRepository::new(database.clone());
    let owner_uid = format!("plan-list-owner-{}", Uuid::new_v4());
    let other_owner_uid = format!("plan-list-other-{}", Uuid::new_v4());
    let older_item_id = Uuid::new_v4();
    let newer_item_id = Uuid::new_v4();
    let foreign_item_id = Uuid::new_v4();
    let older_plan_id = Uuid::new_v4();
    let newer_plan_id = Uuid::new_v4();
    let foreign_plan_id = Uuid::new_v4();
    let newer_second_step_id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO inbox_items (id, owner_uid, source_type, original_text, status)
        VALUES
            ($1, $2, 'text', 'Older private note', 'planned'),
            ($3, $2, 'text', 'Newer private note', 'planned'),
            ($4, $5, 'text', 'Another persons note', 'planned')
        "#,
    )
    .bind(older_item_id)
    .bind(&owner_uid)
    .bind(newer_item_id)
    .bind(foreign_item_id)
    .bind(&other_owner_uid)
    .execute(&database)
    .await
    .expect("test Inbox items should insert");
    sqlx::query(
        r#"
        INSERT INTO plans (id, inbox_item_id, summary, status, created_at, updated_at)
        VALUES
            ($1, $2, 'Older private Plan', 'ready', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z'),
            ($3, $4, 'Newer private Plan', 'ready', '2026-01-02T00:00:00Z', '2026-01-02T00:00:00Z'),
            ($5, $6, 'Another persons Plan', 'ready', '2026-01-03T00:00:00Z', '2026-01-03T00:00:00Z')
        "#,
    )
    .bind(older_plan_id)
    .bind(older_item_id)
    .bind(newer_plan_id)
    .bind(newer_item_id)
    .bind(foreign_plan_id)
    .bind(foreign_item_id)
    .execute(&database)
    .await
    .expect("test Plans should insert");
    sqlx::query(
        r#"
        INSERT INTO plan_steps (id, plan_id, position, title, rationale, status, is_next_action, updated_at)
        VALUES
            ($1, $2, 0, 'Older step', 'Keeps the older Plan private.', 'ready', true, '2026-01-01T00:00:00Z'),
            ($3, $4, 1, 'Newer second step', 'Proves ordered steps.', 'ready', false, '2026-01-02T00:00:00Z'),
            ($5, $4, 0, 'Newer first step', 'Proves ordered steps.', 'ready', true, '2026-01-03T00:00:00Z'),
            ($6, $7, 0, 'Foreign step', 'Must never be listed.', 'ready', true, '2026-01-03T00:00:00Z')
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(older_plan_id)
    .bind(newer_second_step_id)
    .bind(newer_plan_id)
    .bind(Uuid::new_v4())
    .bind(foreign_plan_id)
    .execute(&database)
    .await
    .expect("test Plan steps should insert");

    let listed = repository
        .list_plans(&owner_uid)
        .await
        .expect("owner Plan list should succeed");
    assert_eq!(
        listed.iter().map(|plan| plan.id).collect::<Vec<_>>(),
        [newer_plan_id, older_plan_id]
    );
    assert_eq!(
        listed[0]
            .steps
            .iter()
            .map(|step| step.position)
            .collect::<Vec<_>>(),
        [0, 1]
    );
    assert_eq!(listed[0].steps[1].id, newer_second_step_id);
    assert_eq!(
        listed[0].steps[0].updated_at,
        time::OffsetDateTime::from_unix_timestamp(1_767_398_400).unwrap()
    );

    let other_owner_plans = repository
        .list_plans(&other_owner_uid)
        .await
        .expect("other owner Plan list should succeed");
    assert_eq!(
        other_owner_plans
            .iter()
            .map(|plan| plan.id)
            .collect::<Vec<_>>(),
        [foreign_plan_id]
    );

    sqlx::query("DELETE FROM plans WHERE inbox_item_id = ANY($1)")
        .bind(vec![older_item_id, newer_item_id, foreign_item_id])
        .execute(&database)
        .await
        .expect("test Plans should clean up");
    sqlx::query("DELETE FROM inbox_items WHERE id = ANY($1)")
        .bind(vec![older_item_id, newer_item_id, foreign_item_id])
        .execute(&database)
        .await
        .expect("test Inbox items should clean up");
}

struct CleanupProvider {
    delete_results: Mutex<VecDeque<Result<(), AiError>>>,
}

impl CleanupProvider {
    fn new(delete_results: Vec<Result<(), AiError>>) -> Self {
        Self {
            delete_results: Mutex::new(delete_results.into()),
        }
    }
}

#[async_trait]
impl AiProvider for CleanupProvider {
    async fn extract(&self, _input: ai::ExtractionInput) -> AiCall<Extraction> {
        AiCall {
            result: Err(AiError::Unavailable),
            cleanup_file_id: None,
        }
    }

    async fn plan(&self, _suggestions: &[Suggestion]) -> Result<NewPlan, AiError> {
        Err(AiError::Unavailable)
    }

    async fn delete_file(&self, _file_id: &str) -> Result<(), AiError> {
        self.delete_results
            .lock()
            .expect("test cleanup result queue should not be poisoned")
            .pop_front()
            .unwrap_or(Err(AiError::Unavailable))
    }
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for an isolated PostgreSQL database"]
async fn cleanup_queue_retries_provider_file_deletion_without_capture_content() {
    let database = test_database().await;
    let file_id = format!("file-test-{}", Uuid::new_v4());
    ai::enqueue_cleanup(&database, &file_id)
        .await
        .expect("cleanup identifier should persist");

    let failing = CleanupProvider::new(vec![Err(AiError::Transient)]);
    ai::retry_cleanup(&database, &failing)
        .await
        .expect("failed cleanup attempt should remain durable");
    let attempts: i32 =
        sqlx::query_scalar("SELECT attempts FROM openai_file_cleanup WHERE file_id = $1")
            .bind(&file_id)
            .fetch_one(&database)
            .await
            .expect("cleanup row should remain after a transient provider failure");
    assert_eq!(attempts, 1);

    let successful = CleanupProvider::new(vec![Ok(())]);
    ai::retry_cleanup(&database, &successful)
        .await
        .expect("successful cleanup should delete the durable queue row");
    let remaining: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM openai_file_cleanup WHERE file_id = $1")
            .bind(&file_id)
            .fetch_one(&database)
            .await
            .expect("cleanup row query should succeed");
    assert_eq!(remaining, 0);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for an isolated PostgreSQL database"]
async fn cleanup_queue_retries_items_after_a_failed_batch() {
    let database = test_database().await;
    let file_ids = (0..21)
        .map(|index| format!("file-test-{index}-{}", Uuid::new_v4()))
        .collect::<Vec<_>>();
    for file_id in &file_ids {
        ai::enqueue_cleanup(&database, file_id)
            .await
            .expect("cleanup identifier should persist");
    }

    let first_batch =
        CleanupProvider::new((0..20).map(|_| Err(AiError::Transient)).collect::<Vec<_>>());
    ai::retry_cleanup(&database, &first_batch)
        .await
        .expect("first cleanup attempt should finish");

    let second_batch =
        CleanupProvider::new((0..20).map(|_| Err(AiError::Transient)).collect::<Vec<_>>());
    ai::retry_cleanup(&database, &second_batch)
        .await
        .expect("second cleanup attempt should finish");
    let last_attempts: i32 =
        sqlx::query_scalar("SELECT attempts FROM openai_file_cleanup WHERE file_id = $1")
            .bind(&file_ids[20])
            .fetch_one(&database)
            .await
            .expect("the item after the original batch should be retried");
    assert_eq!(last_attempts, 1);

    sqlx::query("DELETE FROM openai_file_cleanup WHERE file_id = ANY($1)")
        .bind(&file_ids)
        .execute(&database)
        .await
        .expect("cleanup queue rows should clean up");
}
