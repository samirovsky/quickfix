//! gRPC TradingService.
//!
//! Implements the four RPCs declared in `proto/trading.proto`:
//!
//! - `OrderSession`        — bidi stream, recommended for native clients.
//! - `PlaceOrder`          — unary, returns every exec report up to terminal.
//! - `CancelOrder`         — unary, returns the cancel ack.
//! - `SubscribeMarketData` — server stream of market data events.
//!
//! All of these share the same engine: orders are converted to the
//! internal `protocol::NewOrderBody` / `CancelOrderBody` and pushed onto
//! the engine's `crossbeam_channel`. Outbound exec reports flow through
//! the same `ConnRegistry` the TCP transport uses, with a per-stream
//! `tokio::sync::mpsc::UnboundedSender` registered as `ConnSender::Async`.

use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;

use crossbeam_channel::Sender as XSender;
use futures_util::{Stream, StreamExt};
use tokio::sync::mpsc;
use tonic::{transport::Server as TonicServer, Request, Response, Status, Streaming};
use tracing::{debug, info, warn};
use zerocopy::FromBytes;

use crate::engine::EngineMsg;
use crate::marketdata::{MarketDataBus, MarketDataEvent};
use crate::pb::trading_service_server::{TradingService, TradingServiceServer};
use crate::pb::{
    self, CancelOrderRequest, CancelOrderResponse, ClientMessage, ExecutionReport,
    MarketDataUpdate, PlaceOrderRequest, PlaceOrderResponse, ServerMessage,
    SubscribeMarketDataRequest,
};
use crate::protocol::{
    self, CancelOrderBody, ExecReportBody, ExecStatus, InboundMessage, NewOrderBody, OrdType,
    Side, Tif, EXEC_REPORT_SIZE, HEADER_SIZE,
};
use crate::server::{ConnIdAllocator, ConnRegistry, ConnSender};

const PER_STREAM_QUEUE: usize = 1024;

#[derive(Clone)]
pub struct TradingServiceImpl {
    engine_tx: XSender<EngineMsg>,
    registry: Arc<ConnRegistry>,
    next_conn_id: Arc<ConnIdAllocator>,
    md_bus: MarketDataBus,
}

impl std::fmt::Debug for TradingServiceImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TradingServiceImpl").finish()
    }
}

impl TradingServiceImpl {
    pub fn new(
        engine_tx: XSender<EngineMsg>,
        registry: Arc<ConnRegistry>,
        next_conn_id: Arc<ConnIdAllocator>,
        md_bus: MarketDataBus,
    ) -> Self {
        Self {
            engine_tx,
            registry,
            next_conn_id,
            md_bus,
        }
    }
}

/// Run the gRPC server until the process exits. Designed to be `spawn`ed
/// onto the tokio runtime.
pub async fn serve(
    addr: SocketAddr,
    engine_tx: XSender<EngineMsg>,
    registry: Arc<ConnRegistry>,
    next_conn_id: Arc<ConnIdAllocator>,
    md_bus: MarketDataBus,
) -> anyhow::Result<()> {
    let svc = TradingServiceImpl::new(engine_tx, registry, next_conn_id, md_bus);
    info!(%addr, "gRPC TradingService listening");
    TonicServer::builder()
        .add_service(TradingServiceServer::new(svc))
        .serve(addr)
        .await?;
    Ok(())
}

// -----------------------------------------------------------------------------
// Conversions: prost-generated <-> internal protocol structs
// -----------------------------------------------------------------------------

fn pb_to_new_order(o: &pb::NewOrder) -> Result<NewOrderBody, Status> {
    let side = match pb::Side::try_from(o.side) {
        Ok(pb::Side::Buy) => Side::Buy,
        Ok(pb::Side::Sell) => Side::Sell,
        _ => return Err(Status::invalid_argument("side must be BUY or SELL")),
    };
    let ord_type = match pb::OrdType::try_from(o.ord_type) {
        Ok(pb::OrdType::Limit) => OrdType::Limit,
        Ok(pb::OrdType::Market) => OrdType::Market,
        _ => return Err(Status::invalid_argument("ord_type must be LIMIT or MARKET")),
    };
    let tif = match pb::Tif::try_from(o.tif) {
        Ok(pb::Tif::Day) => Tif::Day,
        Ok(pb::Tif::Ioc) => Tif::Ioc,
        Ok(pb::Tif::Fok) => Tif::Fok,
        _ => return Err(Status::invalid_argument("tif must be DAY, IOC, or FOK")),
    };
    Ok(NewOrderBody {
        price: o.price,
        qty: o.qty,
        client_id: o.client_id,
        order_id: o.order_id,
        symbol_id: o.symbol_id,
        side: side as u8,
        ord_type: ord_type as u8,
        tif: tif as u8,
        _pad: 0,
        _pad2: 0,
        _pad3: 0,
    })
}

