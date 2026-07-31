use std::sync::Arc;

use axum::{
    Json, Router,
    body::Bytes,
    extract::{
        DefaultBodyLimit, FromRequest, Multipart, Path, Request, State, multipart::MultipartError,
        rejection::JsonRejection,
    },
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, patch, post},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;
use time::{
    Date, OffsetDateTime, format_description::well_known::Rfc3339, macros::format_description,
};
use tower_http::trace::TraceLayer;
use uuid::Uuid;

use crate::{
    ai::{AiError, AiProvider, ExtractionInput, enqueue_cleanup},
    auth::{AuthenticatedUser, TokenVerifier},
    domain::{CaptureSourceType, InboxStatus, PlanStatus},
    inbox::{
        FileCapture, InboxItem, InboxItemDetail, InboxRepository, NewSuggestion, Plan, PlanStep,
        PlanStepUpdate, Suggestion, SuggestionKind, UpdatePlanStepResult,
    },
    storage::PrivateObjectStore,
};

const MAX_TEXT_CAPTURE_CHARACTERS: usize = 10_000;
const MAX_FILE_BYTES: usize = 10 * 1024 * 1024;
const MAX_MULTIPART_BODY_BYTES: usize = MAX_FILE_BYTES + 1024 * 1024;
const MAX_SUGGESTIONS: usize = 25;
const MAX_WAITING_ON_CHARACTERS: usize = 2_000;

#[derive(Clone)]
pub struct AppState {
    pub database: PgPool,
    pub inbox_repository: Arc<dyn InboxRepository>,
    pub token_verifier: Arc<dyn TokenVerifier>,
    pub object_store: Arc<dyn PrivateObjectStore>,
    pub ai_provider: Arc<dyn AiProvider>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/ready", get(readiness))
        .route("/api/v1/me", get(current_user))
        .route(
            "/api/v1/inbox-items",
            get(list_inbox_items).post(create_text_capture),
        )
        .route(
            "/api/v1/inbox-items/{item_id}",
            get(get_inbox_item).patch(replace_suggestions),
        )
        .route(
            "/api/v1/inbox-items/{item_id}/extract",
            post(extract_inbox_item),
        )
        .route(
            "/api/v1/inbox-items/{item_id}/file",
            get(get_inbox_item_pdf),
        )
        .route("/api/v1/inbox-items/{item_id}/plans", post(create_plan))
        .route("/api/v1/plans/{plan_id}", get(get_plan))
        .route(
            "/api/v1/plans/{plan_id}/steps/{step_id}",
            patch(update_plan_step),
        )
        .route(
            "/api/v1/inbox-items/files",
            post(create_file_capture).layer(DefaultBodyLimit::max(MAX_MULTIPART_BODY_BYTES)),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn health() -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn readiness(State(state): State<AppState>) -> impl IntoResponse {
    match sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.database)
        .await
    {
        Ok(_) => (StatusCode::OK, Json(json!({ "status": "ok" }))),
        Err(error) => {
            tracing::error!(%error, "PostgreSQL readiness check failed");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "status": "database_unavailable" })),
            )
        }
    }
}

async fn current_user(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Ok(user) = authenticated_user(&headers, state.token_verifier.as_ref()).await else {
        return unauthenticated_response();
    };
    (StatusCode::OK, Json(json!({ "user": user }))).into_response()
}

async fn create_text_capture(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Result<Json<CreateTextCaptureRequest>, JsonRejection>,
) -> Response {
    let Ok(user) = authenticated_user(&headers, state.token_verifier.as_ref()).await else {
        return unauthenticated_response();
    };
    let Ok(Json(request)) = request else {
        return text_validation_error_response();
    };
    if !is_valid_text_capture(&request.text) {
        return text_validation_error_response();
    }
    let item = match state
        .inbox_repository
        .create_text(&user.uid, &request.text)
        .await
    {
        Ok(item) => item,
        Err(error) => {
            tracing::error!(%error, "could not create Inbox item");
            return internal_error_response("Could not save Inbox item.");
        }
    };
    capture_response_after_extraction(&state, &user.uid, item).await
}

async fn create_file_capture(State(state): State<AppState>, request: Request) -> Response {
    // Authenticate before multipart parsing, private storage, or any model interaction.
    let Ok(user) = authenticated_user(request.headers(), state.token_verifier.as_ref()).await
    else {
        return unauthenticated_response();
    };
    if request
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<usize>().ok())
        .is_some_and(|length| length > MAX_MULTIPART_BODY_BYTES)
    {
        return file_too_large_response();
    }
    let mut multipart = match Multipart::from_request(request, &state).await {
        Ok(multipart) => multipart,
        Err(_) => return file_validation_error_response(),
    };
    let field = match multipart.next_field().await {
        Ok(Some(field)) => field,
        Ok(None) => return file_validation_error_response(),
        Err(error) => return multipart_error_response(&error),
    };
    let field_name = field.name().map(str::to_owned);
    let filename = field.file_name().map(str::to_owned);
    let declared_content_type = field.content_type().map(str::to_owned);
    if field_name.as_deref() != Some("file") {
        return file_validation_error_response();
    }
    let content = match field.bytes().await {
        Ok(content) => content,
        Err(error) => return multipart_error_response(&error),
    };
    if content.len() > MAX_FILE_BYTES {
        return file_too_large_response();
    }
    match multipart.next_field().await {
        Ok(None) => {}
        Ok(Some(_)) => return file_validation_error_response(),
        Err(error) => return multipart_error_response(&error),
    }
    let Some(filename) = filename.filter(|value| is_safe_filename(value)) else {
        return file_validation_error_response();
    };
    let Some((source_type, content_type)) =
        validated_file_type(declared_content_type.as_deref(), &content)
    else {
        return file_validation_error_response();
    };
    let object_key = Uuid::new_v4().to_string();
    if state
        .object_store
        .upload(&object_key, content_type, &content)
        .await
        .is_err()
    {
        return storage_unavailable_response();
    }
    let capture = FileCapture {
        source_type,
        original_filename: filename,
        content_type: content_type.to_owned(),
        storage_key: object_key.clone(),
        byte_size: content.len() as i64,
    };
    let item = match state
        .inbox_repository
        .create_file(&user.uid, &capture)
        .await
    {
        Ok(item) => item,
        Err(error) => {
            if state.object_store.delete(&object_key).await.is_err() {
                tracing::error!(%error, "could not remove private object after Inbox database failure");
            }
            tracing::error!(%error, "could not create Inbox item for a private upload");
            return internal_error_response("Could not save Inbox item.");
        }
    };
    capture_response_after_extraction(&state, &user.uid, item).await
}

async fn capture_response_after_extraction(
    state: &AppState,
    owner_uid: &str,
    item: InboxItem,
) -> Response {
    let original = InboxItemResponse::from_item(&item, state.ai_provider.as_ref());
    match item.source_type {
        CaptureSourceType::Image => (
            StatusCode::CREATED,
            Json(CaptureResponse {
                inbox_item: original,
                extraction: ExtractionState::NotSupported,
            }),
        )
            .into_response(),
        CaptureSourceType::Text | CaptureSourceType::Pdf => {
            match extract_owned_item(state, owner_uid, item.id).await {
                Ok(detail) => (
                    StatusCode::CREATED,
                    Json(CaptureResponse {
                        inbox_item: InboxItemResponse::from_item(
                            &detail.item,
                            state.ai_provider.as_ref(),
                        ),
                        extraction: ExtractionState::Ready,
                    }),
                )
                    .into_response(),
                Err(ExtractionError::Unsupported) => (
                    StatusCode::CREATED,
                    Json(CaptureResponse {
                        inbox_item: original,
                        extraction: ExtractionState::NotSupported,
                    }),
                )
                    .into_response(),
                Err(error) => {
                    tracing::warn!(kind = ?error.kind(), "automatic extraction was unavailable after capture");
                    (
                        StatusCode::CREATED,
                        Json(CaptureResponse {
                            inbox_item: original,
                            extraction: ExtractionState::Retryable,
                        }),
                    )
                        .into_response()
                }
            }
        }
    }
}

