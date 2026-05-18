//! End-to-end integration test.
//!
//! Spins up the full server on a random localhost port, connects a TCP
//! client, sends a sequence of buy/sell orders, and asserts the server
//! emits the expected execution reports.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::Duration;

use crossbeam_channel::bounded;
use zerocopy::FromBytes;

use trading_server::engine::{Engine, EngineMsg, OutboundExec};
use trading_server::protocol::{
    self, CancelOrderBody, ExecReportBody, ExecStatus, Header, NewOrderBody, OrdType, Side,
    Tif, EXEC_REPORT_SIZE, HEADER_SIZE,
};
use trading_server::server::{spawn_exec_dispatcher, ConnRegistry, Server};
use trading_server::wal::WalRecord;
use trading_server::PRICE_SCALE;

fn new_order(id: u64, side: Side, price: i64, qty: u64) -> NewOrderBody {
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

#[test]
fn cross_two_clients_emits_fills() {
    let (engine_tx, engine_rx) = bounded::<EngineMsg>(1024);
    let (exec_tx, exec_rx) = bounded::<OutboundExec>(1024);
    let (wal_tx, wal_rx) = bounded::<WalRecord>(1024);
    let registry = Arc::new(ConnRegistry::default());

    let _engine_thread = std::thread::Builder::new()
        .name("engine".into())
        .spawn(move || {
            let mut engine = Engine::new(4);
            engine.run(engine_rx, exec_tx, wal_tx);
        })
        .unwrap();

    let _dispatcher = spawn_exec_dispatcher(exec_rx, Arc::clone(&registry));
    let _wal_drainer = std::thread::spawn(move || while wal_rx.recv().is_ok() {});

    let server = Server::bind(
        "127.0.0.1:0".parse().unwrap(),
        engine_tx,
        Arc::clone(&registry),
    )
    .expect("bind");
    let addr = server.local_addr().unwrap();
    let shutdown = server.shutdown_handle();
    let server_thread = std::thread::Builder::new()
        .name("server".into())
        .spawn(move || server.run().unwrap())
        .unwrap();

    let mut buyer = TcpStream::connect(addr).expect("buyer connect");
    let mut seller = TcpStream::connect(addr).expect("seller connect");
    buyer
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    seller
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();

    // Seller posts a resting ask of 5 @ 100.
    let mut buf = Vec::with_capacity(128);
    protocol::encode_new_order(1, &new_order(1, Side::Sell, 100, 5), &mut buf);
    seller.write_all(&buf).unwrap();
    let resting = read_exec_report(&mut seller);
    assert_eq!(resting.status, ExecStatus::New as u8);
    assert_eq!(resting.order_id, 1);
    assert_eq!(resting.leaves_qty, 5);

    // Buyer crosses for 3 @ 100. Expect a partial-fill report + a terminal
    // Filled report.
    buf.clear();
    protocol::encode_new_order(2, &new_order(2, Side::Buy, 100, 3), &mut buf);
    buyer.write_all(&buf).unwrap();
    let r1 = read_exec_report(&mut buyer);
    let r2 = read_exec_report(&mut buyer);
    let reports = [r1, r2];
    assert!(
        reports
            .iter()
            .any(|r| r.last_qty == 3 && r.last_price == 100 * PRICE_SCALE),
        "expected a 3@100 fill report, got {reports:?}"
    );
    assert!(
        reports
            .iter()
            .any(|r| r.status == ExecStatus::Filled as u8),
        "expected a Filled terminal report, got {reports:?}"
    );

    // Seller cancels the residual 2 — expect a Cancelled report.
    buf.clear();
    let cancel = CancelOrderBody {
        order_id: 1,
        symbol_id: 0,
        _pad: 0,
    };
    protocol::encode_cancel(3, &cancel, &mut buf);
    seller.write_all(&buf).unwrap();
    let cancel_report = read_exec_report(&mut seller);
    assert_eq!(cancel_report.status, ExecStatus::Cancelled as u8);
    assert_eq!(cancel_report.order_id, 1);

    shutdown.store(false, std::sync::atomic::Ordering::Relaxed);
    let _ = server_thread.join();
}

fn read_exec_report(stream: &mut TcpStream) -> ExecReportBody {
    let mut header_buf = [0u8; HEADER_SIZE];
    read_exact(stream, &mut header_buf);
    let header = Header::read_from(&header_buf[..]).expect("header");
    assert_eq!(header.magic, protocol::MAGIC);
    assert_eq!(header.length as usize, EXEC_REPORT_SIZE);
    let mut body_buf = [0u8; EXEC_REPORT_SIZE];
    read_exact(stream, &mut body_buf);
    ExecReportBody::read_from(&body_buf[..]).expect("body")
}

fn read_exact(stream: &mut TcpStream, buf: &mut [u8]) {
    let mut read = 0;
    while read < buf.len() {
        let n = stream.read(&mut buf[read..]).expect("client read");
        if n == 0 {
            panic!("eof before reading {} bytes (read {read})", buf.len());
        }
        read += n;
    }
}
