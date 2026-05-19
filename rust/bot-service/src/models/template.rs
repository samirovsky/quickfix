//! Bot templates. Loaded from `templates/library.json` at startup, served
//! read-only.

use serde::{Deserialize, Serialize};

use crate::models::strategy::{AssetFilter, Strategy};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Template {
    pub id: String,
    pub name: String,
    pub category: TemplateCategory,
    pub description: String,
    pub risk_level: u8,
    pub asset_filter: AssetFilter,
    pub strategy: Strategy,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub enum TemplateCategory {
    TrendFollowing,
    MeanReversion,
    Breakout,
    Grid,
    DCA,
    AiGenerated,
}

#[derive(Debug, Serialize)]
pub struct TemplateSummary {
    pub id: String,
    pub name: String,
    pub category: TemplateCategory,
    pub description: String,
    pub risk_level: u8,
}

impl From<&Template> for TemplateSummary {
    fn from(t: &Template) -> Self {
        Self {
            id: t.id.clone(),
            name: t.name.clone(),
            category: t.category,
            description: t.description.clone(),
            risk_level: t.risk_level,
        }
    }
}