fn pb_to_cancel(c: &pb::CancelOrder) -> CancelOrderBody {
    CancelOrderBody {
        order_id: c.order_id,
        symbol_id: c.symbol_id,
        _pad: 0,
    }
}

fn exec_to_pb(e: &ExecReportBody) -> ExecutionReport {
    let status = match e.status {
        x if x == ExecStatus::New as u8 => pb::ExecStatus::New,
        x if x == ExecStatus::PartiallyFilled as u8 => pb::ExecStatus::PartiallyFilled,
        x if x == ExecStatus::Filled as u8 => pb::ExecStatus::Filled,
        x if x == ExecStatus::Cancelled as u8 => pb::ExecStatus::Cancelled,
        _ => pb::ExecStatus::Rejected,
    };
    let side = match e.side {
        x if x == Side::Buy as u8 => pb::Side::Buy,
        x if x == Side::Sell as u8 => pb::Side::Sell,
        _ => pb::Side::Unspecified,
    };
    ExecutionReport {
        order_id: e.order_id,
        exec_id: e.exec_id,
        symbol_id: e.symbol_id,
        status: status as i32,
        side: side as i32,
        last_price: e.last_price,
        last_qty: e.last_qty,
        leaves_qty: e.leaves_qty,
    }
}

fn md_event_to_pb(ev: MarketDataEvent) -> MarketDataUpdate {
    use pb::market_data_update::Event;
    let event = match ev {
        MarketDataEvent::Trade {
            symbol_id,
            price,
            qty,
            ts_ns,
        } => Event::Trade(pb::TradeEvent {
            symbol_id,
            price,
            qty,
            ts_ns,
        }),
        MarketDataEvent::Quote {
            symbol_id,
            bid_price,
            bid_qty,
            ask_price,
            ask_qty,
            ts_ns,
        } => Event::Quote(pb::QuoteEvent {
            symbol_id,
            bid_price,
            bid_qty,
            ask_price,
            ask_qty,
            ts_ns,
        }),
        MarketDataEvent::BookSnapshot { symbol_id, ts_ns } => {
            Event::Snapshot(pb::BookSnapshotEvent { symbol_id, ts_ns })
        }
    };
    MarketDataUpdate { event: Some(event) }
}

/// Decode a QFTX-framed exec report (header + 48-byte body) into the
/// protobuf type, dropping the header. The dispatcher always emits
/// fixed-size single-frame payloads so this is straightforward.
fn frame_to_exec_pb(bytes: &[u8]) -> Option<ExecutionReport> {
    if bytes.len() != HEADER_SIZE + EXEC_REPORT_SIZE {
        return None;
    }
    let body = ExecReportBody::read_from(&bytes[HEADER_SIZE..])?;
    Some(exec_to_pb(&body))
}

async fn submit_order(engine_tx: &XSender<EngineMsg>, msg: EngineMsg) -> Result<(), Status> {
    // Async-friendly back-pressure: try_send and yield briefly on Full.
    let mut to_send = msg;
    for _ in 0..200 {
        match engine_tx.try_send(to_send) {
            Ok(()) => return Ok(()),
            Err(crossbeam_channel::TrySendError::Full(m)) => {
                to_send = m;
                tokio::time::sleep(std::time::Duration::from_micros(50)).await;
            }
            Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
                return Err(Status::unavailable("engine shut down"));
            }
        }
    }
    Err(Status::resource_exhausted("engine inbox saturated"))
}

