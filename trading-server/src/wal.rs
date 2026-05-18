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

/// A WAL record handed across the channel. Owning the bytes here keeps the
/// writer thread independent of the engine's buffers.
#[derive(Debug, Clone)]
pub struct WalRecord {
    pub record_type: u8,
    pub payload: Vec<u8>,
}

impl WalRecord {
    pub fn new_order(body: NewOrderBody) -> Self {
        Self {
            record_type: RECORD_NEW_ORDER,
            payload: body.as_bytes().to_vec(),
        }
    }

    pub fn cancel(body: CancelOrderBody) -> Self {
        Self {
            record_type: RECORD_CANCEL,
            payload: body.as_bytes().to_vec(),
        }
    }

    pub fn exec_report(body: ExecReportBody) -> Self {
        Self {
            record_type: RECORD_EXEC_REPORT,
            payload: body.as_bytes().to_vec(),
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
        let body_len = 1 + record.payload.len();
        let len = (4 + body_len) as u32; // crc(4) + record_type(1) + payload
        let mut crc = crc32fast::Hasher::new();
        crc.update(std::slice::from_ref(&record.record_type));
        crc.update(&record.payload);
        let crc = crc.finalize();
        self.scratch.extend_from_slice(&len.to_le_bytes());
        self.scratch.extend_from_slice(&crc.to_le_bytes());
        self.scratch.push(record.record_type);
        self.scratch.extend_from_slice(&record.payload);
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

/// Read back every record in a WAL file. Used by tests; not in the hot path.
pub fn read_all(path: impl AsRef<Path>) -> Result<Vec<WalRecord>, Error> {
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
        out.push(WalRecord {
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
            writer.append(&WalRecord::new_order(sample_order(i))).unwrap();
        }
        writer.flush().unwrap();
        drop(writer);

        let records = read_all(&path).unwrap();
        assert_eq!(records.len(), 100);
        for (i, r) in records.iter().enumerate() {
            assert_eq!(r.record_type, RECORD_NEW_ORDER);
            let expected = WalRecord::new_order(sample_order(i as u64));
            assert_eq!(r.payload, expected.payload);
        }
        let _ = std::fs::remove_file(&path);
    }
}
