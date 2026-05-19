import React, { useEffect, useMemo, useState } from 'react';
import {
  Alert,
  Modal,
  Pressable,
  ScrollView,
  StyleSheet,
  Text,
  TextInput,
  View,
} from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import { useNavigation, useRoute } from '@react-navigation/native';

import { BotConfig, PositionSizing, Strategy } from '../bots/api';
import { Card } from '../components/Card';
import { PerformanceChart } from '../components/PerformanceChart';
import { SegmentedControl } from '../components/SegmentedControl';
import { useMyBots } from '../state/myBots';
import { useTheme } from '../theme/ThemeProvider';

const fmtCents = (c: number) => `$${(c / 100).toFixed(2)}`;

export const MyBotDetailScreen: React.FC = () => {
  const navigation = useNavigation<{ goBack: () => void }>();
  const route = useRoute();
  const { botId } = route.params as { botId: string };

  const { colors } = useTheme();
  const { bot, botPerf, botLoading, loadBot, updateBot, deleteBot, publish, unpublish } =
    useMyBots();

  const [window, setWindow] = useState<7 | 30 | 90>(30);
  const [publishOpen, setPublishOpen] = useState(false);

  useEffect(() => {
    void loadBot(botId, window);
  }, [botId, window, loadBot]);

  if (!bot || bot.id !== botId) {
    return (
      <SafeAreaView style={[styles.safe, { backgroundColor: colors.bg }]} edges={['top']}>
        <ScrollView contentContainerStyle={styles.scroll}>
          <Pressable onPress={navigation.goBack} style={styles.backBtn}>
            <Text style={{ color: colors.primary, fontWeight: '600' }}>‹ Build</Text>
          </Pressable>
          <Card title="Loading…">
            <Text style={{ color: colors.textMuted }}>
              {botLoading ? 'Fetching bot detail.' : 'No data yet.'}
            </Text>
          </Card>
        </ScrollView>
      </SafeAreaView>
    );
  }

  const published = bot.published_listing_id !== null;

  const onDelete = () => {
    Alert.alert('Delete bot?', `"${bot.name}" will be removed permanently.`, [
      { text: 'Keep', style: 'cancel' },
      {
        text: 'Delete',
        style: 'destructive',
        onPress: async () => {
          try {
            await deleteBot(bot.id);
            navigation.goBack();
          } catch (e) {
            Alert.alert('Delete failed', (e as Error).message);
          }
        },
      },
    ]);
  };

  return (
    <SafeAreaView style={[styles.safe, { backgroundColor: colors.bg }]} edges={['top']}>
      <ScrollView contentContainerStyle={styles.scroll}>
        <Pressable onPress={navigation.goBack} style={styles.backBtn}>
          <Text style={{ color: colors.primary, fontWeight: '600' }}>‹ Build</Text>
        </Pressable>

        <View style={styles.headerRow}>
          <Text style={[styles.title, { color: colors.text }]}>{bot.name}</Text>
          {published && (
            <View style={[styles.publishedTag, { borderColor: colors.primary }]}>
              <Text style={{ color: colors.primary, fontSize: 10, fontWeight: '700' }}>
                PUBLISHED
              </Text>
            </View>
          )}
        </View>

        <NameDescriptionCard bot={bot} onSave={updateBot} colors={colors} />

        <Card title="Risk &amp; sizing">
          <RiskEditor bot={bot} onSave={updateBot} colors={colors} />
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
          {botPerf ? (
            <>
              <View style={styles.statsRow}>
                <Stat
                  label="P&L"
                  value={fmtCents(botPerf.total_realised_pnl_cents)}
                  tone={botPerf.total_realised_pnl_cents >= 0 ? colors.ok : colors.danger}
                />
                <Stat
                  label="Trades"
                  value={String(botPerf.total_trades)}
                  tone={colors.text}
                />
                <Stat
                  label="Win rate"
                  value={
                    botPerf.total_trades > 0
                      ? `${(botPerf.win_rate * 100).toFixed(1)}%`
                      : '—'
                  }
                  tone={colors.text}
                />
              </View>
              <View style={{ height: 8 }} />
              <PerformanceChart metrics={botPerf} />
            </>
          ) : (
            <Text style={{ color: colors.textMuted }}>Loading metrics…</Text>
          )}
        </Card>

        <Card title="Marketplace">
          {published ? (
            <>
              <Text style={{ color: colors.text, marginBottom: 8 }}>
                This bot is live in the marketplace.
              </Text>
              <Pressable
                onPress={() =>
                  Alert.alert(
                    'Unpublish?',
                    'The listing will be hidden and no new subscribers can join.',
                    [
                      { text: 'Keep listed', style: 'cancel' },
                      {
                        text: 'Unpublish',
                        style: 'destructive',
                        onPress: async () => {
                          try {
                            await unpublish(bot.id, bot.published_listing_id!);
                          } catch (e) {
                            Alert.alert('Unpublish failed', (e as Error).message);
                          }
                        },
                      },
                    ]
                  )
                }
                style={[styles.cta, { backgroundColor: colors.bgElevated, borderColor: colors.danger, borderWidth: StyleSheet.hairlineWidth }]}
              >
                <Text style={{ color: colors.danger, fontWeight: '700' }}>UNPUBLISH</Text>
              </Pressable>
            </>
          ) : (
            <Pressable
              onPress={() => setPublishOpen(true)}
              style={[styles.cta, { backgroundColor: colors.primary }]}
            >
              <Text style={{ color: colors.primaryFg, fontWeight: '700' }}>
                PUBLISH TO MARKETPLACE
              </Text>
            </Pressable>
          )}
        </Card>

        <Card title="Danger zone">
          <Pressable
            onPress={onDelete}
            style={[
              styles.cta,
              { backgroundColor: colors.bgElevated, borderColor: colors.danger, borderWidth: StyleSheet.hairlineWidth },
            ]}
          >
            <Text style={{ color: colors.danger, fontWeight: '700' }}>DELETE BOT</Text>
          </Pressable>
        </Card>
      </ScrollView>

      <PublishModal
        visible={publishOpen}
        onClose={() => setPublishOpen(false)}
        defaultTitle={bot.name}
        defaultSummary={bot.description}
        onConfirm={async (title, summary, priceCents) => {
          try {
            await publish(bot.id, title, summary, priceCents);
            setPublishOpen(false);
          } catch (e) {
            Alert.alert('Publish failed', (e as Error).message);
          }
        }}
      />
    </SafeAreaView>
  );
};

