import React, { useMemo, useState } from 'react';
import { Pressable, ScrollView, StyleSheet, Text, View } from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';

import { Card } from '../components/Card';
import { OpenOrdersList } from '../components/OpenOrdersList';
import { useTrading } from '../state/trading';
import { useTheme } from '../theme/ThemeProvider';

export const OrdersScreen: React.FC = () => {
  const { colors } = useTheme();
  const openOrders = useTrading(s => s.openOrders);
  const cancelAll = useTrading(s => s.cancelAll);
  const [filter, setFilter] = useState<number | null>(null);

  const orders = useMemo(() => Object.values(openOrders), [openOrders]);
  const visible = filter === null ? orders : orders.filter(o => o.symbolId === filter);

  const symbols = useMemo(() => {
    const s = new Set<number>();
    orders.forEach(o => s.add(o.symbolId));
    return Array.from(s).sort((a, b) => a - b);
  }, [orders]);

  return (
    <SafeAreaView style={[styles.safe, { backgroundColor: colors.bg }]} edges={['top']}>
      <ScrollView contentContainerStyle={styles.scroll}>
        <View style={styles.header}>
          <Text style={[styles.title, { color: colors.text }]}>Open orders</Text>
          <Pressable
            onPress={() => cancelAll(filter ?? undefined)}
            disabled={visible.length === 0}
            style={[
              styles.cancelAll,
              { borderColor: colors.danger, opacity: visible.length === 0 ? 0.4 : 1 },
            ]}
          >
            <Text style={[styles.cancelAllLabel, { color: colors.danger }]}>
              CANCEL ALL ({visible.length})
            </Text>
          </Pressable>
        </View>

        <View style={styles.filterRow}>
          <Pressable
            onPress={() => setFilter(null)}
            style={[
              styles.chip,
              {
                backgroundColor: filter === null ? colors.primary : colors.bgElevated,
                borderColor: filter === null ? colors.primary : colors.border,
              },
            ]}
          >
            <Text
              style={{
                color: filter === null ? colors.primaryFg : colors.text,
                fontWeight: filter === null ? '700' : '500',
              }}
            >
              ALL
            </Text>
          </Pressable>
          {symbols.map(s => (
            <Pressable
              key={s}
              onPress={() => setFilter(s)}
              style={[
                styles.chip,
                {
                  backgroundColor: filter === s ? colors.primary : colors.bgElevated,
                  borderColor: filter === s ? colors.primary : colors.border,
                },
              ]}
            >
              <Text
                style={{
                  color: filter === s ? colors.primaryFg : colors.text,
                  fontWeight: filter === s ? '700' : '500',
                }}
              >
                SYM-{s}
              </Text>
            </Pressable>
          ))}
        </View>

        <Card title={`Resting · ${visible.length}`}>
          <OpenOrdersList orders={visible} />
        </Card>
      </ScrollView>
    </SafeAreaView>
  );
};

const styles = StyleSheet.create({
  safe: { flex: 1 },
  scroll: { padding: 16, paddingBottom: 32 },
  header: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    marginBottom: 12,
  },
  title: { fontSize: 22, fontWeight: '700' },
  cancelAll: {
    paddingHorizontal: 12,
    paddingVertical: 8,
    borderRadius: 10,
    borderWidth: StyleSheet.hairlineWidth,
  },
  cancelAllLabel: { fontWeight: '700', fontSize: 11, letterSpacing: 0.5 },
  filterRow: { flexDirection: 'row', flexWrap: 'wrap', gap: 6, marginBottom: 8 },
  chip: {
    paddingHorizontal: 10,
    paddingVertical: 5,
    borderRadius: 999,
    borderWidth: StyleSheet.hairlineWidth,
  },
});
