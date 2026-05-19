use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::models::bot_config::{
    BotConfig, BotConfigSummary, BotStatus, CreateBotRequest, CreateFromTemplateRequest,
    UpdateBotRequest, UpdateStatusRequest,
};
use crate::state::AppState;
use crate::store::bot_repo;

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    /// Filter by exact status: `draft|paper|live|paused|stopped`.
    /// Omitted means "all of the caller's bots".
    pub status: Option<String>,
}

pub async fn create(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Json(req): Json<CreateBotRequest>,
) -> ApiResult<(StatusCode, Json<BotConfig>)> {
    if req.name.trim().is_empty() {
        return Err(ApiError::BadRequest("name must not be empty".into()));
    }
    let cfg = bot_repo::create(
        &state.db,
        &user.id,
        req.name.trim(),
        &req.description,
        &req.strategy,
        &req.asset_filter,
        &req.source,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(cfg)))
}

pub async fn create_from_template(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(template_id): Path<String>,
    Json(req): Json<CreateFromTemplateRequest>,
) -> ApiResult<(StatusCode, Json<BotConfig>)> {
    let tpl = state
        .templates
        .iter()
        .find(|t| t.id == template_id)
        .ok_or(ApiError::NotFound)?;
    let name = req.name.trim();
    if name.is_empty() {
        return Err(ApiError::BadRequest("name must not be empty".into()));
    }
    let desc = req.description.unwrap_or_else(|| tpl.description.clone());
    let source = format!("template:{}", tpl.id);
    let cfg = bot_repo::create(
        &state.db,
        &user.id,
        name,
        &desc,
        &tpl.strategy,
        &tpl.asset_filter,
        &source,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(cfg)))
}

pub async fn list(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Query(q): Query<ListQuery>,
) -> ApiResult<Json<Vec<BotConfigSummary>>> {
    let filter = match q.status.as_deref() {
        None => None,
        Some(s) => Some(
            BotStatus::parse(s)
                .ok_or_else(|| ApiError::BadRequest(format!("unknown status '{s}'")))?,
        ),
    };
    let configs = bot_repo::list(&state.db, &user.id, filter).await?;
    Ok(Json(configs.iter().map(BotConfigSummary::from).collect()))
}

pub async fn get(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<String>,
) -> ApiResult<Json<BotConfig>> {
    let cfg = bot_repo::get(&state.db, &user.id, &id).await?;
    Ok(Json(cfg))
}

pub async fn update(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<String>,
    Json(req): Json<UpdateBotRequest>,
) -> ApiResult<Json<BotConfig>> {
    let cfg = bot_repo::update(
        &state.db,
        &user.id,
        &id,
        req.name.as_deref(),
        req.description.as_deref(),
        req.strategy.as_ref(),
        req.asset_filter.as_ref(),
    )
    .await?;
    Ok(Json(cfg))
}

pub async fn delete(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    bot_repo::delete(&state.db, &user.id, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn set_status(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<String>,
    Json(req): Json<UpdateStatusRequest>,
) -> ApiResult<Json<BotConfig>> {
    let cfg = bot_repo::set_status(&state.db, &user.id, &id, req.status).await?;
    Ok(Json(cfg))
}
