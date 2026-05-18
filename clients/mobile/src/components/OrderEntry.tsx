import React, { useState } from 'react';
import {
  Keyboard,
  Pressable,
  StyleSheet,
  Text,
  TextInput,
  View,
} from 'react-native';

import { OrdType, Side, Tif } from '../protocol/types';
import { useTrading } from '../state/trading';
import { useTheme } from '../theme/ThemeProvider';
import { SegmentedControl } from './SegmentedControl';

interface Props {
  symbolId: number;
}

export const OrderEntry: React.FC<Props> = ({ symbolId }) => {
  const { colors } = useTheme();
  const placeOrder = useTrading(s => s.placeOrder);
  const isLive = useTrading(s => s.conn.kind === 'open');

  const [side, setSide] = useState<Side>(Side.Buy);
  const [ordType, setOrdType] = useState<OrdType>(OrdType.Limit);
  const [tif, setTif] = useState<Tif>(Tif.Day);
  const [priceText, setPriceText] = useState('100.00');
  const [qtyText, setQtyText] = useState('1');
  const [lastSubmitted, setLastSubmitted] = useState<string | null>(null);

  const submit = () => {
    const priceWhole = Number.parseFloat(priceText);
    const qty = BigInt(Math.max(0, Math.floor(Number.parseFloat(qtyText) || 0)));
    if (!Number.isFinite(priceWhole) || priceWhole <= 0 || qty <= 0n) return;
    const orderId = placeOrder({
      symbolId,
      side,
      ordType,
      tif,
      priceWhole,
      qty,
    });
    if (orderId !== null) {
      setLastSubmitted(`Sent order #${orderId} (${qty.toString()} @ ${priceWhole})`);
      Keyboard.dismiss();
    }
  };

  return (
    <View>
      <SegmentedControl<Side>
        value={side}
        options={[
          { label: 'BUY', value: Side.Buy, tone: 'buy' },
          { label: 'SELL', value: Side.Sell, tone: 'sell' },
        ]}
        onChange={setSide}
      />

      <View style={styles.spacer} />

      <View style={styles.fieldRow}>
        <View style={styles.field}>
          <Text style={[styles.label, { color: colors.textMuted }]}>PRICE</Text>
          <TextInput
            value={priceText}
            onChangeText={setPriceText}
            keyboardType="decimal-pad"
            style={[
              styles.input,
              { color: colors.text, borderColor: colors.border, backgroundColor: colors.bgElevated },
            ]}
            placeholderTextColor={colors.textMuted}
            editable={ordType === OrdType.Limit}
          />
        </View>
        <View style={styles.field}>
          <Text style={[styles.label, { color: colors.textMuted }]}>QUANTITY</Text>
          <TextInput
            value={qtyText}
            onChangeText={setQtyText}
            keyboardType="number-pad"
            style={[
              styles.input,
              { color: colors.text, borderColor: colors.border, backgroundColor: colors.bgElevated },
            ]}
            placeholderTextColor={colors.textMuted}
          />
        </View>
      </View>

      <View style={styles.spacer} />

      <SegmentedControl<OrdType>
        value={ordType}
        options={[
          { label: 'LIMIT', value: OrdType.Limit },
          { label: 'MARKET', value: OrdType.Market },
        ]}
        onChange={setOrdType}
      />

      <View style={styles.spacer} />

      <SegmentedControl<Tif>
        value={tif}
        options={[
          { label: 'DAY', value: Tif.Day },
          { label: 'IOC', value: Tif.Ioc },
          { label: 'FOK', value: Tif.Fok },
        ]}
        onChange={setTif}
      />

      <View style={styles.spacer} />

      <Pressable
        accessibilityRole="button"
        disabled={!isLive}
        onPress={submit}
        style={({ pressed }) => [
          styles.submit,
          {
            backgroundColor: side === Side.Buy ? colors.buy : colors.sell,
            opacity: !isLive ? 0.4 : pressed ? 0.85 : 1,
          },
        ]}
      >
        <Text style={[styles.submitLabel, { color: colors.primaryFg }]}>
          {side === Side.Buy ? 'PLACE BUY' : 'PLACE SELL'} {qtyText} @{' '}
          {ordType === OrdType.Limit ? priceText : 'MKT'}
        </Text>
      </Pressable>

      {lastSubmitted && (
        <Text style={[styles.lastSent, { color: colors.textMuted }]}>{lastSubmitted}</Text>
      )}
    </View>
  );
};

const styles = StyleSheet.create({
  spacer: { height: 10 },
  fieldRow: { flexDirection: 'row', gap: 10 },
  field: { flex: 1 },
  label: { fontSize: 11, fontWeight: '700', letterSpacing: 1, marginBottom: 4 },
  input: {
    borderWidth: StyleSheet.hairlineWidth,
    borderRadius: 8,
    paddingHorizontal: 12,
    paddingVertical: 10,
    fontSize: 16,
  },
  submit: {
    paddingVertical: 14,
    borderRadius: 10,
    alignItems: 'center',
  },
  submitLabel: { fontWeight: '700', fontSize: 14, letterSpacing: 0.5 },
  lastSent: { marginTop: 8, fontSize: 12 },
});
