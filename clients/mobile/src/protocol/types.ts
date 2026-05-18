// Mirror of rust/trading-server/src/protocol.rs enums + sizes.

export const MAGIC = 0x51465458; // "QFTX" little-endian
export const VERSION = 1;

export const HEADER_SIZE = 32;
export const NEW_ORDER_BODY_SIZE = 48;
export const CANCEL_ORDER_BODY_SIZE = 16;
export const EXEC_REPORT_BODY_SIZE = 48;

export const PRICE_SCALE = 100_000_000n;

export enum MsgType {
  NewOrder = 1,
  CancelOrder = 2,
  ExecutionReport = 3,
  Heartbeat = 4,
}

export enum Side {
  Buy = 0,
  Sell = 1,
}

export enum OrdType {
  Limit = 0,
  Market = 1,
}

export enum Tif {
  Day = 0,
  Ioc = 1,
  Fok = 2,
}

export enum ExecStatus {
  New = 0,
  PartiallyFilled = 1,
  Filled = 2,
  Cancelled = 3,
  Rejected = 4,
}

export const sideLabel = (s: Side) => (s === Side.Buy ? 'BUY' : 'SELL');
export const ordTypeLabel = (t: OrdType) => (t === OrdType.Limit ? 'LIMIT' : 'MARKET');
export const tifLabel = (t: Tif) => (t === Tif.Day ? 'DAY' : t === Tif.Ioc ? 'IOC' : 'FOK');
export const statusLabel = (s: ExecStatus): string =>
  ({
    [ExecStatus.New]: 'NEW',
    [ExecStatus.PartiallyFilled]: 'PARTIAL',
    [ExecStatus.Filled]: 'FILLED',
    [ExecStatus.Cancelled]: 'CANCELLED',
    [ExecStatus.Rejected]: 'REJECTED',
  }[s] ?? `?(${s})`);

export interface NewOrder {
  orderId: bigint;
  symbolId: number;
  side: Side;
  ordType: OrdType;
  tif: Tif;
  priceTicks: bigint; // raw i64 already scaled
  qty: bigint;
  clientId: bigint;
}

export interface CancelOrder {
  orderId: bigint;
  symbolId: number;
}

export interface ExecutionReport {
  seq: bigint;
  timestampNs: bigint;
  orderId: bigint;
  execId: bigint;
  symbolId: number;
  status: ExecStatus;
  side: Side;
  lastPriceTicks: bigint;
  lastQty: bigint;
  leavesQty: bigint;
}

export const priceFromWhole = (whole: number, fraction = 0): bigint => {
  // priceTicks = whole * 1e8 + fraction (fraction is in ticks too)
  return BigInt(whole) * PRICE_SCALE + BigInt(fraction);
};

export const ticksToNumber = (ticks: bigint): number => Number(ticks) / Number(PRICE_SCALE);

export const formatPrice = (ticks: bigint, fractionDigits = 2): string =>
  ticksToNumber(ticks).toFixed(fractionDigits);
