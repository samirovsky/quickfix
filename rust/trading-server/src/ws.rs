//! WebSocket transport.
//!
//! One WebSocket connection = one trading session. Frames are the
//! byte-identical QFTX binary frames documented in
//! `docs/api/binary-protocol.md`. Each WS **binary** message contains
//! exactly one QFTX frame (32-byte header + body). Multi-frame messages
//! and **text** messages are rejected with close code 1003.
//!
//! Clients are expected to negotiate subprotocol `qftx-binary.v1` during
//! the handshake; this server advertises it and prefers it, but for
//! the demo it does not reject clients that omit a subprotocol — that
//! lets the browser examples work without extra ceremony.

use std::net::SocketAddr;
use std::sync::Arc;

use crossbeam_channel::Sender as XSender;
use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::protocol::{frame::coding::CloseCode, CloseFrame, Message};
use tracing::{debug, info, warn};

use crate::engine::EngineMsg;
use crate::error::Error;
use crate::protocol::{self, HEADER_SIZE};
use crate::server::{ConnIdAllocator, ConnRegistry, ConnSender};
use crate::ConnId;

pub const SUBPROTOCOL: &str = "qftx-binary.v1";

pub async fn serve(
    addr: SocketAddr,
    engine_tx: XSender<EngineMsg>,
    registry: Arc<ConnRegistry>,
    next_conn_id: Arc<ConnIdAllocator>,
) -> anyhow::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    info!(%addr, "WebSocket listening (subprotocol={SUBPROTOCOL})");
    loop {
        let (sock, peer) = match listener.accept().await {
            Ok(p) => p,
            Err(e) => {
                warn!(error = %e, "ws accept failed");
                continue;
            }
        };
        let id = next_conn_id.allocate();
        let engine_tx = engine_tx.clone();
        let registry = Arc::clone(&registry);
        tokio::spawn(async move {
            if let Err(e) = handle_conn(id, sock, engine_tx, registry).await {
                debug!(conn = id, error = %e, peer = ?peer, "ws session ended");
            }
        });
    }
}

async fn handle_conn(
    conn_id: ConnId,
    sock: TcpStream,
    engine_tx: XSender<EngineMsg>,
    registry: Arc<ConnRegistry>,
) -> Result<(), Error> {
    sock.set_nodelay(true).ok();
    let ws = match tokio_tungstenite::accept_hdr_async(
        sock,
        |req: &tokio_tungstenite::tungstenite::handshake::server::Request, mut resp: tokio_tungstenite::tungstenite::handshake::server::Response| {
            // RFC 6455: only echo Sec-WebSocket-Protocol when the client
            // offered one we recognise. Sending it unconditionally trips
            // strict clients (incl. tungstenite).
            let client_offered = req
                .headers()
                .get_all(http::header::SEC_WEBSOCKET_PROTOCOL)
                .iter()
                .filter_map(|v| v.to_str().ok())
                .any(|raw| raw.split(',').any(|s| s.trim() == SUBPROTOCOL));
            if client_offered {
                resp.headers_mut().insert(
                    http::header::SEC_WEBSOCKET_PROTOCOL,
                    http::HeaderValue::from_static(SUBPROTOCOL),
                );
            }
            Ok(resp)
        },
    )
    .await
    {
        Ok(ws) => ws,
        Err(e) => return Err(Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e))),
    };
    info!(conn = conn_id, "WebSocket session opened");

    let (mut sink, mut stream) = ws.split();
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    registry.register(conn_id, ConnSender::Async(out_tx));

    // Writer task: drain outbound bytes from the registry-side channel
    // and ship them as WS binary messages.
    let writer = tokio::spawn(async move {
        while let Some(bytes) = out_rx.recv().await {
            if sink.send(Message::Binary(bytes.into())).await.is_err() {
                break;
            }
        }
        let _ = sink.close().await;
    });

    // Reader: parse each inbound WS binary message as exactly one QFTX
    // frame and push it to the engine.
    let mut close_reason: Option<(CloseCode, &'static str)> = None;
    while let Some(msg) = stream.next().await {
        let msg = match msg {
            Ok(m) => m,
            Err(e) => {
                debug!(conn = conn_id, error = %e, "ws read error");
                break;
            }
        };
        match msg {
            Message::Binary(bytes) => {
                if bytes.len() < HEADER_SIZE {
                    close_reason = Some((CloseCode::Invalid, "frame shorter than header"));
                    break;
                }
                match protocol::parse(&bytes) {
                    Ok((parsed, used)) => {
                        if used != bytes.len() {
                            close_reason =
                                Some((CloseCode::Invalid, "multiple frames per WS message"));
                            break;
                        }
                        let to_send = EngineMsg::Order {
                            conn_id,
                            msg: parsed,
                        };
                        if !submit(&engine_tx, to_send).await {
                            close_reason = Some((CloseCode::Again, "engine unavailable"));
                            break;
                        }
                    }
                    Err(e) => {
                        warn!(conn = conn_id, error = %e, "ws parse error");
                        close_reason = Some((CloseCode::Invalid, "malformed QFTX frame"));
                        break;
                    }
                }
            }
            Message::Text(_) => {
                close_reason = Some((CloseCode::Unsupported, "text frames not supported"));
                break;
            }
            Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => {
                // Tungstenite handles pings/pongs at the protocol layer;
                // these are forwarded for visibility but require no action.
            }
            Message::Close(_) => {
                debug!(conn = conn_id, "client requested close");
                break;
            }
        }
    }
    registry.remove(conn_id);
    let _ = writer.await;
    if let Some((code, reason)) = close_reason {
        debug!(conn = conn_id, ?code, reason, "closing ws");
        // Best-effort: the writer task already closed the sink on exit.
        let _ = CloseFrame {
            code,
            reason: reason.into(),
        };
    }
    Ok(())
}

async fn submit(engine_tx: &XSender<EngineMsg>, msg: EngineMsg) -> bool {
    let mut to_send = msg;
    for _ in 0..200 {
        match engine_tx.try_send(to_send) {
            Ok(()) => return true,
            Err(crossbeam_channel::TrySendError::Full(m)) => {
                to_send = m;
                tokio::time::sleep(std::time::Duration::from_micros(50)).await;
            }
            Err(crossbeam_channel::TrySendError::Disconnected(_)) => return false,
        }
    }
    false
}
