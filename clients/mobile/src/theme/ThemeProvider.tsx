import React, { createContext, useContext, useMemo } from 'react';
import { useColorScheme } from 'react-native';

import { dark, light, Palette } from './colors';
import { useSettings } from '../state/settings';

interface ThemeContextValue {
  colors: Palette;
  isDark: boolean;
}

const ThemeContext = createContext<ThemeContextValue>({ colors: dark, isDark: true });

export const ThemeProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const themeMode = useSettings(s => s.values.themeMode);
  const system = useColorScheme();
  const isDark = useMemo(() => {
    if (themeMode === 'dark') return true;
    if (themeMode === 'light') return false;
    return system !== 'light';
  }, [themeMode, system]);
  const colors = isDark ? dark : light;
  return <ThemeContext.Provider value={{ colors, isDark }}>{children}</ThemeContext.Provider>;
};

export const useTheme = (): ThemeContextValue => useContext(ThemeContext);
