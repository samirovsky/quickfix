import React, { useEffect, useMemo, useState } from 'react';
import {
  Alert,
  Pressable,
  ScrollView,
  StyleSheet,
  Text,
  TextInput,
  View,
} from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import { useNavigation, useRoute } from '@react-navigation/native';

import { Card } from '../components/Card';
import { PerformanceChart } from '../components/PerformanceChart';
import { SegmentedControl } from '../components/SegmentedControl';
import { useMarketplace } from '../state/marketplace';
import { useTheme } from '../theme/ThemeProvider';

const formatCents = (c: number) => `$${(c / 100).toFixed(2)}`;
const formatPct = (p: number) => `${(p * 100).toFixed(1)}%`;

export const ListingDetailScreen: React.FC = () => {
  const { colors } = useTheme();
  const navigation = useNavigation();
  const route = useRoute();
  const { listingId } = route.params as { listingId: string };
  const {
    details,
    performance,
    loadingPerformance,
    loadListingDetail,
    subscribing,
    subscribe,
    subscriptions,
    refreshSubscriptions,
  } = useMarketplace();

  const [window, setWindow] = useState<7 | 30 | 90>(30);
  const [allocateInput, setAllocateInput] = useState('100');

  useEffect(() => {
    void loadListingDetail(listingId, window);
  }, [listingId, window, loadListingDetail]);

  useEffect(() => {
    void refreshSubscriptions();
  }, [refreshSubscriptions]);

  const listing = details[listingId];
  const metrics = performance[listingId];
  const isLoading = !!loadingPerformance[listingId];
  const isSubscribed = useMemo(
    () => subscriptions.some(s => s.listing_id === listingId && s.status === 'active'),
    [subscriptions, listingId]
  );

  const onSubscribe = async () => {
    const dollars = Number.parseFloat(allocateInput);
    if (!Number.isFinite(dollars) || dollars <= 0) {
      Alert.alert('Bad allocation', 'Enter a positive dollar amount.');
      return;
    }
    try {
      await subscribe(listingId, Math.round(dollars * 100));
      Alert.alert('Subscribed', `Allocated ${formatCents(Math.round(dollars * 100))}.`);
    } catch (e) {
      Alert.alert('Subscribe failed', (e as Error).message);
    }
  };

  return (
    <SafeAreaView style={[styles.safe, { backgroundColor: colors.bg }]} edges={['top']}>
      <ScrollView contentContainerStyle={styles.scroll}>
        <Pressable onPress={navigation.goBack} style={styles.backBtn}>
          <Text style={{ color: colors.primary, fontWeight: '600' }}>‹ Marketplace</Text>
        </Pressable>

        {!listing ? (
          <Card title="Loading…">
            <Text style={{ color: colors.textMuted }}>
              {isLoading ? 'Fetching listing detail.' : 'No data yet.'}
            </Text>
          </Card>
        ) : (
          <>
            <View style={styles.headerRow}>
              <Text style={[styles.title, { color: colors.text }]}>{listing.title}</Text>
              <Text style={[styles.creator, { color: colors.textMuted }]}>
                by {listing.creator_name} · {listing.total_subscribers} subscribers
              </Text>
            </View>

            <Card title="About">
              <Text style={{ color: colors.text, lineHeight: 19 }}>{listing.summary}</Text>
            </Card>

            <Card title="Performance">
              <View style={{ marginBottom: 10 }}>
                <SegmentedControl<7 | 30 | 90>
                  value={window}
                  options={[
                    { label: '7D', value: 7 },
                    { label: '30D', value: 30 },
                    { label: '90D', value: 90 },
                  ]}
                  onChange={setWindow}
                />
              </View>

              {metrics ? (
                <>
                  <View style={styles.statsRow}>
                    <Stat
                      label="P&L"
                      value={formatCents(metrics.total_realised_pnl_cents)}
                      tone={metrics.total_realised_pnl_cents >= 0 ? colors.ok : colors.danger}
                    />
                    <Stat label="Trades" value={String(metrics.total_trades)} tone={colors.text} />
                    <Stat
                      label="Win rate"
                      value={metrics.total_trades > 0 ? formatPct(metrics.win_rate) : '—'}
                      tone={colors.text}
                    />
                  </View>
                  <View style={styles.statsRow}>
                    <Stat
                      label="Best"
                      value={formatCents(metrics.largest_win_cents)}
                      tone={colors.ok}
                    />
                    <Stat
                      label="Worst"
                      value={formatCents(metrics.largest_loss_cents)}
                      tone={colors.danger}
                    />
                    <View style={styles.stat} />
                  </View>
                  <View style={{ height: 10 }} />
                  <PerformanceChart metrics={metrics} />
                </>
              ) : (
                <Text style={{ color: colors.textMuted }}>Loading metrics…</Text>
              )}
            </Card>

            <Card title={isSubscribed ? 'Subscribed' : 'Subscribe'}>
              <Text style={[styles.priceLine, { color: colors.text }]}>
                {formatCents(listing.monthly_price_cents)}{' '}
                <Text style={{ color: colors.textMuted, fontSize: 13 }}>/ month</Text>
              </Text>
              <Text style={[styles.smallNote, { color: colors.textMuted }]}>
                Billing is stubbed in this build — no money will move.
              </Text>

              <Text style={[styles.label, { color: colors.textMuted, marginTop: 12 }]}>
                ALLOCATED CAPITAL ($)
              </Text>
              <TextInput
                value={allocateInput}
                onChangeText={setAllocateInput}
                keyboardType="decimal-pad"
                editable={!isSubscribed}
                style={[
                  styles.input,
                  {
                    color: colors.text,
                    borderColor: colors.border,
                    backgroundColor: colors.bgElevated,
                    opacity: isSubscribed ? 0.6 : 1,
                  },
                ]}
              />
              <Pressable
                onPress={onSubscribe}
                disabled={isSubscribed || !!subscribing[listingId]}
                style={({ pressed }) => [
                  styles.cta,
                  {
                    backgroundColor: isSubscribed ? colors.bgElevated : colors.primary,
                    borderColor: isSubscribed ? colors.border : 'transparent',
                    borderWidth: isSubscribed ? StyleSheet.hairlineWidth : 0,
                    opacity: pressed ? 0.9 : 1,
                  },
                ]}
              >
                <Text
                  style={[
                    styles.ctaLabel,
                    { color: isSubscribed ? colors.text : colors.primaryFg },
                  ]}
                >
                  {isSubscribed
                    ? 'ALREADY SUBSCRIBED'
                    : subscribing[listingId]
                      ? 'SUBSCRIBING…'
                      : 'SUBSCRIBE'}
                </Text>
              </Pressable>
            </Card>
          </>
        )}
      </ScrollView>
    </SafeAreaView>
  );
};

