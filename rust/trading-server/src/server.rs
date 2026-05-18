//! TCP server.
//!
//! Per-connection model: one accept thread, one reader thread per connection
//! (parses framed messages and pushes to the engine), one writer thread per
//! connection (drains outbound bytes to the socket). All thread-to-thread
//! communication uses `crossbeam_channel` so the engine has zero tokio /
//! async overhead on the hot path.
//!
//! We chose `std::net` + `std::thread` over `tokio` deliberately: the engine
//! is a blocking single-threaded loop, and a sync I/O model with one thread
//! per connection is the lowest-overhead way to feed it. This scales to
//! hundreds of clients comfortably — far more than the demo needs.

use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use crossbeam_channel::{Receiver, Sender};
use parking_lot::RwLock;
use rustc_hash::FxHashMap;
use tracing::{debug, info, warn};

use crate::engine::{EngineMsg, OutboundExec};
use crate::error::Error;
use crate::protocol::{self, EXEC_REPORT_SIZE, HEADER_SIZE};
use crate::ConnId;

const READ_BUF_CAP: usize = 64 * 1024;
const PER_CONN_QUEUE: usize = 1024;

/// Per-connection outbound sink.
///
/// `Sync` is used by the std-thread TCP writer (existing fast path).
/// `Async` is used by gRPC stream handlers and per-connection WebSocket
/// tasks living on the tokio runtime — `tokio::sync::mpsc::UnboundedSender`
/// is callable from non-tokio threads and never blocks, so the
/// `exec-dispatcher` thread can push outbound bytes uniformly regardless
/// of which transport the client used.
#[derive(Debug)]
pub enum ConnSender {
    Sync(Sender<Vec<u8>>),
    Async(tokio::sync::mpsc::UnboundedSender<Vec<u8>>),
}

impl ConnSender {
    #[inline]
    pub fn try_send(&self, bytes: Vec<u8>) -> bool {
        match self {
            ConnSender::Sync(tx) => tx.try_send(bytes).is_ok(),
            ConnSender::Async(tx) => tx.send(bytes).is_ok(),
        }
    }
}

#[derive(Debug, Default)]
pub struct ConnRegistry {
    inner: RwLock<FxHashMap<ConnId, ConnSender>>,
}

impl ConnRegistry {
    pub fn register(&self, id: ConnId, sender: ConnSender) {
        self.inner.write().insert(id, sender);
    }

    pub fn remove(&self, id: ConnId) {
        self.inner.write().remove(&id);
    }

    #[inline]
    pub fn try_send(&self, id: ConnId, bytes: Vec<u8>) -> bool {
        let guard = self.inner.read();
        if let Some(tx) = guard.get(&id) {
            tx.try_send(bytes)
        } else {
            false
        }
    }
}

/// Monotonic connection-id source shared across every transport (TCP,
/// gRPC, WebSocket) so the engine sees a single global ConnId space.
#[derive(Debug, Default)]
pub struct ConnIdAllocator(AtomicU64);

impl ConnIdAllocator {
    pub fn new() -> Self {
        Self(AtomicU64::new(1))
    }

    #[inline]
    pub fn allocate(&self) -> ConnId {
        self.0.fetch_add(1, Ordering::Relaxed)
    }
}

pub struct Server {
    listener: TcpListener,
    engine_tx: Sender<EngineMsg>,
    registry: Arc<ConnRegistry>,
    next_conn_id: Arc<ConnIdAllocator>,
    running: Arc<AtomicBool>,
}

impl std::fmt::Debug for Server {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Server")
            .field("local_addr", &self.listener.local_addr().ok())
            .finish()
    }
}

