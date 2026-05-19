// In-memory implementation of `BotService` for demo mode. Everything
// lives in memory for the lifetime of the page/app session — refresh
// resets it. Lets the deployed web app be fully explorable with zero
// backend, no CORS, no secrets.
//
// The data shapes match the real bot-service exactly, so screens don't
// know or care which implementation they're talking to.

import {
  AiGenerateResponse,
  AssetFilter,
  BotConfig,
  BotConfigSummary,
  BotService,
  BotStatus,
  DailyMetric,
  Listing,
  PerformanceMetrics,
  Strategy,
  Subscription,
  Template,
  TemplateSummary,
} from './api';

let counter = 1;
const newId = (prefix: string) => `${prefix}_mock${counter++}`;

const nowIso = () => new Date().toISOString();

function strategy(rsi: boolean): Strategy {
  return {
    version: 1,
    entry: {
      all_of: [
        rsi
          ? {
              type: 'indicator_threshold',
              indicator: 'rsi',
              params: { period: 14, timeframe: '1h' },
              op: 'lt',
              value: 30,
            }
          : {
              type: 'indicator_threshold',
              indicator: 'ema_cross',
              params: { fast: 12, slow: 26, timeframe: '1h' },
              op: 'gt',
              value: 0,
            },
      ],
    },
    exit: {
      any_of: [
        { type: 'take_profit_pct', value: 2 },
        { type: 'stop_loss_pct', value: 1 },
      ],
    },
    position_sizing: { kind: 'percent_portfolio', value: 5, max_notional_cents: 100000 },
    risk: { max_concurrent: 3, max_daily_loss_cents: 5000, halt_after_n_losses: 4 },
    schedule: { active_hours_utc: [[13, 21]], active_days: ['Mon', 'Tue', 'Wed', 'Thu', 'Fri'] },
  } as Strategy;
}

const ASSET: AssetFilter = { symbols: [0], market: 'qftx' };

const TEMPLATES: Template[] = [
  {
    id: 'tpl_rsi_oversold',
    name: 'RSI Oversold Bounce',
    category: 'MeanReversion',
    description: 'Buys when RSI(14) on the 1h chart drops below 30. Exits on 2% TP or 1% SL.',
    risk_level: 3,
    asset_filter: ASSET,
    strategy: strategy(true),
  },
  {
    id: 'tpl_ema_cross',
    name: 'EMA 12/26 Crossover',
    category: 'TrendFollowing',
    description: 'Buys when EMA(12) crosses above EMA(26). Trailing stop at 0.5%.',
    risk_level: 3,
    asset_filter: ASSET,
    strategy: strategy(false),
  },
  {
    id: 'tpl_breakout_box',
    name: '20-bar Breakout',
    category: 'Breakout',
    description: 'Buys a 20-bar high breakout with ATR-scaled stops.',
    risk_level: 4,
    asset_filter: ASSET,
    strategy: strategy(false),
  },
];

// Deterministic-ish performance curve so a given bot id always renders
// the same chart within a session.
function perf(seedStr: string, days: number, bias: number): PerformanceMetrics {
  let s = 0xcafe;
  for (const ch of seedStr) s = (s * 31 + ch.charCodeAt(0)) >>> 0;
  const rand = () => {
    s = (s * 1103515245 + 12345) & 0x7fffffff;
    return s / 0x7fffffff;
  };
  const daily: DailyMetric[] = [];
  let total = 0;
  let wins = 0;
  let trades = 0;
  let best = 0;
  let worst = 0;
  const today = new Date();
  today.setUTCHours(0, 0, 0, 0);
  for (let i = days - 1; i >= 0; i--) {
    const d = new Date(today);
    d.setUTCDate(today.getUTCDate() - i);
    const t = Math.floor(rand() * 4) + 1;
    let dayPnl = 0;
    for (let k = 0; k < t; k++) {
      const win = rand() < 0.5 + bias;
      const mag = Math.round((rand() * 400 + 50) * (win ? 1 : -0.9));
      dayPnl += mag;
      total += mag;
      trades += 1;
      if (win) wins += 1;
      if (mag > best) best = mag;
      if (mag < worst) worst = mag;
    }
    daily.push({ date: d.toISOString().slice(0, 10), realised_pnl_cents: dayPnl, trades: t });
  }
  daily.reverse(); // newest first, matching the real API
  return {
    window_days: days,
    total_trades: trades,
    total_realised_pnl_cents: total,
    win_rate: trades > 0 ? wins / trades : 0,
    largest_win_cents: best,
    largest_loss_cents: worst,
    daily,
  };
}

