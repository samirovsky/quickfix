import React, { useEffect } from 'react';
import { StatusBar } from 'expo-status-bar';
import { StyleSheet, View } from 'react-native';
import { SafeAreaProvider } from 'react-native-safe-area-context';
import { NavigationContainer, DefaultTheme, DarkTheme } from '@react-navigation/native';
import { createBottomTabNavigator } from '@react-navigation/bottom-tabs';

import { ConnectScreen } from './src/screens/ConnectScreen';
import { FillsScreen } from './src/screens/FillsScreen';
import { OrdersScreen } from './src/screens/OrdersScreen';
import { SettingsScreen } from './src/screens/SettingsScreen';
import { TradingScreen } from './src/screens/TradingScreen';
import { useSettings } from './src/state/settings';
import { useTrading } from './src/state/trading';
import { ThemeProvider, useTheme } from './src/theme/ThemeProvider';

const Tabs = createBottomTabNavigator();

const Shell: React.FC = () => {
  const { colors, isDark } = useTheme();
  const navTheme = {
    ...(isDark ? DarkTheme : DefaultTheme),
    colors: {
      ...(isDark ? DarkTheme : DefaultTheme).colors,
      background: colors.bg,
      card: colors.bgElevated,
      text: colors.text,
      border: colors.border,
      primary: colors.primary,
    },
  };

  return (
    <View style={[styles.root, { backgroundColor: colors.bg }]}>
      <StatusBar style={isDark ? 'light' : 'dark'} />
      <NavigationContainer theme={navTheme}>
        <Tabs.Navigator
          screenOptions={{
            headerShown: false,
            tabBarLabelStyle: { fontSize: 11, fontWeight: '600' },
          }}
        >
          <Tabs.Screen name="Connect" component={ConnectScreen} options={{ tabBarLabel: 'CONNECT' }} />
          <Tabs.Screen name="Trade" component={TradingScreen} options={{ tabBarLabel: 'TRADE' }} />
          <Tabs.Screen name="Orders" component={OrdersScreen} options={{ tabBarLabel: 'ORDERS' }} />
          <Tabs.Screen name="Fills" component={FillsScreen} options={{ tabBarLabel: 'FILLS' }} />
          <Tabs.Screen name="Settings" component={SettingsScreen} options={{ tabBarLabel: 'SETTINGS' }} />
        </Tabs.Navigator>
      </NavigationContainer>
    </View>
  );
};

export default function App() {
  const initTrading = useTrading(s => s.init);
  const hydrateSettings = useSettings(s => s.hydrate);
  const hydrated = useSettings(s => s.hydrated);

  useEffect(() => {
    initTrading();
    hydrateSettings();
  }, [initTrading, hydrateSettings]);

  if (!hydrated) {
    // Render a blank app shell until settings load — avoids flashing wrong theme.
    return (
      <SafeAreaProvider>
        <View style={styles.boot} />
      </SafeAreaProvider>
    );
  }

  return (
    <SafeAreaProvider>
      <ThemeProvider>
        <Shell />
      </ThemeProvider>
    </SafeAreaProvider>
  );
}

const styles = StyleSheet.create({
  root: { flex: 1 },
  boot: { flex: 1, backgroundColor: '#0b0d12' },
});
