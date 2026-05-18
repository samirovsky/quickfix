import React from 'react';
import { Pressable, ScrollView, StyleSheet, Text, View } from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';

import { Card } from '../components/Card';
import { SegmentedControl } from '../components/SegmentedControl';
import { ThemeMode, useSettings } from '../state/settings';
import { useTrading } from '../state/trading';
import { useTheme } from '../theme/ThemeProvider';

export const SettingsScreen: React.FC = () => {
  const { colors } = useTheme();
  const { values, update, reset } = useSettings();
  const resetSession = useTrading(s => s.resetSession);
  const clearFeed = useTrading(s => s.clearFeed);

  return (
    <SafeAreaView style={[styles.safe, { backgroundColor: colors.bg }]} edges={['top']}>
      <ScrollView contentContainerStyle={styles.scroll}>
        <Text style={[styles.title, { color: colors.text }]}>Settings</Text>

        <Card title="Theme">
          <SegmentedControl<ThemeMode>
            value={values.themeMode}
            options={[
              { label: 'SYSTEM', value: 'system' },
              { label: 'LIGHT', value: 'light' },
              { label: 'DARK', value: 'dark' },
            ]}
            onChange={mode => update({ themeMode: mode })}
          />
        </Card>

        <Card title="Connection">
          <Row label="WebSocket URL" value={values.serverUrl} colors={colors} />
          <Row label="Default symbol" value={`SYM-${values.defaultSymbolId}`} colors={colors} />
          <Row label="Symbol count" value={String(values.symbolCount)} colors={colors} />
          <Text style={[styles.help, { color: colors.textMuted }]}>
            Edit these on the Connect tab.
          </Text>
        </Card>

        <Card title="Session">
          <Pressable
            onPress={clearFeed}
            style={[styles.btn, { backgroundColor: colors.bgElevated, borderColor: colors.border }]}
          >
            <Text style={{ color: colors.text, fontWeight: '600' }}>Clear execution feed</Text>
          </Pressable>
          <View style={styles.btnSpacer} />
          <Pressable
            onPress={resetSession}
            style={[styles.btn, { backgroundColor: colors.bgElevated, borderColor: colors.warn }]}
          >
            <Text style={{ color: colors.warn, fontWeight: '600' }}>
              Reset session state (orders, fills, P&amp;L)
            </Text>
          </Pressable>
          <View style={styles.btnSpacer} />
          <Pressable
            onPress={reset}
            style={[styles.btn, { backgroundColor: colors.bgElevated, borderColor: colors.danger }]}
          >
            <Text style={{ color: colors.danger, fontWeight: '600' }}>
              Reset all settings to defaults
            </Text>
          </Pressable>
        </Card>

        <Card title="About">
          <Text style={{ color: colors.textMuted, lineHeight: 18, fontSize: 12 }}>
            QFTX Trader is a cross-platform mobile client for the QFTX low-latency trading server.
            It speaks the same custom binary QFTX frames as the TCP transport, packaged one frame
            per WebSocket binary message.{'\n\n'}
            Order book and depth chart show the client&apos;s own resting orders only.
            Market-wide depth needs the gRPC SubscribeMarketData stream (planned).
          </Text>
        </Card>
      </ScrollView>
    </SafeAreaView>
  );
};

const Row: React.FC<{ label: string; value: string; colors: { text: string; textMuted: string; border: string } }> = ({
  label,
  value,
  colors,
}) => (
  <View style={[styles.row, { borderBottomColor: colors.border }]}>
    <Text style={{ color: colors.textMuted, fontSize: 12 }}>{label}</Text>
    <Text
      numberOfLines={1}
      style={{ color: colors.text, flexShrink: 1, marginLeft: 12, textAlign: 'right' }}
    >
      {value}
    </Text>
  </View>
);

const styles = StyleSheet.create({
  safe: { flex: 1 },
  scroll: { padding: 16, paddingBottom: 32 },
  title: { fontSize: 22, fontWeight: '700', marginBottom: 12 },
  row: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    paddingVertical: 8,
    borderBottomWidth: StyleSheet.hairlineWidth,
  },
  help: { marginTop: 8, fontSize: 11 },
  btn: {
    paddingVertical: 12,
    borderRadius: 10,
    borderWidth: StyleSheet.hairlineWidth,
    alignItems: 'center',
  },
  btnSpacer: { height: 8 },
});
