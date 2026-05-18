// Order-book view built from the CLIENT's own resting orders.
//
// The WebSocket transport currently only emits ExecutionReports for orders
// this client submitted, so this view reflects "my book" — it is not the
// market-wide depth. Aggregating across clients requires a market-data
// subscription channel (planned via gRPC SubscribeMarketData).

import React, { useMemo } from 'react';
import { StyleSheet, Text, View } from 'react-native';

import { OpenOrder } from '../state/trading';
import { Side, formatPrice } from '../protocol/types';
import { useTheme } from '../theme/ThemeProvider';

interface Level {
  priceTicks: bigint;
  qty: bigint;
}

interface Props {
  symbolId: number;
  orders: OpenOrder[];
}

const aggregate = (orders: OpenOrder[], side: Side): Level[] => {
  const acc = new Map<string, Level>();
  for (const o of orders) {
    if (o.side !== side) continue;
    const key = o.priceTicks.toString();
    const cur = acc.get(key);
    if (cur) {
      cur.qty += o.leavesQty;
    } else {
      acc.set(key, { priceTicks: o.priceTicks, qty: o.leavesQty });
    }
  }
  const arr = Array.from(acc.values());
  arr.sort((a, b) => {
    if (a.priceTicks === b.priceTicks) return 0;
    if (side === Side.Buy) return a.priceTicks > b.priceTicks ? -1 : 1;
    return a.priceTicks < b.priceTicks ? -1 : 1;
  });
  return arr.slice(0, 8);
};

export const OrderBook: React.FC<Props> = ({ symbolId, orders }) => {
  const { colors } = useTheme();
  const filtered = useMemo(() => orders.filter(o => o.symbolId === symbolId), [orders, symbolId]);
  const bids = aggregate(filtered, Side.Buy);
  const asks = aggregate(filtered, Side.Sell);
  const maxQty = useMemo(() => {
    let m = 0n;
    for (const l of [...bids, ...asks]) if (l.qty > m) m = l.qty;
    return m === 0n ? 1n : m;
  }, [bids, asks]);

  return (
    <View>
      <View style={styles.header}>
        <Text style={[styles.col, styles.colLabel, { color: colors.textMuted }]}>BID QTY</Text>
        <Text style={[styles.col, styles.colLabel, styles.center, { color: colors.textMuted }]}>
          PRICE
        </Text>
        <Text style={[styles.col, styles.colLabel, styles.right, { color: colors.textMuted }]}>
          ASK QTY
        </Text>
      </View>

      {asks
        .slice()
        .reverse()
        .map(l => (
          <Row
            key={`a-${l.priceTicks.toString()}`}
            level={l}
            maxQty={maxQty}
            side={Side.Sell}
            bg={colors.sell + '22'}
            fg={colors.sell}
            colors={colors}
          />
        ))}
      <View style={[styles.divider, { backgroundColor: colors.border }]} />
      {bids.map(l => (
        <Row
          key={`b-${l.priceTicks.toString()}`}
          level={l}
          maxQty={maxQty}
          side={Side.Buy}
          bg={colors.buy + '22'}
          fg={colors.buy}
          colors={colors}
        />
      ))}

      {bids.length === 0 && asks.length === 0 && (
        <Text style={[styles.empty, { color: colors.textMuted }]}>
          No resting orders on this symbol.
        </Text>
      )}
    </View>
  );
};

const Row: React.FC<{
  level: Level;
  maxQty: bigint;
  side: Side;
  bg: string;
  fg: string;
  colors: { text: string };
}> = ({ level, maxQty, side, bg, fg, colors }) => {
  // Build the depth bar width relative to the largest visible level.
  const pct = Number((level.qty * 100n) / maxQty);
  const isBid = side === Side.Buy;
  return (
    <View style={styles.rowWrap}>
      <View
        style={[
          styles.bar,
          {
            backgroundColor: bg,
            width: `${pct}%`,
            alignSelf: isBid ? 'flex-start' : 'flex-end',
          },
        ]}
      />
      <View style={styles.row}>
        <Text style={[styles.col, { color: isBid ? colors.text : 'transparent' }]}>
          {isBid ? level.qty.toString() : '·'}
        </Text>
        <Text style={[styles.col, styles.center, { color: fg, fontWeight: '700' }]}>
          {formatPrice(level.priceTicks)}
        </Text>
        <Text style={[styles.col, styles.right, { color: !isBid ? colors.text : 'transparent' }]}>
          {!isBid ? level.qty.toString() : '·'}
        </Text>
      </View>
    </View>
  );
};

const styles = StyleSheet.create({
  header: {
    flexDirection: 'row',
    paddingBottom: 6,
  },
  divider: { height: StyleSheet.hairlineWidth, marginVertical: 4 },
  rowWrap: { position: 'relative', height: 26, justifyContent: 'center' },
  bar: { position: 'absolute', height: '100%', top: 0, borderRadius: 4 },
  row: { flexDirection: 'row', alignItems: 'center', paddingHorizontal: 6 },
  col: { flex: 1, fontVariant: ['tabular-nums'], fontSize: 13 },
  colLabel: { fontSize: 10, letterSpacing: 1, fontWeight: '700' },
  center: { textAlign: 'center' },
  right: { textAlign: 'right' },
  empty: { paddingVertical: 14, textAlign: 'center' },
});
