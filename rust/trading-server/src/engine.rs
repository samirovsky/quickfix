//! Matching engine driver.
//!
//! The engine owns one `OrderBook` per `SymbolId` (dense `Vec` index) and is
//! the only writer to those books. It is single-threaded — input arrives on
//! a `crossbeam_channel`, outputs (execution reports, market-data updates,
//! WAL records) are pushed onto other channels. The hot path does not
//! allocate: a single scratch `Vec<Fill>` and `Vec<u8>` are reused per call.

use crossbeam_channel::{Receiver, Sender, TrySendError};

use crate::marketdata::MarketDataEvent;
use crate::metrics::EngineMetrics;
use crate::orderbook::{Fill, OrderBook, SubmitOutcome};
use crate::protocol::{
    ExecReportBody, ExecStatus, InboundMessage, NewOrderBody, OrdType, Side, Tif,
};
use crate::wal::WalRecord;
use crate::{ConnId, SymbolId};

/// A message handed to the engine. The hot path operates on these values
/// without further allocation.
#[derive(Debug, Clone)]
pub enum EngineMsg {
    Order { conn_id: ConnId, msg: InboundMessage },
    MarketData(MarketDataEvent),
    Shutdown,
}

/// An execution report routed back to a specific client connection.
#[derive(Debug, Clone)]
pub struct OutboundExec {
    pub conn_id: ConnId,
    pub body: ExecReportBody,
}

pub struct Engine {
    books: Vec<OrderBook>,
    next_exec_id: u64,
    fills_scratch: Vec<Fill>,
    metrics: EngineMetrics,
}

impl Engine {
    pub fn new(symbols: u32) -> Self {
        let books = (0..symbols).map(OrderBook::new).collect();
        Self {
            books,
            next_exec_id: 1,
            fills_scratch: Vec::with_capacity(64),
            metrics: EngineMetrics::new(),
        }
    }

    pub fn metrics(&self) -> &EngineMetrics {
        &self.metrics
    }

    pub fn symbol_count(&self) -> usize {
        self.books.len()
    }

    pub fn book(&self, symbol_id: SymbolId) -> Option<&OrderBook> {
        self.books.get(symbol_id as usize)
    }

    /// Run the engine loop until a `Shutdown` is received. Designed to be
    /// invoked on a dedicated OS thread.
    pub fn run(
        &mut self,
        inbox: Receiver<EngineMsg>,
        execs: Sender<OutboundExec>,
        wal: Sender<WalRecord>,
    ) {
        while let Ok(msg) = inbox.recv() {
            match msg {
                EngineMsg::Order { conn_id, msg } => {
                    let start = std::time::Instant::now();
                    self.handle_order(conn_id, msg, &execs, &wal);
                    self.metrics.record_order_latency(start.elapsed());
                }
                EngineMsg::MarketData(_ev) => {
                    // Placeholder hook: real implementations might update
                    // reference prices or auction state from external feeds.
                    self.metrics.record_market_data();
                }
                EngineMsg::Shutdown => break,
            }
        }
    }

    fn handle_order(
        &mut self,
        conn_id: ConnId,
        msg: InboundMessage,
        execs: &Sender<OutboundExec>,
        wal: &Sender<WalRecord>,
    ) {
        match msg {
            InboundMessage::NewOrder { body, .. } => {
                self.wal_enqueue(wal, WalRecord::new_order(body));
                self.process_new_order(conn_id, body, execs);
            }
            InboundMessage::Cancel { body, .. } => {
                self.wal_enqueue(wal, WalRecord::cancel(body));
                let cancelled = self
                    .books
                    .get_mut(body.symbol_id as usize)
                    .map(|b| b.cancel(body.order_id))
                    .unwrap_or(0);
                let status = if cancelled > 0 {
                    ExecStatus::Cancelled
                } else {
                    ExecStatus::Rejected
                };
                let exec_id = self.next_exec();
                let report = ExecReportBody {
                    order_id: body.order_id,
                    exec_id,
                    last_price: 0,
                    last_qty: 0,
                    leaves_qty: 0,
                    symbol_id: body.symbol_id,
                    status: status as u8,
                    side: 0,
                    _pad: [0; 2],
                };
                let _ = execs.try_send(OutboundExec {
                    conn_id,
                    body: report,
                });
            }
            InboundMessage::Heartbeat { .. } => {
                // Echo a heartbeat-ish status? For now, do nothing — heartbeats
                // are accounted for at the protocol layer.
            }
        }
    }

