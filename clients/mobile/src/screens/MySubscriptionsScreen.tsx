import React, { useEffect } from 'react';
import {
  Alert,
  Pressable,
  RefreshControl,
  ScrollView,
  StyleSheet,
  Text,
  View,
} from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import { useNavigation } from '@react-navigation/native';

import { Card } from '../components/Card';
import { useMarketplace } from '../state/marketplace';
import { useSettings } from '../state/settings';
import { useTheme } from '../theme/ThemeProvider';

const formatCents = (c: number) => `$${(c / 100).toFixed(2)}`;

export const MySubscriptionsScreen: React.FC = () => {
  const navigation = useNavigation<{
    navigate: (screen: string, params?: object) => void;
  }>();
  const { colors } = useTheme();
  const { values } = useSettings();
  const { client, configure, subscriptions, subscriptionsLoading, refreshSubscriptions, cancel } =
    useMarketplace();

  useEffect(() => {
    configure(values.botServiceUrl, values.botServiceKey);
  }, [values.botServiceUrl, values.botServiceKey, configure]);

  useEffect(() => {
    if (client) void refreshSubscriptions();
  }, [client, refreshSubscriptions]);

  const onCancel = (subId: string, listingTitle: string) => {
    Alert.alert('Cancel subscription?', `You will stop following "${listingTitle}".`, [
      { text: 'Keep', style: 'cancel' },
      {
        text: 'Cancel',
        style: 'destructive',
        onPress: async () => {
          try {
            await cancel(subId);
          } catch (e) {
            Alert.alert('Cancel failed', (e as Error).message);
          }
        },
      },
    ]);
  };

  return (
    <SafeAreaView style={[styles.safe, { backgroundColor: colors.bg }]} edges={['top']}>
      <ScrollView
        contentContainerStyle={styles.scroll}
        refreshControl={
          <RefreshControl
            refreshing={subscriptionsLoading}
            onRefresh={() => {
              void refreshSubscriptions();
            }}
            tintColor={colors.text}
          />
        }
      >
        <Text style={[styles.title, { color: colors.text }]}>My Subscriptions</Text>

        {!client ? (
          <Card title="Bot service not configured">
            <Text style={{ color: colors.textMuted }}>
              Set the bot-service URL and API key on the Settings tab first.
            </Text>
          </Card>
        ) : subscriptions.length === 0 ? (
          <Card title="Nothing yet">
            <Text style={{ color: colors.textMuted, lineHeight: 19 }}>
              Browse the Marketplace tab to subscribe to a bot. Subscriptions show up here with
              live performance from the creator&apos;s track record.
            </Text>
          </Card>
        ) : (
          subscriptions.map(sub => (
            <Pressable
              key={sub.id}
              onPress={() =>
                navigation.navigate('ListingDetail', { listingId: sub.listing.id })
              }
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
                <Text style={[styles.rowTitle, { color: colors.text }]}>
                  {sub.listing.title}
                </Text>
                <Text style={[styles.rowMeta, { color: colors.textMuted }]}>
                  by {sub.listing.creator_name} ·{' '}
                  {formatCents(sub.listing.monthly_price_cents)}/mo
                </Text>
                <Text style={[styles.rowMeta, { color: colors.textMuted }]}>
                  Allocated {formatCents(sub.allocated_capital_cents)}
                </Text>
              </View>
              <Pressable
                onPress={() => onCancel(sub.id, sub.listing.title)}
                style={[styles.cancelBtn, { borderColor: colors.danger }]}
              >
                <Text style={[styles.cancelLabel, { color: colors.danger }]}>CANCEL</Text>
              </Pressable>
            </Pressable>
          ))
        )}
      </ScrollView>
    </SafeAreaView>
  );
};

const styles = StyleSheet.create({
  safe: { flex: 1 },
  scroll: { padding: 16, paddingBottom: 32 },
  title: { fontSize: 22, fontWeight: '700', marginBottom: 12 },
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
  cancelBtn: {
    paddingHorizontal: 10,
    paddingVertical: 6,
    borderRadius: 8,
    borderWidth: StyleSheet.hairlineWidth,
  },
  cancelLabel: { fontSize: 10, fontWeight: '700', letterSpacing: 1 },
});
