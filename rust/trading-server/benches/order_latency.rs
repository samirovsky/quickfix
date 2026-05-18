//! Criterion benchmark for the order-processing hot path.

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use crossbeam_channel::{bounded, Receiver, Sender};

use trading_server::engine::{Engine, OutboundExec};
use trading_server::protocol::{NewOrderBody, OrdType, Side, Tif};
use trading_server::PRICE_SCALE;

fn order(id: u64, side: Side, price: i64, qty: u64) -> NewOrderBody {
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

fn populate(engine: &mut Engine, tx: &Sender<OutboundExec>, rx: &Receiver<OutboundExec>) {
    let mut id = 1u64;
    for level in 0..100i64 {
        for _ in 0..50 {
            engine.process_new_order(1, order(id, Side::Buy, 1000 - level, 10), tx);
            id += 1;
            engine.process_new_order(1, order(id, Side::Sell, 1100 + level, 10), tx);
            id += 1;
            while rx.try_recv().is_ok() {}
        }
    }
}

fn bench_match(c: &mut Criterion) {
    let mut group = c.benchmark_group("engine");
    group.sample_size(200);

    group.bench_function("process_new_order_marketable", |b| {
        let (tx, rx) = bounded::<OutboundExec>(1 << 20);
        let mut engine = Engine::new(4);
        populate(&mut engine, &tx, &rx);

        // Keep adding a maker so the book never empties, and time a single
        // crossing taker.
        let mut next_id = 10_000_000u64;
        b.iter_batched(
            || {
                let maker_id = next_id;
                next_id += 1;
                engine.process_new_order(1, order(maker_id, Side::Sell, 1100, 1), &tx);
                while rx.try_recv().is_ok() {}
                let taker_id = next_id;
                next_id += 1;
                order(taker_id, Side::Buy, 1100, 1)
            },
            |o| {
                engine.process_new_order(2, o, &tx);
                while rx.try_recv().is_ok() {}
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("process_new_order_resting", |b| {
        let (tx, rx) = bounded::<OutboundExec>(1 << 20);
        let mut engine = Engine::new(4);
        populate(&mut engine, &tx, &rx);
        let mut next_id = 20_000_000u64;
        b.iter_batched(
            || {
                let id = next_id;
                next_id += 1;
                // Far away from the inner book — will rest.
                order(id, Side::Buy, 500, 1)
            },
            |o| {
                engine.process_new_order(2, o, &tx);
                while rx.try_recv().is_ok() {}
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

criterion_group!(benches, bench_match);
criterion_main!(benches);