async fn list_inbox_items(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Ok(user) = authenticated_user(&headers, state.token_verifier.as_ref()).await else {
        return unauthenticated_response();
    };
    match state.inbox_repository.list(&user.uid).await {
        Ok(items) => (
            StatusCode::OK,
            Json(ListInboxItemsResponse {
                inbox_items: items
                    .iter()
                    .map(|item| InboxItemResponse::from_item(item, state.ai_provider.as_ref()))
                    .collect(),
            }),
        )
            .into_response(),
        Err(error) => {
            tracing::error!(%error, "could not list Inbox items");
            internal_error_response("Could not load Inbox items.")
        }
    }
}

async fn get_inbox_item(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(item_id): Path<String>,
) -> Response {
    let Ok(user) = authenticated_user(&headers, state.token_verifier.as_ref()).await else {
        return unauthenticated_response();
    };
    let Ok(item_id) = Uuid::parse_str(&item_id) else {
        return inbox_item_id_validation_error_response();
    };
    match state.inbox_repository.get(&user.uid, item_id).await {
        Ok(Some(item)) => (
            StatusCode::OK,
            Json(GetInboxItemResponse {
                inbox_item: InboxItemDetailResponse::from_item(&item, state.ai_provider.as_ref()),
            }),
        )
            .into_response(),
        Ok(None) => inbox_item_not_found_response(),
        Err(error) => {
            tracing::error!(%error, "could not load Inbox item");
            internal_error_response("Could not load Inbox item.")
        }
    }
}

async fn extract_inbox_item(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(item_id): Path<String>,
) -> Response {
    let Ok(user) = authenticated_user(&headers, state.token_verifier.as_ref()).await else {
        return unauthenticated_response();
    };
    let Ok(item_id) = Uuid::parse_str(&item_id) else {
        return inbox_item_id_validation_error_response();
    };
    match extract_owned_item(&state, &user.uid, item_id).await {
        Ok(item) => (
            StatusCode::OK,
            Json(GetInboxItemResponse {
                inbox_item: InboxItemDetailResponse::from_item(&item, state.ai_provider.as_ref()),
            }),
        )
            .into_response(),
        Err(error) => extraction_error_response(error),
    }
}

async fn replace_suggestions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(item_id): Path<String>,
    request: Result<Json<ReplaceSuggestionsRequest>, JsonRejection>,
) -> Response {
    let Ok(user) = authenticated_user(&headers, state.token_verifier.as_ref()).await else {
        return unauthenticated_response();
    };
    let Ok(item_id) = Uuid::parse_str(&item_id) else {
        return inbox_item_id_validation_error_response();
    };
    let Ok(Json(request)) = request else {
        return suggestion_validation_error_response();
    };
    let Ok(suggestions) = request
        .suggestions
        .into_iter()
        .map(TryInto::try_into)
        .collect::<Result<Vec<NewSuggestion>, _>>()
    else {
        return suggestion_validation_error_response();
    };
    if suggestions.len() > MAX_SUGGESTIONS {
        return suggestion_validation_error_response();
    }
    match state.inbox_repository.get(&user.uid, item_id).await {
        Ok(Some(item)) if item.item.status == InboxStatus::Reviewing => {}
        Ok(Some(_)) => {
            return invalid_state_response(
                "Suggestions can only be edited while an item is under review.",
            );
        }
        Ok(None) => return inbox_item_not_found_response(),
        Err(error) => {
            tracing::error!(%error, "could not load Inbox item before updating suggestions");
            return internal_error_response("Could not load Inbox item.");
        }
    }
    match state
        .inbox_repository
        .replace_suggestions(&user.uid, item_id, &suggestions)
        .await
    {
        Ok(Some(item)) => (
            StatusCode::OK,
            Json(GetInboxItemResponse {
                inbox_item: InboxItemDetailResponse::from_item(&item, state.ai_provider.as_ref()),
            }),
        )
            .into_response(),
        Ok(None) => {
            invalid_state_response("Suggestions can only be edited while an item is under review.")
        }
        Err(error) => {
            tracing::error!(%error, "could not update reviewed suggestions");
            internal_error_response("Could not save reviewed suggestions.")
        }
    }
}

async fn get_inbox_item_pdf(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(item_id): Path<String>,
) -> Response {
    let Ok(user) = authenticated_user(&headers, state.token_verifier.as_ref()).await else {
        return unauthenticated_response();
    };
    let Ok(item_id) = Uuid::parse_str(&item_id) else {
        return inbox_item_id_validation_error_response();
    };
    let file = match state.inbox_repository.get_file(&user.uid, item_id).await {
        Ok(Some(file)) => file,
        Ok(None) => return inbox_item_not_found_response(),
        Err(error) => {
            tracing::error!(%error, "could not load private PDF metadata");
            return internal_error_response("Could not load Inbox item.");
        }
    };
    let content = match state.object_store.download(&file.storage_key).await {
        Ok(content) => content,
        Err(_) => return storage_unavailable_response(),
    };
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/pdf"),
            (header::CACHE_CONTROL, "private, no-store"),
            (header::CONTENT_DISPOSITION, "inline"),
        ],
        Bytes::from(content),
    )
        .into_response()
}

async fn create_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(item_id): Path<String>,
) -> Response {
    let Ok(user) = authenticated_user(&headers, state.token_verifier.as_ref()).await else {
        return unauthenticated_response();
    };
    let Ok(item_id) = Uuid::parse_str(&item_id) else {
        return inbox_item_id_validation_error_response();
    };
    let item = match state.inbox_repository.get(&user.uid, item_id).await {
        Ok(Some(item)) => item,
        Ok(None) => return inbox_item_not_found_response(),
        Err(error) => {
            tracing::error!(%error, "could not load reviewed Inbox item for planning");
            return internal_error_response("Could not load Inbox item.");
        }
    };
    if item.item.status != InboxStatus::Reviewing || item.suggestions.is_empty() {
        return invalid_state_response(
            "Save at least one reviewed suggestion before generating a plan.",
        );
    }
    let generated = match state.ai_provider.plan(&item.suggestions).await {
        Ok(plan) => plan,
        Err(error) => return ai_error_response(error),
    };
    match state
        .inbox_repository
        .create_plan(&user.uid, item_id, &generated)
        .await
    {
        Ok(Some(plan)) => (StatusCode::CREATED, Json(PlanResponse::from(&plan))).into_response(),
        Ok(None) => invalid_state_response("A plan can only be generated once after review."),
        Err(error) => {
            tracing::error!(%error, "could not persist approved plan");
            internal_error_response("Could not save the plan.")
        }
    }
}

async fn get_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(plan_id): Path<String>,
) -> Response {
    let Ok(user) = authenticated_user(&headers, state.token_verifier.as_ref()).await else {
        return unauthenticated_response();
    };
    let Ok(plan_id) = Uuid::parse_str(&plan_id) else {
        return plan_id_validation_error_response();
    };
    match state.inbox_repository.get_plan(&user.uid, plan_id).await {
        Ok(Some(plan)) => (StatusCode::OK, Json(PlanResponse::from(&plan))).into_response(),
        Ok(None) => api_error_response(StatusCode::NOT_FOUND, "NOT_FOUND", "Plan not found."),
        Err(error) => {
            tracing::error!(%error, "could not load Plan");
            internal_error_response("Could not load the plan.")
        }
    }
}

async fn update_plan_step(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((plan_id, step_id)): Path<(String, String)>,
    request: Result<Json<UpdatePlanStepRequest>, JsonRejection>,
) -> Response {
    let Ok(user) = authenticated_user(&headers, state.token_verifier.as_ref()).await else {
        return unauthenticated_response();
    };
    let Ok(plan_id) = Uuid::parse_str(&plan_id) else {
        return plan_id_validation_error_response();
    };
    let Ok(step_id) = Uuid::parse_str(&step_id) else {
        return plan_step_id_validation_error_response();
    };
    let Ok(Json(request)) = request else {
        return plan_step_validation_error_response();
    };
    let Ok(update) = PlanStepUpdate::try_from(request) else {
        return plan_step_validation_error_response();
    };

    match state
        .inbox_repository
        .update_plan_step(&user.uid, plan_id, step_id, &update)
        .await
    {
        Ok(UpdatePlanStepResult::Updated(plan)) => {
            (StatusCode::OK, Json(PlanResponse::from(&plan))).into_response()
        }
        Ok(UpdatePlanStepResult::NotFound) => plan_step_not_found_response(),
        Ok(UpdatePlanStepResult::InvalidState) => {
            invalid_state_response("This Plan step can no longer be changed.")
        }
        Err(error) => {
            tracing::error!(%error, "could not update Plan step");
            internal_error_response("Could not update the Plan step.")
        }
    }
}

