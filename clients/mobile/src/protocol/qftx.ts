// QFTX binary frame encoder/decoder.
//
// Layouts mirror rust/trading-server/src/protocol.rs exactly. Header is
// 32 bytes, bodies are fixed-size. All multi-byte fields are little-endian.

import {
  CANCEL_ORDER_BODY_SIZE,
  CancelOrder,
  EXEC_REPORT_BODY_SIZE,
  ExecStatus,
  ExecutionReport,
  HEADER_SIZE,
  MAGIC,
  MsgType,
  NEW_ORDER_BODY_SIZE,
  NewOrder,
  Side,
  VERSION,
} from './types';

let seqCounter = 0n;
export const nextSeq = (): bigint => {
  seqCounter += 1n;
  return seqCounter;
};

const nowNs = (): bigint => BigInt(Date.now()) * 1_000_000n;

function writeHeader(dv: DataView, seq: bigint, msgType: MsgType, bodyLen: number) {
  dv.setBigUint64(0, seq, true);
  dv.setBigUint64(8, nowNs(), true);
  dv.setUint32(16, MAGIC, true);
  dv.setUint32(20, bodyLen, true);
  dv.setUint8(24, VERSION);
  dv.setUint8(25, msgType);
  // 26..31 padding stays zero
}

export function encodeNewOrder(o: NewOrder, seq: bigint = nextSeq()): ArrayBuffer {
  const buf = new ArrayBuffer(HEADER_SIZE + NEW_ORDER_BODY_SIZE);
  const dv = new DataView(buf);
  writeHeader(dv, seq, MsgType.NewOrder, NEW_ORDER_BODY_SIZE);
  const off = HEADER_SIZE;
  dv.setBigInt64(off + 0, o.priceTicks, true);
  dv.setBigUint64(off + 8, o.qty, true);
  dv.setBigUint64(off + 16, o.clientId, true);
  dv.setBigUint64(off + 24, o.orderId, true);
  dv.setUint32(off + 32, o.symbolId, true);
  dv.setUint8(off + 36, o.side);
  dv.setUint8(off + 37, o.ordType);
  dv.setUint8(off + 38, o.tif);
  // 39..47 padding stays zero
  return buf;
}

export function encodeCancelOrder(c: CancelOrder, seq: bigint = nextSeq()): ArrayBuffer {
  const buf = new ArrayBuffer(HEADER_SIZE + CANCEL_ORDER_BODY_SIZE);
  const dv = new DataView(buf);
  writeHeader(dv, seq, MsgType.CancelOrder, CANCEL_ORDER_BODY_SIZE);
  const off = HEADER_SIZE;
  dv.setBigUint64(off + 0, c.orderId, true);
  dv.setUint32(off + 8, c.symbolId, true);
  // 12..15 padding stays zero
  return buf;
}

export class ParseError extends Error {}

export function decodeFrame(buf: ArrayBuffer): ExecutionReport {
  if (buf.byteLength < HEADER_SIZE) {
    throw new ParseError(`frame smaller than header: ${buf.byteLength}`);
  }
  const dv = new DataView(buf);
  const magic = dv.getUint32(16, true);
  if (magic !== MAGIC) {
    throw new ParseError(`bad magic 0x${magic.toString(16)}`);
  }
  const version = dv.getUint8(24);
  if (version !== VERSION) {
    throw new ParseError(`unsupported version ${version}`);
  }
  const msgType = dv.getUint8(25) as MsgType;
  const length = dv.getUint32(20, true);
  if (HEADER_SIZE + length !== buf.byteLength) {
    throw new ParseError(`frame length mismatch: header=${length} bytes=${buf.byteLength}`);
  }
  if (msgType !== MsgType.ExecutionReport) {
    throw new ParseError(`unexpected inbound msg_type ${msgType}`);
  }
  if (length !== EXEC_REPORT_BODY_SIZE) {
    throw new ParseError(`exec report body size mismatch: ${length}`);
  }
  const seq = dv.getBigUint64(0, true);
  const timestampNs = dv.getBigUint64(8, true);
  const off = HEADER_SIZE;
  return {
    seq,
    timestampNs,
    orderId: dv.getBigUint64(off + 0, true),
    execId: dv.getBigUint64(off + 8, true),
    lastPriceTicks: dv.getBigInt64(off + 16, true),
    lastQty: dv.getBigUint64(off + 24, true),
    leavesQty: dv.getBigUint64(off + 32, true),
    symbolId: dv.getUint32(off + 40, true),
    status: dv.getUint8(off + 44) as ExecStatus,
    side: dv.getUint8(off + 45) as Side,
  };
}
