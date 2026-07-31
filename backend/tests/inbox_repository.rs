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
use inbox::{InboxRepository, NewPlan, SqlxInboxRepository, Suggestion};
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