#[derive(Debug)]
enum ExtractionError {
    NotFound,
    InvalidState,
    Unsupported,
    Storage,
    Ai(AiError),
    Persistence,
}

impl ExtractionError {
    fn kind(&self) -> &'static str {
        match self {
            Self::NotFound => "not_found",
            Self::InvalidState => "invalid_state",
            Self::Unsupported => "unsupported",
            Self::Storage => "storage",
            Self::Ai(_) => "ai",
            Self::Persistence => "persistence",
        }
    }
}

async fn extract_owned_item(
    state: &AppState,
    owner_uid: &str,
    item_id: Uuid,
) -> Result<InboxItemDetail, ExtractionError> {
    let detail = state
        .inbox_repository
        .get(owner_uid, item_id)
        .await
        .map_err(|_| ExtractionError::Persistence)?
        .ok_or(ExtractionError::NotFound)?;
    if detail.item.status != InboxStatus::Captured {
        return Err(ExtractionError::InvalidState);
    }
    let input = match detail.item.source_type {
        CaptureSourceType::Text => {
            ExtractionInput::Text(detail.original_text.ok_or(ExtractionError::Persistence)?)
        }
        CaptureSourceType::Pdf => {
            let file = state
                .inbox_repository
                .get_file(owner_uid, item_id)
                .await
                .map_err(|_| ExtractionError::Persistence)?
                .ok_or(ExtractionError::NotFound)?;
            let content = state
                .object_store
                .download(&file.storage_key)
                .await
                .map_err(|_| ExtractionError::Storage)?;
            ExtractionInput::Pdf {
                filename: detail
                    .original_filename
                    .unwrap_or_else(|| "capture.pdf".to_owned()),
                content,
            }
        }
        CaptureSourceType::Image => return Err(ExtractionError::Unsupported),
    };
    let suggestions = match extract_with_single_retry(state, input).await {
        Ok(suggestions) => suggestions,
        Err(AiError::Unsupported) => return Err(ExtractionError::Unsupported),
        Err(error) => return Err(ExtractionError::Ai(error)),
    };
    state
        .inbox_repository
        .save_extraction(owner_uid, item_id, &suggestions)
        .await
        .map_err(|_| ExtractionError::Persistence)?
        .ok_or(ExtractionError::InvalidState)
}

async fn extract_with_single_retry(
    state: &AppState,
    input: ExtractionInput,
) -> Result<Vec<NewSuggestion>, AiError> {
    for attempt in 0..2 {
        let call = state.ai_provider.extract(input.clone()).await;
        if let Some(file_id) = call.cleanup_file_id
            && enqueue_cleanup(&state.database, &file_id).await.is_err()
        {
            tracing::error!("could not persist a pending OpenAI file deletion");
        }
        match call.result {
            Ok(extraction) => return Ok(extraction.suggestions),
            Err(error) if error.is_transient() && attempt == 0 => continue,
            Err(error) => return Err(error),
        }
    }
    Err(AiError::Transient)
}

async fn authenticated_user(
    headers: &HeaderMap,
    verifier: &dyn TokenVerifier,
) -> Result<AuthenticatedUser, ()> {
    verifier.verify(bearer_token(headers).ok_or(())?).await
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let token = headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")?;
    if token.is_empty() || token.chars().any(char::is_whitespace) {
        None
    } else {
        Some(token)
    }
}

fn unauthenticated_response() -> Response {
    api_error_response(
        StatusCode::UNAUTHORIZED,
        "UNAUTHENTICATED",
        "Authentication required.",
    )
}

fn text_validation_error_response() -> Response {
    api_error_response(
        StatusCode::BAD_REQUEST,
        "VALIDATION_ERROR",
        "Text must be a non-empty value of at most 10,000 characters.",
    )
}

fn suggestion_validation_error_response() -> Response {
    api_error_response(
        StatusCode::BAD_REQUEST,
        "VALIDATION_ERROR",
        "Suggestions must contain valid kinds, non-empty content, and ISO calendar dates.",
    )
}

fn inbox_item_id_validation_error_response() -> Response {
    api_error_response(
        StatusCode::BAD_REQUEST,
        "VALIDATION_ERROR",
        "Inbox item ID must be a valid UUID.",
    )
}

fn plan_id_validation_error_response() -> Response {
    api_error_response(
        StatusCode::BAD_REQUEST,
        "VALIDATION_ERROR",
        "Plan ID must be a valid UUID.",
    )
}

fn plan_step_id_validation_error_response() -> Response {
    api_error_response(
        StatusCode::BAD_REQUEST,
        "VALIDATION_ERROR",
        "Plan step ID must be a valid UUID.",
    )
}

fn plan_step_validation_error_response() -> Response {
    api_error_response(
        StatusCode::BAD_REQUEST,
        "VALIDATION_ERROR",
        "Plan step updates require a valid status and waiting-on detail.",
    )
}

fn inbox_item_not_found_response() -> Response {
    api_error_response(StatusCode::NOT_FOUND, "NOT_FOUND", "Inbox item not found.")
}

fn plan_step_not_found_response() -> Response {
    api_error_response(StatusCode::NOT_FOUND, "NOT_FOUND", "Plan step not found.")
}

fn invalid_state_response(message: &str) -> Response {
    api_error_response(StatusCode::CONFLICT, "INVALID_STATE", message)
}

fn file_validation_error_response() -> Response {
    api_error_response(
        StatusCode::BAD_REQUEST,
        "VALIDATION_ERROR",
        "Upload exactly one PDF, JPEG, or PNG file with a safe filename.",
    )
}

fn multipart_error_response(error: &MultipartError) -> Response {
    if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
        file_too_large_response()
    } else {
        file_validation_error_response()
    }
}

fn file_too_large_response() -> Response {
    api_error_response(
        StatusCode::PAYLOAD_TOO_LARGE,
        "FILE_TOO_LARGE",
        "Files must not exceed 10 MiB.",
    )
}

fn storage_unavailable_response() -> Response {
    api_error_response(
        StatusCode::SERVICE_UNAVAILABLE,
        "STORAGE_UNAVAILABLE",
        "Private file storage is temporarily unavailable. Please try again later.",
    )
}

fn extraction_error_response(error: ExtractionError) -> Response {
    match error {
        ExtractionError::NotFound => inbox_item_not_found_response(),
        ExtractionError::InvalidState => {
            invalid_state_response("This item is no longer available for extraction.")
        }
        ExtractionError::Unsupported => api_error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "UNSUPPORTED_CAPTURE",
            "AI extraction is not available for this capture type.",
        ),
        ExtractionError::Storage => storage_unavailable_response(),
        ExtractionError::Ai(error) => ai_error_response(error),
        ExtractionError::Persistence => {
            internal_error_response("Could not save extracted suggestions.")
        }
    }
}

fn ai_error_response(error: AiError) -> Response {
    match error {
        AiError::Unsupported => api_error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "AI_UNSUPPORTED",
            "The configured AI provider does not support this operation.",
        ),
        AiError::InvalidOutput => api_error_response(
            StatusCode::BAD_GATEWAY,
            "AI_OUTPUT_INVALID",
            "The AI response could not be used. Please try again.",
        ),
        AiError::Unavailable | AiError::Transient | AiError::Failed => api_error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "AI_UNAVAILABLE",
            "AI sorting is temporarily unavailable. Please try again.",
        ),
    }
}

fn internal_error_response(message: &str) -> Response {
    api_error_response(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", message)
}

fn api_error_response(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(json!({ "error": { "code": code, "message": message } })),
    )
        .into_response()
}

fn is_valid_text_capture(text: &str) -> bool {
    !text.trim().is_empty() && text.chars().count() <= MAX_TEXT_CAPTURE_CHARACTERS
}

