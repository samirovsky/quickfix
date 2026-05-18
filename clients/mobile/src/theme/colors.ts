export interface Palette {
  bg: string;
  bgElevated: string;
  surface: string;
  border: string;
  text: string;
  textMuted: string;
  textInverted: string;
  primary: string;
  primaryFg: string;
  buy: string;
  sell: string;
  danger: string;
  warn: string;
  ok: string;
}

export const dark: Palette = {
  bg: '#0b0d12',
  bgElevated: '#11141b',
  surface: '#181c25',
  border: '#262c38',
  text: '#e6e8ec',
  textMuted: '#9aa1ad',
  textInverted: '#0b0d12',
  primary: '#5b8def',
  primaryFg: '#ffffff',
  buy: '#23c45a',
  sell: '#ef4060',
  danger: '#ef4060',
  warn: '#f7b32b',
  ok: '#23c45a',
};

export const light: Palette = {
  bg: '#f7f8fa',
  bgElevated: '#ffffff',
  surface: '#ffffff',
  border: '#e2e5ec',
  text: '#0b0d12',
  textMuted: '#6b7280',
  textInverted: '#ffffff',
  primary: '#2a6df2',
  primaryFg: '#ffffff',
  buy: '#0f8c3d',
  sell: '#d12d4e',
  danger: '#d12d4e',
  warn: '#b76b00',
  ok: '#0f8c3d',
};
