// Trading state derived from the stream of execution reports the server
// emits over the WebSocket. The store keeps:
//   - connection state
//   - open orders (orderId -> resting qty + price + side + symbol)
//   - fills history (per-trade rows)
//   - per-symbol position + realised P&L
//   - all execution reports, capped, for the live feed

import { create } from 'zustand';

import {
  CancelOrder,
  ExecStatus,
  ExecutionReport,
  NewOrder,
  OrdType,
  PRICE_SCALE,
  Side,
  Tif,
} from '../protocol/types';
import { ConnState, TradingClient } from '../transport/client';

const MAX_FEED = 500;
const MAX_FILLS = 500;

export interface OpenOrder {
  orderId: bigint;
  symbolId: number;
  side: Side;
  priceTicks: bigint;
  leavesQty: bigint;
  submittedAt: number;
}

export interface Fill {
  execId: bigint;
  orderId: bigint;
  symbolId: number;
  side: Side;
  priceTicks: bigint;
  qty: bigint;
  timestampNs: bigint;
}

export interface Position {
  netQty: bigint; // positive = long, negative = short
  averagePriceTicks: bigint; // weighted-average entry of the open position
  realisedPnlTicks: bigint;
}

export interface PlaceOrderInput {
  symbolId: number;
  side: Side;
  ordType: OrdType;
  tif: Tif;
  priceWhole: number; // user-facing whole price (e.g. 100.50)
  qty: bigint;
}

interface TradingStore {
  conn: ConnState;
  openOrders: Record<string, OpenOrder>; // keyed by orderId.toString()
  fills: Fill[];
  feed: ExecutionReport[];
  positions: Record<number, Position>;
  nextOrderId: bigint;
  clientId: bigint;
  client: TradingClient | null;

  init: () => void;
  connect: (url: string) => void;
  disconnect: () => void;
  placeOrder: (input: PlaceOrderInput) => bigint | null;
  cancelOrder: (orderId: bigint, symbolId: number) => void;
  cancelAll: (symbolId?: number) => number;
  clearFeed: () => void;
  resetSession: () => void;
}

const keyOf = (orderId: bigint) => orderId.toString();

const fractionalPriceToTicks = (whole: number): bigint => {
  // Accept whole prices with up to 8 decimal places.
  const rounded = Math.round(whole * Number(PRICE_SCALE));
  return BigInt(rounded);
};

function applyFill(positions: Record<number, Position>, fill: Fill): Record<number, Position> {
  const prev: Position = positions[fill.symbolId] ?? {
    netQty: 0n,
    averagePriceTicks: 0n,
    realisedPnlTicks: 0n,
  };

  const signedDelta = fill.side === Side.Buy ? fill.qty : -fill.qty;
  const newQty = prev.netQty + signedDelta;

  let avg = prev.averagePriceTicks;
  let realised = prev.realisedPnlTicks;

  const sameDirection = (prev.netQty >= 0n && signedDelta >= 0n) || (prev.netQty <= 0n && signedDelta <= 0n);
  if (prev.netQty === 0n || sameDirection) {
    // Opening or adding to existing position — weighted average update.
    const prevAbs = prev.netQty < 0n ? -prev.netQty : prev.netQty;
    const addAbs = fill.qty;
    const totalAbs = prevAbs + addAbs;
    if (totalAbs === 0n) {
      avg = 0n;
    } else {
      avg = (prev.averagePriceTicks * prevAbs + fill.priceTicks * addAbs) / totalAbs;
    }
  } else {
    // Reducing or flipping. Realise PnL on the portion that closes.
    const prevAbs = prev.netQty < 0n ? -prev.netQty : prev.netQty;
    const closeAbs = prevAbs < fill.qty ? prevAbs : fill.qty;
    const direction = prev.netQty > 0n ? 1n : -1n;
    // realised += closeAbs * (fillPrice - avg) * direction, scaled back to ticks
    realised += ((fill.priceTicks - prev.averagePriceTicks) * closeAbs * direction);
    if (newQty === 0n) {
      avg = 0n;
    } else if ((prev.netQty > 0n && newQty < 0n) || (prev.netQty < 0n && newQty > 0n)) {
      // Flipped — the leftover qty opened a new position at the fill price.
      avg = fill.priceTicks;
    }
    // If we just reduced (not flipped, not flat), avg stays the same.
  }

  return {
    ...positions,
    [fill.symbolId]: {
      netQty: newQty,
      averagePriceTicks: newQty === 0n ? 0n : avg,
      realisedPnlTicks: realised,
    },
  };
}

