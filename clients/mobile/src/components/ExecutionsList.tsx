import React from 'react';
import { FlatList, StyleSheet, Text, View } from 'react-native';

import {
  ExecStatus,
  ExecutionReport,
  Side,
  formatPrice,
  sideLabel,
  statusLabel,
} from '../protocol/types';
import { useTheme } from '../theme/ThemeProvider';

interface Props {
  reports: ExecutionReport[];
  limit?: number;
}

const fmtTime = (ts: bigint): string => {
  const ms = Number(ts / 1_000_000n);
  const d = new Date(ms);
  return `${d.getHours().toString().padStart(2, '0')}:${d.getMinutes().toString().padStart(2, '0')}:${d.getSeconds().toString().padStart(2, '0')}.${(d.getMilliseconds() + '').padStart(3, '0')}`;
};

const statusColor = (s: ExecStatus, c: { buy: string; sell: string; warn: string; danger: string; text: string }) => {
  switch (s) {
    case ExecStatus.Filled:
      return c.buy;
    case ExecStatus.PartiallyFilled:
      return c.warn;
    case ExecStatus.Cancelled:
      return c.danger;
    case ExecStatus.Rejected:
      return c.danger;
    default:
      return c.text;
  }
};

export const ExecutionsList: React.FC<Props> = ({ reports, limit }) => {
  const { colors } = useTheme();
  const data = limit ? reports.slice(0, limit) : reports;
  return (
    <FlatList
      data={data}
      keyExtractor={item => `${item.execId.toString()}-${item.orderId.toString()}`}
      renderItem={({ item }) => (
        <View style={[styles.row, { borderBottomColor: colors.border }]}>
          <Text style={[styles.cellTime, { color: colors.textMuted }]}>
            {fmtTime(item.timestampNs)}
          </Text>
          <Text style={[styles.cell, { color: colors.text }]}>
            #{item.orderId.toString()} · SYM-{item.symbolId}
          </Text>
          <Text style={[styles.cell, { color: item.side === Side.Buy ? colors.buy : colors.sell }]}>
            {sideLabel(item.side)}
          </Text>
          <Text style={[styles.cell, styles.numeric, { color: colors.text }]}>
            {item.lastQty > 0n ? `${item.lastQty} @ ${formatPrice(item.lastPriceTicks)}` : '—'}
          </Text>
          <Text style={[styles.cellStatus, { color: statusColor(item.status, colors) }]}>
            {statusLabel(item.status)}
          </Text>
        </View>
      )}
      ListEmptyComponent={
        <Text style={[styles.empty, { color: colors.textMuted }]}>No executions yet.</Text>
      }
      style={styles.list}
    />
  );
};

const styles = StyleSheet.create({
  list: { maxHeight: 320 },
  row: {
    flexDirection: 'row',
    alignItems: 'center',
    paddingVertical: 6,
    borderBottomWidth: StyleSheet.hairlineWidth,
    gap: 8,
  },
  cellTime: { fontSize: 11, fontVariant: ['tabular-nums'], width: 80 },
  cell: { fontSize: 12, flexShrink: 1 },
  numeric: { fontVariant: ['tabular-nums'] },
  cellStatus: { fontSize: 11, fontWeight: '700', letterSpacing: 0.5 },
  empty: { padding: 12, textAlign: 'center' },
});
