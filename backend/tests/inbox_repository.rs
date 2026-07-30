#[path = "../src/domain.rs"]
mod domain;
#[allow(dead_code)]
#[path = "../src/inbox.rs"]
mod inbox;

use std::env;

use inbox::{InboxRepository, SqlxInboxRepository};
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
