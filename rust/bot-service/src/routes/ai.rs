//! AI strategy generator — STUB.
//!
//! The wire shape matches what a real LLM-backed impl would return:
//! the caller POSTs free-text, the service returns a typed
//! `Strategy` that the same `bot-service` would accept on
//! `POST /v1/bots`. Today's logic is a keyword match against the
//! bundled template library; swapping in an Anthropic SDK call is a
//! one-function change inside `generate_strategy_stub`.

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::error::{ApiError, ApiResult};
use crate::models::strategy::{AssetFilter, Strategy};
use crate::models::template::Template;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct GenerateRequest {
    pub prompt: String,
}

#[derive(Debug, Serialize)]
pub struct GenerateResponse {
    pub strategy: Strategy,
    pub asset_filter: AssetFilter,
    /// Which bundled template the stub matched on. Real LLM impls will
    /// drop this or use it for "based on" attribution.
    pub source_template_id: String,
    /// `true` while we're still serving the stub; flips to `false`
    /// once a real LLM is wired in. Clients can show a "preview"
    /// banner accordingly.
    pub stub: bool,
}

pub async fn generate(
    State(state): State<AppState>,
    Json(req): Json<GenerateRequest>,
) -> ApiResult<Json<GenerateResponse>> {
    let prompt = req.prompt.trim();
    if prompt.is_empty() {
        return Err(ApiError::BadRequest("prompt must not be empty".into()));
    }
    let matched = match_template(&state.templates, prompt);
    Ok(Json(GenerateResponse {
        strategy: matched.strategy.clone(),
        asset_filter: matched.asset_filter.clone(),
        source_template_id: matched.id.clone(),
        stub: true,
    }))
}

/// Stub matcher: lowercase the prompt, look for unambiguous keywords,
/// fall back to the first template. The set of keywords mirrors the
/// bundled template categories so the response stays understandable.
fn match_template<'a>(templates: &'a [Template], prompt: &str) -> &'a Template {
    let p = prompt.to_lowercase();
    let preferred_id = if p.contains("rsi") || p.contains("oversold") || p.contains("mean revers") {
        "tpl_rsi_oversold"
    } else if p.contains("ema")
        || p.contains("crossover")
        || p.contains("cross over")
        || p.contains("golden cross")
    {
        "tpl_ema_cross"
    } else if p.contains("breakout") || p.contains("donchian") || p.contains("high of the day") {
        "tpl_breakout_box"
    } else if p.contains("grid") || p.contains("range") {
        "tpl_grid"
    } else if p.contains("dca") || p.contains("dollar cost") || p.contains("accumulate") {
        "tpl_dca_weekly"
    } else {
        "tpl_rsi_oversold"
    };
    templates
        .iter()
        .find(|t| t.id == preferred_id)
        .or_else(|| templates.first())
        .expect("template library is empty — should be impossible")
}
