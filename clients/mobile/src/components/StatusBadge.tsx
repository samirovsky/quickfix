import React from 'react';
import { StyleSheet, Text, View } from 'react-native';

import { useTheme } from '../theme/ThemeProvider';
import { ConnState } from '../transport/client';

export const StatusBadge: React.FC<{ state: ConnState }> = ({ state }) => {
  const { colors } = useTheme();
  const [label, dotColor]: [string, string] = (() => {
    switch (state.kind) {
      case 'open':
        return ['LIVE', colors.ok];
      case 'connecting':
        return ['CONNECTING', colors.warn];
      case 'closed':
        return [`CLOSED${state.code ? ` (${state.code})` : ''}`, colors.textMuted];
      case 'error':
        return ['ERROR', colors.danger];
      case 'idle':
      default:
        return ['IDLE', colors.textMuted];
    }
  })();
  return (
    <View style={[styles.row, { borderColor: colors.border, backgroundColor: colors.bgElevated }]}>
      <View style={[styles.dot, { backgroundColor: dotColor }]} />
      <Text style={[styles.label, { color: colors.text }]}>{label}</Text>
    </View>
  );
};

const styles = StyleSheet.create({
  row: {
    flexDirection: 'row',
    alignItems: 'center',
    paddingHorizontal: 8,
    paddingVertical: 4,
    borderRadius: 8,
    borderWidth: StyleSheet.hairlineWidth,
    gap: 6,
  },
  dot: { width: 8, height: 8, borderRadius: 4 },
  label: { fontSize: 11, fontWeight: '700', letterSpacing: 1 },
});
