import React from 'react';
import { Pressable, StyleSheet, Text, View } from 'react-native';

import { useTheme } from '../theme/ThemeProvider';

interface Option<T> {
  label: string;
  value: T;
  tone?: 'buy' | 'sell' | 'neutral';
}

interface Props<T> {
  value: T;
  options: Option<T>[];
  onChange: (v: T) => void;
}

export function SegmentedControl<T>({ value, options, onChange }: Props<T>) {
  const { colors } = useTheme();
  return (
    <View style={[styles.row, { backgroundColor: colors.bgElevated, borderColor: colors.border }]}>
      {options.map(o => {
        const active = o.value === value;
        const tone =
          o.tone === 'buy' ? colors.buy : o.tone === 'sell' ? colors.sell : colors.primary;
        const bg = active ? tone : 'transparent';
        const fg = active ? colors.primaryFg : colors.text;
        return (
          <Pressable
            key={String(o.value)}
            onPress={() => onChange(o.value)}
            style={[styles.btn, { backgroundColor: bg }]}
            accessibilityRole="button"
          >
            <Text style={[styles.label, { color: fg }]}>{o.label}</Text>
          </Pressable>
        );
      })}
    </View>
  );
}

const styles = StyleSheet.create({
  row: {
    flexDirection: 'row',
    borderRadius: 10,
    borderWidth: StyleSheet.hairlineWidth,
    padding: 3,
    gap: 3,
  },
  btn: {
    flex: 1,
    paddingVertical: 8,
    borderRadius: 7,
    alignItems: 'center',
  },
  label: {
    fontWeight: '600',
    fontSize: 13,
  },
});
