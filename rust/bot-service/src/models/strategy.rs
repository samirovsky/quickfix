//! Strongly-typed strategy schema.
//!
//! Whatever passes serde here is what the rest of the platform agrees a
//! "strategy" is. Slice 4's execution engine deserialises the same types
//! out of `bot_configs.strategy_json`, so adding a new condition kind is a
//! one-place change.
//!
//! Anything not matching the schema fails at write time with a 400, before
//! it hits the database.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Strategy {
    pub version: u32,
    pub entry: ConditionGroup,
    pub exit: ConditionGroup,
    pub position_sizing: PositionSizing,
    pub risk: RiskConfig,
    pub schedule: Schedule,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ConditionGroup {
    AllOf(Vec<Condition>),
    AnyOf(Vec<Condition>),
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Condition {
    IndicatorThreshold {
        indicator: String,
        params: serde_json::Value,
        op: ComparisonOp,
        value: f64,
    },
    TakeProfitPct {
        value: f64,
    },
    StopLossPct {
        value: f64,
    },
    TrailingStopPct {
        value: f64,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonOp {
    Lt,
    Lte,
    Gt,
    Gte,
    Eq,
    Ne,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PositionSizing {
    FixedAmount {
        value_cents: u64,
    },
    PercentPortfolio {
        value: f64,
        max_notional_cents: u64,
    },
    Kelly {
        factor: f64,
        max_notional_cents: u64,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct RiskConfig {
    pub max_concurrent: u32,
    pub max_daily_loss_cents: u64,
    pub halt_after_n_losses: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Schedule {
    /// Pairs of `[start_hour, end_hour)` in UTC. End is exclusive so
    /// `[[0, 24]]` is the whole day.
    pub active_hours_utc: Vec<[u8; 2]>,
    pub active_days: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct AssetFilter {
    /// Dense `SymbolId`s, matching the matching engine's symbol space.
    pub symbols: Vec<u32>,
    pub market: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_minimal() {
        let s = Strategy {
            version: 1,
            entry: ConditionGroup::AllOf(vec![Condition::IndicatorThreshold {
                indicator: "rsi".into(),
                params: serde_json::json!({ "period": 14 }),
                op: ComparisonOp::Lt,
                value: 30.0,
            }]),
            exit: ConditionGroup::AnyOf(vec![Condition::TakeProfitPct { value: 2.0 }]),
            position_sizing: PositionSizing::PercentPortfolio {
                value: 5.0,
                max_notional_cents: 100_000,
            },
            risk: RiskConfig {
                max_concurrent: 3,
                max_daily_loss_cents: 5_000,
                halt_after_n_losses: 4,
            },
            schedule: Schedule {
                active_hours_utc: vec![[13, 21]],
                active_days: vec!["Mon".into()],
            },
        };
        let json = serde_json::to_string(&s).unwrap();
        let parsed: Strategy = serde_json::from_str(&json).unwrap();
        assert_eq!(s, parsed);
    }

    #[test]
    fn rejects_unknown_condition_type() {
        let bad = serde_json::json!({
            "version": 1,
            "entry": { "all_of": [ { "type": "nope" } ] },
            "exit":  { "any_of": [] },
            "position_sizing": { "kind": "fixed_amount", "value_cents": 100 },
            "risk":   { "max_concurrent": 1, "max_daily_loss_cents": 0, "halt_after_n_losses": 1 },
            "schedule": { "active_hours_utc": [], "active_days": [] }
        });
        assert!(serde_json::from_value::<Strategy>(bad).is_err());
    }
}