impl Server {
    pub fn bind(
        addr: SocketAddr,
        engine_tx: Sender<EngineMsg>,
        registry: Arc<ConnRegistry>,
        next_conn_id: Arc<ConnIdAllocator>,
    ) -> Result<Self, Error> {
        let listener = TcpListener::bind(addr)?;
        Ok(Self {
            listener,
            engine_tx,
            registry,
            next_conn_id,
            running: Arc::new(AtomicBool::new(true)),
        })
    }

    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    pub fn shutdown_handle(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.running)
    }

    /// Run the accept loop on the calling thread until `shutdown_handle()`
    /// is set to `false`.
    pub fn run(self) -> Result<(), Error> {
        info!(addr = ?self.listener.local_addr(), "trading server listening");
        // Non-blocking accept so we can poll `running`.
        self.listener.set_nonblocking(true)?;
        loop {
            if !self.running.load(Ordering::Relaxed) {
                return Ok(());
            }
            match self.listener.accept() {
                Ok((sock, peer)) => {
                    let id = self.next_conn_id.allocate();
                    info!(conn = id, ?peer, "client connected");
                    sock.set_nodelay(true).ok();
                    sock.set_nonblocking(false).ok();
                    let (out_tx, out_rx) = crossbeam_channel::bounded(PER_CONN_QUEUE);
                    self.registry.register(id, ConnSender::Sync(out_tx));
                    let reader_sock = sock.try_clone()?;
                    let engine_tx = self.engine_tx.clone();
                    let registry = Arc::clone(&self.registry);
                    let registry_for_writer = Arc::clone(&self.registry);
                    thread::Builder::new()
                        .name(format!("conn-r-{id}"))
                        .spawn(move || {
                            if let Err(e) = run_reader(id, reader_sock, engine_tx) {
                                debug!(conn = id, error = %e, "reader exited");
                            }
                            registry.remove(id);
                        })?;
                    thread::Builder::new()
                        .name(format!("conn-w-{id}"))
                        .spawn(move || {
                            if let Err(e) = run_writer(id, sock, out_rx) {
                                debug!(conn = id, error = %e, "writer exited");
                            }
                            registry_for_writer.remove(id);
                        })?;
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => {
                    thread::sleep(std::time::Duration::from_millis(1));
                }
                Err(e) => {
                    warn!(error = %e, "accept failed");
                    return Err(e.into());
                }
            }
        }
    }
}

fn run_reader(
    conn_id: ConnId,
    mut sock: TcpStream,
    engine_tx: Sender<EngineMsg>,
) -> Result<(), Error> {
    let mut buf = vec![0u8; READ_BUF_CAP];
    let mut filled = 0usize;
    loop {
        if filled == buf.len() {
            // Frame too large for the buffer; defensively reset.
            warn!(conn = conn_id, "frame exceeds buffer, dropping");
            filled = 0;
        }
        let n = sock.read(&mut buf[filled..])?;
        if n == 0 {
            return Ok(());
        }
        filled += n;
        let mut consumed = 0;
        while filled - consumed >= HEADER_SIZE {
            match protocol::parse(&buf[consumed..filled]) {
                Ok((msg, used)) => {
                    let mut to_send = EngineMsg::Order { conn_id, msg };
                    loop {
                        match engine_tx.try_send(to_send) {
                            Ok(()) => break,
                            Err(crossbeam_channel::TrySendError::Full(m)) => {
                                to_send = m;
                                thread::sleep(std::time::Duration::from_micros(50));
                            }
                            Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
                                return Ok(());
                            }
                        }
                    }
                    consumed += used;
                }
                Err(Error::ShortBuffer { .. }) => break,
                Err(e) => {
                    warn!(conn = conn_id, error = %e, "parse error, closing connection");
                    let _ = sock.shutdown(Shutdown::Both);
                    return Ok(());
                }
            }
        }
        if consumed > 0 {
            buf.copy_within(consumed..filled, 0);
            filled -= consumed;
        }
    }
}

fn run_writer(
    _conn_id: ConnId,
    mut sock: TcpStream,
    out_rx: Receiver<Vec<u8>>,
) -> Result<(), Error> {
    while let Ok(bytes) = out_rx.recv() {
        sock.write_all(&bytes)?;
    }
    Ok(())
}

/// Dispatcher thread: drains `OutboundExec`s from the engine, encodes each
/// to bytes, and routes to the right connection writer via the registry.
///
/// Kept separate from the engine so the engine thread never does syscalls,
/// never touches the registry's `RwLock`, and never allocates per fill.
pub fn spawn_exec_dispatcher(
    exec_rx: Receiver<OutboundExec>,
    registry: Arc<ConnRegistry>,
) -> JoinHandle<()> {
    thread::Builder::new()
        .name("exec-dispatcher".into())
        .spawn(move || {
            let mut seq = 0u64;
            while let Ok(out) = exec_rx.recv() {
                seq = seq.wrapping_add(1);
                if out.conn_id == 0 {
                    continue;
                }
                let mut buf = Vec::with_capacity(HEADER_SIZE + EXEC_REPORT_SIZE);
                protocol::encode_exec_report(seq, &out.body, &mut buf);
                let _ = registry.try_send(out.conn_id, buf);
            }
        })
        .expect("spawn dispatcher")
}
