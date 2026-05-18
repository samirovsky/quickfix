//! Write-ahead log.
//!
//! The hot path enqueues records on a bounded `crossbeam_channel`; a
//! dedicated writer thread drains the channel and appends to disk. The
//! engine **never blocks** on this channel: on `Full` it counts a drop and
//! moves on. That trade-off (availability over in-flight durability) is
//! explicit and documented in the plan.
//!
//! Record format on disk:
//!
//! ```text
//! len:u32_le | crc32:u32_le | record_type:u8 | payload[..]
//! ```
//!
//! `payload` is the raw wire-format body bytes from `protocol.rs`. The CRC
//! covers `record_type || payload`. `len` is the number of bytes after the
//! length field itself (i.e. `4 + 1 + payload.len()`).

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, RecvTimeoutError};
use zerocopy::AsBytes;

use crate::error::Error;
use crate::protocol::{CancelOrderBody, ExecReportBody, NewOrderBody};

pub const RECORD_NEW_ORDER: u8 = 1;
pub const RECORD_CANCEL: u8 = 2;
pub const RECORD_EXEC_REPORT: u8 = 3;

const FLUSH_INTERVAL: Duration = Duration::from_micros(100);
const FLUSH_BYTES: usize = 4 * 1024;
const SCRATCH_CAP: usize = 64 * 1024;

/// A WAL record handed across the channel.
///
/// Stored as a `Copy` enum of fixed-size POD bodies — no heap allocation
/// when the engine constructs or sends one. Encoding to bytes happens on
/// the WAL writer thread, off the hot path.
#[derive(Debug, Clone, Copy)]
pub enum WalRecord {
    NewOrder(NewOrderBody),
    Cancel(CancelOrderBody),
    ExecReport(ExecReportBody),
}

impl WalRecord {
    #[inline]
    pub fn new_order(body: NewOrderBody) -> Self {
        WalRecord::NewOrder(body)
    }

    #[inline]
    pub fn cancel(body: CancelOrderBody) -> Self {
        WalRecord::Cancel(body)
    }

    #[inline]
    pub fn exec_report(body: ExecReportBody) -> Self {
        WalRecord::ExecReport(body)
    }

    #[inline]
    pub fn record_type(&self) -> u8 {
        match self {
            WalRecord::NewOrder(_) => RECORD_NEW_ORDER,
            WalRecord::Cancel(_) => RECORD_CANCEL,
            WalRecord::ExecReport(_) => RECORD_EXEC_REPORT,
        }
    }

    fn payload_bytes(&self) -> &[u8] {
        match self {
            WalRecord::NewOrder(b) => b.as_bytes(),
            WalRecord::Cancel(b) => b.as_bytes(),
            WalRecord::ExecReport(b) => b.as_bytes(),
        }
    }
}

pub struct WalWriter {
    file: BufWriter<File>,
    scratch: Vec<u8>,
    last_flush: Instant,
    pending_bytes: usize,
}

impl WalWriter {
    pub fn create(path: impl AsRef<Path>) -> Result<Self, Error> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path.as_ref())?;
        Ok(Self {
            file: BufWriter::with_capacity(SCRATCH_CAP, file),
            scratch: Vec::with_capacity(SCRATCH_CAP),
            last_flush: Instant::now(),
            pending_bytes: 0,
        })
    }

    pub fn append(&mut self, record: &WalRecord) -> Result<(), Error> {
        self.scratch.clear();
        let record_type = record.record_type();
        let payload = record.payload_bytes();
        let body_len = 1 + payload.len();
        let len = (4 + body_len) as u32; // crc(4) + record_type(1) + payload
        let mut crc = crc32fast::Hasher::new();
        crc.update(std::slice::from_ref(&record_type));
        crc.update(payload);
        let crc = crc.finalize();
        self.scratch.extend_from_slice(&len.to_le_bytes());
        self.scratch.extend_from_slice(&crc.to_le_bytes());
        self.scratch.push(record_type);
        self.scratch.extend_from_slice(payload);
        self.file.write_all(&self.scratch)?;
        self.pending_bytes += self.scratch.len();
        Ok(())
    }

    pub fn maybe_flush(&mut self) -> Result<(), Error> {
        if self.pending_bytes >= FLUSH_BYTES || self.last_flush.elapsed() >= FLUSH_INTERVAL {
            self.flush()?;
        }
        Ok(())
    }

    pub fn flush(&mut self) -> Result<(), Error> {
        self.file.flush()?;
        self.pending_bytes = 0;
        self.last_flush = Instant::now();
        Ok(())
    }
}

