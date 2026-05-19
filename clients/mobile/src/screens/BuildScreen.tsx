import React, { useEffect } from 'react';
import {
  FlatList,
  Pressable,
  RefreshControl,
  StyleSheet,
  Text,
  View,
} from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import { useNavigation } from '@react-navigation/native';

import { BotConfigSummary } from '../bots/api';
import { Card } from '../components/Card';
import { useMyBots } from '../state/myBots';
import { useSettings } from '../state/settings';
import { useTheme } from '../theme/ThemeProvider';

export const BuildScreen: React.FC = () => {
  const navigation = useNavigation<{
    navigate: (screen: string, params?: object) => void;
  }>();
  const { colors } = useTheme();
  const { values } = useSettings();
  const { client, configure, bots, botsLoading, botsError, refreshBots } = useMyBots();

  useEffect(() => {
    configure(values.botServiceUrl, values.botServiceKey);
  }, [values.botServiceUrl, values.botServiceKey, configure]);

  useEffect(() => {
    if (client) void refreshBots();
  }, [client, refreshBots]);

  const renderRow = ({ item }: { item: BotConfigSummary }) => (
    <Pressable
      onPress={() => navigation.navigate('MyBotDetail', { botId: item.id })}
      style={({ pressed }) => [
        styles.row,
        {
          backgroundColor: colors.surface,
          borderColor: colors.border,
          opacity: pressed ? 0.85 : 1,
        },
      ]}
    >
      <View style={{ flex: 1 }}>
        <Text style={[styles.rowTitle, { color: colors.text }]} numberOfLines={1}>
          {item.name}
        </Text>
        <Text style={[styles.rowMeta, { color: colors.textMuted }]}>
          source: {item.source}
        </Text>
      </View>
      <View style={styles.badges}>
        <Badge label={item.status.toUpperCase()} tone={statusTone(item.status, colors)} />
        {item.published_listing_id && (
          <Badge label="PUBLISHED" tone={colors.primary} />
        )}
      </View>
    </Pressable>
  );

  return (
    <SafeAreaView style={[styles.safe, { backgroundColor: colors.bg }]} edges={['top']}>
      <View style={styles.headerRow}>
        <Text style={[styles.heading, { color: colors.text }]}>Build</Text>
        <Pressable
          onPress={() => navigation.navigate('TemplatePicker')}
          style={[styles.newBtn, { backgroundColor: colors.primary }]}
        >
          <Text style={{ color: colors.primaryFg, fontWeight: '700', fontSize: 12 }}>
            + NEW
          </Text>
        </Pressable>
      </View>

      {!client ? (
        <Card title="Bot service not configured">
          <Text style={{ color: colors.textMuted }}>
            Set the bot-service URL and API key on the Connect tab first.
          </Text>
        </Card>
      ) : botsError ? (
        <Card title="Could not load bots">
          <Text style={{ color: colors.danger }}>{botsError}</Text>
        </Card>
      ) : null}

      <FlatList
        data={bots}
        keyExtractor={b => b.id}
        renderItem={renderRow}
        contentContainerStyle={styles.list}
        refreshControl={
          <RefreshControl
            refreshing={botsLoading}
            onRefresh={() => {
              void refreshBots();
            }}
            tintColor={colors.text}
          />
        }
        ListEmptyComponent={
          client && !botsLoading ? (
            <Text style={[styles.empty, { color: colors.textMuted }]}>
              No bots yet. Tap{' '}
              <Text style={{ color: colors.primary, fontWeight: '700' }}>+ NEW</Text> to
              start from a template.
            </Text>
          ) : null
        }
      />
    </SafeAreaView>
  );
};

const Badge: React.FC<{ label: string; tone: string }> = ({ label, tone }) => {
  const { colors } = useTheme();
  return (
    <View style={[styles.badge, { borderColor: tone, backgroundColor: colors.bgElevated }]}>
      <Text style={[styles.badgeLabel, { color: tone }]}>{label}</Text>
    </View>
  );
};

function statusTone(
  s: BotConfigSummary['status'],
  colors: { ok: string; warn: string; danger: string; textMuted: string }
): string {
  switch (s) {
    case 'paper':
      return colors.warn;
    case 'live':
      return colors.ok;
    case 'paused':
    case 'stopped':
      return colors.danger;
    case 'draft':
    default:
      return colors.textMuted;
  }
}

const styles = StyleSheet.create({
  safe: { flex: 1 },
  headerRow: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingHorizontal: 16,
    paddingTop: 8,
    paddingBottom: 12,
  },
  heading: { fontSize: 22, fontWeight: '700' },
  newBtn: { paddingHorizontal: 14, paddingVertical: 8, borderRadius: 8 },
  list: { padding: 16, paddingTop: 0 },
  row: {
    flexDirection: 'row',
    alignItems: 'center',
    padding: 12,
    borderRadius: 12,
    borderWidth: StyleSheet.hairlineWidth,
    marginBottom: 10,
    gap: 10,
  },
  rowTitle: { fontSize: 15, fontWeight: '700' },
  rowMeta: { fontSize: 11, marginTop: 2 },
  badges: { gap: 4, alignItems: 'flex-end' },
  badge: {
    paddingHorizontal: 6,
    paddingVertical: 2,
    borderRadius: 999,
    borderWidth: StyleSheet.hairlineWidth,
  },
  badgeLabel: { fontSize: 9, fontWeight: '700', letterSpacing: 0.8 },
  empty: { padding: 32, textAlign: 'center', fontSize: 13, lineHeight: 18 },
});
