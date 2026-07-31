use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use firebase_admin::{
    core::ServiceAccountKey,
    messaging::{Message, MessagingClient, SendResult, WebpushConfig},
};
use sqlx::{FromRow, PgPool};
use time::{Date, OffsetDateTime};
use uuid::Uuid;

const MAX_FCM_TOKEN_LENGTH: usize = 4_096;
pub(crate) const MAX_FCM_REGISTRATION_TOKENS_PER_OWNER: i64 = 10;
const DUE_NOTIFICATION_RETRY_AFTER: &str = "5 minutes";

#[derive(Clone, Debug)]
pub struct FcmRegistrationToken(pub String);

impl FcmRegistrationToken {
    pub fn validate(&self) -> Result<()> {
        if self.0.is_empty()
            || self.0.len() > MAX_FCM_TOKEN_LENGTH
            || !self.0.chars().all(|character| character.is_ascii_graphic())
        {
            bail!("the Firebase Cloud Messaging token is invalid");
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct FcmTokenRegistrationLimitReached;

impl std::fmt::Display for FcmTokenRegistrationLimitReached {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("the owner has reached the Firebase Cloud Messaging registration limit")
    }
}

impl std::error::Error for FcmTokenRegistrationLimitReached {}

#[async_trait]
pub trait NotificationService: Send + Sync {
    async fn save_token(&self, _owner_uid: &str, _token: &FcmRegistrationToken) -> Result<()> {
        Ok(())
    }

    async fn remove_token(&self, _owner_uid: &str, _token: &str) -> Result<()> {
        Ok(())
    }

    async fn suggestions_ready(&self, _owner_uid: &str, _item_id: Uuid) -> Result<()> {
        Ok(())
    }

    async fn plan_ready(&self, _owner_uid: &str, _plan_id: Uuid) -> Result<()> {
        Ok(())
    }

    async fn send_due_notifications(&self) -> Result<()> {
        Ok(())
    }

    fn is_configured(&self) -> bool {
        false
    }
}

#[derive(Default)]
pub struct DisabledFcmNotificationService;

#[async_trait]
impl NotificationService for DisabledFcmNotificationService {}

pub struct FcmNotificationService {
    database: PgPool,
    messaging: MessagingClient,
}

impl FcmNotificationService {
    pub fn new(database: PgPool, project_id: &str, service_account_json: &str) -> Result<Self> {
        let service_account = ServiceAccountKey::from_json(service_account_json)
            .context("FIREBASE_SERVICE_ACCOUNT_JSON is not a valid service account")?;
        let messaging = MessagingClient::builder(project_id)
            .service_account_key(service_account)
            .build()
            .context("could not initialize the Firebase Cloud Messaging client")?;

        Ok(Self {
            database,
            messaging,
        })
    }

    async fn send_to_owner(
        &self,
        owner_uid: &str,
        notification: FcmNotification,
    ) -> Result<Delivery> {
        let tokens = sqlx::query_scalar::<_, String>(
            "SELECT token FROM fcm_registration_tokens WHERE owner_uid = $1",
        )
        .bind(owner_uid)
        .fetch_all(&self.database)
        .await
        .context("could not load Firebase Cloud Messaging tokens")?;
        if tokens.is_empty() {
            return Ok(Delivery::Handled);
        }

        let mut has_success = false;
        let mut has_retryable_failure = false;
        for token_batch in tokens.chunks(500) {
            let messages = token_batch
                .iter()
                .map(|token| notification.message_for(token))
                .collect::<Vec<_>>();
            match self.messaging.send_each(&messages, false).await {
                Ok(batch) => {
                    has_success |= batch.success_count > 0;
                    for (token, result) in token_batch.iter().zip(batch.responses) {
                        if let SendResult::Failure { error } = result {
                            if error.error_code.as_deref() == Some("UNREGISTERED") {
                                sqlx::query("DELETE FROM fcm_registration_tokens WHERE token = $1")
                                    .bind(token)
                                    .execute(&self.database)
                                    .await
                                    .context("could not remove an unregistered FCM token")?;
                            } else {
                                has_retryable_failure = true;
                                tracing::warn!(
                                    status = error.status,
                                    "Firebase Cloud Messaging did not accept a notification"
                                );
                            }
                        }
                    }
                }
                Err(_) => {
                    has_retryable_failure = true;
                    tracing::warn!("could not deliver a Firebase Cloud Messaging notification");
                }
            }
        }

        Ok(delivery_from_attempts(has_success, has_retryable_failure))
    }

    async fn claim_due_notifications(&self, due_on: Date) -> Result<Vec<DueNotification>> {
        claim_due_notifications(&self.database, due_on).await
    }

    async fn mark_due_notification_sent(&self, notification: &DueNotification) -> Result<()> {
        sqlx::query(
            "UPDATE due_fcm_notification_claims SET sent_at = now() WHERE plan_step_id = $1 AND due_on = $2",
        )
        .bind(notification.plan_step_id)
        .bind(notification.due_on)
        .execute(&self.database)
        .await
        .context("could not record a due Firebase Cloud Messaging notification")?;
        Ok(())
    }
}

#[async_trait]
impl NotificationService for FcmNotificationService {
    fn is_configured(&self) -> bool {
        true
    }

    async fn save_token(&self, owner_uid: &str, token: &FcmRegistrationToken) -> Result<()> {
        save_fcm_registration_token(&self.database, owner_uid, token).await
    }

    async fn remove_token(&self, owner_uid: &str, token: &str) -> Result<()> {
        sqlx::query("DELETE FROM fcm_registration_tokens WHERE token = $1 AND owner_uid = $2")
            .bind(token)
            .bind(owner_uid)
            .execute(&self.database)
            .await
            .context("could not remove a Firebase Cloud Messaging token")?;
        Ok(())
    }

    async fn suggestions_ready(&self, owner_uid: &str, item_id: Uuid) -> Result<()> {
        self.send_to_owner(
            owner_uid,
            FcmNotification::new(
                "Suggestions ready",
                "Your suggestions are ready for review.",
                format!("/inbox/{item_id}/review"),
                "suggestions-ready",
            ),
        )
        .await
        .map(|_| ())
    }

    async fn plan_ready(&self, owner_uid: &str, plan_id: Uuid) -> Result<()> {
        self.send_to_owner(
            owner_uid,
            FcmNotification::new(
                "Plan ready",
                "Your Plan is ready to review.",
                format!("/plans/{plan_id}"),
                "plan-ready",
            ),
        )
        .await
        .map(|_| ())
    }

    async fn send_due_notifications(&self) -> Result<()> {
        let due_on = OffsetDateTime::now_utc().date();
        for due_notification in self.claim_due_notifications(due_on).await? {
            match self
                .send_to_owner(
                    &due_notification.owner_uid,
                    FcmNotification::new(
                        "Plan step due",
                        "A Plan step is due today.",
                        format!("/plans/{}", due_notification.plan_id),
                        "plan-step-due",
                    ),
                )
                .await?
            {
                Delivery::Handled => self.mark_due_notification_sent(&due_notification).await?,
                Delivery::RetryableFailure => {
                    tracing::warn!(
                        "will retry a due Firebase Cloud Messaging notification after its lease"
                    );
                }
            }
        }
        Ok(())
    }
}

pub fn spawn_due_notification_worker(service: Arc<dyn NotificationService>) {
    if !service.is_configured() {
        return;
    }
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            if let Err(error) = service.send_due_notifications().await {
                tracing::error!(%error, "could not process due Firebase Cloud Messaging notifications");
            }
        }
    });
}

