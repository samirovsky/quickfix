use axum::extract::{Path, State};
use axum::Json;

use crate::error::{ApiError, ApiResult};
use crate::models::template::{Template, TemplateSummary};
use crate::state::AppState;

pub async fn list(State(state): State<AppState>) -> Json<Vec<TemplateSummary>> {
    Json(state.templates.iter().map(TemplateSummary::from).collect())
}

pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Template>> {
    state
        .templates
        .iter()
        .find(|t| t.id == id)
        .cloned()
        .map(Json)
        .ok_or(ApiError::NotFound)
}
