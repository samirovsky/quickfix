use axum::extract::{Extension, State};
use axum::Json;

use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::models::billing::{self, EstimateRequest, EstimateResponse, UsageResponse};
use crate::state::AppState;
use crate::store::billing_repo;

pub async fn usage(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
) -> ApiResult<Json<UsageResponse>> {
    let ytd = billing_repo::fees_year_to_date(&state.db, &user.id).await?;
    let this_month = billing_repo::fees_this_month(&state.db, &user.id).await?;
    Ok(Json(UsageResponse {
        subscription_status: "free",
        transaction_fees_ytd_cents: ytd,
        transaction_fees_this_month_cents: this_month,
    }))
}

pub async fn estimate(Json(req): Json<EstimateRequest>) -> Json<EstimateResponse> {
    Json(billing::estimate(&req))
}
