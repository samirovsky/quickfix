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

import { Listing } from '../bots/api';
import { Card } from '../components/Card';
import { useMarketplace } from '../state/marketplace';
import { useSettings } from '../state/settings';
import { useTheme } from '../theme/ThemeProvider';

const formatPrice = (cents: number) => `$${(cents / 100).toFixed(2)}`;

export const MarketplaceScreen: React.FC = () => {
  const navigation = useNavigation<{
    navigate: (screen: string, params?: object) => void;
  }>();
  const { colors } = useTheme();
  const { values } = useSettings();
  const {
    client,
    configure,
    listings,
    listingsLoading,
    listingsError,
    refreshListings,
  } = useMarketplace();

  // Re-configure the REST client whenever the URL or key changes.
  useEffect(() => {
    configure({ demoMode: values.demoMode, baseUrl: values.botServiceUrl, apiKey: values.botServiceKey });
  }, [values.demoMode, values.botServiceUrl, values.botServiceKey, configure]);

  // Initial load once the client exists.
  useEffect(() => {
    if (client) void refreshListings();
  }, [client, refreshListings]);

  const renderItem = ({ item }: { item: Listing }) => (
    <Pressable
      onPress={() => navigation.navigate('ListingDetail', { listingId: item.id })}
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
        <Text style={[styles.title, { color: colors.text }]} numberOfLines={1}>
          {item.title}
        </Text>
        <Text style={[styles.creator, { color: colors.textMuted }]} numberOfLines={1}>
          by {item.creator_name}
        </Text>
        <Text style={[styles.summary, { color: colors.textMuted }]} numberOfLines={2}>
          {item.summary}
        </Text>
      </View>
      <View style={styles.rightCol}>
        <Text style={[styles.price, { color: colors.primary }]}>
          {formatPrice(item.monthly_price_cents)}
        </Text>
        <Text style={[styles.permo, { color: colors.textMuted }]}>/month</Text>
        <Text style={[styles.subs, { color: colors.textMuted }]}>
          {item.total_subscribers} subs
        </Text>
      </View>
    </Pressable>
  );

  return (
    <SafeAreaView style={[styles.safe, { backgroundColor: colors.bg }]} edges={['top']}>
      <View style={styles.headerRow}>
        <Text style={[styles.heading, { color: colors.text }]}>Marketplace</Text>
      </View>
      {!client ? (
        <Card title="Bot service not configured">
          <Text style={{ color: colors.textMuted, marginBottom: 8 }}>
            Set the bot-service URL and API key on the Settings tab to browse listings.
          </Text>
        </Card>
      ) : listingsError ? (
        <Card title="Could not load marketplace">
          <Text style={{ color: colors.danger }}>{listingsError}</Text>
        </Card>
      ) : null}

      <FlatList
        data={listings}
        keyExtractor={l => l.id}
        renderItem={renderItem}
        contentContainerStyle={styles.list}
        refreshControl={
          <RefreshControl
            refreshing={listingsLoading}
            onRefresh={() => {
              void refreshListings();
            }}
            tintColor={colors.text}
          />
        }
        ListEmptyComponent={
          client && !listingsLoading ? (
            <Text style={[styles.empty, { color: colors.textMuted }]}>
              No published bots yet. Run the bot-service with
              {' '}<Text style={styles.mono}>BOT_SERVICE_SEED_DEMO=1</Text>{' '}
              to populate the demo data.
            </Text>
          ) : null
        }
      />
    </SafeAreaView>
  );
};

const styles = StyleSheet.create({
  safe: { flex: 1 },
  headerRow: { paddingHorizontal: 16, paddingTop: 8, paddingBottom: 12 },
  heading: { fontSize: 22, fontWeight: '700' },
  list: { padding: 16, paddingTop: 0 },
  row: {
    flexDirection: 'row',
    padding: 12,
    borderRadius: 12,
    borderWidth: StyleSheet.hairlineWidth,
    marginBottom: 10,
    gap: 10,
  },
  title: { fontSize: 15, fontWeight: '700' },
  creator: { fontSize: 11, marginTop: 2 },
  summary: { fontSize: 12, marginTop: 6 },
  rightCol: { alignItems: 'flex-end' },
  price: { fontSize: 16, fontWeight: '700', fontVariant: ['tabular-nums'] },
  permo: { fontSize: 10 },
  subs: { fontSize: 11, marginTop: 6 },
  empty: { padding: 32, textAlign: 'center', fontSize: 13, lineHeight: 18 },
  mono: { fontFamily: 'monospace' },
});