/// True only for fully-terminal statuses — used by unary `PlaceOrder` to
/// know when to stop draining. `PartiallyFilled` is **not** terminal in
/// the middle of a multi-fill order; we treat it as terminal only when
/// `leaves_qty == 0` (handled in the call site).
#[inline]
fn is_done(status: i32, leaves_qty: u64) -> bool {
    match pb::ExecStatus::try_from(status) {
        Ok(pb::ExecStatus::Filled)
        | Ok(pb::ExecStatus::Cancelled)
        | Ok(pb::ExecStatus::Rejected)
        | Ok(pb::ExecStatus::New) => true,
        Ok(pb::ExecStatus::PartiallyFilled) => leaves_qty == 0,
        _ => false,
    }
}

// -----------------------------------------------------------------------------
// TradingService impl
// -----------------------------------------------------------------------------

type OrderSessionStream =
    Pin<Box<dyn Stream<Item = Result<ServerMessage, Status>> + Send + 'static>>;

type SubscribeMdStream =
    Pin<Box<dyn Stream<Item = Result<MarketDataUpdate, Status>> + Send + 'static>>;

#[tonic::async_trait]
impl TradingService for TradingServiceImpl {
    // -------------------------------------------------------------------------
    // PlaceOrder (unary) — returns every exec report for the submitted order
    // up to and including the terminal one.
    // -------------------------------------------------------------------------
    async fn place_order(
        &self,
        request: Request<PlaceOrderRequest>,
    ) -> Result<Response<PlaceOrderResponse>, Status> {
        let req = request.into_inner();
        let order_pb = req
            .order
            .ok_or_else(|| Status::invalid_argument("order is required"))?;
        let order = pb_to_new_order(&order_pb)?;
        let order_id = order.order_id;

        let conn_id = self.next_conn_id.allocate();
        let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
        self.registry.register(conn_id, ConnSender::Async(tx));

        // Submit to the engine.
        submit_order(
            &self.engine_tx,
            EngineMsg::Order {
                conn_id,
                msg: InboundMessage::NewOrder {
                    header: synthetic_header(protocol::MsgType::NewOrder, 0),
                    body: order,
                },
            },
        )
        .await?;

        // Collect reports until we see a terminal one. Apply a soft
        // deadline so misbehaving clients can't hold a registration
        // forever.
        let mut reports = Vec::with_capacity(4);
        let deadline =
            tokio::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            match tokio::time::timeout(remaining, rx.recv()).await {
                Ok(Some(bytes)) => {
                    let Some(pb_exec) = frame_to_exec_pb(&bytes) else {
                        continue;
                    };
                    if pb_exec.order_id != order_id {
                        continue;
                    }
                    let done = is_done(pb_exec.status, pb_exec.leaves_qty);
                    reports.push(pb_exec);
                    if done {
                        break;
                    }
                }
                Ok(None) | Err(_) => break,
            }
        }
        self.registry.remove(conn_id);
        Ok(Response::new(PlaceOrderResponse { reports }))
    }

    // -------------------------------------------------------------------------
    // CancelOrder (unary)
    // -------------------------------------------------------------------------
    async fn cancel_order(
        &self,
        request: Request<CancelOrderRequest>,
    ) -> Result<Response<CancelOrderResponse>, Status> {
        let req = request.into_inner();
        let cancel_pb = req
            .cancel
            .ok_or_else(|| Status::invalid_argument("cancel is required"))?;
        let cancel = pb_to_cancel(&cancel_pb);
        let order_id = cancel.order_id;

        let conn_id = self.next_conn_id.allocate();
        let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
        self.registry.register(conn_id, ConnSender::Async(tx));

        submit_order(
            &self.engine_tx,
            EngineMsg::Order {
                conn_id,
                msg: InboundMessage::Cancel {
                    header: synthetic_header(protocol::MsgType::CancelOrder, 0),
                    body: cancel,
                },
            },
        )
        .await?;

        let report = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
            .await
            .ok()
            .flatten()
            .and_then(|b| frame_to_exec_pb(&b))
            .filter(|r| r.order_id == order_id);
        self.registry.remove(conn_id);
        let report = report.ok_or_else(|| Status::deadline_exceeded("no cancel ack"))?;
        Ok(Response::new(CancelOrderResponse {
            report: Some(report),
        }))
    }

    // -------------------------------------------------------------------------
    // OrderSession (bidi streaming)
    // -------------------------------------------------------------------------
    type OrderSessionStream = OrderSessionStream;

    async fn order_session(
        &self,
        request: Request<Streaming<ClientMessage>>,
    ) -> Result<Response<Self::OrderSessionStream>, Status> {
        let conn_id = self.next_conn_id.allocate();
        let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
        self.registry.register(conn_id, ConnSender::Async(tx));
        info!(conn = conn_id, "gRPC OrderSession opened");

        let engine_tx = self.engine_tx.clone();
        let registry = Arc::clone(&self.registry);
        let mut inbound = request.into_inner();

        // Reader task: consume the client's request stream, submit each
        // message to the engine.
        tokio::spawn(async move {
            while let Some(msg) = inbound.next().await {
                match msg {
                    Ok(ClientMessage { body: Some(body) }) => {
                        let submission = match body {
                            pb::client_message::Body::NewOrder(o) => match pb_to_new_order(&o) {
                                Ok(body) => EngineMsg::Order {
                                    conn_id,
                                    msg: InboundMessage::NewOrder {
                                        header: synthetic_header(
                                            protocol::MsgType::NewOrder,
                                            0,
                                        ),
                                        body,
                                    },
                                },
                                Err(e) => {
                                    warn!(conn = conn_id, error = %e, "bad new order");
                                    continue;
                                }
                            },
                            pb::client_message::Body::Cancel(c) => EngineMsg::Order {
                                conn_id,
                                msg: InboundMessage::Cancel {
                                    header: synthetic_header(
                                        protocol::MsgType::CancelOrder,
                                        0,
                                    ),
                                    body: pb_to_cancel(&c),
                                },
                            },
                        };
                        if submit_order(&engine_tx, submission).await.is_err() {
                            break;
                        }
                    }
                    Ok(_) => {} // unknown body, ignore
                    Err(e) => {
                        debug!(conn = conn_id, error = %e, "client stream error");
                        break;
                    }
                }
            }
            registry.remove(conn_id);
            debug!(conn = conn_id, "gRPC OrderSession reader exited");
        });

        // Writer side: turn outbound QFTX bytes into ServerMessage protos.
        let _ = PER_STREAM_QUEUE; // hint that bounded channels exist for tuning
        let out: OrderSessionStream = Box::pin(async_stream::stream! {
            while let Some(bytes) = rx.recv().await {
                if let Some(exec) = frame_to_exec_pb(&bytes) {
                    yield Ok(ServerMessage {
                        body: Some(pb::server_message::Body::Exec(exec)),
                    });
                }
            }
        });
        Ok(Response::new(out))
    }

    // -------------------------------------------------------------------------
    // SubscribeMarketData (server streaming)
    // -------------------------------------------------------------------------
    type SubscribeMarketDataStream = SubscribeMdStream;

    async fn subscribe_market_data(
        &self,
        request: Request<SubscribeMarketDataRequest>,
    ) -> Result<Response<Self::SubscribeMarketDataStream>, Status> {
        let req = request.into_inner();
        let filter: Option<rustc_hash::FxHashSet<u32>> = if req.symbol_ids.is_empty() {
            None
        } else {
            Some(req.symbol_ids.iter().copied().collect())
        };
        let mut rx = self.md_bus.subscribe();
        let out: SubscribeMdStream = Box::pin(async_stream::stream! {
            loop {
                match rx.recv().await {
                    Ok(ev) => {
                        if let Some(f) = &filter {
                            if !f.contains(&ev.symbol_id()) {
                                continue;
                            }
                        }
                        yield Ok(md_event_to_pb(ev));
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        warn!(skipped = n, "market data subscriber lagged");
                        continue;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });
        Ok(Response::new(out))
    }
}

/// Synthetic QFTX header used only as a placeholder inside `EngineMsg`
/// for gRPC/WS submissions. The engine reads the body, not the header,
/// for everything that matters; we fill `seq` and `timestamp_ns` with
/// zeros and let the dispatcher generate its own outbound headers.
fn synthetic_header(msg_type: protocol::MsgType, body_len: u32) -> protocol::Header {
    protocol::Header {
        seq: 0,
        timestamp_ns: 0,
        magic: protocol::MAGIC,
        length: body_len,
        version: protocol::VERSION,
        msg_type: msg_type as u8,
        _pad: [0; 6],
    }
}
