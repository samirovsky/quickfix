//! Custom binary wire protocol.
//!
//! Layout is fixed and little-endian. Parsing copies into a `#[repr(C)]`
//! struct via `zerocopy::FromBytes::read_from` — no heap allocations, no
//! string parsing, and no dependency on TCP buffer alignment.

use std::time::{SystemTime, UNIX_EPOCH};

use zerocopy::{AsBytes, FromBytes, FromZeroes};

use crate::error::Error;

/// `QFTX` in ASCII, little-endian.
pub const MAGIC: u32 = u32::from_le_bytes(*b"QFTX");
pub const VERSION: u8 = 1;
pub const HEADER_SIZE: usize = 32;
pub const NEW_ORDER_SIZE: usize = 48;
pub const CANCEL_ORDER_SIZE: usize = 16;
pub const EXEC_REPORT_SIZE: usize = 48;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MsgType {
    NewOrder = 1,
    CancelOrder = 2,
    ExecutionReport = 3,
    Heartbeat = 4,
}

impl MsgType {
    pub fn from_u8(v: u8) -> Result<Self, Error> {
        Ok(match v {
            1 => MsgType::NewOrder,
            2 => MsgType::CancelOrder,
            3 => MsgType::ExecutionReport,
            4 => MsgType::Heartbeat,
            _ => return Err(Error::UnknownMsgType(v)),
        })
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Buy = 0,
    Sell = 1,
}

impl Side {
    #[inline]
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Side::Buy),
            1 => Some(Side::Sell),
            _ => None,
        }
    }

    #[inline]
    pub fn opposite(self) -> Self {
        match self {
            Side::Buy => Side::Sell,
            Side::Sell => Side::Buy,
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrdType {
    Limit = 0,
    Market = 1,
}

impl OrdType {
    #[inline]
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(OrdType::Limit),
            1 => Some(OrdType::Market),
            _ => None,
        }
    }
}

/// Time-in-force.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tif {
    Day = 0,
    Ioc = 1,
    Fok = 2,
}

impl Tif {
    #[inline]
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Tif::Day),
            1 => Some(Tif::Ioc),
            2 => Some(Tif::Fok),
            _ => None,
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecStatus {
    New = 0,
    PartiallyFilled = 1,
    Filled = 2,
    Cancelled = 3,
    Rejected = 4,
}

// All wire structs put 8-byte fields first so that `#[repr(C)]` yields zero
// internal compiler padding. `zerocopy::AsBytes` refuses to derive when a
// struct has padding bytes, so the field order matters.

#[repr(C)]
#[derive(Debug, Clone, Copy, FromBytes, FromZeroes, AsBytes)]
pub struct Header {
    pub seq: u64,
    pub timestamp_ns: u64,
    pub magic: u32,
    pub length: u32,
    pub version: u8,
    pub msg_type: u8,
    pub _pad: [u8; 6],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, FromBytes, FromZeroes, AsBytes)]
pub struct NewOrderBody {
    pub price: i64,
    pub qty: u64,
    pub client_id: u64,
    pub order_id: u64,
    pub symbol_id: u32,
    pub side: u8,
    pub ord_type: u8,
    pub tif: u8,
    pub _pad: u8,
    pub _pad2: u32,
    pub _pad3: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, FromBytes, FromZeroes, AsBytes)]
pub struct CancelOrderBody {
    pub order_id: u64,
    pub symbol_id: u32,
    pub _pad: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, FromBytes, FromZeroes, AsBytes)]
pub struct ExecReportBody {
    pub order_id: u64,
    pub exec_id: u64,
    pub last_price: i64,
    pub last_qty: u64,
    pub leaves_qty: u64,
    pub symbol_id: u32,
    pub status: u8,
    pub side: u8,
    pub _pad: [u8; 2],
}

const _: () = {
    assert!(std::mem::size_of::<Header>() == HEADER_SIZE);
    assert!(std::mem::size_of::<NewOrderBody>() == NEW_ORDER_SIZE);
    assert!(std::mem::size_of::<CancelOrderBody>() == CANCEL_ORDER_SIZE);
    assert!(std::mem::size_of::<ExecReportBody>() == EXEC_REPORT_SIZE);
};

/// A parsed inbound message. Bodies are copied out of the buffer, so the
/// returned value does not borrow from `bytes` and is cheap to forward across
/// a channel.
#[derive(Debug, Clone)]
pub enum InboundMessage {
    NewOrder { header: Header, body: NewOrderBody },
    Cancel { header: Header, body: CancelOrderBody },
    Heartbeat { header: Header },
}