impl std::fmt::Debug for WalWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WalWriter")
            .field("pending_bytes", &self.pending_bytes)
            .finish()
    }
}

/// Spawn the writer thread. Returns a handle that can be `join()`ed on
/// shutdown. The thread exits when the channel is disconnected.
pub fn spawn(
    path: PathBuf,
    inbox: Receiver<WalRecord>,
) -> std::thread::JoinHandle<Result<(), Error>> {
    std::thread::Builder::new()
        .name("wal-writer".into())
        .spawn(move || run(path, inbox))
        .expect("spawn wal writer")
}

fn run(path: PathBuf, inbox: Receiver<WalRecord>) -> Result<(), Error> {
    let mut writer = WalWriter::create(&path)?;
    loop {
        match inbox.recv_timeout(FLUSH_INTERVAL) {
            Ok(rec) => {
                writer.append(&rec)?;
                writer.maybe_flush()?;
            }
            Err(RecvTimeoutError::Timeout) => {
                writer.maybe_flush()?;
            }
            Err(RecvTimeoutError::Disconnected) => {
                writer.flush()?;
                return Ok(());
            }
        }
    }
}

/// A raw record decoded from disk. Returned by `read_all` for tests and
/// recovery tooling — not used on the hot path.
#[derive(Debug, Clone)]
pub struct DecodedRecord {
    pub record_type: u8,
    pub payload: Vec<u8>,
}

pub fn read_all(path: impl AsRef<Path>) -> Result<Vec<DecodedRecord>, Error> {
    use std::io::Read;
    let mut f = File::open(path)?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;
    let mut out = Vec::new();
    let mut i = 0;
    while i < buf.len() {
        if buf.len() - i < 4 {
            break;
        }
        let len = u32::from_le_bytes(buf[i..i + 4].try_into().unwrap()) as usize;
        i += 4;
        if buf.len() - i < len {
            break;
        }
        let crc = u32::from_le_bytes(buf[i..i + 4].try_into().unwrap());
        let record_type = buf[i + 4];
        let payload = buf[i + 5..i + len].to_vec();
        let mut h = crc32fast::Hasher::new();
        h.update(std::slice::from_ref(&record_type));
        h.update(&payload);
        let actual = h.finalize();
        debug_assert_eq!(crc, actual, "wal crc mismatch");
        out.push(DecodedRecord {
            record_type,
            payload,
        });
        i += len;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{NewOrderBody, OrdType, Side, Tif};

    fn tempfile_path(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("trading-server-wal-{}-{}.log", name, std::process::id()));
        let _ = std::fs::remove_file(&p);
        p
    }

    fn sample_order(id: u64) -> NewOrderBody {
        NewOrderBody {
            price: 100,
            qty: 10,
            client_id: 1,
            order_id: id,
            symbol_id: 1,
            side: Side::Buy as u8,
            ord_type: OrdType::Limit as u8,
            tif: Tif::Day as u8,
            _pad: 0,
            _pad2: 0,
            _pad3: 0,
        }
    }

    #[test]
    fn roundtrip_writes_and_reads_back_byte_equal() {
        let path = tempfile_path("roundtrip");
        let mut writer = WalWriter::create(&path).unwrap();
        for i in 0..100 {
            writer
                .append(&WalRecord::new_order(sample_order(i)))
                .unwrap();
        }
        writer.flush().unwrap();
        drop(writer);

        let records = read_all(&path).unwrap();
        assert_eq!(records.len(), 100);
        for (i, r) in records.iter().enumerate() {
            assert_eq!(r.record_type, RECORD_NEW_ORDER);
            let expected_body = sample_order(i as u64);
            assert_eq!(r.payload, expected_body.as_bytes());
        }
        let _ = std::fs::remove_file(&path);
    }
}