export const useTrading = create<TradingStore>((set, get) => ({
  conn: { kind: 'idle' },
  openOrders: {},
  fills: [],
  feed: [],
  positions: {},
  nextOrderId: 1n,
  clientId: BigInt(Date.now() & 0xffffffff),
  client: null,

  init: () => {
    if (get().client) return;
    const client = new TradingClient({
      onState: state => set({ conn: state }),
      onExec: report => {
        set(prev => {
          const feed = [report, ...prev.feed].slice(0, MAX_FEED);
          let { openOrders, fills, positions } = prev;
          openOrders = { ...openOrders };

          const wasMyOrder = !!openOrders[keyOf(report.orderId)];

          // Update open-orders index based on terminal status / leaves_qty.
          const existing = openOrders[keyOf(report.orderId)];
          if (existing) {
            if (
              report.status === ExecStatus.New ||
              (report.status === ExecStatus.PartiallyFilled && report.leavesQty > 0n)
            ) {
              openOrders[keyOf(report.orderId)] = {
                ...existing,
                leavesQty: report.leavesQty,
              };
            } else {
              // Filled, PartiallyFilled-and-done, Cancelled, Rejected.
              delete openOrders[keyOf(report.orderId)];
            }
          } else if (report.status === ExecStatus.New) {
            // Rare: server emitted a fresh New we didn't track yet (e.g. test client).
            openOrders[keyOf(report.orderId)] = {
              orderId: report.orderId,
              symbolId: report.symbolId,
              side: report.side,
              priceTicks: report.lastPriceTicks,
              leavesQty: report.leavesQty,
              submittedAt: Date.now(),
            };
          }

          // Record fills + update positions only for reports tied to our orders.
          const isFill =
            report.lastQty > 0n &&
            (report.status === ExecStatus.PartiallyFilled ||
              report.status === ExecStatus.Filled);

          if (isFill && wasMyOrder) {
            const fill: Fill = {
              execId: report.execId,
              orderId: report.orderId,
              symbolId: report.symbolId,
              side: report.side,
              priceTicks: report.lastPriceTicks,
              qty: report.lastQty,
              timestampNs: report.timestampNs,
            };
            fills = [fill, ...fills].slice(0, MAX_FILLS);
            positions = applyFill(positions, fill);
          }

          return { feed, openOrders, fills, positions };
        });
      },
      onParseError: e => console.warn('[qftx] parse error', e),
    });
    set({ client });
  },

  connect: url => {
    const client = get().client;
    client?.connect(url);
  },

  disconnect: () => {
    get().client?.disconnect();
  },

  placeOrder: input => {
    const { client, nextOrderId, clientId } = get();
    if (!client) return null;
    const orderId = nextOrderId;
    const priceTicks = fractionalPriceToTicks(input.priceWhole);
    const order: NewOrder = {
      orderId,
      symbolId: input.symbolId,
      side: input.side,
      ordType: input.ordType,
      tif: input.tif,
      priceTicks,
      qty: input.qty,
      clientId,
    };
    set(prev => ({
      nextOrderId: prev.nextOrderId + 1n,
      openOrders: {
        ...prev.openOrders,
        [keyOf(orderId)]: {
          orderId,
          symbolId: input.symbolId,
          side: input.side,
          priceTicks,
          leavesQty: input.qty,
          submittedAt: Date.now(),
        },
      },
    }));
    client.placeOrder(order);
    return orderId;
  },

  cancelOrder: (orderId, symbolId) => {
    const client = get().client;
    if (!client) return;
    const cancel: CancelOrder = { orderId, symbolId };
    client.cancelOrder(cancel);
  },

  cancelAll: symbolId => {
    const { openOrders } = get();
    const targets = Object.values(openOrders).filter(
      o => symbolId === undefined || o.symbolId === symbolId
    );
    targets.forEach(o => get().cancelOrder(o.orderId, o.symbolId));
    return targets.length;
  },

  clearFeed: () => set({ feed: [] }),

  resetSession: () =>
    set({
      openOrders: {},
      fills: [],
      feed: [],
      positions: {},
      nextOrderId: 1n,
      clientId: BigInt(Date.now() & 0xffffffff),
    }),
}));
