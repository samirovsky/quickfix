import React, { useEffect, useState } from 'react';
import {
  Alert,
  FlatList,
  Pressable,
  StyleSheet,
  Text,
  TextInput,
  View,
} from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import { useNavigation } from '@react-navigation/native';

import { AiGenerateResponse, TemplateSummary } from '../bots/api';
import { Card } from '../components/Card';
import { useMyBots } from '../state/myBots';
import { useSettings } from '../state/settings';
import { useTheme } from '../theme/ThemeProvider';

export const TemplatePickerScreen: React.FC = () => {
  const navigation = useNavigation<{
    goBack: () => void;
    replace: (screen: string, params?: object) => void;
  }>();
  const { colors } = useTheme();
  const { values } = useSettings();
  const { client, configure, templates, refreshTemplates, createFromTemplate, createFromStrategy } =
    useMyBots();

  // Re-configure if the user just edited the URL/key — same pattern as
  // the other tabs.
  useEffect(() => {
    configure({ demoMode: values.demoMode, baseUrl: values.botServiceUrl, apiKey: values.botServiceKey });
  }, [values.demoMode, values.botServiceUrl, values.botServiceKey, configure]);

  const [selected, setSelected] = useState<string | null>(null);
  const [name, setName] = useState('');
  const [creating, setCreating] = useState(false);

  // AI-prompt state — held separate from the template-selection state so
  // the two flows don't fight over `selected`.
  const [prompt, setPrompt] = useState('');
  const [generating, setGenerating] = useState(false);
  const [aiResult, setAiResult] = useState<AiGenerateResponse | null>(null);
  const [aiName, setAiName] = useState('');

  useEffect(() => {
    if (client && templates.length === 0) void refreshTemplates();
  }, [client, templates.length, refreshTemplates]);

  const onCreate = async () => {
    if (!selected || !name.trim()) return;
    setCreating(true);
    try {
      const bot = await createFromTemplate(selected, name.trim());
      if (bot) {
        navigation.replace('MyBotDetail', { botId: bot.id });
      }
    } catch (e) {
      Alert.alert('Create failed', (e as Error).message);
      setCreating(false);
    }
  };

  const onGenerate = async () => {
    if (!client || !prompt.trim()) return;
    setGenerating(true);
    try {
      const result = await client.generateStrategy(prompt.trim());
      setAiResult(result);
      // Seed the name based on the matched template; user can edit.
      const tpl = templates.find(t => t.id === result.source_template_id);
      setAiName(tpl ? `${tpl.name} (from prompt)` : 'AI-generated bot');
    } catch (e) {
      Alert.alert('Generate failed', (e as Error).message);
    } finally {
      setGenerating(false);
    }
  };

  const onSaveAi = async () => {
    if (!aiResult || !aiName.trim()) return;
    setCreating(true);
    try {
      const bot = await createFromStrategy({
        name: aiName.trim(),
        description: `Generated from prompt: ${prompt.trim().slice(0, 200)}`,
        strategy: aiResult.strategy,
        asset_filter: aiResult.asset_filter,
        source: 'ai',
      });
      if (bot) {
        navigation.replace('MyBotDetail', { botId: bot.id });
      }
    } catch (e) {
      Alert.alert('Create failed', (e as Error).message);
      setCreating(false);
    }
  };

  const matchedTemplate = aiResult
    ? templates.find(t => t.id === aiResult.source_template_id) ?? null
    : null;

  const renderTemplate = ({ item }: { item: TemplateSummary }) => {
    const isActive = item.id === selected;
    return (
      <Pressable
        onPress={() => {
          setSelected(item.id);
          setAiResult(null);
        }}
        style={[
          styles.tpl,
          {
            backgroundColor: isActive ? colors.bgElevated : colors.surface,
            borderColor: isActive ? colors.primary : colors.border,
          },
        ]}
      >
        <View style={{ flex: 1 }}>
          <Text style={[styles.tplName, { color: colors.text }]}>{item.name}</Text>
          <Text style={[styles.tplDesc, { color: colors.textMuted }]} numberOfLines={2}>
            {item.description}
          </Text>
          <Text style={[styles.tplMeta, { color: colors.textMuted }]}>
            {item.category} · risk {item.risk_level}/5
          </Text>
        </View>
        {isActive && (
          <View style={[styles.dot, { backgroundColor: colors.primary }]}>
            <Text style={{ color: colors.primaryFg, fontWeight: '700', fontSize: 12 }}>
              ✓
            </Text>
          </View>
        )}
      </Pressable>
    );
  };

  return (
    <SafeAreaView style={[styles.safe, { backgroundColor: colors.bg }]} edges={['top']}>
      <View style={styles.headerRow}>
        <Pressable onPress={navigation.goBack}>
          <Text style={{ color: colors.primary, fontWeight: '600' }}>‹ Build</Text>
        </Pressable>
        <Text style={[styles.heading, { color: colors.text }]}>New bot</Text>
        <View style={{ width: 40 }} />
      </View>

      <FlatList
        data={templates}
        keyExtractor={t => t.id}
        renderItem={renderTemplate}
        contentContainerStyle={styles.list}
        ListHeaderComponent={
          <Card title="Generate with AI (preview)">
            <Text style={[styles.aiHint, { color: colors.textMuted }]}>
              Describe the strategy in plain English. The current build uses a stub that
              matches your prompt to a bundled template — a real LLM-backed generator
              will replace it without changing this UI.
            </Text>
            <TextInput
              value={prompt}
              onChangeText={setPrompt}
              placeholder="e.g. Buy bitcoin when RSI drops below 30"
              placeholderTextColor={colors.textMuted}
              multiline
              style={[
                styles.input,
                styles.inputMulti,
                {
                  color: colors.text,
                  borderColor: colors.border,
                  backgroundColor: colors.bgElevated,
                },
              ]}
            />
            <Pressable
              disabled={!prompt.trim() || generating || !client}
              onPress={onGenerate}
              style={({ pressed }) => [
                styles.cta,
                {
                  backgroundColor: colors.primary,
                  opacity: !prompt.trim() || generating || !client ? 0.4 : pressed ? 0.9 : 1,
                },
              ]}
            >
              <Text style={{ color: colors.primaryFg, fontWeight: '700' }}>
                {generating ? 'GENERATING…' : 'GENERATE'}
              </Text>
            </Pressable>

            {aiResult && (
              <View style={[styles.aiResult, { borderColor: colors.border, backgroundColor: colors.bgElevated }]}>
                <Text style={[styles.aiResultLabel, { color: colors.textMuted }]}>
                  GENERATED STRATEGY
                </Text>
                <Text style={[styles.aiResultBody, { color: colors.text }]}>
                  Closest match: {matchedTemplate?.name ?? aiResult.source_template_id}
                </Text>
                {aiResult.stub && (
                  <Text style={[styles.aiStubBadge, { color: colors.warn }]}>
                    Stubbed — real AI generation lands in a follow-up slice.
                  </Text>
                )}
                <Text style={[styles.aiHint, { color: colors.textMuted, marginTop: 10 }]}>
                  Name your bot:
                </Text>
                <TextInput
                  value={aiName}
                  onChangeText={setAiName}
                  style={[
                    styles.input,
                    {
                      color: colors.text,
                      borderColor: colors.border,
                      backgroundColor: colors.surface,
                    },
                  ]}
                />
                <Pressable
                  disabled={!aiName.trim() || creating}
                  onPress={onSaveAi}
                  style={({ pressed }) => [
                    styles.cta,
                    {
                      backgroundColor: colors.primary,
                      opacity: !aiName.trim() || creating ? 0.4 : pressed ? 0.9 : 1,
                    },
                  ]}
                >
                  <Text style={{ color: colors.primaryFg, fontWeight: '700' }}>
                    {creating ? 'CREATING…' : 'SAVE AS MY BOT'}
                  </Text>
                </Pressable>
              </View>
            )}
          </Card>
        }
        ListEmptyComponent={
          <Card title="Templates">
            <Text style={{ color: colors.textMuted }}>
              {client ? 'Loading templates…' : 'Configure bot-service first.'}
            </Text>
          </Card>
        }
        ListFooterComponent={
          selected ? (
            <Card title="Name your bot">
              <TextInput
                value={name}
                onChangeText={setName}
                placeholder="e.g. My RSI Bouncer"
                placeholderTextColor={colors.textMuted}
                style={[
                  styles.input,
                  {
                    color: colors.text,
                    borderColor: colors.border,
                    backgroundColor: colors.bgElevated,
                  },
                ]}
              />
              <Pressable
                disabled={!name.trim() || creating}
                onPress={onCreate}
                style={({ pressed }) => [
                  styles.cta,
                  {
                    backgroundColor: colors.primary,
                    opacity: !name.trim() || creating ? 0.5 : pressed ? 0.9 : 1,
                  },
                ]}
              >
                <Text style={{ color: colors.primaryFg, fontWeight: '700' }}>
                  {creating ? 'CREATING…' : 'CREATE BOT'}
                </Text>
              </Pressable>
              <Text style={[styles.hint, { color: colors.textMuted }]}>
                You can edit risk parameters and publish to the marketplace from the bot
                detail screen.
              </Text>
            </Card>
          ) : null
        }
      />
    </SafeAreaView>
  );
};

