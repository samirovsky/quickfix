import React, { useMemo } from 'react';
import { ScrollView, StyleSheet, Text, View } from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';

import { Card } from '../components/Card';
import { DepthChart } from '../components/DepthChart';
import { ExecutionsList } from '../components/ExecutionsList';
import { OrderBook } from '../components/OrderBook';
import { OrderEntry } from '../components/OrderEntry';
import { StatusBadge } from '../components/StatusBadge';
import { SymbolPicker } from '../components/SymbolPicker';
import { useSettings } from '../state/settings';
import { useTrading } from '../state/trading';
import { useTheme } from '../theme/ThemeProvider';

export const TradingScreen: React.FC = () => {
  const { colors } = useTheme();
  const { values, update } = useSettings();
  const conn = useTrading(s => s.conn);
  const feed = useTrading(s => s.feed);
  const openOrders = useTrading(s => s.openOrders);

  const ordersForSymbol = useMemo(
    () => Object.values(openOrders),
    [openOrders]
  );

  return (
    <SafeAreaView style={[styles.safe, { backgroundColor: colors.bg }]} edges={['top']}>
      <ScrollView contentContainerStyle={styles.scroll}>
        <View style={styles.header}>
          <Text style={[styles.title, { color: colors.text }]}>Trade</Text>
          <StatusBadge state={conn} />
        </View>

        <SymbolPicker
          count={values.symbolCount}
          value={values.defaultSymbolId}
          onChange={n => update({ defaultSymbolId: n })}
        />

        <View style={styles.spacer} />

        <Card title={`Order book · SYM-${values.defaultSymbolId} (own resting)`}>
          <OrderBook symbolId={values.defaultSymbolId} orders={ordersForSymbol} />
        </Card>

        <Card title="Place order">
          <OrderEntry symbolId={values.defaultSymbolId} />
        </Card>

        <Card title="Depth (cumulative, own orders)">
          <DepthChart symbolId={values.defaultSymbolId} orders={ordersForSymbol} />
        </Card>

        <Card title="Execution feed">
          <ExecutionsList reports={feed} limit={20} />
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
  spacer: { height: 12 },
});
