import React, { useEffect, useState } from 'react';
import {
  KeyboardAvoidingView,
  Platform,
  Pressable,
  ScrollView,
  StyleSheet,
  Text,
  TextInput,
  View,
} from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';

import { Card } from '../components/Card';
import { StatusBadge } from '../components/StatusBadge';
import { useSettings } from '../state/settings';
import { useTrading } from '../state/trading';
import { useTheme } from '../theme/ThemeProvider';

export const ConnectScreen: React.FC = () => {
  const { colors } = useTheme();
  const { values, update } = useSettings();
  const conn = useTrading(s => s.conn);
  const connect = useTrading(s => s.connect);
  const disconnect = useTrading(s => s.disconnect);

  const [urlDraft, setUrlDraft] = useState(values.serverUrl);

  useEffect(() => {
    setUrlDraft(values.serverUrl);
  }, [values.serverUrl]);

  const onConnect = async () => {
    await update({ serverUrl: urlDraft });
    connect(urlDraft);
  };

  return (
    <SafeAreaView style={[styles.safe, { backgroundColor: colors.bg }]} edges={['top']}>
      <KeyboardAvoidingView
        behavior={Platform.OS === 'ios' ? 'padding' : undefined}
        style={styles.flex}
      >
        <ScrollView contentContainerStyle={styles.scroll}>
          <View style={styles.headerRow}>
            <Text style={[styles.title, { color: colors.text }]}>QFTX Trader</Text>
            <StatusBadge state={conn} />
          </View>
          <Text style={[styles.subtitle, { color: colors.textMuted }]}>
            Connect to a trading-server WebSocket endpoint to place orders, watch fills, and
            manage open positions.
          </Text>

          <Card title="Server">
            <Text style={[styles.label, { color: colors.textMuted }]}>WebSocket URL</Text>
            <TextInput
              value={urlDraft}
              onChangeText={setUrlDraft}
              autoCapitalize="none"
              autoCorrect={false}
              keyboardType="url"
              placeholder="ws://10.0.2.2:9002/ws"
              placeholderTextColor={colors.textMuted}
              style={[
                styles.input,
                { color: colors.text, borderColor: colors.border, backgroundColor: colors.bgElevated },
              ]}
            />
            <Text style={[styles.help, { color: colors.textMuted }]}>
              On the Android emulator, host loopback is <Text style={styles.mono}>10.0.2.2</Text>.
              On a real device on the same Wi-Fi, use the dev machine&apos;s LAN IP.
            </Text>
            <View style={styles.actions}>
              <Pressable
                onPress={onConnect}
                style={[styles.btn, { backgroundColor: colors.primary }]}
              >
                <Text style={[styles.btnLabel, { color: colors.primaryFg }]}>
                  {conn.kind === 'open' ? 'RECONNECT' : 'CONNECT'}
                </Text>
              </Pressable>
              <Pressable
                onPress={disconnect}
                style={[
                  styles.btn,
                  styles.btnSecondary,
                  { borderColor: colors.border },
                ]}
              >
                <Text style={[styles.btnLabel, { color: colors.text }]}>DISCONNECT</Text>
              </Pressable>
            </View>
          </Card>

          <Card title="Defaults">
            <Text style={[styles.label, { color: colors.textMuted }]}>Default symbol id</Text>
            <TextInput
              value={values.defaultSymbolId.toString()}
              onChangeText={t => {
                const n = parseInt(t, 10);
                if (Number.isFinite(n) && n >= 0 && n < values.symbolCount) {
                  update({ defaultSymbolId: n });
                }
              }}
              keyboardType="number-pad"
              style={[
                styles.input,
                { color: colors.text, borderColor: colors.border, backgroundColor: colors.bgElevated },
              ]}
            />
            <Text style={[styles.label, { color: colors.textMuted, marginTop: 12 }]}>
              Symbol count (must match server TRADING_SYMBOLS)
            </Text>
            <TextInput
              value={values.symbolCount.toString()}
              onChangeText={t => {
                const n = parseInt(t, 10);
                if (Number.isFinite(n) && n > 0 && n <= 4096) update({ symbolCount: n });
              }}
              keyboardType="number-pad"
              style={[
                styles.input,
                { color: colors.text, borderColor: colors.border, backgroundColor: colors.bgElevated },
              ]}
            />
          </Card>

          <Card title="Bot service (marketplace)">
            <Text style={[styles.label, { color: colors.textMuted }]}>HTTP URL</Text>
            <TextInput
              value={values.botServiceUrl}
              onChangeText={t => update({ botServiceUrl: t })}
              autoCapitalize="none"
              autoCorrect={false}
              keyboardType="url"
              placeholder="http://10.0.2.2:9100"
              placeholderTextColor={colors.textMuted}
              style={[
                styles.input,
                { color: colors.text, borderColor: colors.border, backgroundColor: colors.bgElevated },
              ]}
            />
            <Text style={[styles.label, { color: colors.textMuted, marginTop: 12 }]}>
              API key
            </Text>
            <TextInput
              value={values.botServiceKey}
              onChangeText={t => update({ botServiceKey: t })}
              autoCapitalize="none"
              autoCorrect={false}
              secureTextEntry
              placeholder="X-API-Key for bot-service"
              placeholderTextColor={colors.textMuted}
              style={[
                styles.input,
                { color: colors.text, borderColor: colors.border, backgroundColor: colors.bgElevated },
              ]}
            />
            <Text style={[styles.help, { color: colors.textMuted }]}>
              Run the server with{' '}
              <Text style={styles.mono}>BOT_SERVICE_SEED_DEMO=1</Text>{' '}
              to get two pre-provisioned keys:{' '}
              <Text style={styles.mono}>demo-alice-please-rotate</Text>
              {' '}/{' '}
              <Text style={styles.mono}>demo-bob-please-rotate</Text>.
            </Text>
          </Card>

          {conn.kind === 'error' && (
            <Card title="Last error" style={{ borderColor: colors.danger }}>
              <Text style={{ color: colors.danger }}>{conn.message}</Text>
            </Card>
          )}
          {conn.kind === 'closed' && conn.reason && (
            <Card title="Last close">
              <Text style={{ color: colors.text }}>
                {`code ${conn.code ?? '?'} — ${conn.reason}`}
              </Text>
            </Card>
          )}
        </ScrollView>
      </KeyboardAvoidingView>
    </SafeAreaView>
  );
};

const styles = StyleSheet.create({
  safe: { flex: 1 },
  flex: { flex: 1 },
  scroll: { padding: 16, paddingBottom: 32 },
  headerRow: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    marginBottom: 4,
  },
  title: { fontSize: 22, fontWeight: '700' },
  subtitle: { fontSize: 13, marginBottom: 16 },
  label: { fontSize: 11, fontWeight: '700', letterSpacing: 1, marginBottom: 4 },
  input: {
    borderWidth: StyleSheet.hairlineWidth,
    borderRadius: 8,
    paddingHorizontal: 12,
    paddingVertical: 10,
    fontSize: 15,
  },
  help: { fontSize: 11, marginTop: 8, lineHeight: 16 },
  mono: { fontFamily: Platform.select({ ios: 'Menlo', android: 'monospace' }) },
  actions: { flexDirection: 'row', gap: 8, marginTop: 12 },
  btn: { flex: 1, paddingVertical: 12, borderRadius: 8, alignItems: 'center' },
  btnSecondary: { backgroundColor: 'transparent', borderWidth: StyleSheet.hairlineWidth },
  btnLabel: { fontWeight: '700', letterSpacing: 0.5 },
});
