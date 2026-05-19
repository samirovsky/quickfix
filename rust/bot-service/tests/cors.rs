//! CORS preflight smoke test. Verifies that a browser-shaped
//! preflight against a non-allowlisted origin gets the expected
//! `access-control-allow-origin` headers back. Run-on-empty origins
//! is also tested to make sure the local-dev path stays the same.

use std::net::SocketAddr;
use std::time::Duration;

use bot_service::{build_state, cors_layer, router_with_cors};
use tokio::net::TcpListener;

async fn spawn_with_origins(origins: Vec<String>) -> (String, reqwest::Client) {
    let state = build_state("sqlite::memory:").await.unwrap();
    let cors = cors_layer(&origins);
    let app = router_with_cors(state, cors);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    tokio::time::sleep(Duration::from_millis(20)).await;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    (format!("http://{addr}"), client)
}

#[tokio::test]
async fn allowlisted_origin_gets_cors_headers() {
    let (base, c) = spawn_with_origins(vec!["https://example.app".into()]).await;
    let resp = c
        .request(
            reqwest::Method::OPTIONS,
            format!("{base}/v1/marketplace/listings"),
        )
        .header("Origin", "https://example.app")
        .header("Access-Control-Request-Method", "GET")
        .header("Access-Control-Request-Headers", "x-api-key,content-type")
        .send()
        .await
        .unwrap();
    let h = resp.headers();
    assert_eq!(
        h.get("access-control-allow-origin")
            .and_then(|v| v.to_str().ok()),
        Some("https://example.app")
    );
    let methods = h
        .get("access-control-allow-methods")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(methods.to_lowercase().contains("get"), "got {methods}");
}

#[tokio::test]
async fn wildcard_origin_works() {
    let (base, c) = spawn_with_origins(vec!["*".into()]).await;
    let resp = c
        .request(
            reqwest::Method::OPTIONS,
            format!("{base}/v1/marketplace/listings"),
        )
        .header("Origin", "https://anywhere.example")
        .header("Access-Control-Request-Method", "GET")
        .send()
        .await
        .unwrap();
    let allow = resp
        .headers()
        .get("access-control-allow-origin")
        .and_then(|v| v.to_str().ok());
    assert_eq!(allow, Some("*"));
}

#[tokio::test]
async fn no_origins_means_no_cors_headers() {
    let (base, c) = spawn_with_origins(vec![]).await;
    let resp = c
        .request(
            reqwest::Method::OPTIONS,
            format!("{base}/v1/marketplace/listings"),
        )
        .header("Origin", "https://anywhere.example")
        .header("Access-Control-Request-Method", "GET")
        .send()
        .await
        .unwrap();
    assert!(resp.headers().get("access-control-allow-origin").is_none());
}
