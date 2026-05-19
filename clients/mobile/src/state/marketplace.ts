import { create } from 'zustand';

import {
  BotServiceClient,
  Listing,
  PerformanceMetrics,
  Subscription,
} from '../bots/api';

interface MarketplaceStore {
  client: BotServiceClient | null;
  configure: (baseUrl: string, apiKey: string) => void;

  listings: Listing[];
  listingsLoading: boolean;
  listingsError: string | null;
  refreshListings: () => Promise<void>;

  details: Record<string, Listing>;
  performance: Record<string, PerformanceMetrics>;
  loadingPerformance: Record<string, boolean>;
  loadListingDetail: (id: string, days?: number) => Promise<void>;

  subscriptions: Subscription[];
  subscriptionsLoading: boolean;
  refreshSubscriptions: () => Promise<void>;

  subscribing: Record<string, boolean>;
  subscribe: (listingId: string, allocatedCents: number) => Promise<Subscription | null>;
  cancel: (subId: string) => Promise<void>;
}

export const useMarketplace = create<MarketplaceStore>((set, get) => ({
  client: null,
  configure: (baseUrl, apiKey) => {
    if (!baseUrl || !apiKey) {
      set({ client: null });
      return;
    }
    set({ client: new BotServiceClient(baseUrl, apiKey) });
  },

  listings: [],
  listingsLoading: false,
  listingsError: null,
  refreshListings: async () => {
    const c = get().client;
    if (!c) {
      set({ listingsError: 'configure bot-service URL + key in Settings first' });
      return;
    }
    set({ listingsLoading: true, listingsError: null });
    try {
      const listings = await c.listMarketplace();
      set({ listings, listingsLoading: false });
    } catch (e) {
      set({ listingsLoading: false, listingsError: (e as Error).message });
    }
  },

  details: {},
  performance: {},
  loadingPerformance: {},
  loadListingDetail: async (id, days = 30) => {
    const c = get().client;
    if (!c) return;
    set(state => ({
      loadingPerformance: { ...state.loadingPerformance, [id]: true },
    }));
    try {
      const [listing, perf] = await Promise.all([
        c.getListing(id),
        c.listingPerformance(id, days),
      ]);
      set(state => ({
        details: { ...state.details, [id]: listing },
        performance: { ...state.performance, [id]: perf },
        loadingPerformance: { ...state.loadingPerformance, [id]: false },
      }));
    } catch (e) {
      console.warn('[marketplace] load detail failed', e);
      set(state => ({
        loadingPerformance: { ...state.loadingPerformance, [id]: false },
      }));
    }
  },

  subscriptions: [],
  subscriptionsLoading: false,
  refreshSubscriptions: async () => {
    const c = get().client;
    if (!c) return;
    set({ subscriptionsLoading: true });
    try {
      const subs = await c.mySubscriptions();
      set({ subscriptions: subs, subscriptionsLoading: false });
    } catch (e) {
      console.warn('[marketplace] refresh subscriptions failed', e);
      set({ subscriptionsLoading: false });
    }
  },

  subscribing: {},
  subscribe: async (listingId, allocatedCents) => {
    const c = get().client;
    if (!c) return null;
    set(state => ({
      subscribing: { ...state.subscribing, [listingId]: true },
    }));
    try {
      const sub = await c.subscribe(listingId, allocatedCents);
      set(state => ({
        subscriptions: [sub, ...state.subscriptions.filter(s => s.id !== sub.id)],
        subscribing: { ...state.subscribing, [listingId]: false },
      }));
      // Refresh the listing so the subscriber count moves in the UI.
      void get().loadListingDetail(listingId);
      return sub;
    } catch (e) {
      console.warn('[marketplace] subscribe failed', e);
      set(state => ({
        subscribing: { ...state.subscribing, [listingId]: false },
      }));
      throw e;
    }
  },

  cancel: async subId => {
    const c = get().client;
    if (!c) return;
    try {
      await c.cancelSubscription(subId);
      set(state => ({
        subscriptions: state.subscriptions.filter(s => s.id !== subId),
      }));
    } catch (e) {
      console.warn('[marketplace] cancel failed', e);
      throw e;
    }
  },
}));
