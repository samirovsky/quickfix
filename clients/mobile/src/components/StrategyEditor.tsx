// Strategy editor — exposes the *values* in a Strategy that we expect
// users to tweak (TP%, SL%, trailing stop%, indicator-threshold values),
// while leaving the *structure* (which conditions exist, the indicator
// names, the AND/OR shape) intact. Adding new conditions or swapping
// types is intentionally out of scope for this iteration — that's a UI
// problem worth its own pass.

import React, { useMemo, useState } from 'react';
import { Alert, Pressable, StyleSheet, Text, TextInput, View } from 'react-native';

import { Strategy } from '../bots/api';
import { useTheme } from '../theme/ThemeProvider';

// ---------- runtime guards ----------

interface AllOf {
  all_of: Condition[];
}
interface AnyOf {
  any_of: Condition[];
}
type ConditionGroup = AllOf | AnyOf;

type Condition =
  | {
      type: 'indicator_threshold';
      indicator: string;
      params: unknown;
      op: string;
      value: number;
    }
  | { type: 'take_profit_pct'; value: number }
  | { type: 'stop_loss_pct'; value: number }
  | { type: 'trailing_stop_pct'; value: number };

const isGroup = (x: unknown): x is ConditionGroup =>
  typeof x === 'object' &&
  x !== null &&
  ('all_of' in (x as object) || 'any_of' in (x as object));

const groupItems = (g: ConditionGroup): Condition[] =>
  'all_of' in g ? g.all_of : g.any_of;

const setGroupItems = (g: ConditionGroup, items: Condition[]): ConditionGroup =>
  'all_of' in g ? { all_of: items } : { any_of: items };

// ---------- editable fields ----------

interface FieldDescriptor {
  path: 'entry' | 'exit';
  index: number;
  label: string;
  unit: '%' | '';
  value: number;
}

function fieldsFor(group: ConditionGroup, path: 'entry' | 'exit'): FieldDescriptor[] {
  const out: FieldDescriptor[] = [];
  groupItems(group).forEach((cond, index) => {
    switch (cond.type) {
      case 'take_profit_pct':
        out.push({ path, index, label: 'Take profit', unit: '%', value: cond.value });
        break;
      case 'stop_loss_pct':
        out.push({ path, index, label: 'Stop loss', unit: '%', value: cond.value });
        break;
      case 'trailing_stop_pct':
        out.push({ path, index, label: 'Trailing stop', unit: '%', value: cond.value });
        break;
      case 'indicator_threshold':
        out.push({
          path,
          index,
          label: `${cond.indicator.toUpperCase()} threshold (${cond.op})`,
          unit: '',
          value: cond.value,
        });
        break;
    }
  });
  return out;
}

function applyEdit(
  group: ConditionGroup,
  index: number,
  newValue: number
): ConditionGroup {
  const items = groupItems(group).slice();
  const cond = items[index];
  if (!cond) return group;
  items[index] = { ...cond, value: newValue } as Condition;
  return setGroupItems(group, items);
}

// ---------- component ----------

interface Props {
  strategy: Strategy;
  onSave: (next: Strategy) => Promise<unknown>;
}