/// Parse one framed message from the front of `bytes`. Returns the parsed
/// message and the total number of bytes it consumed (header + body).
pub fn parse(bytes: &[u8]) -> Result<(InboundMessage, usize), Error> {
    if bytes.len() < HEADER_SIZE {
        return Err(Error::ShortBuffer {
            need: HEADER_SIZE,
            have: bytes.len(),
        });
    }
    let header = Header::read_from(&bytes[..HEADER_SIZE]).expect("size checked");
    if header.magic != MAGIC {
        return Err(Error::BadMagic {
            expected: MAGIC,
            got: header.magic,
        });
    }
    if header.version != VERSION {
        return Err(Error::BadVersion(header.version));
    }
    let body_len = header.length as usize;
    let total = HEADER_SIZE + body_len;
    if bytes.len() < total {
        return Err(Error::ShortBuffer {
            need: total,
            have: bytes.len(),
        });
    }
    let body_bytes = &bytes[HEADER_SIZE..total];
    let msg = match MsgType::from_u8(header.msg_type)? {
        MsgType::NewOrder => {
            if body_len != NEW_ORDER_SIZE {
                return Err(Error::LengthMismatch {
                    header: header.length,
                    body: NEW_ORDER_SIZE,
                });
            }
            let body = NewOrderBody::read_from(body_bytes).expect("size checked");
            InboundMessage::NewOrder { header, body }
        }
        MsgType::CancelOrder => {
            if body_len != CANCEL_ORDER_SIZE {
                return Err(Error::LengthMismatch {
                    header: header.length,
                    body: CANCEL_ORDER_SIZE,
                });
            }
            let body = CancelOrderBody::read_from(body_bytes).expect("size checked");
            InboundMessage::Cancel { header, body }
        }
        MsgType::Heartbeat => {
            if body_len != 0 {
                return Err(Error::LengthMismatch {
                    header: header.length,
                    body: 0,
                });
            }
            InboundMessage::Heartbeat { header }
        }
        MsgType::ExecutionReport => {
            return Err(Error::UnknownMsgType(header.msg_type));
        }
    };
    Ok((msg, total))
}

/// Build a `NewOrder` frame into `out`, returning the number of bytes
/// written. `out` must have capacity for at least `HEADER_SIZE + NEW_ORDER_SIZE`.
fn make_header(seq: u64, msg_type: MsgType, body_len: u32) -> Header {
    Header {
        seq,
        timestamp_ns: now_ns(),
        magic: MAGIC,
        length: body_len,
        version: VERSION,
        msg_type: msg_type as u8,
        _pad: [0; 6],
    }
}

pub fn encode_new_order(seq: u64, body: &NewOrderBody, out: &mut Vec<u8>) {
    let header = make_header(seq, MsgType::NewOrder, NEW_ORDER_SIZE as u32);
    out.extend_from_slice(header.as_bytes());
    out.extend_from_slice(body.as_bytes());
}

pub fn encode_cancel(seq: u64, body: &CancelOrderBody, out: &mut Vec<u8>) {
    let header = make_header(seq, MsgType::CancelOrder, CANCEL_ORDER_SIZE as u32);
    out.extend_from_slice(header.as_bytes());
    out.extend_from_slice(body.as_bytes());
}

pub fn encode_exec_report(seq: u64, body: &ExecReportBody, out: &mut Vec<u8>) {
    let header = make_header(seq, MsgType::ExecutionReport, EXEC_REPORT_SIZE as u32);
    out.extend_from_slice(header.as_bytes());
    out.extend_from_slice(body.as_bytes());
}

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_new_order() -> NewOrderBody {
        NewOrderBody {
            price: 100 * crate::PRICE_SCALE,
            qty: 500,
            client_id: 99,
            order_id: 42,
            symbol_id: 7,
            side: Side::Buy as u8,
            ord_type: OrdType::Limit as u8,
            tif: Tif::Day as u8,
            _pad: 0,
            _pad2: 0,
        _pad3: 0,
        }
    }

    #[test]
    fn header_size_matches_constant() {
        assert_eq!(std::mem::size_of::<Header>(), HEADER_SIZE);
    }

    #[test]
    fn new_order_round_trip() {
        let body = sample_new_order();
        let mut buf = Vec::with_capacity(128);
        encode_new_order(1, &body, &mut buf);
        let (msg, consumed) = parse(&buf).expect("parse");
        assert_eq!(consumed, HEADER_SIZE + NEW_ORDER_SIZE);
        match msg {
            InboundMessage::NewOrder { body: parsed, .. } => {
                assert_eq!(parsed.order_id, body.order_id);
                assert_eq!(parsed.symbol_id, body.symbol_id);
                assert_eq!(parsed.price, body.price);
                assert_eq!(parsed.qty, body.qty);
                assert_eq!(parsed.client_id, body.client_id);
                assert_eq!(parsed.side, body.side);
            }
            _ => panic!("expected NewOrder"),
        }
    }

    #[test]
    fn rejects_bad_magic() {
        let body = sample_new_order();
        let mut buf = Vec::with_capacity(128);
        encode_new_order(1, &body, &mut buf);
        // `magic` starts at byte 16 of the header (see `Header`).
        buf[16] ^= 0xff;
        assert!(matches!(parse(&buf), Err(Error::BadMagic { .. })));
    }

    #[test]
    fn rejects_short_header() {
        let buf = [0u8; 10];
        assert!(matches!(parse(&buf), Err(Error::ShortBuffer { .. })));
    }

    #[test]
    fn cancel_round_trip() {
        let cancel = CancelOrderBody {
            order_id: 7,
            symbol_id: 1,
            _pad: 0,
        };
        let mut buf = Vec::with_capacity(64);
        encode_cancel(2, &cancel, &mut buf);
        let (msg, consumed) = parse(&buf).expect("parse");
        assert_eq!(consumed, HEADER_SIZE + CANCEL_ORDER_SIZE);
        match msg {
            InboundMessage::Cancel { body, .. } => {
                assert_eq!(body.order_id, 7);
                assert_eq!(body.symbol_id, 1);
            }
            _ => panic!("expected Cancel"),
        }
    }
}
