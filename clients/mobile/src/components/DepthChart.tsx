// Cumulative depth-curve view (own resting orders only — see OrderBook.tsx).

import React, { useMemo } from 'react';
import { StyleSheet, Text, View } from 'react-native';

import { OpenOrder } from '../state/trading';
import { Side, formatPrice } from '../protocol/types';
import { useTheme } from '../theme/ThemeProvider';

interface Props {
  symbolId: number;
  orders: OpenOrder[];
}

interface Bucket {
  priceTicks: bigint;
  cumQty: bigint;
}

const cumulative = (orders: OpenOrder[], side: Side): Bucket[] => {
  const byPrice = new Map<string, bigint>();
  for (const o of orders) {
    if (o.side !== side) continue;
    const key = o.priceTicks.toString();
    byPrice.set(key, (byPrice.get(key) ?? 0n) + o.leavesQty);
  }
  const entries = Array.from(byPrice.entries()).map(([k, qty]) => ({
    priceTicks: BigInt(k),
    qty,
  }));
  entries.sort((a, b) =>
    a.priceTicks === b.priceTicks
      ? 0
      : side === Side.Buy
        ? a.priceTicks > b.priceTicks
          ? -1
          : 1
        : a.priceTicks < b.priceTicks
          ? -1
          : 1
  );
  let acc = 0n;
  return entries.map(e => {
    acc += e.qty;
    return { priceTicks: e.priceTicks, cumQty: acc };
  });
};

export const DepthChart: React.FC<Props> = ({ symbolId, orders }) => {
  const { colors } = useTheme();
  const filtered = useMemo(
    () => orders.filter(o => o.symbolId === symbolId),
    [orders, symbolId]
  );
  const bids = cumulative(filtered, Side.Buy);
  const asks = cumulative(filtered, Side.Sell);
  const total = useMemo(() => {
    let s = 0n;
    for (const b of bids) if (b.cumQty > s) s = b.cumQty;
    for (const a of asks) if (a.cumQty > s) s = a.cumQty;
    return s === 0n ? 1n : s;
  }, [bids, asks]);

  if (bids.length === 0 && asks.length === 0) {
    return (
      <Text style={[styles.empty, { color: colors.textMuted }]}>
        Place resting orders to populate the depth chart.
      </Text>
    );
  }

  return (
    <View>
      <View style={styles.chart}>
        <View style={[styles.half, styles.left]}>
          {bids
            .slice()
            .reverse()
            .map(b => (
              <View
                key={`b-${b.priceTicks.toString()}`}
                style={[
                  styles.bar,
                  {
                    backgroundColor: colors.buy + '55',
                    width: `${Number((b.cumQty * 100n) / total)}%`,
                  },
                ]}
              >
                <Text style={[styles.barLabel, { color: colors.buy }]}>
                  {formatPrice(b.priceTicks)} · {b.cumQty.toString()}
                </Text>
              </View>
            ))}
        </View>
        <View style={[styles.half, styles.right]}>
          {asks.map(a => (
            <View
              key={`a-${a.priceTicks.toString()}`}
              style={[
                styles.bar,
                styles.asksBar,
                {
                  backgroundColor: colors.sell + '55',
                  width: `${Number((a.cumQty * 100n) / total)}%`,
                },
              ]}
            >
              <Text style={[styles.barLabel, styles.asksLabel, { color: colors.sell }]}>
                {formatPrice(a.priceTicks)} · {a.cumQty.toString()}
              </Text>
            </View>
          ))}
        </View>
      </View>
    </View>
  );
};

const styles = StyleSheet.create({
  chart: { flexDirection: 'row', gap: 6 },
  half: { flex: 1, gap: 3 },
  left: { alignItems: 'flex-end' },
  right: { alignItems: 'flex-start' },
  bar: {
    minWidth: 20,
    paddingHorizontal: 6,
    paddingVertical: 4,
    borderRadius: 4,
  },
  asksBar: {},
  barLabel: { fontSize: 11, fontWeight: '600', textAlign: 'right' },
  asksLabel: { textAlign: 'left' },
  empty: { padding: 14, textAlign: 'center' },
});