const Stat: React.FC<{ label: string; value: string; tone: string }> = ({
  label,
  value,
  tone,
}) => {
  const { colors } = useTheme();
  return (
    <View style={styles.stat}>
      <Text style={[styles.statLabel, { color: colors.textMuted }]}>{label}</Text>
      <Text style={[styles.statValue, { color: tone }]}>{value}</Text>
    </View>
  );
};

const styles = StyleSheet.create({
  safe: { flex: 1 },
  scroll: { padding: 16, paddingBottom: 32 },
  backBtn: { paddingVertical: 6, marginBottom: 8 },
  headerRow: { marginBottom: 12 },
  title: { fontSize: 22, fontWeight: '700' },
  creator: { fontSize: 12, marginTop: 4 },
  statsRow: { flexDirection: 'row', gap: 10, marginBottom: 6 },
  stat: { flex: 1 },
  statLabel: { fontSize: 10, fontWeight: '700', letterSpacing: 0.8 },
  statValue: { fontSize: 16, fontWeight: '700', fontVariant: ['tabular-nums'], marginTop: 2 },
  priceLine: { fontSize: 20, fontWeight: '700' },
  smallNote: { fontSize: 11, marginTop: 4 },
  label: { fontSize: 11, fontWeight: '700', letterSpacing: 0.8, marginBottom: 4 },
  input: {
    borderWidth: StyleSheet.hairlineWidth,
    borderRadius: 8,
    paddingHorizontal: 12,
    paddingVertical: 10,
    fontSize: 15,
  },
  cta: {
    marginTop: 12,
    paddingVertical: 14,
    borderRadius: 10,
    alignItems: 'center',
  },
  ctaLabel: { fontWeight: '700', letterSpacing: 0.5 },
});
