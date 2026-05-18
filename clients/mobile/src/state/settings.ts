import AsyncStorage from '@react-native-async-storage/async-storage';
import { create } from 'zustand';

const KEY = '@qftx/settings/v1';

export type ThemeMode = 'system' | 'light' | 'dark';

export interface Settings {
  serverUrl: string;
  defaultSymbolId: number;
  themeMode: ThemeMode;
  // Number of symbols the server is configured with (must match TRADING_SYMBOLS).
  symbolCount: number;
}

const DEFAULTS: Settings = {
  serverUrl: 'ws://10.0.2.2:9002/ws',
  defaultSymbolId: 0,
  themeMode: 'system',
  symbolCount: 64,
};

interface SettingsStore {
  hydrated: boolean;
  values: Settings;
  hydrate: () => Promise<void>;
  update: (patch: Partial<Settings>) => Promise<void>;
  reset: () => Promise<void>;
}

export const useSettings = create<SettingsStore>((set, get) => ({
  hydrated: false,
  values: DEFAULTS,
  hydrate: async () => {
    try {
      const raw = await AsyncStorage.getItem(KEY);
      const parsed = raw ? (JSON.parse(raw) as Partial<Settings>) : {};
      set({ hydrated: true, values: { ...DEFAULTS, ...parsed } });
    } catch {
      set({ hydrated: true });
    }
  },
  update: async patch => {
    const next = { ...get().values, ...patch };
    set({ values: next });
    try {
      await AsyncStorage.setItem(KEY, JSON.stringify(next));
    } catch {
      /* ignore */
    }
  },
  reset: async () => {
    set({ values: DEFAULTS });
    try {
      await AsyncStorage.removeItem(KEY);
    } catch {
      /* ignore */
    }
  },
}));
