//! End-to-end gRPC integration test.
//!
//! Spins up the full server (engine + dispatcher + WAL + gRPC listener)
//! on a random localhost port, opens a tonic client, and exercises the
//! unary `PlaceOrder` + `CancelOrder` RPCs as well as the bidi
//! `OrderSession` stream.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use crossbeam_channel::bounded;
use futures_util::StreamExt;

use trading_server::engine::{Engine, EngineMsg, OutboundExec};
use trading_server::grpc;
use trading_server::marketdata::MarketDataBus;
use trading_server::pb::trading_service_client::TradingServiceClient;
use trading_server::pb::{
    self, CancelOrder, CancelOrderRequest, ClientMessage, ExecStatus, NewOrder, OrdType,
    PlaceOrderRequest, Side, Tif,
};
use trading_server::server::{spawn_exec_dispatcher, ConnIdAllocator, ConnRegistry};
use trading_server::wal::WalRecord;
use trading_server::PRICE_SCALE;

fn new_order(id: u64, side: Side, price: i64, qty: u64) -> NewOrder {
    NewOrder {
        order_id: id,
        symbol_id: 0,
        side: side as i32,
        ord_type: OrdType::Limit as i32,
        tif: Tif::Day as i32,
        price: price * PRICE_SCALE,
        qty,
        client_id: id,
    }
}

struct Harness {
    grpc_addr: SocketAddr,
}

fn start_server() -> Harness {
    let (engine_tx, engine_rx) = bounded::<EngineMsg>(1024);
    let (exec_tx, exec_rx) = bounded::<OutboundExec>(1024);
    let (wal_tx, wal_rx) = bounded::<WalRecord>(1024);
    let registry = Arc::new(ConnRegistry::default());
    let alloc = Arc::new(ConnIdAllocator::new());
    let bus = MarketDataBus::default();

    let _engine = std::thread::Builder::new()
        .name("engine".into())
        .spawn(move || {
            let mut engine = Engine::new(4);
            engine.run(engine_rx, exec_tx, wal_tx);
        })
        .unwrap();
    let _disp = spawn_exec_dispatcher(exec_rx, Arc::clone(&registry));
    let _wal = std::thread::spawn(move || while wal_rx.recv().is_ok() {});

    // Reserve a random port by binding then immediately reusing.
    let std_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let grpc_addr = std_listener.local_addr().unwrap();
    drop(std_listener);

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    std::thread::spawn(move || {
        rt.block_on(async {
            grpc::serve(grpc_addr, engine_tx, registry, alloc, bus)
                .await
                .unwrap();
        });
    });
    // Tiny wait for the bind to complete.
    std::thread::sleep(Duration::from_millis(200));
    Harness { grpc_addr }
}

async fn client_for(addr: SocketAddr) -> TradingServiceClient<tonic::transport::Channel> {
    let url = format!("http://{}", addr);
    TradingServiceClient::connect(url).await.unwrap()
}

#[test]
fn place_order_then_cancel_via_unary_rpcs() {
    let h = start_server();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let mut c = client_for(h.grpc_addr).await;

        // Resting buy 5 @ 100.
        let resp = c
            .place_order(PlaceOrderRequest {
                order: Some(new_order(1, Side::Buy, 100, 5)),
            })
            .await
            .unwrap()
            .into_inner();
        assert!(!resp.reports.is_empty(), "got no reports");
        let last = resp.reports.last().unwrap();
        assert_eq!(last.status, ExecStatus::New as i32, "report = {last:?}");
        assert_eq!(last.leaves_qty, 5);
        assert_eq!(last.order_id, 1);

        // Cancel it.
        let cancel = c
            .cancel_order(CancelOrderRequest {
                cancel: Some(CancelOrder {
                    order_id: 1,
                    symbol_id: 0,
                }),
            })
            .await
            .unwrap()
            .into_inner();
        let r = cancel.report.expect("cancel report");
        assert_eq!(r.status, ExecStatus::Cancelled as i32, "report = {r:?}");
        assert_eq!(r.order_id, 1);
    });
}

#[test]
fn order_session_bidi_emits_exec_reports() {
    let h = start_server();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let mut c = client_for(h.grpc_addr).await;

        // Send a resting sell, then a crossing buy, on the same session.
        let (req_tx, req_rx) = tokio::sync::mpsc::channel::<ClientMessage>(8);
        let req_stream = tokio_stream::wrappers::ReceiverStream::new(req_rx);
        let resp = c.order_session(req_stream).await.unwrap();
        let mut resp_stream = resp.into_inner();

        req_tx
            .send(ClientMessage {
                body: Some(pb::client_message::Body::NewOrder(new_order(
                    101,
                    Side::Sell,
                    100,
                    5,
                ))),
            })
            .await
            .unwrap();
        req_tx
            .send(ClientMessage {
                body: Some(pb::client_message::Body::NewOrder(new_order(
                    102,
                    Side::Buy,
                    100,
                    3,
                ))),
            })
            .await
            .unwrap();

        // Drain a handful of reports with a short overall deadline.
        let mut collected = Vec::new();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
        while collected.len() < 2 && tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(
                deadline.saturating_duration_since(tokio::time::Instant::now()),
                resp_stream.next(),
            )
            .await
            {
                Ok(Some(Ok(m))) => {
                    if let Some(pb::server_message::Body::Exec(e)) = m.body {
                        collected.push(e);
                    }
                }
                _ => break,
            }
        }

        // Order 101 should rest (status=New). Order 102 should fill (status=Filled).
        let any_new = collected
            .iter()
            .any(|r| r.order_id == 101 && r.status == ExecStatus::New as i32);
        let any_fill = collected.iter().any(|r| {
            r.order_id == 102
                && r.status == ExecStatus::Filled as i32
                && r.last_qty == 3
                && r.last_price == 100 * PRICE_SCALE
        });
        assert!(any_new, "missing New for 101 in {collected:?}");
        assert!(any_fill, "missing Filled for 102 in {collected:?}");

        drop(req_tx);
    });
}