#[derive(FromRow)]
pub(crate) struct DueNotification {
    pub(crate) plan_step_id: Uuid,
    pub(crate) due_on: Date,
    pub(crate) owner_uid: String,
    pub(crate) plan_id: Uuid,
}

pub(crate) async fn save_fcm_registration_token(
    database: &PgPool,
    owner_uid: &str,
    token: &FcmRegistrationToken,
) -> Result<()> {
    token.validate()?;
    let mut transaction = database
        .begin()
        .await
        .context("could not start a Firebase Cloud Messaging token transaction")?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(owner_uid)
        .execute(&mut *transaction)
        .await
        .context("could not lock Firebase Cloud Messaging token registrations")?;
    let existing_owner = sqlx::query_scalar::<_, String>(
        "SELECT owner_uid FROM fcm_registration_tokens WHERE token = $1 FOR UPDATE",
    )
    .bind(&token.0)
    .fetch_optional(&mut *transaction)
    .await
    .context("could not inspect the Firebase Cloud Messaging token")?;
    if existing_owner.as_deref() != Some(owner_uid) {
        let registration_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM fcm_registration_tokens WHERE owner_uid = $1",
        )
        .bind(owner_uid)
        .fetch_one(&mut *transaction)
        .await
        .context("could not count Firebase Cloud Messaging token registrations")?;
        if registration_count >= MAX_FCM_REGISTRATION_TOKENS_PER_OWNER {
            return Err(FcmTokenRegistrationLimitReached.into());
        }
    }
    sqlx::query(
        r#"
        INSERT INTO fcm_registration_tokens (token, owner_uid)
        VALUES ($1, $2)
        ON CONFLICT (token) DO UPDATE
        SET owner_uid = EXCLUDED.owner_uid,
            updated_at = now()
        "#,
    )
    .bind(&token.0)
    .bind(owner_uid)
    .execute(&mut *transaction)
    .await
    .context("could not save a Firebase Cloud Messaging token")?;
    transaction
        .commit()
        .await
        .context("could not commit the Firebase Cloud Messaging token transaction")?;
    Ok(())
}

