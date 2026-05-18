//! Hard p99 latency gate.
//!
//! Drives 100k buy/sell orders through a populated book and asserts that
//! `Engine::process_new_order` (the hot path) completes within 1 ms at the
//! p99. Expected actual is in the low microseconds.

use std::time::Instant;

use crossbeam_channel::bounded;
use hdrhistogram::Histogram;

use trading_server::engine::{Engine, OutboundExec};
use trading_server::protocol::{NewOrderBody, OrdType, Side, Tif};
use trading_server::PRICE_SCALE;

fn new_order(order_id: u64, side: Side, price: i64, qty: u64) -> NewOrderBody {
    NewOrderBody {
        price: price * PRICE_SCALE,
        qty,
        client_id: order_id,
        order_id,
        symbol_id: 0,
        side: side as u8,
        ord_type: OrdType::Limit as u8,
        tif: Tif::Day as u8,
        _pad: 0,
        _pad2: 0,
        _pad3: 0,
    }
}

fn populate(engine: &mut Engine, exec_tx: &crossbeam_channel::Sender<OutboundExec>) {
    // 100 price levels per side, 50 resting orders per level = 10k resting.
    let mut id = 1u64;
    for level in 0..100i64 {
        for _ in 0..50 {
            engine.process_new_order(1, new_order(id, Side::Buy, 1000 - level, 10), exec_tx);
            id += 1;
            engine.process_new_order(1, new_order(id, Side::Sell, 1100 + level, 10), exec_tx);
            id += 1;
        }
    }
}

#[test]
fn p99_under_one_ms() {
    let mut engine = Engine::new(4);
    let (exec_tx, exec_rx) = bounded::<OutboundExec>(1 << 20);

    // Populate the book and drain the exec channel so it doesn't back up.
    populate(&mut engine, &exec_tx);
    drain(&exec_rx);

    let mut hist = Histogram::<u64>::new_with_bounds(1, 60_000_000_000, 3).expect("hist");

    // 100k crossing orders alternating sides at the inner price.
    let mut id = 1_000_000u64;
    let iterations = 100_000u64;
    for i in 0..iterations {
        // Re-add a resting maker on the side we're about to consume so the
        // book never empties; the order will partially fill.
        let side = if i % 2 == 0 { Side::Buy } else { Side::Sell };
        let maker_side = side.opposite();
        let price = if matches!(maker_side, Side::Buy) {
            1000
        } else {
            1100
        };
        engine.process_new_order(1, new_order(id, maker_side, price, 1), &exec_tx);
        id += 1;
        drain(&exec_rx);

        let order = new_order(id, side, price, 1);
        id += 1;

        let start = Instant::now();
        engine.process_new_order(2, order, &exec_tx);
        let dt = start.elapsed();
        hist.record(dt.as_nanos().max(1) as u64).unwrap();
        drain(&exec_rx);
    }

    let p50 = hist.value_at_quantile(0.50);
    let p99 = hist.value_at_quantile(0.99);
    let p999 = hist.value_at_quantile(0.999);
    let max = hist.max();
    eprintln!(
        "process_new_order ns: p50={p50} p99={p99} p99.9={p999} max={max} samples={}",
        hist.len()
    );

    assert!(
        p99 < 1_000_000,
        "p99 {p99} ns exceeds 1 ms latency budget"
    );
}

fn drain(rx: &crossbeam_channel::Receiver<OutboundExec>) {
    while rx.try_recv().is_ok() {}
}