fn is_safe_filename(filename: &str) -> bool {
    !filename.trim().is_empty()
        && filename.len() <= 255
        && filename != "."
        && filename != ".."
        && !filename
            .chars()
            .any(|character| character.is_control() || matches!(character, '/' | '\\'))
}

fn validated_file_type(
    content_type: Option<&str>,
    content: &[u8],
) -> Option<(CaptureSourceType, &'static str)> {
    match content_type? {
        "image/jpeg" if content.starts_with(&[0xFF, 0xD8, 0xFF]) => {
            Some((CaptureSourceType::Image, "image/jpeg"))
        }
        "image/png" if content.starts_with(b"\x89PNG\r\n\x1a\n") => {
            Some((CaptureSourceType::Image, "image/png"))
        }
        "application/pdf" if content.starts_with(b"%PDF-") => {
            Some((CaptureSourceType::Pdf, "application/pdf"))
        }
        _ => None,
    }
}

fn format_timestamp(timestamp: OffsetDateTime) -> String {
    timestamp
        .format(&Rfc3339)
        .expect("database timestamps must be RFC 3339")
}

fn format_date(date: Date) -> String {
    date.format(format_description!("[year]-[month]-[day]"))
        .expect("database dates must be ISO dates")
}

fn parse_date(value: Option<String>) -> Result<Option<Date>, ()> {
    value
        .map(|value| {
            Date::parse(&value, format_description!("[year]-[month]-[day]")).map_err(|_| ())
        })
        .transpose()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateTextCaptureRequest {
    text: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ReplaceSuggestionsRequest {
    suggestions: Vec<SuggestionInput>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SuggestionInput {
    kind: SuggestionKind,
    content: String,
    due_on: Option<String>,
}

impl TryFrom<SuggestionInput> for NewSuggestion {
    type Error = ();

    fn try_from(value: SuggestionInput) -> Result<Self, Self::Error> {
        let content = value.content.trim().to_owned();
        if content.is_empty() || content.chars().count() > 2_000 {
            return Err(());
        }
        Ok(Self {
            kind: value.kind,
            content,
            due_on: parse_date(value.due_on)?,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct UpdatePlanStepRequest {
    status: PlanStatus,
    waiting_on: Value,
}

impl TryFrom<UpdatePlanStepRequest> for PlanStepUpdate {
    type Error = ();

    fn try_from(value: UpdatePlanStepRequest) -> Result<Self, Self::Error> {
        let waiting_on = match value.waiting_on {
            Value::Null => None,
            Value::String(detail) => Some(detail.trim().to_owned()),
            _ => return Err(()),
        };
        match (value.status, waiting_on) {
            (PlanStatus::Waiting, Some(detail))
                if !detail.is_empty() && detail.chars().count() <= MAX_WAITING_ON_CHARACTERS =>
            {
                Ok(Self {
                    status: PlanStatus::Waiting,
                    waiting_on: Some(detail),
                })
            }
            (PlanStatus::Waiting, _) => Err(()),
            (status, None) => Ok(Self {
                status,
                waiting_on: None,
            }),
            (_, Some(_)) => Err(()),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CaptureResponse {
    inbox_item: InboxItemResponse,
    extraction: ExtractionState,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum ExtractionState {
    Ready,
    Retryable,
    NotSupported,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ListInboxItemsResponse {
    inbox_items: Vec<InboxItemResponse>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GetInboxItemResponse {
    inbox_item: InboxItemDetailResponse,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InboxItemResponse {
    id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    plan_id: Option<Uuid>,
    source_type: CaptureSourceType,
    status: InboxStatus,
    can_retry_extraction: bool,
    created_at: String,
    updated_at: String,
}

impl InboxItemResponse {
    fn from_item(item: &InboxItem, provider: &dyn AiProvider) -> Self {
        Self {
            id: item.id,
            plan_id: item.plan_id,
            source_type: item.source_type,
            status: item.status,
            can_retry_extraction: can_retry_extraction(item, provider),
            created_at: format_timestamp(item.created_at),
            updated_at: format_timestamp(item.updated_at),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InboxItemDetailResponse {
    id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    plan_id: Option<Uuid>,
    source_type: CaptureSourceType,
    status: InboxStatus,
    can_retry_extraction: bool,
    original_text: Option<String>,
    original_filename: Option<String>,
    content_type: Option<String>,
    byte_size: Option<i64>,
    suggestions: Vec<SuggestionResponse>,
    created_at: String,
    updated_at: String,
}

impl InboxItemDetailResponse {
    fn from_item(item: &InboxItemDetail, provider: &dyn AiProvider) -> Self {
        Self {
            id: item.item.id,
            plan_id: item.item.plan_id,
            source_type: item.item.source_type,
            status: item.item.status,
            can_retry_extraction: can_retry_extraction(&item.item, provider),
            original_text: item.original_text.clone(),
            original_filename: item.original_filename.clone(),
            content_type: item.content_type.clone(),
            byte_size: item.byte_size,
            suggestions: item
                .suggestions
                .iter()
                .map(SuggestionResponse::from)
                .collect(),
            created_at: format_timestamp(item.item.created_at),
            updated_at: format_timestamp(item.item.updated_at),
        }
    }
}

fn can_retry_extraction(item: &InboxItem, provider: &dyn AiProvider) -> bool {
    item.status == InboxStatus::Captured
        && match item.source_type {
            CaptureSourceType::Text => true,
            CaptureSourceType::Pdf => provider.supports_pdf_extraction(),
            CaptureSourceType::Image => false,
        }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SuggestionResponse {
    id: Uuid,
    kind: SuggestionKind,
    content: String,
    due_on: Option<String>,
    position: i32,
}

impl From<&Suggestion> for SuggestionResponse {
    fn from(value: &Suggestion) -> Self {
        Self {
            id: value.id,
            kind: value.kind,
            content: value.content.clone(),
            due_on: value.due_on.map(format_date),
            position: value.position,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlanResponse {
    plan: PlanDetailResponse,
}

impl From<&Plan> for PlanResponse {
    fn from(plan: &Plan) -> Self {
        Self {
            plan: PlanDetailResponse::from(plan),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlanDetailResponse {
    id: Uuid,
    inbox_item_id: Uuid,
    summary: String,
    status: PlanStatus,
    steps: Vec<PlanStepResponse>,
    created_at: String,
    updated_at: String,
}

impl From<&Plan> for PlanDetailResponse {
    fn from(plan: &Plan) -> Self {
        Self {
            id: plan.id,
            inbox_item_id: plan.inbox_item_id,
            summary: plan.summary.clone(),
            status: plan.status,
            steps: plan.steps.iter().map(PlanStepResponse::from).collect(),
            created_at: format_timestamp(plan.created_at),
            updated_at: format_timestamp(plan.updated_at),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlanStepResponse {
    id: Uuid,
    position: i32,
    title: String,
    rationale: String,
    status: PlanStatus,
    due_on: Option<String>,
    waiting_on: Option<String>,
    is_next_action: bool,
}

impl From<&PlanStep> for PlanStepResponse {
    fn from(step: &PlanStep) -> Self {
        Self {
            id: step.id,
            position: step.position,
            title: step.title.clone(),
            rationale: step.rationale.clone(),
            status: step.status,
            due_on: step.due_on.map(format_date),
            waiting_on: step.waiting_on.clone(),
            is_next_action: step.is_next_action,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, VecDeque},
        sync::{
            Arc, Mutex,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use async_trait::async_trait;
    use axum::{
        Router,
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use serde_json::Value;
    use sqlx::postgres::PgPoolOptions;
    use time::OffsetDateTime;
    use tower::ServiceExt;
    use uuid::Uuid;

    use super::{AppState, MAX_FILE_BYTES, can_retry_extraction, router};
    use crate::{
        ai::{
            AiApiMode, AiCall, AiError, AiProvider, DisabledAiProvider, Extraction,
            ExtractionInput, OpenAiProvider,
        },
        auth::{AuthenticatedUser, TokenVerifier},
        domain::{
            CaptureSourceType, InboxStatus, PlanStatus, PlanStepState, derived_plan_status,
            highlighted_next_action,
        },
        inbox::{
            FileCapture, InboxItem, InboxItemDetail, InboxRepository, NewPlan, NewPlanStep,
            NewSuggestion, Plan, PlanStep, PlanStepUpdate, Suggestion, SuggestionKind,
            UpdatePlanStepResult,
        },
        storage::PrivateObjectStore,
    };

    struct TestTokenVerifier;
    #[async_trait]
    impl TokenVerifier for TestTokenVerifier {
        async fn verify(&self, token: &str) -> Result<AuthenticatedUser, ()> {
            let (uid, email) = match token {
                "valid-token" => ("user-123", "member@example.com"),
                "other-token" => ("other-user", "other@example.com"),
                _ => return Err(()),
            };
            Ok(AuthenticatedUser {
                uid: uid.to_owned(),
                email: email.to_owned(),
            })
        }
    }

    #[derive(Default)]
    struct TestInboxRepository {
        created: Mutex<Vec<(String, String)>>,
        files: Mutex<Vec<FileCapture>>,
    }
    #[async_trait]
    impl InboxRepository for TestInboxRepository {
        async fn create_text(&self, owner_uid: &str, text: &str) -> anyhow::Result<InboxItem> {
            self.created
                .lock()
                .unwrap()
                .push((owner_uid.to_owned(), text.to_owned()));
            Ok(item(CaptureSourceType::Text))
        }
        async fn create_file(
            &self,
            _owner_uid: &str,
            capture: &FileCapture,
        ) -> anyhow::Result<InboxItem> {
            self.files.lock().unwrap().push(capture.clone());
            Ok(item(capture.source_type))
        }
        async fn list(&self, _owner_uid: &str) -> anyhow::Result<Vec<InboxItem>> {
            Ok(Vec::new())
        }
        async fn get(
            &self,
            _owner_uid: &str,
            _item_id: Uuid,
        ) -> anyhow::Result<Option<InboxItemDetail>> {
            Ok(None)
        }
    }

    #[derive(Default)]
    struct WorkflowInboxRepository {
        items: Mutex<HashMap<Uuid, (String, InboxItemDetail)>>,
        plans: Mutex<HashMap<Uuid, (String, Plan)>>,
    }

    impl WorkflowInboxRepository {
        fn insert(&self, owner_uid: &str, detail: InboxItemDetail) {
            self.items
                .lock()
                .unwrap()
                .insert(detail.item.id, (owner_uid.to_owned(), detail));
        }

        fn detail(&self, item_id: Uuid) -> InboxItemDetail {
            self.items
                .lock()
                .unwrap()
                .get(&item_id)
                .expect("test item should exist")
                .1
                .clone()
        }

        fn insert_plan(&self, owner_uid: &str, plan: Plan) {
            self.plans
                .lock()
                .unwrap()
                .insert(plan.id, (owner_uid.to_owned(), plan));
        }
    }

    #[async_trait]
    impl InboxRepository for WorkflowInboxRepository {
        async fn create_text(&self, owner_uid: &str, text: &str) -> anyhow::Result<InboxItem> {
            let item = InboxItem {
                id: Uuid::new_v4(),
                plan_id: None,
                source_type: CaptureSourceType::Text,
                status: InboxStatus::Captured,
                created_at: OffsetDateTime::UNIX_EPOCH,
                updated_at: OffsetDateTime::UNIX_EPOCH,
            };
            self.insert(
                owner_uid,
                InboxItemDetail {
                    item: item.clone(),
                    original_text: Some(text.to_owned()),
                    original_filename: None,
                    content_type: None,
                    byte_size: None,
                    suggestions: Vec::new(),
                },
            );
            Ok(item)
        }

        async fn create_file(
            &self,
            _owner_uid: &str,
            _capture: &FileCapture,
        ) -> anyhow::Result<InboxItem> {
            anyhow::bail!("not needed by workflow tests")
        }

        async fn list(&self, owner_uid: &str) -> anyhow::Result<Vec<InboxItem>> {
            Ok(self
                .items
                .lock()
                .unwrap()
                .values()
                .filter(|(owner, _)| owner == owner_uid)
                .map(|(_, detail)| detail.item.clone())
                .collect())
        }

        async fn get(
            &self,
            owner_uid: &str,
            item_id: Uuid,
        ) -> anyhow::Result<Option<InboxItemDetail>> {
            Ok(self
                .items
                .lock()
                .unwrap()
                .get(&item_id)
                .filter(|(owner, _)| owner == owner_uid)
                .map(|(_, detail)| detail.clone()))
        }

        async fn get_file(
            &self,
            owner_uid: &str,
            item_id: Uuid,
        ) -> anyhow::Result<Option<crate::inbox::FileReference>> {
            Ok(self
                .items
                .lock()
                .unwrap()
                .get(&item_id)
                .filter(|(owner, detail)| {
                    owner == owner_uid && detail.item.source_type == CaptureSourceType::Pdf
                })
                .map(|_| crate::inbox::FileReference {
                    storage_key: "private-object-key".to_owned(),
                    content_type: "application/pdf".to_owned(),
                }))
        }

        async fn save_extraction(
            &self,
            owner_uid: &str,
            item_id: Uuid,
            suggestions: &[NewSuggestion],
        ) -> anyhow::Result<Option<InboxItemDetail>> {
            let mut items = self.items.lock().unwrap();
            let Some((owner, detail)) = items.get_mut(&item_id) else {
                return Ok(None);
            };
            if owner != owner_uid || detail.item.status != InboxStatus::Captured {
                return Ok(None);
            }
            detail.item.status = InboxStatus::Reviewing;
            detail.suggestions = suggestions
                .iter()
                .enumerate()
                .map(|(position, suggestion)| Suggestion {
                    id: Uuid::new_v4(),
                    kind: suggestion.kind,
                    content: suggestion.content.clone(),
                    due_on: suggestion.due_on,
                    position: position as i32,
                })
                .collect();
            Ok(Some(detail.clone()))
        }

        async fn replace_suggestions(
            &self,
            owner_uid: &str,
            item_id: Uuid,
            suggestions: &[NewSuggestion],
        ) -> anyhow::Result<Option<InboxItemDetail>> {
            let mut items = self.items.lock().unwrap();
            let Some((owner, detail)) = items.get_mut(&item_id) else {
                return Ok(None);
            };
            if owner != owner_uid || detail.item.status != InboxStatus::Reviewing {
                return Ok(None);
            }
            detail.suggestions = suggestions
                .iter()
                .enumerate()
                .map(|(position, suggestion)| Suggestion {
                    id: Uuid::new_v4(),
                    kind: suggestion.kind,
                    content: suggestion.content.clone(),
                    due_on: suggestion.due_on,
                    position: position as i32,
                })
                .collect();
            Ok(Some(detail.clone()))
        }

        async fn create_plan(
            &self,
            owner_uid: &str,
            item_id: Uuid,
            plan: &NewPlan,
        ) -> anyhow::Result<Option<Plan>> {
            let mut items = self.items.lock().unwrap();
            let Some((owner, detail)) = items.get_mut(&item_id) else {
                return Ok(None);
            };
            if owner != owner_uid || detail.item.status != InboxStatus::Reviewing {
                return Ok(None);
            }
            detail.item.status = InboxStatus::Planned;
            let plan = Plan {
                id: Uuid::new_v4(),
                inbox_item_id: item_id,
                summary: plan.summary.clone(),
                status: PlanStatus::Ready,
                created_at: OffsetDateTime::UNIX_EPOCH,
                updated_at: OffsetDateTime::UNIX_EPOCH,
                steps: plan
                    .steps
                    .iter()
                    .enumerate()
                    .map(|(position, step)| PlanStep {
                        id: Uuid::new_v4(),
                        position: position as i32,
                        title: step.title.clone(),
                        rationale: step.rationale.clone(),
                        status: step.status,
                        due_on: step.due_on,
                        waiting_on: step.waiting_on.clone(),
                        is_next_action: position == 0,
                    })
                    .collect(),
            };
            detail.item.plan_id = Some(plan.id);
            self.plans
                .lock()
                .unwrap()
                .insert(plan.id, (owner_uid.to_owned(), plan.clone()));
            Ok(Some(plan))
        }

        async fn get_plan(&self, owner_uid: &str, plan_id: Uuid) -> anyhow::Result<Option<Plan>> {
            Ok(self
                .plans
                .lock()
                .unwrap()
                .get(&plan_id)
                .filter(|(owner, _)| owner == owner_uid)
                .map(|(_, plan)| plan.clone()))
        }

        async fn update_plan_step(
            &self,
            owner_uid: &str,
            plan_id: Uuid,
            step_id: Uuid,
            update: &PlanStepUpdate,
        ) -> anyhow::Result<UpdatePlanStepResult> {
            let mut plans = self.plans.lock().unwrap();
            let Some((owner, plan)) = plans.get_mut(&plan_id) else {
                return Ok(UpdatePlanStepResult::NotFound);
            };
            if owner != owner_uid {
                return Ok(UpdatePlanStepResult::NotFound);
            }
            let Some(step) = plan.steps.iter_mut().find(|step| step.id == step_id) else {
                return Ok(UpdatePlanStepResult::NotFound);
            };
            if !step.status.can_transition_to(update.status) {
                return Ok(UpdatePlanStepResult::InvalidState);
            }
            step.status = update.status;
            step.waiting_on = update.waiting_on.clone();

            let states = plan
                .steps
                .iter()
                .map(|step| PlanStepState {
                    position: u32::try_from(step.position)
                        .expect("test Plan step positions must be non-negative"),
                    status: step.status,
                })
                .collect::<Vec<_>>();
            plan.status =
                derived_plan_status(&states).expect("test Plans must contain at least one step");
            let next_position = highlighted_next_action(&states);
            for step in &mut plan.steps {
                step.is_next_action = u32::try_from(step.position).ok() == next_position;
            }
            plan.updated_at = OffsetDateTime::now_utc();
            Ok(UpdatePlanStepResult::Updated(plan.clone()))
        }
    }

    struct SequenceExtractionProvider {
        calls: AtomicUsize,
        results: Mutex<VecDeque<Result<Extraction, AiError>>>,
    }

    impl SequenceExtractionProvider {
        fn new(results: Vec<Result<Extraction, AiError>>) -> Self {
            Self {
                calls: AtomicUsize::new(0),
                results: Mutex::new(results.into()),
            }
        }
    }

    #[async_trait]
    impl AiProvider for SequenceExtractionProvider {
        async fn extract(&self, _input: ExtractionInput) -> AiCall<Extraction> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            AiCall {
                result: self
                    .results
                    .lock()
                    .unwrap()
                    .pop_front()
                    .unwrap_or(Err(AiError::Unavailable)),
                cleanup_file_id: None,
            }
        }

        async fn plan(&self, _suggestions: &[Suggestion]) -> Result<NewPlan, AiError> {
            Err(AiError::Unavailable)
        }

        async fn delete_file(&self, _file_id: &str) -> Result<(), AiError> {
            Err(AiError::Unavailable)
        }
    }

    #[derive(Default)]
    struct PlanningProvider {
        received: Mutex<Vec<Suggestion>>,
    }

    #[async_trait]
    impl AiProvider for PlanningProvider {
        async fn extract(&self, _input: ExtractionInput) -> AiCall<Extraction> {
            AiCall {
                result: Err(AiError::Unavailable),
                cleanup_file_id: None,
            }
        }

        async fn plan(&self, suggestions: &[Suggestion]) -> Result<NewPlan, AiError> {
            self.received.lock().unwrap().extend_from_slice(suggestions);
            Ok(NewPlan {
                summary: "Renew before the trip.".to_owned(),
                steps: vec![
                    NewPlanStep {
                        title: "Check the official renewal requirements.".to_owned(),
                        rationale: "Confirms the deadline and documents.".to_owned(),
                        status: PlanStatus::Ready,
                        due_on: None,
                        waiting_on: None,
                    },
                    NewPlanStep {
                        title: "Prepare the required documents.".to_owned(),
                        rationale: "Makes the application ready to submit.".to_owned(),
                        status: PlanStatus::Ready,
                        due_on: None,
                        waiting_on: None,
                    },
                ],
            })
        }

        async fn delete_file(&self, _file_id: &str) -> Result<(), AiError> {
            Err(AiError::Unavailable)
        }
    }

    fn item(source_type: CaptureSourceType) -> InboxItem {
        InboxItem {
            id: Uuid::nil(),
            plan_id: None,
            source_type,
            status: InboxStatus::Captured,
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        }
    }

    fn plan_step(id: Uuid, position: i32, status: PlanStatus, is_next_action: bool) -> PlanStep {
        PlanStep {
            id,
            position,
            title: format!("Step {position}"),
            rationale: format!("Why step {position} matters."),
            status,
            due_on: None,
            waiting_on: None,
            is_next_action,
        }
    }

    #[test]
    fn chat_completions_marks_pdf_captures_as_not_retryable() {
        let provider = OpenAiProvider::new(
            "test-key".to_owned(),
            "deepseek-v4-pro".to_owned(),
            "https://api.deepseek.com".to_owned(),
            AiApiMode::ChatCompletions,
        )
        .unwrap();

        assert!(!can_retry_extraction(
            &item(CaptureSourceType::Pdf),
            &provider
        ));
        assert!(can_retry_extraction(
            &item(CaptureSourceType::Text),
            &provider
        ));
    }

    #[derive(Default)]
    struct TestObjectStore {
        uploads: Mutex<usize>,
    }
    #[async_trait]
    impl PrivateObjectStore for TestObjectStore {
        async fn upload(
            &self,
            _key: &str,
            _content_type: &str,
            _content: &[u8],
        ) -> anyhow::Result<()> {
            *self.uploads.lock().unwrap() += 1;
            Ok(())
        }
        async fn download(&self, _key: &str) -> anyhow::Result<Vec<u8>> {
            Ok(b"%PDF-1.7 private preview".to_vec())
        }
        async fn delete(&self, _key: &str) -> anyhow::Result<()> {
            Ok(())
        }
    }

    fn test_app() -> Router {
        test_app_with_ai(Arc::new(DisabledAiProvider))
    }

    fn test_app_with_ai(ai_provider: Arc<dyn AiProvider>) -> Router {
        let database = PgPoolOptions::new()
            .acquire_timeout(Duration::from_millis(10))
            .connect_lazy("postgres://app:app@127.0.0.1:1/app")
            .unwrap();
        router(AppState {
            database,
            inbox_repository: Arc::new(TestInboxRepository::default()),
            token_verifier: Arc::new(TestTokenVerifier),
            object_store: Arc::new(TestObjectStore::default()),
            ai_provider,
        })
    }

    fn workflow_app(
        inbox_repository: Arc<WorkflowInboxRepository>,
        ai_provider: Arc<dyn AiProvider>,
    ) -> Router {
        let database = PgPoolOptions::new()
            .acquire_timeout(Duration::from_millis(10))
            .connect_lazy("postgres://app:app@127.0.0.1:1/app")
            .unwrap();
        router(AppState {
            database,
            inbox_repository,
            token_verifier: Arc::new(TestTokenVerifier),
            object_store: Arc::new(TestObjectStore::default()),
            ai_provider,
        })
    }

    async fn response_json(response: axum::response::Response) -> Value {
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
    }
    fn valid(request: Request<Body>) -> Request<Body> {
        let (mut parts, body) = request.into_parts();
        parts
            .headers
            .insert("authorization", "Bearer valid-token".parse().unwrap());
        Request::from_parts(parts, body)
    }
    fn other_user(request: Request<Body>) -> Request<Body> {
        let (mut parts, body) = request.into_parts();
        parts
            .headers
            .insert("authorization", "Bearer other-token".parse().unwrap());
        Request::from_parts(parts, body)
    }
    fn file_request(content: Vec<u8>) -> Request<Body> {
        let boundary = "test-boundary";
        let mut body = format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"letter.pdf\"\r\nContent-Type: application/pdf\r\n\r\n").into_bytes();
        body.extend(content);
        body.extend(format!("\r\n--{boundary}--\r\n").as_bytes());
        Request::builder()
            .method("POST")
            .uri("/api/v1/inbox-items/files")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(body))
            .unwrap()
    }
    fn image_file_request() -> Request<Body> {
        let boundary = "test-boundary";
        let mut body = format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"photo.png\"\r\nContent-Type: image/png\r\n\r\n").into_bytes();
        body.extend(b"\x89PNG\r\n\x1a\nprivate image");
        body.extend(format!("\r\n--{boundary}--\r\n").as_bytes());
        Request::builder()
            .method("POST")
            .uri("/api/v1/inbox-items/files")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(body))
            .unwrap()
    }

    #[tokio::test]
    async fn text_capture_keeps_a_saved_item_when_automatic_ai_is_unavailable() {
        let response = test_app()
            .oneshot(valid(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/inbox-items")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"text":"Renew passport"}"#))
                    .unwrap(),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(response_json(response).await["extraction"], "retryable");
    }

    #[tokio::test]
    async fn oversized_file_is_rejected_before_private_storage() {
        let mut pdf = b"%PDF-1.7\n".to_vec();
        pdf.resize(MAX_FILE_BYTES + 1, b'a');
        let response = test_app().oneshot(valid(file_request(pdf))).await.unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn image_capture_is_saved_without_an_ai_extraction_attempt() {
        let provider = Arc::new(SequenceExtractionProvider::new(Vec::new()));
        let response = test_app_with_ai(provider.clone())
            .oneshot(valid(image_file_request()))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(response_json(response).await["extraction"], "not_supported");
        assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn unsupported_ai_input_is_saved_without_a_retryable_extraction_state() {
        let repository = Arc::new(WorkflowInboxRepository::default());
        let provider = Arc::new(SequenceExtractionProvider::new(vec![Err(
            AiError::Unsupported,
        )]));
        let response = workflow_app(repository.clone(), provider)
            .oneshot(valid(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/inbox-items")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"text":"Renew passport"}"#))
                    .unwrap(),
            ))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);
        let payload = response_json(response).await;
        assert_eq!(payload["extraction"], "not_supported");
        let item_id = Uuid::parse_str(payload["inboxItem"]["id"].as_str().unwrap()).unwrap();
        assert_eq!(
            repository.detail(item_id).item.status,
            InboxStatus::Captured
        );
    }

    #[tokio::test]
    async fn automatic_extraction_retries_one_transient_failure_and_enters_review() {
        let repository = Arc::new(WorkflowInboxRepository::default());
        let provider = Arc::new(SequenceExtractionProvider::new(vec![
            Err(AiError::Transient),
            Ok(Extraction {
                suggestions: vec![NewSuggestion {
                    kind: SuggestionKind::Task,
                    content: "Check the official passport renewal requirements.".to_owned(),
                    due_on: None,
                }],
            }),
        ]));
        let response = workflow_app(repository.clone(), provider.clone())
            .oneshot(valid(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/inbox-items")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"text":"Renew passport before my trip"}"#))
                    .unwrap(),
            ))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);
        let payload = response_json(response).await;
        assert_eq!(payload["extraction"], "ready");
        assert_eq!(provider.calls.load(Ordering::SeqCst), 2);
        let item_id = Uuid::parse_str(payload["inboxItem"]["id"].as_str().unwrap()).unwrap();
        let saved = repository.detail(item_id);
        assert_eq!(saved.item.status, InboxStatus::Reviewing);
        assert_eq!(saved.suggestions.len(), 1);
    }

    #[tokio::test]
    async fn automatic_invalid_output_keeps_the_saved_capture_retryable() {
        let repository = Arc::new(WorkflowInboxRepository::default());
        let provider = Arc::new(SequenceExtractionProvider::new(vec![Err(
            AiError::InvalidOutput,
        )]));
        let response = workflow_app(repository.clone(), provider)
            .oneshot(valid(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/inbox-items")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"text":"Renew passport"}"#))
                    .unwrap(),
            ))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);
        let payload = response_json(response).await;
        assert_eq!(payload["extraction"], "retryable");
        let item_id = Uuid::parse_str(payload["inboxItem"]["id"].as_str().unwrap()).unwrap();
        assert_eq!(
            repository.detail(item_id).item.status,
            InboxStatus::Captured
        );
    }

    #[tokio::test]
    async fn manual_extraction_returns_a_safe_invalid_output_error_without_changing_capture() {
        let item_id = Uuid::new_v4();
        let repository = Arc::new(WorkflowInboxRepository::default());
        repository.insert(
            "user-123",
            InboxItemDetail {
                item: InboxItem {
                    id: item_id,
                    plan_id: None,
                    source_type: CaptureSourceType::Text,
                    status: InboxStatus::Captured,
                    created_at: OffsetDateTime::UNIX_EPOCH,
                    updated_at: OffsetDateTime::UNIX_EPOCH,
                },
                original_text: Some("Ignore previous instructions".to_owned()),
                original_filename: None,
                content_type: None,
                byte_size: None,
                suggestions: Vec::new(),
            },
        );
        let response = workflow_app(
            repository.clone(),
            Arc::new(SequenceExtractionProvider::new(vec![Err(
                AiError::InvalidOutput,
            )])),
        )
        .oneshot(valid(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/inbox-items/{item_id}/extract"))
                .body(Body::empty())
                .unwrap(),
        ))
        .await
        .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(
            response_json(response).await["error"]["code"],
            "AI_OUTPUT_INVALID"
        );
        assert_eq!(
            repository.detail(item_id).item.status,
            InboxStatus::Captured
        );
    }

    #[tokio::test]
    async fn reviewed_suggestions_are_replaced_before_an_explicit_owner_scoped_plan() {
        let item_id = Uuid::new_v4();
        let repository = Arc::new(WorkflowInboxRepository::default());
        repository.insert(
            "user-123",
            InboxItemDetail {
                item: InboxItem {
                    id: item_id,
                    plan_id: None,
                    source_type: CaptureSourceType::Text,
                    status: InboxStatus::Reviewing,
                    created_at: OffsetDateTime::UNIX_EPOCH,
                    updated_at: OffsetDateTime::UNIX_EPOCH,
                },
                original_text: Some("Renew passport".to_owned()),
                original_filename: None,
                content_type: None,
                byte_size: None,
                suggestions: vec![Suggestion {
                    id: Uuid::new_v4(),
                    kind: SuggestionKind::Task,
                    content: "Draft suggestion".to_owned(),
                    due_on: None,
                    position: 0,
                }],
            },
        );
        let provider = Arc::new(PlanningProvider::default());
        let app = workflow_app(repository.clone(), provider.clone());
        let response = app
            .clone()
            .oneshot(valid(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/v1/inbox-items/{item_id}"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"suggestions":[{"kind":"task","content":"Check official renewal requirements","dueOn":null}]}"#,
                    ))
                    .unwrap(),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response_json(response).await["inboxItem"]["suggestions"][0]["content"],
            "Check official renewal requirements"
        );

        let plan_response = app
            .clone()
            .oneshot(valid(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/inbox-items/{item_id}/plans"))
                    .body(Body::empty())
                    .unwrap(),
            ))
            .await
            .unwrap();
        assert_eq!(plan_response.status(), StatusCode::CREATED);
        let plan = response_json(plan_response).await;
        assert_eq!(plan["plan"]["steps"].as_array().unwrap().len(), 2);
        assert_eq!(plan["plan"]["steps"][0]["isNextAction"], true);
        assert_eq!(
            provider.received.lock().unwrap()[0].content,
            "Check official renewal requirements"
        );
        assert_eq!(repository.detail(item_id).item.status, InboxStatus::Planned);

        let plan_id = plan["plan"]["id"].as_str().unwrap();
        let planned_item_response = app
            .oneshot(valid(
                Request::builder()
                    .uri(format!("/api/v1/inbox-items/{item_id}"))
                    .body(Body::empty())
                    .unwrap(),
            ))
            .await
            .unwrap();
        assert_eq!(
            response_json(planned_item_response).await["inboxItem"]["planId"],
            plan_id
        );
        let foreign_response = workflow_app(repository, provider)
            .oneshot(other_user(
                Request::builder()
                    .uri(format!("/api/v1/plans/{plan_id}"))
                    .body(Body::empty())
                    .unwrap(),
            ))
            .await
            .unwrap();
        assert_eq!(foreign_response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn plan_step_updates_recompute_the_next_action_and_plan_status() {
        let plan_id = Uuid::new_v4();
        let first_step_id = Uuid::new_v4();
        let second_step_id = Uuid::new_v4();
        let repository = Arc::new(WorkflowInboxRepository::default());
        repository.insert_plan(
            "user-123",
            Plan {
                id: plan_id,
                inbox_item_id: Uuid::new_v4(),
                summary: "Renew before travelling.".to_owned(),
                status: PlanStatus::Ready,
                created_at: OffsetDateTime::UNIX_EPOCH,
                updated_at: OffsetDateTime::UNIX_EPOCH,
                steps: vec![
                    plan_step(first_step_id, 0, PlanStatus::Ready, true),
                    plan_step(second_step_id, 1, PlanStatus::Ready, false),
                ],
            },
        );
        let app = workflow_app(repository, Arc::new(DisabledAiProvider));

        let completed = app
            .clone()
            .oneshot(valid(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/v1/plans/{plan_id}/steps/{first_step_id}"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"status":"complete","waitingOn":null}"#))
                    .unwrap(),
            ))
            .await
            .unwrap();
        assert_eq!(completed.status(), StatusCode::OK);
        let completed = response_json(completed).await;
        assert_eq!(completed["plan"]["status"], "ready");
        assert_eq!(completed["plan"]["steps"][0]["status"], "complete");
        assert_eq!(completed["plan"]["steps"][0]["isNextAction"], false);
        assert_eq!(completed["plan"]["steps"][1]["isNextAction"], true);

        let waiting = app
            .clone()
            .oneshot(valid(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/v1/plans/{plan_id}/steps/{second_step_id}"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"status":"waiting","waitingOn":"  a reply from the agency  "}"#,
                    ))
                    .unwrap(),
            ))
            .await
            .unwrap();
        assert_eq!(waiting.status(), StatusCode::OK);
        let waiting = response_json(waiting).await;
        assert_eq!(waiting["plan"]["status"], "waiting");
        assert_eq!(
            waiting["plan"]["steps"][1]["waitingOn"],
            "a reply from the agency"
        );
        assert!(
            waiting["plan"]["steps"]
                .as_array()
                .unwrap()
                .iter()
                .all(|step| step["isNextAction"] == false)
        );

        let ready_again = app
            .clone()
            .oneshot(valid(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/v1/plans/{plan_id}/steps/{second_step_id}"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"status":"ready","waitingOn":null}"#))
                    .unwrap(),
            ))
            .await
            .unwrap();
        assert_eq!(ready_again.status(), StatusCode::OK);
        let ready_again = response_json(ready_again).await;
        assert_eq!(ready_again["plan"]["status"], "ready");
        assert_eq!(ready_again["plan"]["steps"][1]["waitingOn"], Value::Null);
        assert_eq!(ready_again["plan"]["steps"][1]["isNextAction"], true);

        let all_complete = app
            .clone()
            .oneshot(valid(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/v1/plans/{plan_id}/steps/{second_step_id}"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"status":"complete","waitingOn":null}"#))
                    .unwrap(),
            ))
            .await
            .unwrap();
        assert_eq!(all_complete.status(), StatusCode::OK);
        let all_complete = response_json(all_complete).await;
        assert_eq!(all_complete["plan"]["status"], "complete");
        assert!(
            all_complete["plan"]["steps"]
                .as_array()
                .unwrap()
                .iter()
                .all(|step| step["status"] == "complete" && step["isNextAction"] == false)
        );

        let reopen = app
            .oneshot(valid(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/v1/plans/{plan_id}/steps/{second_step_id}"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"status":"ready","waitingOn":null}"#))
                    .unwrap(),
            ))
            .await
            .unwrap();
        assert_eq!(reopen.status(), StatusCode::CONFLICT);
        assert_eq!(
            response_json(reopen).await["error"]["code"],
            "INVALID_STATE"
        );
    }

    #[tokio::test]
    async fn plan_step_updates_validate_input_and_hide_foreign_or_mismatched_steps() {
        let plan_id = Uuid::new_v4();
        let step_id = Uuid::new_v4();
        let repository = Arc::new(WorkflowInboxRepository::default());
        repository.insert_plan(
            "user-123",
            Plan {
                id: plan_id,
                inbox_item_id: Uuid::new_v4(),
                summary: "Renew before travelling.".to_owned(),
                status: PlanStatus::Ready,
                created_at: OffsetDateTime::UNIX_EPOCH,
                updated_at: OffsetDateTime::UNIX_EPOCH,
                steps: vec![plan_step(step_id, 0, PlanStatus::Ready, true)],
            },
        );
        let app = workflow_app(repository, Arc::new(DisabledAiProvider));

        let invalid_waiting = app
            .clone()
            .oneshot(valid(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/v1/plans/{plan_id}/steps/{step_id}"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"status":"waiting","waitingOn":"  "}"#))
                    .unwrap(),
            ))
            .await
            .unwrap();
        assert_eq!(invalid_waiting.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response_json(invalid_waiting).await["error"]["code"],
            "VALIDATION_ERROR"
        );

        let missing_waiting_detail = app
            .clone()
            .oneshot(valid(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/v1/plans/{plan_id}/steps/{step_id}"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"status":"complete"}"#))
                    .unwrap(),
            ))
            .await
            .unwrap();
        assert_eq!(missing_waiting_detail.status(), StatusCode::BAD_REQUEST);

        let unauthenticated = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/v1/plans/{plan_id}/steps/{step_id}"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"status":"complete","waitingOn":null}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

        let foreign = app
            .clone()
            .oneshot(other_user(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/v1/plans/{plan_id}/steps/{step_id}"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"status":"complete","waitingOn":null}"#))
                    .unwrap(),
            ))
            .await
            .unwrap();
        assert_eq!(foreign.status(), StatusCode::NOT_FOUND);

        let missing_step = app
            .clone()
            .oneshot(valid(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/v1/plans/{plan_id}/steps/{}", Uuid::new_v4()))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"status":"complete","waitingOn":null}"#))
                    .unwrap(),
            ))
            .await
            .unwrap();
        assert_eq!(missing_step.status(), StatusCode::NOT_FOUND);

        let malformed_step = app
            .oneshot(valid(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/v1/plans/{plan_id}/steps/not-a-uuid"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"status":"complete","waitingOn":null}"#))
                    .unwrap(),
            ))
            .await
            .unwrap();
        assert_eq!(malformed_step.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn pdf_preview_streams_only_to_its_owner_without_a_storage_identifier() {
        let item_id = Uuid::new_v4();
        let repository = Arc::new(WorkflowInboxRepository::default());
        repository.insert(
            "user-123",
            InboxItemDetail {
                item: InboxItem {
                    id: item_id,
                    plan_id: None,
                    source_type: CaptureSourceType::Pdf,
                    status: InboxStatus::Reviewing,
                    created_at: OffsetDateTime::UNIX_EPOCH,
                    updated_at: OffsetDateTime::UNIX_EPOCH,
                },
                original_text: None,
                original_filename: Some("letter.pdf".to_owned()),
                content_type: Some("application/pdf".to_owned()),
                byte_size: Some(24),
                suggestions: Vec::new(),
            },
        );
        let app = workflow_app(repository, Arc::new(DisabledAiProvider));
        let response = app
            .clone()
            .oneshot(valid(
                Request::builder()
                    .uri(format!("/api/v1/inbox-items/{item_id}/file"))
                    .body(Body::empty())
                    .unwrap(),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["content-type"], "application/pdf");
        assert_eq!(response.headers()["cache-control"], "private, no-store");
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(body.as_ref(), b"%PDF-1.7 private preview");

        let foreign_response = app
            .oneshot(other_user(
                Request::builder()
                    .uri(format!("/api/v1/inbox-items/{item_id}/file"))
                    .body(Body::empty())
                    .unwrap(),
            ))
            .await
            .unwrap();
        assert_eq!(foreign_response.status(), StatusCode::NOT_FOUND);
    }
}
