import React from 'react';
import { FlatList, Pressable, StyleSheet, Text, View } from 'react-native';

import { OpenOrder, useTrading } from '../state/trading';
import { Side, formatPrice, sideLabel } from '../protocol/types';
import { useTheme } from '../theme/ThemeProvider';

interface Props {
  orders: OpenOrder[];
}

export const OpenOrdersList: React.FC<Props> = ({ orders }) => {
  const { colors } = useTheme();
  const cancel = useTrading(s => s.cancelOrder);
  return (
    <FlatList
      data={orders}
      keyExtractor={o => o.orderId.toString()}
      renderItem={({ item }) => (
        <View style={[styles.row, { borderBottomColor: colors.border }]}>
          <View style={styles.left}>
            <Text style={[styles.id, { color: colors.text }]}>#{item.orderId.toString()}</Text>
            <Text style={[styles.meta, { color: colors.textMuted }]}>
              SYM-{item.symbolId} ·{' '}
              <Text style={{ color: item.side === Side.Buy ? colors.buy : colors.sell }}>
                {sideLabel(item.side)}
              </Text>
            </Text>
          </View>
          <Text style={[styles.numeric, { color: colors.text }]}>
            {item.leavesQty.toString()} @ {formatPrice(item.priceTicks)}
          </Text>
          <Pressable
            onPress={() => cancel(item.orderId, item.symbolId)}
            style={[styles.cancel, { borderColor: colors.danger }]}
          >
            <Text style={[styles.cancelLabel, { color: colors.danger }]}>CANCEL</Text>
          </Pressable>
        </View>
      )}
      ListEmptyComponent={
        <Text style={[styles.empty, { color: colors.textMuted }]}>No open orders.</Text>
      }
    />
  );
};

const styles = StyleSheet.create({
  row: {
    flexDirection: 'row',
    alignItems: 'center',
    paddingVertical: 8,
    borderBottomWidth: StyleSheet.hairlineWidth,
    gap: 10,
  },
  left: { flex: 1 },
  id: { fontSize: 13, fontWeight: '600' },
  meta: { fontSize: 11, marginTop: 2 },
  numeric: { fontSize: 13, fontVariant: ['tabular-nums'] },
  cancel: {
    paddingHorizontal: 10,
    paddingVertical: 5,
    borderWidth: StyleSheet.hairlineWidth,
    borderRadius: 8,
  },
  cancelLabel: { fontSize: 10, fontWeight: '700', letterSpacing: 1 },
  empty: { padding: 14, textAlign: 'center' },
});
