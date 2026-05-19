//! Shared helpers for integration tests.
//!
//! Each test spawns a fresh service on a random port backed by an
//! in-memory sqlite, provisions one user, and hands back the api key
//! along with the base URL.

use std::net::SocketAddr;
use std::time::Duration;

use bot_service::store::user_repo;
use bot_service::{build_state, router, AppState};
use tokio::net::TcpListener;

#[allow(dead_code)] // `state` is unused in some test files; included in all spawns
pub struct TestApp {
    pub base_url: String,
    pub api_key: String,
    pub state: AppState,
}

pub async fn spawn() -> TestApp {
    let _ = tracing_subscriber::fmt()
        .with_test_writer()
        .with_env_filter("warn")
        .try_init();

    let state: AppState = build_state("sqlite::memory:")
        .await
        .expect("build state with in-memory sqlite");

    let api_key = format!("test-key-{}", uuid::Uuid::new_v4().simple());
    user_repo::upsert_with_key(&state.db, "test-user", &api_key)
        .await
        .expect("provision test user");

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr: SocketAddr = listener.local_addr().expect("local_addr");
    let app = router(state.clone());

    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    // Give the listener a moment to start accepting connections.
    tokio::time::sleep(Duration::from_millis(20)).await;

    TestApp {
        base_url: format!("http://{addr}"),
        api_key,
        state,
    }
}

pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("reqwest client")
}
