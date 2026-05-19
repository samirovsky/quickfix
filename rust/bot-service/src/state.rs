use std::sync::Arc;

use sqlx::SqlitePool;

use crate::models::template::Template;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub templates: Arc<Vec<Template>>,
}
