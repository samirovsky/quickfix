mod common;

use common::{client, spawn};
use serde_json::{json, Value};

#[tokio::test]
async fn full_crud_lifecycle_via_template() {
    let app = spawn().await;
    let c = client();
    let auth = |req: reqwest::RequestBuilder| req.header("X-API-Key", &app.api_key);

    // List templates — at least the 5 we bundled.
    let templates: Vec<Value> = auth(c.get(format!("{}/v1/templates", app.base_url)))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(templates.len() >= 5, "got {} templates", templates.len());
    let tpl_id = templates[0]["id"].as_str().unwrap().to_string();

    // Create a bot from that template.
    let created: Value = auth(
        c.post(format!("{}/v1/bots/from-template/{tpl_id}", app.base_url))
            .json(&json!({ "name": "first bot" })),
    )
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    let bot_id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["name"], "first bot");
    assert_eq!(created["status"], "draft");
    assert_eq!(created["source"], format!("template:{tpl_id}"));

    // Fetch it back.
    let got: Value = auth(c.get(format!("{}/v1/bots/{bot_id}", app.base_url)))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(got["id"], bot_id);

    // Rename + change description.
    let updated: Value = auth(
        c.put(format!("{}/v1/bots/{bot_id}", app.base_url))
            .json(&json!({ "name": "renamed", "description": "tweaked" })),
    )
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    assert_eq!(updated["name"], "renamed");
    assert_eq!(updated["description"], "tweaked");

    // Transition to paper.
    let papered: Value = auth(
        c.post(format!("{}/v1/bots/{bot_id}/status", app.base_url))
            .json(&json!({ "status": "paper" })),
    )
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    assert_eq!(papered["status"], "paper");

    // Refusing 'live' in v1 is enforced.
    let live = auth(
        c.post(format!("{}/v1/bots/{bot_id}/status", app.base_url))
            .json(&json!({ "status": "live" })),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(live.status(), 400);

    // List bots — should contain ours.
    let list: Vec<Value> = auth(c.get(format!("{}/v1/bots", app.base_url)))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(list.iter().any(|b| b["id"] == bot_id.as_str()));

    // Filter by status — `paper` returns our bot; `draft` does not.
    let papers: Vec<Value> = auth(c.get(format!("{}/v1/bots?status=paper", app.base_url)))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(papers.iter().any(|b| b["id"] == bot_id.as_str()));
    let drafts: Vec<Value> = auth(c.get(format!("{}/v1/bots?status=draft", app.base_url)))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(!drafts.iter().any(|b| b["id"] == bot_id.as_str()));

    // Unknown status → 400.
    let bad_status = auth(c.get(format!("{}/v1/bots?status=banana", app.base_url)))
        .send()
        .await
        .unwrap();
    assert_eq!(bad_status.status(), 400);

    // Delete it.
    let del = auth(c.delete(format!("{}/v1/bots/{bot_id}", app.base_url)))
        .send()
        .await
        .unwrap();
    assert_eq!(del.status(), 204);

    // Now 404.
    let after = auth(c.get(format!("{}/v1/bots/{bot_id}", app.base_url)))
        .send()
        .await
        .unwrap();
    assert_eq!(after.status(), 404);
}

#[tokio::test]
async fn rejects_invalid_strategy_at_write_time() {
    let app = spawn().await;
    let c = client();

    let resp = c
        .post(format!("{}/v1/bots", app.base_url))
        .header("X-API-Key", &app.api_key)
        .json(&json!({
            "name": "bad",
            "strategy": { "version": 1, "entry": { "all_of": [ { "type": "no_such_condition" } ] } },
            "asset_filter": { "symbols": [0], "market": "qftx" }
        }))
        .send()
        .await
        .unwrap();
    // Unknown condition type → 422 from axum's JSON extractor (well-formed JSON
    // but doesn't deserialise to our typed schema).
    assert!(
        resp.status() == 422 || resp.status() == 400,
        "got {}",
        resp.status()
    );
}

#[tokio::test]
async fn billing_estimate_returns_expected_shape() {
    let app = spawn().await;
    let c = client();

    let est: Value = c
        .post(format!("{}/v1/billing/estimate", app.base_url))
        .header("X-API-Key", &app.api_key)
        .json(&json!({ "trades_per_month": 50, "asset_class": "stocks" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(est["monthly_cents"], 50);
    assert!(est["breakdown"].is_array());

    let usage: Value = c
        .get(format!("{}/v1/billing/usage", app.base_url))
        .header("X-API-Key", &app.api_key)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(usage["transaction_fees_ytd_cents"], 0);
    assert_eq!(usage["transaction_fees_this_month_cents"], 0);
}
