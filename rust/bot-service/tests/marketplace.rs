//! Marketplace + performance integration tests.

mod common;

use common::{client, spawn};
use serde_json::{json, Value};

async fn get_json(c: &reqwest::Client, key: &str, url: &str) -> Value {
    c.get(url)
        .header("X-API-Key", key)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

#[tokio::test]
async fn publish_browse_subscribe_round_trip() {
    let app = spawn().await;
    let c = client();

    // Provision a second user (bob) on the same DB so we can subscribe
    // from a different identity than the creator.
    let bob_key = format!("bob-{}", uuid::Uuid::new_v4().simple());
    let bob = bot_service::store::user_repo::upsert_with_key(&app.state.db, "bob", &bob_key)
        .await
        .unwrap();
    let _ = bob;

    // Alice (the spawn() user) creates a bot from a template and publishes it.
    let templates: Vec<Value> = serde_json::from_value(
        get_json(&c, &app.api_key, &format!("{}/v1/templates", app.base_url)).await,
    )
    .unwrap();
    let tpl_id = templates[0]["id"].as_str().unwrap();

    let created: Value = c
        .post(format!("{}/v1/bots/from-template/{tpl_id}", app.base_url))
        .header("X-API-Key", &app.api_key)
        .json(&json!({ "name": "alice's bot" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let bot_id = created["id"].as_str().unwrap().to_string();

    let listing: Value = c
        .post(format!("{}/v1/bots/{bot_id}/publish", app.base_url))
        .header("X-API-Key", &app.api_key)
        .json(&json!({
            "title": "Alice's RSI bot",
            "summary": "Mean reversion. Sips on majors.",
            "monthly_price_cents": 1999
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let listing_id = listing["id"].as_str().unwrap().to_string();
    assert_eq!(listing["status"], "published");
    assert_eq!(listing["monthly_price_cents"], 1999);
    assert_eq!(listing["total_subscribers"], 0);

    // GET /v1/bots/:id now reports the listing id so the UI can render a
    // "Published" badge in the builder without a second round-trip.
    let bot_now: Value = get_json(
        &c,
        &app.api_key,
        &format!("{}/v1/bots/{bot_id}", app.base_url),
    )
    .await;
    assert_eq!(bot_now["published_listing_id"], listing_id.as_str());

    // Bob browses.
    let listings: Vec<Value> = c
        .get(format!("{}/v1/marketplace/listings", app.base_url))
        .header("X-API-Key", &bob_key)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(listings.len(), 1, "got {listings:?}");
    assert_eq!(listings[0]["id"], listing_id.as_str());

    // Bob subscribes.
    let sub: Value = c
        .post(format!(
            "{}/v1/marketplace/listings/{listing_id}/subscribe",
            app.base_url
        ))
        .header("X-API-Key", &bob_key)
        .json(&json!({ "allocated_capital_cents": 100_000 }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let sub_id = sub["id"].as_str().unwrap().to_string();
    assert_eq!(sub["status"], "active");

    // Listing now reflects 1 subscriber.
    let updated_listing: Value = c
        .get(format!(
            "{}/v1/marketplace/listings/{listing_id}",
            app.base_url
        ))
        .header("X-API-Key", &bob_key)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(updated_listing["total_subscribers"], 1);

    // Bob's "my subs" returns the subscription joined with the listing.
    let my_subs: Vec<Value> = c
        .get(format!("{}/v1/me/subscriptions", app.base_url))
        .header("X-API-Key", &bob_key)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(my_subs.len(), 1);
    assert_eq!(my_subs[0]["id"], sub_id.as_str());
    assert_eq!(my_subs[0]["listing"]["id"], listing_id.as_str());

    // Subscribing to the same listing twice is rejected.
    let dup = c
        .post(format!(
            "{}/v1/marketplace/listings/{listing_id}/subscribe",
            app.base_url
        ))
        .header("X-API-Key", &bob_key)
        .json(&json!({ "allocated_capital_cents": 500 }))
        .send()
        .await
        .unwrap();
    assert_eq!(dup.status(), 400);

    // Alice cannot subscribe to her own bot.
    let self_sub = c
        .post(format!(
            "{}/v1/marketplace/listings/{listing_id}/subscribe",
            app.base_url
        ))
        .header("X-API-Key", &app.api_key)
        .json(&json!({ "allocated_capital_cents": 500 }))
        .send()
        .await
        .unwrap();
    assert_eq!(self_sub.status(), 400);

    // Bob cancels.
    let cancel = c
        .post(format!("{}/v1/subscriptions/{sub_id}/cancel", app.base_url))
        .header("X-API-Key", &bob_key)
        .send()
        .await
        .unwrap();
    assert_eq!(cancel.status(), 204);

    // /me/subscriptions filters out cancelled ones.
    let after_cancel: Vec<Value> = c
        .get(format!("{}/v1/me/subscriptions", app.base_url))
        .header("X-API-Key", &bob_key)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(after_cancel.is_empty(), "got {after_cancel:?}");

    // Alice unpublishes.
    let un = c
        .post(format!(
            "{}/v1/marketplace/listings/{listing_id}/unpublish",
            app.base_url
        ))
        .header("X-API-Key", &app.api_key)
        .send()
        .await
        .unwrap();
    assert_eq!(un.status(), 204);

    // Listings list is empty.
    let after_unpub: Vec<Value> = c
        .get(format!("{}/v1/marketplace/listings", app.base_url))
        .header("X-API-Key", &bob_key)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(after_unpub.is_empty());
}

#[tokio::test]
async fn performance_endpoint_aggregates_seeded_trades() {
    let app = spawn().await;
    let c = client();

    // Create a bot, then insert known trades directly through the repo.
    let templates: Vec<Value> = serde_json::from_value(
        get_json(&c, &app.api_key, &format!("{}/v1/templates", app.base_url)).await,
    )
    .unwrap();
    let tpl_id = templates[0]["id"].as_str().unwrap();
    let bot: Value = c
        .post(format!("{}/v1/bots/from-template/{tpl_id}", app.base_url))
        .header("X-API-Key", &app.api_key)
        .json(&json!({ "name": "perf bot" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let bot_id = bot["id"].as_str().unwrap().to_string();

    // 3 wins, 2 losses today.
    let now = chrono::Utc::now();
    for (i, pnl) in [200_i64, 300, 500, -100, -50].iter().enumerate() {
        bot_service::store::performance_repo::record_trade(
            &app.state.db,
            &bot_id,
            (i as i64) + 1,
            0,
            10_000_000_000,
            1,
            *pnl,
            5,
            now - chrono::Duration::minutes(i as i64),
        )
        .await
        .unwrap();
    }

    let metrics: Value = c
        .get(format!(
            "{}/v1/bots/{bot_id}/performance?days=7",
            app.base_url
        ))
        .header("X-API-Key", &app.api_key)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(metrics["total_trades"], 5);
    assert_eq!(metrics["total_realised_pnl_cents"], 850); // 200+300+500-100-50
                                                          // win_rate = 3/5 = 0.6 ± float wiggle
    let win_rate = metrics["win_rate"].as_f64().unwrap();
    assert!((win_rate - 0.6).abs() < 1e-9, "win_rate={win_rate}");
    assert_eq!(metrics["largest_win_cents"], 500);
    assert_eq!(metrics["largest_loss_cents"], -100);
    let daily = metrics["daily"].as_array().unwrap();
    assert_eq!(daily.len(), 1);
    assert_eq!(daily[0]["trades"], 5);
}
