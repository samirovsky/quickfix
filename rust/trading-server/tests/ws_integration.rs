//! WebSocket transport integration test.
//!
//! Connects a `tokio-tungstenite` client to a freshly bound WebSocket
//! listener, sends a QFTX `NewOrder` binary frame, reads back the exec
//! report frame, and asserts the bytes are valid QFTX wire data.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use crossbeam_channel::bounded;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;
use zerocopy::FromBytes;

use trading_server::engine::{Engine, EngineMsg, OutboundExec};
use trading_server::protocol::{
    self, ExecReportBody, ExecStatus, Header, NewOrderBody, OrdType, Side, Tif,
    EXEC_REPORT_SIZE, HEADER_SIZE,
};
use trading_server::server::{spawn_exec_dispatcher, ConnIdAllocator, ConnRegistry};
use trading_server::wal::WalRecord;
use trading_server::ws;
use trading_server::PRICE_SCALE;

fn new_order_body(id: u64, side: Side, price: i64, qty: u64) -> NewOrderBody {
    NewOrderBody {
        price: price * PRICE_SCALE,
        qty,
        client_id: id,
        order_id: id,
        symbol_id: 0,
        side: side as u8,
        ord_type: OrdType::Limit as u8,
        tif: Tif::Day as u8,
        _pad: 0,
        _pad2: 0,
        _pad3: 0,
    }
}

fn start_ws_server() -> SocketAddr {
    let (engine_tx, engine_rx) = bounded::<EngineMsg>(1024);
    let (exec_tx, exec_rx) = bounded::<OutboundExec>(1024);
    let (wal_tx, wal_rx) = bounded::<WalRecord>(1024);
    let registry = Arc::new(ConnRegistry::default());
    let alloc = Arc::new(ConnIdAllocator::new());

    let _engine = std::thread::Builder::new()
        .name("engine".into())
        .spawn(move || {
            let mut engine = Engine::new(4);
            engine.run(engine_rx, exec_tx, wal_tx);
        })
        .unwrap();
    let _disp = spawn_exec_dispatcher(exec_rx, Arc::clone(&registry));
    let _wal = std::thread::spawn(move || while wal_rx.recv().is_ok() {});

    let std_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = std_listener.local_addr().unwrap();
    drop(std_listener);

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    std::thread::spawn(move || {
        rt.block_on(async {
            ws::serve(addr, engine_tx, registry, alloc).await.unwrap();
        });
    });
    std::thread::sleep(Duration::from_millis(200));
    addr
}

#[test]
fn ws_round_trip_new_order_to_exec_report() {
    let addr = start_ws_server();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let url = format!("ws://{}/ws", addr);
        let (mut ws, _resp) = tokio_tungstenite::connect_async(&url).await.unwrap();

        // Build a NewOrder frame: resting buy 5 @ 100.
        let body = new_order_body(1, Side::Buy, 100, 5);
        let mut frame = Vec::with_capacity(128);
        protocol::encode_new_order(1, &body, &mut frame);
        ws.send(Message::Binary(frame.into())).await.unwrap();

        // Read one binary message — should be an exec report frame.
        let msg = tokio::time::timeout(Duration::from_secs(5), ws.next())
            .await
            .expect("ws read timed out")
            .expect("ws stream ended")
            .expect("ws read error");
        let bytes = match msg {
            Message::Binary(b) => b,
            other => panic!("expected binary, got {other:?}"),
        };
        assert_eq!(bytes.len(), HEADER_SIZE + EXEC_REPORT_SIZE);

        let header = Header::read_from(&bytes[..HEADER_SIZE]).expect("header");
        assert_eq!(header.magic, protocol::MAGIC);
        assert_eq!(header.length as usize, EXEC_REPORT_SIZE);

        let report = ExecReportBody::read_from(&bytes[HEADER_SIZE..]).expect("body");
        assert_eq!(report.order_id, 1);
        assert_eq!(report.status, ExecStatus::New as u8);
        assert_eq!(report.leaves_qty, 5);
    });
}
