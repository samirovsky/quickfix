//! Bot-config DTOs.
//!
//! Database rows are exchanged with the SQL layer as strings for the JSON
//! columns; the API layer (de)serialises through the typed shapes here so
//! the wire contract stays stable while the storage stays simple.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::models::strategy::{AssetFilter, Strategy};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BotStatus {
    Draft,
    Paper,
    Live,
    Paused,
    Stopped,
}

impl BotStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            BotStatus::Draft => "draft",
            BotStatus::Paper => "paper",
            BotStatus::Live => "live",
            BotStatus::Paused => "paused",
            BotStatus::Stopped => "stopped",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "draft" => BotStatus::Draft,
            "paper" => BotStatus::Paper,
            "live" => BotStatus::Live,
            "paused" => BotStatus::Paused,
            "stopped" => BotStatus::Stopped,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct BotConfig {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub description: String,
    pub status: BotStatus,
    pub strategy: Strategy,
    pub asset_filter: AssetFilter,
    pub source: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BotConfigSummary {
    pub id: String,
    pub name: String,
    pub status: BotStatus,
    pub source: String,
    pub updated_at: DateTime<Utc>,
}

impl From<&BotConfig> for BotConfigSummary {
    fn from(c: &BotConfig) -> Self {
        Self {
            id: c.id.clone(),
            name: c.name.clone(),
            status: c.status.clone(),
            source: c.source.clone(),
            updated_at: c.updated_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateBotRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub strategy: Strategy,
    pub asset_filter: AssetFilter,
    #[serde(default = "default_source")]
    pub source: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateFromTemplateRequest {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateBotRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub strategy: Option<Strategy>,
    pub asset_filter: Option<AssetFilter>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateStatusRequest {
    pub status: BotStatus,
}

fn default_source() -> String {
    "manual".into()
}