// ---------- subcomponents ----------

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

const NameDescriptionCard: React.FC<{
  bot: BotConfig;
  onSave: (
    id: string,
    patch: { name?: string; description?: string; strategy?: Strategy }
  ) => Promise<BotConfig | null>;
  colors: { text: string; textMuted: string; border: string; bgElevated: string; primary: string; primaryFg: string };
}> = ({ bot, onSave, colors }) => {
  const [name, setName] = useState(bot.name);
  const [description, setDescription] = useState(bot.description);
  const dirty = name !== bot.name || description !== bot.description;
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    setName(bot.name);
    setDescription(bot.description);
  }, [bot.name, bot.description]);

  return (
    <Card title="Identity">
      <Text style={[styles.label, { color: colors.textMuted }]}>NAME</Text>
      <TextInput
        value={name}
        onChangeText={setName}
        style={[
          styles.input,
          { color: colors.text, borderColor: colors.border, backgroundColor: colors.bgElevated },
        ]}
      />
      <Text style={[styles.label, { color: colors.textMuted, marginTop: 12 }]}>DESCRIPTION</Text>
      <TextInput
        value={description}
        onChangeText={setDescription}
        multiline
        style={[
          styles.input,
          styles.inputMulti,
          { color: colors.text, borderColor: colors.border, backgroundColor: colors.bgElevated },
        ]}
      />
      <Pressable
        disabled={!dirty || saving}
        onPress={async () => {
          setSaving(true);
          try {
            await onSave(bot.id, { name, description });
          } finally {
            setSaving(false);
          }
        }}
        style={({ pressed }) => [
          styles.cta,
          {
            backgroundColor: colors.primary,
            opacity: !dirty || saving ? 0.4 : pressed ? 0.9 : 1,
          },
        ]}
      >
        <Text style={{ color: colors.primaryFg, fontWeight: '700' }}>
          {saving ? 'SAVING…' : 'SAVE'}
        </Text>
      </Pressable>
    </Card>
  );
};

