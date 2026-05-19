//! API-key authentication middleware.
//!
//! Lookup is by sha256(key) — the raw key is never stored. The DB has a
//! `users.api_key_hash` column, which is what we match against.

use axum::extract::{Request, State};
use axum::http::HeaderName;
use axum::middleware::Next;
use axum::response::Response;
use sha2::{Digest, Sha256};

use crate::error::ApiError;
use crate::state::AppState;

pub const API_KEY_HEADER: HeaderName = HeaderName::from_static("x-api-key");

/// Resolved request identity, attached to extensions by the middleware.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: String,
}

pub fn hash_key(key: &str) -> String {
    let mut h = Sha256::new();
    h.update(key.as_bytes());
    let bytes = h.finalize();
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    s
}

pub async fn require_api_key(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let key = req
        .headers()
        .get(&API_KEY_HEADER)
        .and_then(|v| v.to_str().ok())
        .ok_or(ApiError::Unauthorized)?
        .trim();
    if key.is_empty() {
        return Err(ApiError::Unauthorized);
    }
    let hash = hash_key(key);
    let user_id: Option<String> =
        sqlx::query_scalar("SELECT id FROM users WHERE api_key_hash = ?1")
            .bind(&hash)
            .fetch_optional(&state.db)
            .await?;
    let id = user_id.ok_or(ApiError::Unauthorized)?;
    req.extensions_mut().insert(AuthUser { id });
    Ok(next.run(req).await)
}
