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

import { TemplateSummary } from '../bots/api';
import { Card } from '../components/Card';
import { useMyBots } from '../state/myBots';
import { useTheme } from '../theme/ThemeProvider';

export const TemplatePickerScreen: React.FC = () => {
  const navigation = useNavigation<{
    goBack: () => void;
    replace: (screen: string, params?: object) => void;
  }>();
  const { colors } = useTheme();
  const { client, templates, refreshTemplates, createFromTemplate } = useMyBots();

  const [selected, setSelected] = useState<string | null>(null);
  const [name, setName] = useState('');
  const [creating, setCreating] = useState(false);

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

  const renderTemplate = ({ item }: { item: TemplateSummary }) => {
    const isActive = item.id === selected;
    return (
      <Pressable
        onPress={() => setSelected(item.id)}
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
  cta: {
    marginTop: 12,
    paddingVertical: 14,
    borderRadius: 10,
    alignItems: 'center',
  },
  hint: { marginTop: 8, fontSize: 11, lineHeight: 16 },
});
