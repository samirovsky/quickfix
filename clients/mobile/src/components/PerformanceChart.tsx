// Daily-PnL bar chart drawn with plain Views (no chart library). Newest
// day on the right; greens above zero, reds below.

import React, { useMemo } from 'react';
import { StyleSheet, Text, View } from 'react-native';

import { PerformanceMetrics } from '../bots/api';
import { useTheme } from '../theme/ThemeProvider';

interface Props {
  metrics: PerformanceMetrics;
  height?: number;
}

export const PerformanceChart: React.FC<Props> = ({ metrics, height = 120 }) => {
  const { colors } = useTheme();

  const buckets = useMemo(() => {
    // Sort ascending (oldest → newest) for the chart, and fill in missing
    // days within the window with zero-trade entries so the bar density
    // matches the actual time elapsed.
    const byDate = new Map<string, { pnl: number; trades: number }>();
    for (const d of metrics.daily) {
      byDate.set(d.date, { pnl: d.realised_pnl_cents, trades: d.trades });
    }
    const out: Array<{ date: string; pnl: number; trades: number }> = [];
    const today = new Date();
    today.setUTCHours(0, 0, 0, 0);
    for (let i = metrics.window_days - 1; i >= 0; i--) {
      const d = new Date(today);
      d.setUTCDate(today.getUTCDate() - i);
      const key = d.toISOString().slice(0, 10);
      const entry = byDate.get(key) ?? { pnl: 0, trades: 0 };
      out.push({ date: key, ...entry });
    }
    return out;
  }, [metrics]);

  const { maxAbs } = useMemo(() => {
    let m = 0;
    for (const b of buckets) {
      const a = Math.abs(b.pnl);
      if (a > m) m = a;
    }
    return { maxAbs: m === 0 ? 1 : m };
  }, [buckets]);

  if (metrics.total_trades === 0) {
    return (
      <View style={[styles.empty, { borderColor: colors.border, height }]}>
        <Text style={{ color: colors.textMuted, textAlign: 'center', fontSize: 12 }}>
          No trades in the last {metrics.window_days} days.
        </Text>
      </View>
    );
  }

  return (
    <View>
      <View style={[styles.chart, { height, borderColor: colors.border }]}>
        <View style={[styles.zeroLine, { backgroundColor: colors.border }]} />
        {buckets.map(b => {
          const pct = Math.abs(b.pnl) / maxAbs;
          const barHeight = Math.max(2, pct * (height / 2 - 4));
          const above = b.pnl >= 0;
          return (
            <View key={b.date} style={styles.col}>
              {above ? (
                <View
                  style={[
                    styles.bar,
                    {
                      backgroundColor: colors.buy,
                      height: barHeight,
                      bottom: height / 2,
                    },
                  ]}
                />
              ) : (
                <View
                  style={[
                    styles.bar,
                    {
                      backgroundColor: colors.sell,
                      height: barHeight,
                      top: height / 2,
                    },
                  ]}
                />
              )}
            </View>
          );
        })}
      </View>
      <View style={styles.axisRow}>
        <Text style={[styles.axisLabel, { color: colors.textMuted }]}>
          {buckets[0]?.date}
        </Text>
        <Text style={[styles.axisLabel, { color: colors.textMuted }]}>
          today
        </Text>
      </View>
    </View>
  );
};

const styles = StyleSheet.create({
  empty: {
    borderWidth: StyleSheet.hairlineWidth,
    borderRadius: 8,
    alignItems: 'center',
    justifyContent: 'center',
  },
  chart: {
    flexDirection: 'row',
    alignItems: 'stretch',
    position: 'relative',
    borderBottomWidth: StyleSheet.hairlineWidth,
    paddingHorizontal: 2,
  },
  zeroLine: {
    position: 'absolute',
    left: 0,
    right: 0,
    top: '50%',
    height: StyleSheet.hairlineWidth,
  },
  col: {
    flex: 1,
    marginHorizontal: 1,
    position: 'relative',
  },
  bar: {
    position: 'absolute',
    left: 0,
    right: 0,
    borderRadius: 2,
  },
  axisRow: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    paddingHorizontal: 2,
    marginTop: 4,
  },
  axisLabel: { fontSize: 10 },
});
