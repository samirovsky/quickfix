import React from 'react';
import { Pressable, ScrollView, StyleSheet, Text } from 'react-native';

import { useTheme } from '../theme/ThemeProvider';

interface Props {
  count: number;
  value: number;
  onChange: (v: number) => void;
}

export const SymbolPicker: React.FC<Props> = ({ count, value, onChange }) => {
  const { colors } = useTheme();
  return (
    <ScrollView horizontal showsHorizontalScrollIndicator={false} contentContainerStyle={styles.row}>
      {Array.from({ length: count }, (_, i) => i).map(i => {
        const active = i === value;
        return (
          <Pressable
            key={i}
            onPress={() => onChange(i)}
            style={[
              styles.chip,
              {
                backgroundColor: active ? colors.primary : colors.bgElevated,
                borderColor: active ? colors.primary : colors.border,
              },
            ]}
          >
            <Text
              style={[
                styles.label,
                { color: active ? colors.primaryFg : colors.text, fontWeight: active ? '700' : '500' },
              ]}
            >
              SYM-{i}
            </Text>
          </Pressable>
        );
      })}
    </ScrollView>
  );
};

const styles = StyleSheet.create({
  row: { gap: 6, paddingVertical: 4 },
  chip: {
    paddingHorizontal: 12,
    paddingVertical: 6,
    borderRadius: 999,
    borderWidth: StyleSheet.hairlineWidth,
  },
  label: { fontSize: 12, letterSpacing: 0.5 },
});
