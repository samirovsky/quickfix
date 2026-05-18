import React, { useMemo } from 'react';
import { FlatList, ScrollView, StyleSheet, Text, View } from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';

import { Card } from '../components/Card';
import { Fill, Position, useTrading } from '../state/trading';
import { Side, formatPrice, sideLabel, ticksToNumber } from '../protocol/types';
import { useTheme } from '../theme/ThemeProvider';

const fmtTime = (ts: bigint): string => {
  const ms = Number(ts / 1_000_000n);
  const d = new Date(ms);
  return `${d.getHours().toString().padStart(2, '0')}:${d.getMinutes().toString().padStart(2, '0')}:${d.getSeconds().toString().padStart(2, '0')}`;
};

interface PositionRow {
  symbolId: number;
  position: Position;
}

export const FillsScreen: React.FC = () => {
  const { colors } = useTheme();
  const fills = useTrading(s => s.fills);
  const positions = useTrading(s => s.positions);

  const posRows: PositionRow[] = useMemo(
    () =>
      Object.entries(positions)
        .map(([sym, p]) => ({ symbolId: Number(sym), position: p }))
        .sort((a, b) => a.symbolId - b.symbolId),
    [positions]
  );

  const totalRealised = useMemo(() => {
    let total = 0n;
    for (const p of Object.values(positions)) total += p.realisedPnlTicks;
    return total;
  }, [positions]);

  return (
    <SafeAreaView style={[styles.safe, { backgroundColor: colors.bg }]} edges={['top']}>
      <ScrollView contentContainerStyle={styles.scroll}>
        <Text style={[styles.title, { color: colors.text }]}>Fills &amp; P&amp;L</Text>

        <Card
          title="Realised P&L (all symbols)"
          right={
            <Text
              style={[
                styles.pnlBig,
                { color: totalRealised >= 0n ? colors.ok : colors.danger },
              ]}
            >
              {ticksToNumber(totalRealised).toFixed(2)}
            </Text>
          }
        >
          <Text style={[styles.note, { color: colors.textMuted }]}>
            Computed from this session&apos;s fills. Average-price method, no unrealised mark.
          </Text>
        </Card>

        <Card title={`Positions · ${posRows.length}`}>
          {posRows.length === 0 ? (
            <Text style={[styles.empty, { color: colors.textMuted }]}>No positions yet.</Text>
          ) : (
            posRows.map(({ symbolId, position }) => (
              <View
                key={symbolId}
                style={[styles.posRow, { borderBottomColor: colors.border }]}
              >
                <Text style={[styles.posSym, { color: colors.text }]}>SYM-{symbolId}</Text>
                <Text
                  style={[
                    styles.posQty,
                    {
                      color:
                        position.netQty > 0n
                          ? colors.buy
                          : position.netQty < 0n
                            ? colors.sell
                            : colors.textMuted,
                    },
                  ]}
                >
                  {position.netQty.toString()}
                </Text>
                <Text style={[styles.posAvg, { color: colors.textMuted }]}>
                  avg {position.averagePriceTicks === 0n ? '—' : formatPrice(position.averagePriceTicks)}
                </Text>
                <Text
                  style={[
                    styles.posPnl,
                    {
                      color: position.realisedPnlTicks >= 0n ? colors.ok : colors.danger,
                    },
                  ]}
                >
                  {ticksToNumber(position.realisedPnlTicks).toFixed(2)}
                </Text>
              </View>
            ))
          )}
        </Card>

        <Card title={`Fills · ${fills.length}`}>
          <FlatList
            data={fills}
            scrollEnabled={false}
            keyExtractor={f => f.execId.toString()}
            renderItem={({ item }) => <FillRow fill={item} colors={colors} />}
            ListEmptyComponent={
              <Text style={[styles.empty, { color: colors.textMuted }]}>No fills yet.</Text>
            }
          />
        </Card>
      </ScrollView>
    </SafeAreaView>
  );
};

const FillRow: React.FC<{
  fill: Fill;
  colors: { text: string; buy: string; sell: string; textMuted: string; border: string };
}> = ({ fill, colors }) => (
  <View style={[styles.fillRow, { borderBottomColor: colors.border }]}>
    <Text style={[styles.fillTime, { color: colors.textMuted }]}>{fmtTime(fill.timestampNs)}</Text>
    <Text style={[styles.fillCell, { color: colors.text }]}>SYM-{fill.symbolId}</Text>
    <Text
      style={[
        styles.fillSide,
        { color: fill.side === Side.Buy ? colors.buy : colors.sell },
      ]}
    >
      {sideLabel(fill.side)}
    </Text>
    <Text style={[styles.fillNum, { color: colors.text }]}>
      {fill.qty.toString()} @ {formatPrice(fill.priceTicks)}
    </Text>
  </View>
);

const styles = StyleSheet.create({
  safe: { flex: 1 },
  scroll: { padding: 16, paddingBottom: 32 },
  title: { fontSize: 22, fontWeight: '700', marginBottom: 12 },
  pnlBig: { fontSize: 18, fontWeight: '700', fontVariant: ['tabular-nums'] },
  note: { fontSize: 11 },
  empty: { padding: 14, textAlign: 'center' },
  posRow: {
    flexDirection: 'row',
    alignItems: 'center',
    paddingVertical: 8,
    borderBottomWidth: StyleSheet.hairlineWidth,
    gap: 10,
  },
  posSym: { flex: 1, fontWeight: '600' },
  posQty: { width: 60, fontVariant: ['tabular-nums'], textAlign: 'right' },
  posAvg: { width: 100, fontVariant: ['tabular-nums'], textAlign: 'right' },
  posPnl: { width: 80, fontVariant: ['tabular-nums'], textAlign: 'right', fontWeight: '600' },
  fillRow: {
    flexDirection: 'row',
    alignItems: 'center',
    paddingVertical: 7,
    borderBottomWidth: StyleSheet.hairlineWidth,
    gap: 10,
  },
  fillTime: { width: 70, fontSize: 11, fontVariant: ['tabular-nums'] },
  fillCell: { width: 64, fontSize: 12 },
  fillSide: { width: 44, fontSize: 11, fontWeight: '700' },
  fillNum: { flex: 1, textAlign: 'right', fontVariant: ['tabular-nums'], fontSize: 13 },
});