const RiskEditor: React.FC<{
  bot: BotConfig;
  onSave: (
    id: string,
    patch: { name?: string; description?: string; strategy?: Strategy }
  ) => Promise<BotConfig | null>;
  colors: { text: string; textMuted: string; border: string; bgElevated: string; primary: string; primaryFg: string };
}> = ({ bot, onSave, colors }) => {
  const [maxConcurrent, setMaxConcurrent] = useState(
    String(bot.strategy.risk.max_concurrent)
  );
  const [maxDailyLoss, setMaxDailyLoss] = useState(
    (bot.strategy.risk.max_daily_loss_cents / 100).toFixed(2)
  );
  const [sizing, setSizing] = useState<string>(formatSizing(bot.strategy.position_sizing));
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    setMaxConcurrent(String(bot.strategy.risk.max_concurrent));
    setMaxDailyLoss((bot.strategy.risk.max_daily_loss_cents / 100).toFixed(2));
    setSizing(formatSizing(bot.strategy.position_sizing));
  }, [bot.strategy]);

  const dirty = useMemo(() => {
    return (
      maxConcurrent !== String(bot.strategy.risk.max_concurrent) ||
      maxDailyLoss !== (bot.strategy.risk.max_daily_loss_cents / 100).toFixed(2) ||
      sizing !== formatSizing(bot.strategy.position_sizing)
    );
  }, [maxConcurrent, maxDailyLoss, sizing, bot.strategy]);

  const save = async () => {
    setSaving(true);
    try {
      const updated: Strategy = {
        ...bot.strategy,
        risk: {
          ...bot.strategy.risk,
          max_concurrent: Math.max(1, parseInt(maxConcurrent, 10) || 1),
          max_daily_loss_cents: Math.max(
            0,
            Math.round(parseFloat(maxDailyLoss || '0') * 100)
          ),
        },
        position_sizing: parseSizing(sizing, bot.strategy.position_sizing),
      };
      await onSave(bot.id, { strategy: updated });
    } catch (e) {
      Alert.alert('Save failed', (e as Error).message);
    } finally {
      setSaving(false);
    }
  };

  return (
    <View>
      <Text style={[styles.label, { color: colors.textMuted }]}>MAX CONCURRENT POSITIONS</Text>
      <TextInput
        value={maxConcurrent}
        onChangeText={setMaxConcurrent}
        keyboardType="number-pad"
        style={[
          styles.input,
          { color: colors.text, borderColor: colors.border, backgroundColor: colors.bgElevated },
        ]}
      />

      <Text style={[styles.label, { color: colors.textMuted, marginTop: 12 }]}>
        MAX DAILY LOSS ($)
      </Text>
      <TextInput
        value={maxDailyLoss}
        onChangeText={setMaxDailyLoss}
        keyboardType="decimal-pad"
        style={[
          styles.input,
          { color: colors.text, borderColor: colors.border, backgroundColor: colors.bgElevated },
        ]}
      />

      <Text style={[styles.label, { color: colors.textMuted, marginTop: 12 }]}>
        POSITION SIZING
      </Text>
      <Text style={[styles.help, { color: colors.textMuted }]}>
        {sizingHelp(bot.strategy.position_sizing)}
      </Text>
      <TextInput
        value={sizing}
        onChangeText={setSizing}
        keyboardType="decimal-pad"
        style={[
          styles.input,
          { color: colors.text, borderColor: colors.border, backgroundColor: colors.bgElevated },
        ]}
      />

      <Pressable
        disabled={!dirty || saving}
        onPress={save}
        style={({ pressed }) => [
          styles.cta,
          {
            backgroundColor: colors.primary,
            opacity: !dirty || saving ? 0.4 : pressed ? 0.9 : 1,
            marginTop: 12,
          },
        ]}
      >
        <Text style={{ color: colors.primaryFg, fontWeight: '700' }}>
          {saving ? 'SAVING…' : 'SAVE RISK SETTINGS'}
        </Text>
      </Pressable>
    </View>
  );
};

function formatSizing(ps: PositionSizing): string {
  switch (ps.kind) {
    case 'fixed_amount':
      return (ps.value_cents / 100).toFixed(2);
    case 'percent_portfolio':
      return ps.value.toFixed(2);
    case 'kelly':
      return ps.factor.toFixed(2);
  }
}

function sizingHelp(ps: PositionSizing): string {
  switch (ps.kind) {
    case 'fixed_amount':
      return 'Fixed dollar amount per trade.';
    case 'percent_portfolio':
      return 'Percentage of portfolio per trade (0–100).';
    case 'kelly':
      return 'Kelly criterion fraction (0–1).';
  }
}

function parseSizing(input: string, prev: PositionSizing): PositionSizing {
  const n = parseFloat(input);
  const safeN = Number.isFinite(n) && n >= 0 ? n : 0;
  switch (prev.kind) {
    case 'fixed_amount':
      return { kind: 'fixed_amount', value_cents: Math.round(safeN * 100) };
    case 'percent_portfolio':
      return { ...prev, value: Math.min(safeN, 100) };
    case 'kelly':
      return { ...prev, factor: Math.min(safeN, 1) };
  }
}