export class MockBotService implements BotService {
  private listings: Listing[] = [];
  private subscriptions: Subscription[] = [];
  private bots = new Map<string, BotConfig>();

  constructor() {
    // Seed three marketplace listings owned by "other" creators.
    const seed = (title: string, creator: string, price: number, subs: number): Listing => ({
      id: newId('l'),
      bot_config_id: newId('b'),
      creator_id: newId('u'),
      creator_name: creator,
      title,
      summary: `${title} — demo listing with a simulated track record.`,
      monthly_price_cents: price,
      status: 'published',
      published_at: nowIso(),
      total_subscribers: subs,
    });
    this.listings = [
      seed('RSI Bounce v2', 'alice', 999, 42),
      seed('Breakout Beast', 'alice', 2999, 17),
      seed('EMA 12/26 Crossover', 'bob', 1499, 28),
    ];
  }

  async listMarketplace(): Promise<Listing[]> {
    return this.listings.filter(l => l.status === 'published');
  }

  async getListing(id: string): Promise<Listing> {
    const l = this.listings.find(x => x.id === id);
    if (!l) throw new Error('listing not found');
    return l;
  }

  async listingPerformance(id: string, days = 30): Promise<PerformanceMetrics> {
    return perf(id, days, 0.06);
  }

  async subscribe(listingId: string, allocatedCapitalCents: number): Promise<Subscription> {
    const listing = await this.getListing(listingId);
    const existing = this.subscriptions.find(
      s => s.listing_id === listingId && s.status === 'active'
    );
    if (existing) throw new Error('already subscribed to this listing');
    const sub: Subscription = {
      id: newId('s'),
      subscriber_id: 'u_me',
      listing_id: listingId,
      allocated_capital_cents: allocatedCapitalCents,
      status: 'active',
      started_at: nowIso(),
      listing,
    };
    this.subscriptions.push(sub);
    listing.total_subscribers += 1;
    return sub;
  }

  async cancelSubscription(subId: string): Promise<void> {
    const sub = this.subscriptions.find(s => s.id === subId);
    if (sub) {
      sub.status = 'cancelled';
      const listing = this.listings.find(l => l.id === sub.listing_id);
      if (listing && listing.total_subscribers > 0) listing.total_subscribers -= 1;
    }
  }

  async mySubscriptions(): Promise<Subscription[]> {
    return this.subscriptions.filter(s => s.status === 'active');
  }

  async listTemplates(): Promise<TemplateSummary[]> {
    return TEMPLATES.map(({ id, name, category, description, risk_level }) => ({
      id,
      name,
      category,
      description,
      risk_level,
    }));
  }

  async getTemplate(id: string): Promise<Template> {
    const t = TEMPLATES.find(x => x.id === id);
    if (!t) throw new Error('template not found');
    return t;
  }

  async listMyBots(status?: BotStatus): Promise<BotConfigSummary[]> {
    return [...this.bots.values()]
      .filter(b => !status || b.status === status)
      .map(b => ({
        id: b.id,
        name: b.name,
        status: b.status,
        source: b.source,
        published_listing_id: b.published_listing_id,
        updated_at: b.updated_at,
      }))
      .sort((a, b) => (a.updated_at < b.updated_at ? 1 : -1));
  }

  async getMyBot(id: string): Promise<BotConfig> {
    const b = this.bots.get(id);
    if (!b) throw new Error('bot not found');
    return b;
  }

  async createFromTemplate(templateId: string, name: string): Promise<BotConfig> {
    const t = await this.getTemplate(templateId);
    return this.insert(name, t.description, t.strategy, t.asset_filter, `template:${t.id}`);
  }

