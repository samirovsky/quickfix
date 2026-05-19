mod common;

use common::{client, spawn};

#[tokio::test]
async fn missing_api_key_returns_401() {
    let app = spawn().await;
    let resp = client()
        .get(format!("{}/v1/templates", app.base_url))
        .send()
        .await
        .expect("send");
    assert_eq!(resp.status(), 401, "missing X-API-Key should 401");
}

#[tokio::test]
async fn garbage_api_key_returns_401() {
    let app = spawn().await;
    let resp = client()
        .get(format!("{}/v1/templates", app.base_url))
        .header("X-API-Key", "definitely-not-a-real-key")
        .send()
        .await
        .expect("send");
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn valid_api_key_passes_through() {
    let app = spawn().await;
    let resp = client()
        .get(format!("{}/v1/templates", app.base_url))
        .header("X-API-Key", &app.api_key)
        .send()
        .await
        .expect("send");
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn healthz_is_public() {
    let app = spawn().await;
    let resp = client()
        .get(format!("{}/healthz", app.base_url))
        .send()
        .await
        .expect("send");
    assert_eq!(resp.status(), 200);
}