export const StrategyEditor: React.FC<Props> = ({ strategy, onSave }) => {
  const { colors } = useTheme();

  // Entry/exit may be `unknown` per the API typing. Validate at runtime.
  const entryGroup: ConditionGroup | null = isGroup(strategy.entry) ? strategy.entry : null;
  const exitGroup: ConditionGroup | null = isGroup(strategy.exit) ? strategy.exit : null;

  const initialFields = useMemo<FieldDescriptor[]>(() => {
    const arr: FieldDescriptor[] = [];
    if (entryGroup) arr.push(...fieldsFor(entryGroup, 'entry'));
    if (exitGroup) arr.push(...fieldsFor(exitGroup, 'exit'));
    return arr;
  }, [entryGroup, exitGroup]);

  const [drafts, setDrafts] = useState<string[]>(() =>
    initialFields.map(f => f.value.toString())
  );
  const [saving, setSaving] = useState(false);

  // If the bot reloads under us, reset the drafts.
  React.useEffect(() => {
    setDrafts(initialFields.map(f => f.value.toString()));
  }, [initialFields]);

  const dirty = useMemo(
    () => drafts.some((d, i) => parseFloat(d) !== initialFields[i]?.value),
    [drafts, initialFields]
  );

  const save = async () => {
    if (!entryGroup && !exitGroup) return;
    let entry = entryGroup;
    let exit = exitGroup;
    initialFields.forEach((field, i) => {
      const raw = drafts[i] ?? '';
      const parsed = Number.parseFloat(raw);
      if (!Number.isFinite(parsed)) return;
      if (field.path === 'entry' && entry) {
        entry = applyEdit(entry, field.index, parsed);
      } else if (field.path === 'exit' && exit) {
        exit = applyEdit(exit, field.index, parsed);
      }
    });
    setSaving(true);
    try {
      await onSave({
        ...strategy,
        entry: entry ?? strategy.entry,
        exit: exit ?? strategy.exit,
      });
    } catch (e) {
      Alert.alert('Save failed', (e as Error).message);
    } finally {
      setSaving(false);
    }
  };

  if (initialFields.length === 0) {
    return (
      <Text style={{ color: colors.textMuted, fontSize: 12 }}>
        This strategy has no editable thresholds. Add a template-based bot to start tuning.
      </Text>
    );
  }

  return (
    <View>
      {initialFields.map((field, i) => (
        <View key={`${field.path}-${field.index}`} style={styles.fieldRow}>
          <View style={styles.fieldText}>
            <Text style={[styles.fieldLabel, { color: colors.textMuted }]}>
              {field.path.toUpperCase()}
            </Text>
            <Text style={[styles.fieldName, { color: colors.text }]} numberOfLines={1}>
              {field.label}
            </Text>
          </View>
          <View style={styles.fieldInputWrap}>
            <TextInput
              value={drafts[i]}
              onChangeText={t =>
                setDrafts(prev => prev.map((d, j) => (j === i ? t : d)))
              }
              keyboardType="decimal-pad"
              style={[
                styles.fieldInput,
                {
                  color: colors.text,
                  borderColor: colors.border,
                  backgroundColor: colors.bgElevated,
                },
              ]}
            />
            {field.unit !== '' && (
              <Text style={[styles.fieldUnit, { color: colors.textMuted }]}>
                {field.unit}
              </Text>
            )}
          </View>
        </View>
      ))}

      <Pressable
        disabled={!dirty || saving}
        onPress={save}
        style={({ pressed }) => [
          styles.cta,
          {
            backgroundColor: colors.primary,
            opacity: !dirty || saving ? 0.4 : pressed ? 0.9 : 1,
          },
        ]}
      >
        <Text style={{ color: colors.primaryFg, fontWeight: '700' }}>
          {saving ? 'SAVING…' : 'SAVE STRATEGY'}
        </Text>
      </Pressable>

      <Text style={[styles.note, { color: colors.textMuted }]}>
        You can tune the existing thresholds. Adding or removing conditions, or changing
        the AND/OR shape, will come in a later iteration.
      </Text>
    </View>
  );
};

const styles = StyleSheet.create({
  fieldRow: {
    flexDirection: 'row',
    alignItems: 'center',
    paddingVertical: 8,
    gap: 12,
  },
  fieldText: { flex: 1 },
  fieldLabel: { fontSize: 9, fontWeight: '700', letterSpacing: 1 },
  fieldName: { fontSize: 13, marginTop: 2 },
  fieldInputWrap: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 6,
  },
  fieldInput: {
    width: 90,
    borderWidth: StyleSheet.hairlineWidth,
    borderRadius: 8,
    paddingHorizontal: 10,
    paddingVertical: 8,
    fontSize: 14,
    textAlign: 'right',
    fontVariant: ['tabular-nums'],
  },
  fieldUnit: { fontSize: 13, width: 14 },
  cta: {
    marginTop: 12,
    paddingVertical: 14,
    borderRadius: 10,
    alignItems: 'center',
  },
  note: { marginTop: 8, fontSize: 11, lineHeight: 16 },
});