  async createBot(payload: {
    name: string;
    description?: string;
    strategy: Strategy;
    asset_filter: AssetFilter;
    source?: string;
  }): Promise<BotConfig> {
    return this.insert(
      payload.name,
      payload.description ?? '',
      payload.strategy,
      payload.asset_filter,
      payload.source ?? 'manual'
    );
  }

  private insert(
    name: string,
    description: string,
    strat: Strategy,
    asset: AssetFilter,
    source: string
  ): BotConfig {
    const id = newId('b');
    const bot: BotConfig = {
      id,
      user_id: 'u_me',
      name,
      description,
      status: 'draft',
      source,
      published_listing_id: null,
      strategy: strat,
      asset_filter: asset,
      created_at: nowIso(),
      updated_at: nowIso(),
    };
    this.bots.set(id, bot);
    return bot;
  }

  async updateBot(
    id: string,
    patch: { name?: string; description?: string; strategy?: Strategy; asset_filter?: AssetFilter }
  ): Promise<BotConfig> {
    const b = await this.getMyBot(id);
    if (patch.name !== undefined) b.name = patch.name;
    if (patch.description !== undefined) b.description = patch.description;
    if (patch.strategy !== undefined) b.strategy = patch.strategy;
    if (patch.asset_filter !== undefined) b.asset_filter = patch.asset_filter;
    b.updated_at = nowIso();
    return b;
  }

  async deleteBot(id: string): Promise<void> {
    this.bots.delete(id);
  }

  async setBotStatus(id: string, status: BotStatus): Promise<BotConfig> {
    if (status === 'live') throw new Error('live promotion is not available in demo mode');
    const b = await this.getMyBot(id);
    b.status = status;
    b.updated_at = nowIso();
    return b;
  }

  async publishBot(
    id: string,
    title: string,
    summary: string,
    monthlyPriceCents: number
  ): Promise<Listing> {
    const b = await this.getMyBot(id);
    const listing: Listing = {
      id: newId('l'),
      bot_config_id: b.id,
      creator_id: 'u_me',
      creator_name: 'you',
      title,
      summary,
      monthly_price_cents: monthlyPriceCents,
      status: 'published',
      published_at: nowIso(),
      total_subscribers: 0,
    };
    this.listings.push(listing);
    b.published_listing_id = listing.id;
    b.updated_at = nowIso();
    return listing;
  }

  async unpublishListing(listingId: string): Promise<void> {
    const listing = this.listings.find(l => l.id === listingId);
    if (listing) {
      listing.status = 'unpublished';
      const owner = [...this.bots.values()].find(b => b.published_listing_id === listingId);
      if (owner) owner.published_listing_id = null;
    }
  }

  async botPerformance(id: string, days = 30): Promise<PerformanceMetrics> {
    // A freshly-created bot has no history yet; return an empty window so
    // the chart shows the "no trades" state until the user explores.
    const b = this.bots.get(id);
    if (b && b.status === 'draft') {
      return { window_days: days, total_trades: 0, total_realised_pnl_cents: 0, win_rate: 0, largest_win_cents: 0, largest_loss_cents: 0, daily: [] };
    }
    return perf(id, days, 0.05);
  }

  async generateStrategy(prompt: string): Promise<AiGenerateResponse> {
    const p = prompt.toLowerCase();
    const id = p.includes('breakout')
      ? 'tpl_breakout_box'
      : p.includes('ema') || p.includes('cross')
        ? 'tpl_ema_cross'
        : 'tpl_rsi_oversold';
    const t = await this.getTemplate(id);
    return { strategy: t.strategy, asset_filter: t.asset_filter, source_template_id: id, stub: true };
  }
}

// One mock instance shared by every store, so a bot built on the BUILD
// tab and published shows up on the MARKET tab, a subscription on MARKET
// shows on the SUBS tab, etc. Lives for the page/app session.
let shared: MockBotService | null = null;
export function getSharedMock(): MockBotService {
  if (!shared) shared = new MockBotService();
  return shared;
}
