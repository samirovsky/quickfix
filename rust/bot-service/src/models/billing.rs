//! Billing DTOs. Implementation is stubbed in v1 (per docs/bots/roadmap.md
//! posture); the contract is real so a future Stripe integration is a
//! drop-in replacement.

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct EstimateRequest {
    pub trades_per_month: u32,
    pub asset_class: AssetClass,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AssetClass {
    Stocks,
    Crypto,
    Fx,
}

#[derive(Debug, Serialize)]
pub struct EstimateResponse {
    pub monthly_cents: i64,
    pub breakdown: Vec<FeeLine>,
}

#[derive(Debug, Serialize)]
pub struct FeeLine {
    pub label: String,
    pub amount_cents: i64,
}

#[derive(Debug, Serialize)]
pub struct UsageResponse {
    pub subscription_status: &'static str,
    pub transaction_fees_ytd_cents: i64,
    pub transaction_fees_this_month_cents: i64,
}

/// Spec §3.5.1 rates: $0.01/trade for stocks, 0.05% notional for crypto,
/// and a placeholder for FX (treated like crypto for v1). Stubbed; no
/// money actually moves.
pub fn estimate(req: &EstimateRequest) -> EstimateResponse {
    let trades = req.trades_per_month as i64;
    let per_trade_cents: i64 = match req.asset_class {
        AssetClass::Stocks => 1,
        AssetClass::Crypto | AssetClass::Fx => 5,
    };
    let total = trades * per_trade_cents;
    let label = match req.asset_class {
        AssetClass::Stocks => "Per-trade fee ($0.01)",
        AssetClass::Crypto => "Per-trade fee (5¢ approx, real rate 0.05% notional)",
        AssetClass::Fx => "Per-trade fee (5¢ placeholder)",
    };
    EstimateResponse {
        monthly_cents: total,
        breakdown: vec![FeeLine {
            label: label.into(),
            amount_cents: total,
        }],
    }
}