const styles = StyleSheet.create({
  safe: { flex: 1 },
  headerRow: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingHorizontal: 16,
    paddingVertical: 8,
  },
  heading: { fontSize: 18, fontWeight: '700' },
  list: { padding: 16, paddingTop: 8 },
  tpl: {
    flexDirection: 'row',
    alignItems: 'center',
    padding: 12,
    borderRadius: 12,
    borderWidth: StyleSheet.hairlineWidth,
    marginBottom: 8,
    gap: 10,
  },
  tplName: { fontSize: 15, fontWeight: '700' },
  tplDesc: { fontSize: 12, marginTop: 4, lineHeight: 17 },
  tplMeta: { fontSize: 11, marginTop: 6 },
  dot: {
    width: 26,
    height: 26,
    borderRadius: 13,
    alignItems: 'center',
    justifyContent: 'center',
  },
  input: {
    borderWidth: StyleSheet.hairlineWidth,
    borderRadius: 8,
    paddingHorizontal: 12,
    paddingVertical: 10,
    fontSize: 15,
  },
  inputMulti: { minHeight: 64, textAlignVertical: 'top' },
  cta: {
    marginTop: 12,
    paddingVertical: 14,
    borderRadius: 10,
    alignItems: 'center',
  },
  hint: { marginTop: 8, fontSize: 11, lineHeight: 16 },
  aiHint: { fontSize: 12, marginBottom: 10, lineHeight: 18 },
  aiResult: {
    marginTop: 12,
    padding: 12,
    borderRadius: 10,
    borderWidth: StyleSheet.hairlineWidth,
  },
  aiResultLabel: { fontSize: 10, fontWeight: '700', letterSpacing: 0.8 },
  aiResultBody: { fontSize: 14, marginTop: 4, fontWeight: '600' },
  aiStubBadge: { fontSize: 11, marginTop: 4 },
});