const PublishModal: React.FC<{
  visible: boolean;
  onClose: () => void;
  defaultTitle: string;
  defaultSummary: string;
  onConfirm: (title: string, summary: string, priceCents: number) => Promise<void>;
}> = ({ visible, onClose, defaultTitle, defaultSummary, onConfirm }) => {
  const { colors } = useTheme();
  const [title, setTitle] = useState(defaultTitle);
  const [summary, setSummary] = useState(defaultSummary);
  const [priceDollars, setPriceDollars] = useState('9.99');
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    setTitle(defaultTitle);
    setSummary(defaultSummary);
  }, [defaultTitle, defaultSummary]);

  return (
    <Modal animationType="slide" visible={visible} onRequestClose={onClose} transparent>
      <View style={[styles.modalBg, { backgroundColor: 'rgba(0,0,0,0.55)' }]}>
        <View
          style={[
            styles.modalPanel,
            { backgroundColor: colors.bg, borderColor: colors.border },
          ]}
        >
          <Text style={[styles.modalTitle, { color: colors.text }]}>
            Publish to marketplace
          </Text>

          <Text style={[styles.label, { color: colors.textMuted, marginTop: 12 }]}>
            LISTING TITLE
          </Text>
          <TextInput
            value={title}
            onChangeText={setTitle}
            style={[
              styles.input,
              { color: colors.text, borderColor: colors.border, backgroundColor: colors.bgElevated },
            ]}
          />

          <Text style={[styles.label, { color: colors.textMuted, marginTop: 10 }]}>
            SUMMARY
          </Text>
          <TextInput
            value={summary}
            onChangeText={setSummary}
            multiline
            style={[
              styles.input,
              styles.inputMulti,
              { color: colors.text, borderColor: colors.border, backgroundColor: colors.bgElevated },
            ]}
          />

          <Text style={[styles.label, { color: colors.textMuted, marginTop: 10 }]}>
            MONTHLY PRICE ($)
          </Text>
          <TextInput
            value={priceDollars}
            onChangeText={setPriceDollars}
            keyboardType="decimal-pad"
            style={[
              styles.input,
              { color: colors.text, borderColor: colors.border, backgroundColor: colors.bgElevated },
            ]}
          />

          <View style={styles.modalActions}>
            <Pressable
              onPress={onClose}
              style={[styles.cta, styles.modalBtn, { borderColor: colors.border, borderWidth: StyleSheet.hairlineWidth, backgroundColor: 'transparent' }]}
            >
              <Text style={{ color: colors.text, fontWeight: '600' }}>CANCEL</Text>
            </Pressable>
            <Pressable
              disabled={busy || !title.trim()}
              onPress={async () => {
                setBusy(true);
                const cents = Math.max(0, Math.round(parseFloat(priceDollars || '0') * 100));
                try {
                  await onConfirm(title.trim(), summary, cents);
                } finally {
                  setBusy(false);
                }
              }}
              style={[
                styles.cta,
                styles.modalBtn,
                {
                  backgroundColor: colors.primary,
                  opacity: busy || !title.trim() ? 0.5 : 1,
                },
              ]}
            >
              <Text style={{ color: colors.primaryFg, fontWeight: '700' }}>
                {busy ? 'PUBLISHING…' : 'PUBLISH'}
              </Text>
            </Pressable>
          </View>
        </View>
      </View>
    </Modal>
  );
};

const styles = StyleSheet.create({
  safe: { flex: 1 },
  scroll: { padding: 16, paddingBottom: 32 },
  backBtn: { paddingVertical: 6, marginBottom: 8 },
  headerRow: { flexDirection: 'row', alignItems: 'center', marginBottom: 12, gap: 10 },
  title: { fontSize: 22, fontWeight: '700', flexShrink: 1 },
  publishedTag: {
    paddingHorizontal: 8,
    paddingVertical: 3,
    borderRadius: 999,
    borderWidth: StyleSheet.hairlineWidth,
  },
  statsRow: { flexDirection: 'row', gap: 10, marginBottom: 6 },
  stat: { flex: 1 },
  statLabel: { fontSize: 10, fontWeight: '700', letterSpacing: 0.8 },
  statValue: { fontSize: 16, fontWeight: '700', fontVariant: ['tabular-nums'], marginTop: 2 },
  label: { fontSize: 11, fontWeight: '700', letterSpacing: 0.8, marginBottom: 4 },
  help: { fontSize: 11, marginBottom: 6 },
  input: {
    borderWidth: StyleSheet.hairlineWidth,
    borderRadius: 8,
    paddingHorizontal: 12,
    paddingVertical: 10,
    fontSize: 15,
  },
  inputMulti: { minHeight: 64, textAlignVertical: 'top' },
  cta: { marginTop: 12, paddingVertical: 14, borderRadius: 10, alignItems: 'center' },
  modalBg: { flex: 1, justifyContent: 'flex-end' },
  modalPanel: {
    padding: 16,
    borderTopLeftRadius: 16,
    borderTopRightRadius: 16,
    borderWidth: StyleSheet.hairlineWidth,
  },
  modalTitle: { fontSize: 18, fontWeight: '700' },
  modalActions: { flexDirection: 'row', gap: 10, marginTop: 16 },
  modalBtn: { flex: 1, marginTop: 0 },
});
