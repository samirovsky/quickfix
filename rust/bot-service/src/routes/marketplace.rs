use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::models::marketplace::{
    Listing, PublishRequest, SubscribeRequest, Subscription, SubscriptionStatus,
};
use crate::models::performance::PerformanceMetrics;
use crate::state::AppState;
use crate::store::{bot_repo, listing_repo, performance_repo, subscription_repo};

/// `POST /v1/bots/:id/publish` — owner only.
pub async fn publish(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(bot_id): Path<String>,
    Json(req): Json<PublishRequest>,
) -> ApiResult<(StatusCode, Json<Listing>)> {
    // Verify the caller owns the bot.
    bot_repo::get(&state.db, &user.id, &bot_id).await?;
    let listing = listing_repo::publish(
        &state.db,
        &bot_id,
        &user.id,
        req.title.trim(),
        &req.summary,
        req.monthly_price_cents,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(listing)))
}

/// `POST /v1/marketplace/listings/:id/unpublish` — creator only.
pub async fn unpublish(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(listing_id): Path<String>,
) -> ApiResult<StatusCode> {
    listing_repo::unpublish(&state.db, &listing_id, &user.id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /v1/marketplace/listings`. Public to any authenticated user.
pub async fn list_listings(State(state): State<AppState>) -> ApiResult<Json<Vec<Listing>>> {
    let listings = listing_repo::list_published(&state.db).await?;
    Ok(Json(listings))
}

/// `GET /v1/marketplace/listings/:id`.
pub async fn get_listing(
    State(state): State<AppState>,
    Path(listing_id): Path<String>,
) -> ApiResult<Json<Listing>> {
    let listing = listing_repo::get(&state.db, &listing_id).await?;
    Ok(Json(listing))
}

#[derive(Debug, Deserialize)]
pub struct PerformanceQuery {
    #[serde(default = "default_window_days")]
    pub days: u32,
}
fn default_window_days() -> u32 {
    30
}

/// `GET /v1/marketplace/listings/:id/performance?days=30`.
///
/// Public for any authenticated user — the whole point of the marketplace
/// is to make track records visible before subscribing.
pub async fn listing_performance(
    State(state): State<AppState>,
    Path(listing_id): Path<String>,
    Query(q): Query<PerformanceQuery>,
) -> ApiResult<Json<PerformanceMetrics>> {
    let bot_id = listing_repo::bot_id_for(&state.db, &listing_id).await?;
    let metrics =
        performance_repo::metrics_for_bot(&state.db, &bot_id, q.days.clamp(1, 365)).await?;
    Ok(Json(metrics))
}

/// `GET /v1/bots/:id/performance?days=30` — owner only.
pub async fn bot_performance(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(bot_id): Path<String>,
    Query(q): Query<PerformanceQuery>,
) -> ApiResult<Json<PerformanceMetrics>> {
    bot_repo::get(&state.db, &user.id, &bot_id).await?;
    let metrics =
        performance_repo::metrics_for_bot(&state.db, &bot_id, q.days.clamp(1, 365)).await?;
    Ok(Json(metrics))
}

/// `POST /v1/marketplace/listings/:id/subscribe`.
pub async fn subscribe(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(listing_id): Path<String>,
    Json(req): Json<SubscribeRequest>,
) -> ApiResult<(StatusCode, Json<Subscription>)> {
    // Reject subscribing to your own bot — it doesn't make sense and
    // the UI shouldn't let you.
    let listing = listing_repo::get(&state.db, &listing_id).await?;
    if listing.creator_id == user.id {
        return Err(ApiError::BadRequest(
            "cannot subscribe to your own bot".into(),
        ));
    }
    let sub = subscription_repo::subscribe(
        &state.db,
        &user.id,
        &listing_id,
        req.allocated_capital_cents,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(sub)))
}

/// `POST /v1/subscriptions/:id/cancel`.
pub async fn cancel_subscription(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(sub_id): Path<String>,
) -> ApiResult<StatusCode> {
    subscription_repo::cancel(&state.db, &user.id, &sub_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /v1/me/subscriptions` (and an `?include=listing` flavour).
#[derive(serde::Serialize)]
pub struct SubscriptionWithListing {
    #[serde(flatten)]
    pub subscription: Subscription,
    pub listing: Listing,
}

pub async fn my_subscriptions(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
) -> ApiResult<Json<Vec<SubscriptionWithListing>>> {
    let subs = subscription_repo::list_for_subscriber(&state.db, &user.id).await?;
    let mut out = Vec::with_capacity(subs.len());
    for sub in subs {
        // Skip dangling references just in case a listing was hard-deleted.
        let listing = match listing_repo::get(&state.db, &sub.listing_id).await {
            Ok(l) => l,
            Err(ApiError::NotFound) => continue,
            Err(e) => return Err(e),
        };
        // Only return active subscriptions in the default response — cancelled
        // ones live on for history but the mobile list focuses on what the
        // user is paying for.
        if sub.status == SubscriptionStatus::Active {
            out.push(SubscriptionWithListing {
                subscription: sub,
                listing,
            });
        }
    }
    Ok(Json(out))
}
