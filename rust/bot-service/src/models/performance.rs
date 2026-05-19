//! Performance metrics derived from `bot_trades`. Same shape returned
//! whether the caller is looking at their own bot or at a marketplace
//! listing.

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct PerformanceMetrics {
    pub window_days: u32,
    pub total_trades: i64,
    pub total_realised_pnl_cents: i64,
    pub win_rate: f64, // 0.0..=1.0; 0 when there are no trades
    pub largest_win_cents: i64,
    pub largest_loss_cents: i64,
    /// Newest day first. One entry per day **that had at least one trade**.
    /// Days with zero trades are omitted to keep the payload small; the UI
    /// fills gaps when rendering a continuous chart.
    pub daily: Vec<DailyMetric>,
}

#[derive(Debug, Serialize)]
pub struct DailyMetric {
    pub date: String, // YYYY-MM-DD UTC
    pub realised_pnl_cents: i64,
    pub trades: i64,
}
