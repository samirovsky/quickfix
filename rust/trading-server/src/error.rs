use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("buffer too short: need {need} bytes, have {have}")]
    ShortBuffer { need: usize, have: usize },

    #[error("bad magic: expected 0x{expected:08x}, got 0x{got:08x}")]
    BadMagic { expected: u32, got: u32 },

    #[error("unsupported protocol version: {0}")]
    BadVersion(u8),

    #[error("unknown message type: {0}")]
    UnknownMsgType(u8),

    #[error("length mismatch: header says {header}, body bytes {body}")]
    LengthMismatch { header: u32, body: usize },

    #[error("unknown symbol id: {0}")]
    UnknownSymbol(u32),

    #[error("duplicate order id: {0}")]
    DuplicateOrderId(u64),

    #[error("unknown order id: {0}")]
    UnknownOrderId(u64),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
