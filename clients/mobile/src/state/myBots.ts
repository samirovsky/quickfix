import { create } from 'zustand';

import {
  BotConfig,
  BotConfigSummary,
  BotServiceClient,
  BotStatus,
  PerformanceMetrics,
  Strategy,
  TemplateSummary,
} from '../bots/api';

interface MyBotsStore {
  client: BotServiceClient | null;
  configure: (baseUrl: string, apiKey: string) => void;

  // List
  bots: BotConfigSummary[];
  botsLoading: boolean;
  botsError: string | null;
  refreshBots: (status?: BotStatus) => Promise<void>;

  // Templates
  templates: TemplateSummary[];
  templatesLoaded: boolean;
  refreshTemplates: () => Promise<void>;

  // Single-bot view + perf
  bot: BotConfig | null;
  botPerf: PerformanceMetrics | null;
  botLoading: boolean;
  loadBot: (id: string, days?: number) => Promise<void>;

  // Actions
  createFromTemplate: (templateId: string, name: string) => Promise<BotConfig | null>;
  updateBot: (
    id: string,
    patch: { name?: string; description?: string; strategy?: Strategy }
  ) => Promise<BotConfig | null>;
  deleteBot: (id: string) => Promise<void>;
  setStatus: (id: string, status: BotStatus) => Promise<BotConfig | null>;
  publish: (
    id: string,
    title: string,
    summary: string,
    monthlyPriceCents: number
  ) => Promise<void>;
  unpublish: (botId: string, listingId: string) => Promise<void>;
}

export const useMyBots = create<MyBotsStore>((set, get) => ({
  client: null,
  configure: (baseUrl, apiKey) => {
    if (!baseUrl || !apiKey) {
      set({ client: null });
      return;
    }
    set({ client: new BotServiceClient(baseUrl, apiKey) });
  },

  bots: [],
  botsLoading: false,
  botsError: null,
  refreshBots: async status => {
    const c = get().client;
    if (!c) {
      set({ botsError: 'configure bot-service URL + key in Settings first' });
      return;
    }
    set({ botsLoading: true, botsError: null });
    try {
      const bots = await c.listMyBots(status);
      set({ bots, botsLoading: false });
    } catch (e) {
      set({ botsLoading: false, botsError: (e as Error).message });
    }
  },

  templates: [],
  templatesLoaded: false,
  refreshTemplates: async () => {
    const c = get().client;
    if (!c) return;
    try {
      const templates = await c.listTemplates();
      set({ templates, templatesLoaded: true });
    } catch (e) {
      console.warn('[mybots] templates load failed', e);
    }
  },

  bot: null,
  botPerf: null,
  botLoading: false,
  loadBot: async (id, days = 30) => {
    const c = get().client;
    if (!c) return;
    set({ botLoading: true });
    try {
      const [bot, perf] = await Promise.all([c.getMyBot(id), c.botPerformance(id, days)]);
      set({ bot, botPerf: perf, botLoading: false });
    } catch (e) {
      console.warn('[mybots] load bot failed', e);
      set({ botLoading: false });
    }
  },

  createFromTemplate: async (templateId, name) => {
    const c = get().client;
    if (!c) return null;
    const bot = await c.createFromTemplate(templateId, name);
    set(state => ({ bots: [summary(bot), ...state.bots] }));
    return bot;
  },

  updateBot: async (id, patch) => {
    const c = get().client;
    if (!c) return null;
    const updated = await c.updateBot(id, patch);
    set(state => ({
      bot: state.bot?.id === id ? updated : state.bot,
      bots: state.bots.map(b => (b.id === id ? summary(updated) : b)),
    }));
    return updated;
  },

  deleteBot: async id => {
    const c = get().client;
    if (!c) return;
    await c.deleteBot(id);
    set(state => ({
      bots: state.bots.filter(b => b.id !== id),
      bot: state.bot?.id === id ? null : state.bot,
    }));
  },

  setStatus: async (id, status) => {
    const c = get().client;
    if (!c) return null;
    const updated = await c.setBotStatus(id, status);
    set(state => ({
      bot: state.bot?.id === id ? updated : state.bot,
      bots: state.bots.map(b => (b.id === id ? summary(updated) : b)),
    }));
    return updated;
  },

  publish: async (id, title, summary_, price) => {
    const c = get().client;
    if (!c) return;
    const listing = await c.publishBot(id, title, summary_, price);
    // Update the current bot view so the badge flips immediately.
    set(state => {
      if (state.bot?.id === id) {
        return { bot: { ...state.bot, published_listing_id: listing.id } };
      }
      return {};
    });
    // Refresh the list so the badge appears there too.
    void get().refreshBots();
  },

  unpublish: async (botId, listingId) => {
    const c = get().client;
    if (!c) return;
    await c.unpublishListing(listingId);
    set(state => {
      if (state.bot?.id === botId) {
        return { bot: { ...state.bot, published_listing_id: null } };
      }
      return {};
    });
    void get().refreshBots();
  },
}));

function summary(bot: BotConfig): BotConfigSummary {
  return {
    id: bot.id,
    name: bot.name,
    status: bot.status,
    source: bot.source,
    published_listing_id: bot.published_listing_id,
    updated_at: bot.updated_at,
  };
}
