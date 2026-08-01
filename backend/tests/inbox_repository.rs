#![allow(dead_code)]

#[path = "../src/ai.rs"]
mod ai;
#[path = "../src/domain.rs"]
mod domain;
#[allow(dead_code)]
#[path = "../src/inbox.rs"]
mod inbox;
#[path = "../src/notifications.rs"]
mod notifications;

use std::{collections::VecDeque, env, sync::Mutex};

use ai::{AiCall, AiError, AiProvider, Extraction};
use async_trait::async_trait;
use domain::PlanStatus;
use inbox::{
    ApplyPlanProposalResult, ArchivePlanResult, InboxRepository, NewPlan, NewPlanStep,
    PlanDiscussionReply, PlanDraft, PlanDraftStep, PlanRevisionSource, PlanStepUpdate, PlanUpdate,
    SqlxInboxRepository, Suggestion, UpdatePlanResult, UpdatePlanStepResult,
};
use notifications::{
    FcmRegistrationToken, FcmTokenRegistrationLimitReached, MAX_FCM_REGISTRATION_TOKENS_PER_OWNER,
    claim_due_notifications, save_fcm_registration_token,
};
use sqlx::{PgPool, postgres::PgPoolOptions};
use time::OffsetDateTime;
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
async fn fcm_token_registrations_are_capped_per_owner_without_blocking_refreshes() {
    let database = test_database().await;
    let owner_uid = format!("fcm-registration-owner-{}", Uuid::new_v4());
    let first_token = FcmRegistrationToken("fcm-registration-token-0".to_owned());

    for index in 0..MAX_FCM_REGISTRATION_TOKENS_PER_OWNER {
        save_fcm_registration_token(
            &database,
            &owner_uid,
            &FcmRegistrationToken(format!("fcm-registration-token-{index}")),
        )
        .await
        .expect("owner registrations below the limit should save");
    }
    save_fcm_registration_token(&database, &owner_uid, &first_token)
        .await
        .expect("refreshing an existing owner registration should remain allowed");

    let error = save_fcm_registration_token(
        &database,
        &owner_uid,
        &FcmRegistrationToken("fcm-registration-token-over-limit".to_owned()),
    )
    .await
    .expect_err("a new owner registration beyond the limit should fail");
    assert!(
        error
            .downcast_ref::<FcmTokenRegistrationLimitReached>()
            .is_some()
    );

    let registration_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM fcm_registration_tokens WHERE owner_uid = $1",
    )
    .bind(&owner_uid)
    .fetch_one(&database)
    .await
    .expect("owner registration count should load");
    assert_eq!(registration_count, MAX_FCM_REGISTRATION_TOKENS_PER_OWNER);

    sqlx::query("DELETE FROM fcm_registration_tokens WHERE owner_uid = $1")
        .bind(&owner_uid)
        .execute(&database)
        .await
        .expect("test FCM registrations should clean up");
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for an isolated PostgreSQL database"]
async fn due_fcm_notifications_retry_a_pending_claim_after_utc_midnight() {
    let database = test_database().await;
    let owner_uid = format!("due-fcm-owner-{}", Uuid::new_v4());
    let inbox_item_id = Uuid::new_v4();
    let plan_id = Uuid::new_v4();
    let plan_step_id = Uuid::new_v4();
    let today = OffsetDateTime::now_utc().date();
    let yesterday = today
        .previous_day()
        .expect("today should have a previous day");

    sqlx::query(
        r#"
        INSERT INTO inbox_items (id, owner_uid, source_type, original_text, status)
        VALUES ($1, $2, 'text', 'Prepare the documents.', 'planned')
        "#,
    )
    .bind(inbox_item_id)
    .bind(&owner_uid)
    .execute(&database)
    .await
    .expect("test Inbox item should insert");
    sqlx::query(
        r#"
        INSERT INTO plans (id, inbox_item_id, summary, status)
        VALUES ($1, $2, 'Prepare the documents.', 'ready')
        "#,
    )
    .bind(plan_id)
    .bind(inbox_item_id)
    .execute(&database)
    .await
    .expect("test Plan should insert");
    sqlx::query(
        r#"
        INSERT INTO plan_steps (id, plan_id, position, title, rationale, status, due_on, is_next_action)
        VALUES ($1, $2, 0, 'Prepare documents', 'Keeps the deadline on track.', 'ready', $3, true)
        "#,
    )
    .bind(plan_step_id)
    .bind(plan_id)
    .bind(yesterday)
    .execute(&database)
    .await
    .expect("test Plan step should insert");
    sqlx::query(
        r#"
        INSERT INTO due_fcm_notification_claims
            (plan_step_id, due_on, owner_uid, plan_id, claimed_at)
        VALUES ($1, $2, $3, $4, now() - interval '6 minutes')
        "#,
    )
    .bind(plan_step_id)
    .bind(yesterday)
    .bind(&owner_uid)
    .bind(plan_id)
    .execute(&database)
    .await
    .expect("failed due notification claim should insert");

    let claims = claim_due_notifications(&database, today)
        .await
        .expect("pending due notification should be reclaimed after midnight");
    assert_eq!(claims.len(), 1);
    assert_eq!(claims[0].plan_step_id, plan_step_id);

    sqlx::query("DELETE FROM plans WHERE id = $1")
        .bind(plan_id)
        .execute(&database)
        .await
        .expect("test due notification Plan should clean up");
    sqlx::query("DELETE FROM inbox_items WHERE id = $1")
        .bind(inbox_item_id)
        .execute(&database)
        .await
        .expect("test due notification fixture should clean up");
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
                expected_revision: 1,
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
                expected_revision: 2,
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
                expected_revision: 3,
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
                    expected_revision: 4,
                    status: PlanStatus::Ready,
                    waiting_on: None,
                },
            )
            .await
            .expect("foreign lookup should succeed"),
        UpdatePlanStepResult::NotFound
    ));

    sqlx::query("DELETE FROM plans WHERE id = $1")
        .bind(plan_id)
        .execute(&database)
        .await
        .expect("owned Plan should clean up");
    sqlx::query("DELETE FROM inbox_items WHERE id = $1")
        .bind(inbox_item_id)
        .execute(&database)
        .await
        .expect("owned fixture should clean up");
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for an isolated PostgreSQL database"]
async fn sqlx_repository_persists_plan_revisions_and_discussions() {
    let database = test_database().await;
    let repository = SqlxInboxRepository::new(database.clone());
    let owner_uid = format!("editable-plan-owner-{}", Uuid::new_v4());
    let inbox_item_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO inbox_items (id, owner_uid, source_type, original_text, status) VALUES ($1, $2, 'text', 'Renew the passport.', 'reviewing')",
    )
    .bind(inbox_item_id)
    .bind(&owner_uid)
    .execute(&database)
    .await
    .expect("reviewed Inbox item should insert");
    let created = repository
        .create_plan(
            &owner_uid,
            inbox_item_id,
            &NewPlan {
                summary: "Renew before travelling.".to_owned(),
                steps: vec![NewPlanStep {
                    title: "Confirm requirements".to_owned(),
                    rationale: "Clarifies the renewal deadline.".to_owned(),
                    status: PlanStatus::Ready,
                    due_on: None,
                    waiting_on: None,
                }],
            },
        )
        .await
        .expect("Plan creation should succeed")
        .expect("reviewed item should create a Plan");
    assert_eq!(created.revision, 1);
    let retained_step_id = created.steps[0].id;

    let UpdatePlanResult::Updated(edited) = repository
        .update_plan(
            &owner_uid,
            created.id,
            &PlanUpdate {
                expected_revision: 1,
                draft: PlanDraft {
                    summary: "Renew before travel and confirm the deadline.".to_owned(),
                    steps: vec![
                        PlanDraftStep {
                            id: Some(retained_step_id),
                            title: "Confirm official requirements".to_owned(),
                            rationale: "Clarifies the deadline.".to_owned(),
                            status: PlanStatus::Ready,
                            due_on: None,
                            waiting_on: None,
                        },
                        PlanDraftStep {
                            id: None,
                            title: "Prepare the documents".to_owned(),
                            rationale: "Makes the application ready.".to_owned(),
                            status: PlanStatus::Waiting,
                            due_on: None,
                            waiting_on: Some("The official requirements".to_owned()),
                        },
                    ],
                },
            },
            PlanRevisionSource::Manual,
        )
        .await
        .expect("Plan edit should succeed")
    else {
        panic!("current revision should accept the Plan edit");
    };
    assert_eq!(edited.revision, 2);
    assert_eq!(edited.steps[0].id, retained_step_id);

    let proposal = PlanDraft {
        summary: "Renew before travel.".to_owned(),
        steps: edited
            .steps
            .iter()
            .map(|step| PlanDraftStep {
                id: Some(step.id),
                title: step.title.clone(),
                rationale: step.rationale.clone(),
                status: if step.id == retained_step_id {
                    PlanStatus::Complete
                } else {
                    step.status
                },
                due_on: step.due_on,
                waiting_on: step.waiting_on.clone(),
            })
            .collect(),
    };
    let (_, assistant_message) = repository
        .add_plan_discussion(
            &owner_uid,
            created.id,
            edited.revision,
            "Could you make the first step complete?",
            &PlanDiscussionReply {
                content: "Here is a revision to review.".to_owned(),
                proposal: Some(proposal),
            },
        )
        .await
        .expect("discussion persistence should succeed")
        .expect("active Plan should retain its discussion");
    let page = repository
        .list_plan_messages(&owner_uid, created.id, None, 50)
        .await
        .expect("discussion list should succeed")
        .expect("active Plan should expose its discussion");
    assert_eq!(page.messages.len(), 2);
    assert_eq!(page.messages[0].role, inbox::PlanMessageRole::User);
    assert_eq!(page.messages[1].role, inbox::PlanMessageRole::Assistant);

    let ApplyPlanProposalResult::Updated(applied) = repository
        .apply_plan_proposal(&owner_uid, created.id, assistant_message.id, 2)
        .await
        .expect("proposal application should succeed")
    else {
        panic!("fresh proposal should apply exactly once");
    };
    assert_eq!(applied.revision, 3);
    let revision_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM plan_revisions WHERE plan_id = $1")
            .bind(created.id)
            .fetch_one(&database)
            .await
            .expect("Plan revisions should be queryable");
    assert_eq!(revision_count, 3);

    sqlx::query("DELETE FROM plans WHERE id = $1")
        .bind(created.id)
        .execute(&database)
        .await
        .expect("editable Plan should clean up");
    sqlx::query("DELETE FROM inbox_items WHERE id = $1")
        .bind(inbox_item_id)
        .execute(&database)
        .await
        .expect("editable Inbox item should clean up");
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
    .bind(Uuid::new_v4())
    .bind(foreign_plan_id)
    .execute(&database)
    .await
    .expect("test Plan steps should insert");

    let listed = repository
        .list_plans(&owner_uid, false)
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
        .list_plans(&other_owner_uid, false)
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

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for an isolated PostgreSQL database"]
async fn sqlx_repository_archives_only_owned_plan_pairs_and_restores_their_state() {
    let database = test_database().await;
    let repository = SqlxInboxRepository::new(database.clone());
    let owner_uid = format!("plan-archive-owner-{}", Uuid::new_v4());
    let other_owner_uid = format!("plan-archive-other-{}", Uuid::new_v4());
    let active_item_id = Uuid::new_v4();
    let archived_item_id = Uuid::new_v4();
    let foreign_item_id = Uuid::new_v4();
    let active_plan_id = Uuid::new_v4();
    let archived_plan_id = Uuid::new_v4();
    let foreign_plan_id = Uuid::new_v4();
    let archived_step_id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO inbox_items (id, owner_uid, source_type, original_text, status)
        VALUES
            ($1, $2, 'text', 'Keep active', 'planned'),
            ($3, $2, 'text', 'Archive me', 'planned'),
            ($4, $5, 'text', 'Another persons Plan', 'planned')
        "#,
    )
    .bind(active_item_id)
    .bind(&owner_uid)
    .bind(archived_item_id)
    .bind(foreign_item_id)
    .bind(&other_owner_uid)
    .execute(&database)
    .await
    .expect("test Inbox items should insert");
    sqlx::query(
        r#"
        INSERT INTO plans (id, inbox_item_id, summary, status, created_at, updated_at)
        VALUES
            ($1, $2, 'Keep active', 'ready', '2026-01-03T00:00:00Z', '2026-01-03T00:00:00Z'),
            ($3, $4, 'Archive me', 'waiting', '2026-01-02T00:00:00Z', '2026-01-02T00:00:00Z'),
            ($5, $6, 'Another persons Plan', 'ready', '2026-01-04T00:00:00Z', '2026-01-04T00:00:00Z')
        "#,
    )
    .bind(active_plan_id)
    .bind(active_item_id)
    .bind(archived_plan_id)
    .bind(archived_item_id)
    .bind(foreign_plan_id)
    .bind(foreign_item_id)
    .execute(&database)
    .await
    .expect("test Plans should insert");
    sqlx::query(
        r#"
        INSERT INTO plan_steps (id, plan_id, position, title, rationale, status, waiting_on, is_next_action)
        VALUES
            ($1, $2, 0, 'Keep active step', 'Keeps the active Plan complete.', 'ready', NULL, true),
            ($3, $4, 0, 'Wait for reply', 'Keeps the archived Plan state.', 'waiting', 'A reply from the agency', false),
            ($5, $6, 0, 'Foreign step', 'Must remain private.', 'ready', NULL, true)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(active_plan_id)
    .bind(archived_step_id)
    .bind(archived_plan_id)
    .bind(Uuid::new_v4())
    .bind(foreign_plan_id)
    .execute(&database)
    .await
    .expect("test Plan steps should insert");

    assert_eq!(
        repository
            .archive_plan(&other_owner_uid, archived_plan_id)
            .await
            .expect("foreign archive should complete safely"),
        ArchivePlanResult::NotFound
    );
    assert_eq!(
        repository
            .archive_plan(&owner_uid, archived_plan_id)
            .await
            .expect("owned Plan should archive"),
        ArchivePlanResult::Updated
    );
    assert_eq!(
        repository
            .archive_plan(&owner_uid, archived_plan_id)
            .await
            .expect("repeat archive should complete safely"),
        ArchivePlanResult::InvalidState
    );

    let active_plans = repository
        .list_plans(&owner_uid, false)
        .await
        .expect("active Plans should list");
    assert_eq!(
        active_plans.iter().map(|plan| plan.id).collect::<Vec<_>>(),
        [active_plan_id]
    );
    let archived_plans = repository
        .list_plans(&owner_uid, true)
        .await
        .expect("archived Plans should list");
    assert_eq!(
        archived_plans
            .iter()
            .map(|plan| plan.id)
            .collect::<Vec<_>>(),
        [archived_plan_id]
    );
    assert_eq!(
        archived_plans[0].steps[0].waiting_on.as_deref(),
        Some("A reply from the agency")
    );
    assert!(
        repository
            .get_plan(&owner_uid, archived_plan_id)
            .await
            .expect("archived Plan lookup should succeed")
            .is_none()
    );
    assert!(matches!(
        repository
            .update_plan_step(
                &owner_uid,
                archived_plan_id,
                archived_step_id,
                &PlanStepUpdate {
                    expected_revision: 1,
                    status: PlanStatus::Complete,
                    waiting_on: None,
                },
            )
            .await
            .expect("archived step update should complete safely"),
        UpdatePlanStepResult::NotFound
    ));

    assert_eq!(
        repository
            .restore_plan(&owner_uid, archived_plan_id)
            .await
            .expect("owned Plan should restore"),
        ArchivePlanResult::Updated
    );
    let restored = repository
        .get_plan(&owner_uid, archived_plan_id)
        .await
        .expect("restored Plan lookup should succeed")
        .expect("restored Plan should be visible");
    assert_eq!(restored.status, PlanStatus::Waiting);
    assert_eq!(
        restored.steps[0].waiting_on.as_deref(),
        Some("A reply from the agency")
    );
    assert_eq!(
        repository
            .list_plans(&owner_uid, false)
            .await
            .expect("restored Plans should list in activity order")
            .iter()
            .map(|plan| plan.id)
            .collect::<Vec<_>>(),
        [archived_plan_id, active_plan_id]
    );
    assert_eq!(
        repository
            .list(&owner_uid)
            .await
            .expect("Inbox should list restored source capture")
            .iter()
            .map(|item| item.id)
            .collect::<Vec<_>>()
            .len(),
        2
    );

    sqlx::query("DELETE FROM plans WHERE inbox_item_id = ANY($1)")
        .bind(vec![active_item_id, archived_item_id, foreign_item_id])
        .execute(&database)
        .await
        .expect("test Plans should clean up");
    sqlx::query("DELETE FROM inbox_items WHERE id = ANY($1)")
        .bind(vec![active_item_id, archived_item_id, foreign_item_id])
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