    /// Hot path. Called directly by the benchmark and the engine loop.
    #[inline]
    pub fn process_new_order(
        &mut self,
        conn_id: ConnId,
        body: NewOrderBody,
        execs: &Sender<OutboundExec>,
    ) {
        let symbol_idx = body.symbol_id as usize;
        let Some(book) = self.books.get_mut(symbol_idx) else {
            let exec_id = self.next_exec();
            let _ = execs.try_send(OutboundExec {
                conn_id,
                body: ExecReportBody {
                    order_id: body.order_id,
                    exec_id,
                    last_price: 0,
                    last_qty: 0,
                    leaves_qty: 0,
                    symbol_id: body.symbol_id,
                    status: ExecStatus::Rejected as u8,
                    side: body.side,
                    _pad: [0; 2],
                },
            });
            return;
        };
        let Some(side) = Side::from_u8(body.side) else {
            return;
        };
        let Some(ord_type) = OrdType::from_u8(body.ord_type) else {
            return;
        };
        let Some(tif) = Tif::from_u8(body.tif) else {
            return;
        };

        self.fills_scratch.clear();
        let outcome = book.submit(
            body.order_id,
            body.client_id,
            side,
            ord_type,
            tif,
            body.price,
            body.qty,
            &mut self.fills_scratch,
        );

        let (filled, resting, status) = match outcome {
            SubmitOutcome::FullyFilled => (body.qty, 0, ExecStatus::Filled),
            SubmitOutcome::PartialAndDone { filled_qty } => {
                (filled_qty, 0, ExecStatus::PartiallyFilled)
            }
            SubmitOutcome::NoFill => (0, 0, ExecStatus::New),
            SubmitOutcome::Resting {
                resting_qty,
                filled_qty,
                ..
            } => {
                let status = if filled_qty > 0 {
                    ExecStatus::PartiallyFilled
                } else {
                    ExecStatus::New
                };
                (filled_qty, resting_qty, status)
            }
            SubmitOutcome::Rejected => {
                let exec_id = self.next_exec();
                let _ = execs.try_send(OutboundExec {
                    conn_id,
                    body: ExecReportBody {
                        order_id: body.order_id,
                        exec_id,
                        last_price: 0,
                        last_qty: 0,
                        leaves_qty: 0,
                        symbol_id: body.symbol_id,
                        status: ExecStatus::Rejected as u8,
                        side: body.side,
                        _pad: [0; 2],
                    },
                });
                return;
            }
        };

        // Emit one exec report per fill for the taker. When the order is
        // done (filled, partial-and-done, or rejected after fills), the
        // LAST fill carries the terminal status — saving a separate
        // terminal `try_send`. Maker reports are intentionally omitted
        // (no client-id ↔ conn map; they would route to conn_id=0 and be
        // dropped by the dispatcher).
        //
        // Iterate by index — `Fill` is `Copy`, and the loop body mutates
        // `self.next_exec_id` so we can't hold an immutable borrow of
        // `self.fills_scratch`.
        let total_fills = self.fills_scratch.len();
        if total_fills > 0 {
            let last_idx = total_fills - 1;
            for i in 0..total_fills {
                let fill = self.fills_scratch[i];
                let is_last = i == last_idx;
                let report_status = if is_last {
                    status as u8
                } else {
                    ExecStatus::PartiallyFilled as u8
                };
                let exec_id = self.next_exec();
                let taker_report = ExecReportBody {
                    order_id: fill.taker_order_id,
                    exec_id,
                    last_price: fill.price,
                    last_qty: fill.qty,
                    leaves_qty: if is_last { resting } else { 0 },
                    symbol_id: body.symbol_id,
                    status: report_status,
                    side: body.side,
                    _pad: [0; 2],
                };
                let _ = execs.try_send(OutboundExec {
                    conn_id,
                    body: taker_report,
                });
            }
        } else {
            // No fills — emit a single terminal report (New for resting,
            // NoFill, or Rejected).
            let exec_id = self.next_exec();
            let final_report = ExecReportBody {
                order_id: body.order_id,
                exec_id,
                last_price: 0,
                last_qty: 0,
                leaves_qty: resting,
                symbol_id: body.symbol_id,
                status: status as u8,
                side: body.side,
                _pad: [0; 2],
            };
            let _ = execs.try_send(OutboundExec {
                conn_id,
                body: final_report,
            });
        }
        self.metrics.record_order(filled, total_fills);
    }

    #[inline]
    fn next_exec(&mut self) -> u64 {
        let id = self.next_exec_id;
        self.next_exec_id = self.next_exec_id.wrapping_add(1);
        id
    }

    #[inline]
    fn wal_enqueue(&self, wal: &Sender<WalRecord>, record: WalRecord) {
        // Hot path must never block — use `try_send` and account for drops
        // in metrics. The user-approved trade-off: availability over
        // in-flight durability. `WalRecord` is `Copy`, so no allocation.
        match wal.try_send(record) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {
                self.metrics.record_wal_dropped()
            }
        }
    }
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("symbols", &self.books.len())
            .field("next_exec_id", &self.next_exec_id)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PRICE_SCALE;
    use crossbeam_channel::bounded;

    fn new_order(
        order_id: u64,
        symbol_id: u32,
        side: Side,
        price: i64,
        qty: u64,
    ) -> NewOrderBody {
        NewOrderBody {
            price: price * PRICE_SCALE,
            qty,
            client_id: order_id,
            order_id,
            symbol_id,
            side: side as u8,
            ord_type: OrdType::Limit as u8,
            tif: Tif::Day as u8,
            _pad: 0,
            _pad2: 0,
        _pad3: 0,
        }
    }

    #[test]
    fn resting_then_crossing_emits_reports() {
        let mut eng = Engine::new(4);
        let (exec_tx, exec_rx) = bounded(64);

        eng.process_new_order(1, new_order(1, 0, Side::Sell, 100, 10), &exec_tx);
        eng.process_new_order(2, new_order(2, 0, Side::Buy, 100, 6), &exec_tx);

        let reports: Vec<_> = exec_rx.try_iter().collect();
        // Order 1: one report with status=New (resting on the book).
        // Order 2: one report with status=PartiallyFilled (taker filled
        // 6, maker has leaves_qty=4 but goes back to the seller, not the
        // buyer). For the buyer the single fill is partial-and-done? No —
        // it's a Day limit buy, fully matched at the limit price, so
        // status=Filled. (PartiallyFilled would only happen for IOC.)
        let statuses: Vec<u8> = reports.iter().map(|r| r.body.status).collect();
        assert!(statuses.contains(&(ExecStatus::New as u8)), "got {statuses:?}");
        assert!(
            statuses.contains(&(ExecStatus::Filled as u8)),
            "got {statuses:?}"
        );
    }
}
