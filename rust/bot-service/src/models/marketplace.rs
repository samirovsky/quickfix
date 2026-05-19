//! Marketplace + subscription DTOs.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ListingStatus {
    Published,
    Unpublished,
}

impl ListingStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ListingStatus::Published => "published",
            ListingStatus::Unpublished => "unpublished",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "published" => ListingStatus::Published,
            "unpublished" => ListingStatus::Unpublished,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionStatus {
    Active,
    Cancelled,
}

impl SubscriptionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            SubscriptionStatus::Active => "active",
            SubscriptionStatus::Cancelled => "cancelled",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "active" => SubscriptionStatus::Active,
            "cancelled" => SubscriptionStatus::Cancelled,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Listing {
    pub id: String,
    pub bot_config_id: String,
    pub creator_id: String,
    pub creator_name: String,
    pub title: String,
    pub summary: String,
    pub monthly_price_cents: i64,
    pub status: ListingStatus,
    pub published_at: DateTime<Utc>,
    pub total_subscribers: i64,
}

#[derive(Debug, Deserialize)]
pub struct PublishRequest {
    pub title: String,
    pub summary: String,
    pub monthly_price_cents: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Subscription {
    pub id: String,
    pub subscriber_id: String,
    pub listing_id: String,
    pub allocated_capital_cents: i64,
    pub status: SubscriptionStatus,
    pub started_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct SubscribeRequest {
    pub allocated_capital_cents: i64,
}