pub(crate) async fn claim_due_notifications(
    database: &PgPool,
    due_on: Date,
) -> Result<Vec<DueNotification>> {
    sqlx::query_as::<_, DueNotification>(
        r#"
        INSERT INTO due_fcm_notification_claims
            (plan_step_id, due_on, owner_uid, plan_id, claimed_at)
        SELECT ps.id, ps.due_on, i.owner_uid, p.id, now()
        FROM plan_steps ps
        INNER JOIN plans p ON p.id = ps.plan_id
        INNER JOIN inbox_items i ON i.id = p.inbox_item_id
        LEFT JOIN due_fcm_notification_claims claimed
            ON claimed.plan_step_id = ps.id AND claimed.due_on = ps.due_on
        WHERE ps.due_on <= $1
            AND ps.status <> 'complete'
            AND i.status = 'planned'
            AND (
                (ps.due_on = $1 AND claimed.plan_step_id IS NULL)
                OR (
                    claimed.sent_at IS NULL
                    AND claimed.claimed_at < now() - $2::interval
                )
            )
        ON CONFLICT (plan_step_id, due_on) DO UPDATE
            SET claimed_at = EXCLUDED.claimed_at
        WHERE due_fcm_notification_claims.sent_at IS NULL
            AND due_fcm_notification_claims.claimed_at < now() - $2::interval
        RETURNING plan_step_id, due_on, owner_uid, plan_id
        "#,
    )
    .bind(due_on)
    .bind(DUE_NOTIFICATION_RETRY_AFTER)
    .fetch_all(database)
    .await
    .context("could not claim due Firebase Cloud Messaging notifications")
}

#[derive(Clone)]
struct FcmNotification {
    title: String,
    body: String,
    url: String,
    tag: String,
}

impl FcmNotification {
    fn new(title: &str, body: &str, url: String, tag: &str) -> Self {
        Self {
            title: title.to_owned(),
            body: body.to_owned(),
            url,
            tag: tag.to_owned(),
        }
    }

    fn message_for(&self, token: &str) -> Message {
        let data = HashMap::from([
            ("title".to_owned(), self.title.clone()),
            ("body".to_owned(), self.body.clone()),
            ("url".to_owned(), self.url.clone()),
            ("tag".to_owned(), self.tag.clone()),
        ]);
        Message::to_token(token)
            .with_data(data)
            .with_webpush(WebpushConfig {
                headers: HashMap::from([
                    ("TTL".to_owned(), (24 * 60 * 60).to_string()),
                    ("Urgency".to_owned(), "high".to_owned()),
                ]),
                data: HashMap::new(),
                notification: None,
            })
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Delivery {
    Handled,
    RetryableFailure,
}

fn delivery_from_attempts(has_success: bool, has_retryable_failure: bool) -> Delivery {
    match (has_success, has_retryable_failure) {
        (_, true) => Delivery::RetryableFailure,
        _ => Delivery::Handled,
    }
}

#[cfg(test)]
mod tests {
    use super::{Delivery, FcmRegistrationToken, delivery_from_attempts};

    #[test]
    fn validates_a_non_empty_fcm_token_without_logging_it() {
        assert!(
            FcmRegistrationToken("fcm-token-123".to_owned())
                .validate()
                .is_ok()
        );
        assert!(
            FcmRegistrationToken("token with a space".to_owned())
                .validate()
                .is_err()
        );
    }

    #[test]
    fn retries_due_alerts_when_any_target_has_a_transient_delivery_failure() {
        assert_eq!(
            delivery_from_attempts(true, true),
            Delivery::RetryableFailure
        );
        assert_eq!(delivery_from_attempts(true, false), Delivery::Handled);
    }
}
