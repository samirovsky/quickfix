// Thin REST client for the bot-service. We deliberately don't generate
// types from a schema — the shapes are small and stable enough that
// hand-mirrored interfaces are clearer than codegen ceremony.

export interface Listing {
  id: string;
  bot_config_id: string;
  creator_id: string;
  creator_name: string;
  title: string;
  summary: string;
  monthly_price_cents: number;
  status: 'published' | 'unpublished';
  published_at: string;
  total_subscribers: number;
}

export interface PerformanceMetrics {
  window_days: number;
  total_trades: number;
  total_realised_pnl_cents: number;
  win_rate: number;
  largest_win_cents: number;
  largest_loss_cents: number;
  daily: DailyMetric[];
}

export interface DailyMetric {
  date: string;
  realised_pnl_cents: number;
  trades: number;
}

export interface Subscription {
  id: string;
  subscriber_id: string;
  listing_id: string;
  allocated_capital_cents: number;
  status: 'active' | 'cancelled';
  started_at: string;
  listing: Listing;
}

// ---------- bots, templates ----------

export type BotStatus = 'draft' | 'paper' | 'live' | 'paused' | 'stopped';

export interface TemplateSummary {
  id: string;
  name: string;
  category: string;
  description: string;
  risk_level: number;
}

export interface Template extends TemplateSummary {
  asset_filter: AssetFilter;
  strategy: Strategy;
}

export interface AssetFilter {
  symbols: number[];
  market: string;
}

// The strategy shape is intentionally permissive on the client — we only
// edit a few well-known numeric fields and pass the rest through
// verbatim. `unknown` here means "don't pretend to fully model it".
export interface Strategy {
  version: number;
  entry: unknown;
  exit: unknown;
  position_sizing: PositionSizing;
  risk: RiskConfig;
  schedule: unknown;
}

export type PositionSizing =
  | { kind: 'fixed_amount'; value_cents: number }
  | { kind: 'percent_portfolio'; value: number; max_notional_cents: number }
  | { kind: 'kelly'; factor: number; max_notional_cents: number };

export interface RiskConfig {
  max_concurrent: number;
  max_daily_loss_cents: number;
  halt_after_n_losses: number;
}

export interface BotConfigSummary {
  id: string;
  name: string;
  status: BotStatus;
  source: string;
  published_listing_id: string | null;
  updated_at: string;
}

export interface BotConfig extends BotConfigSummary {
  user_id: string;
  description: string;
  strategy: Strategy;
  asset_filter: AssetFilter;
  created_at: string;
}

export class BotServiceError extends Error {
  constructor(
    public readonly status: number,
    public readonly code: string | undefined,
    message: string
  ) {
    super(message);
  }
}

export class BotServiceClient implements BotService {
  constructor(
    private readonly baseUrl: string,
    private readonly apiKey: string
  ) {}

  private async req<T>(method: string, path: string, body?: unknown): Promise<T> {
    const url = `${this.baseUrl.replace(/\/$/, '')}${path}`;
    const init: RequestInit = {
      method,
      headers: {
        'X-API-Key': this.apiKey,
        'Content-Type': 'application/json',
      },
    };
    if (body !== undefined) init.body = JSON.stringify(body);
    const resp = await fetch(url, init);
    if (resp.status === 204) return undefined as T;
    const text = await resp.text();
    const json = text ? (JSON.parse(text) as unknown) : null;
    if (!resp.ok) {
      const errObj = (json ?? {}) as { error?: string; code?: string };
      throw new BotServiceError(
        resp.status,
        errObj.code,
        errObj.error ?? `request failed: ${resp.status}`
      );
    }
    return json as T;
  }

  listMarketplace(): Promise<Listing[]> {
    return this.req('GET', '/v1/marketplace/listings');
  }

  getListing(id: string): Promise<Listing> {
    return this.req('GET', `/v1/marketplace/listings/${encodeURIComponent(id)}`);
  }

  listingPerformance(id: string, days = 30): Promise<PerformanceMetrics> {
    return this.req(
      'GET',
      `/v1/marketplace/listings/${encodeURIComponent(id)}/performance?days=${days}`
    );
  }

  subscribe(listingId: string, allocatedCapitalCents: number): Promise<Subscription> {
    return this.req('POST', `/v1/marketplace/listings/${encodeURIComponent(listingId)}/subscribe`, {
      allocated_capital_cents: allocatedCapitalCents,
    });
  }

  cancelSubscription(subId: string): Promise<void> {
    return this.req('POST', `/v1/subscriptions/${encodeURIComponent(subId)}/cancel`);
  }

  mySubscriptions(): Promise<Subscription[]> {
    return this.req('GET', '/v1/me/subscriptions');
  }

  // ---------- bot builder ----------

  listTemplates(): Promise<TemplateSummary[]> {
    return this.req('GET', '/v1/templates');
  }

  getTemplate(id: string): Promise<Template> {
    return this.req('GET', `/v1/templates/${encodeURIComponent(id)}`);
  }

  listMyBots(status?: BotStatus): Promise<BotConfigSummary[]> {
    const q = status ? `?status=${encodeURIComponent(status)}` : '';
    return this.req('GET', `/v1/bots${q}`);
  }

  getMyBot(id: string): Promise<BotConfig> {
    return this.req('GET', `/v1/bots/${encodeURIComponent(id)}`);
  }

  createFromTemplate(
    templateId: string,
    name: string,
    description?: string
  ): Promise<BotConfig> {
    return this.req('POST', `/v1/bots/from-template/${encodeURIComponent(templateId)}`, {
      name,
      ...(description !== undefined ? { description } : {}),
    });
  }

  createBot(payload: {
    name: string;
    description?: string;
    strategy: Strategy;
    asset_filter: AssetFilter;
    source?: string;
  }): Promise<BotConfig> {
    return this.req('POST', '/v1/bots', payload);
  }

  updateBot(
    id: string,
    patch: {
      name?: string;
      description?: string;
      strategy?: Strategy;
      asset_filter?: AssetFilter;
    }
  ): Promise<BotConfig> {
    return this.req('PUT', `/v1/bots/${encodeURIComponent(id)}`, patch);
  }

  deleteBot(id: string): Promise<void> {
    return this.req('DELETE', `/v1/bots/${encodeURIComponent(id)}`);
  }

  setBotStatus(id: string, status: BotStatus): Promise<BotConfig> {
    return this.req('POST', `/v1/bots/${encodeURIComponent(id)}/status`, { status });
  }

  publishBot(
    id: string,
    title: string,
    summary: string,
    monthlyPriceCents: number
  ): Promise<Listing> {
    return this.req('POST', `/v1/bots/${encodeURIComponent(id)}/publish`, {
      title,
      summary,
      monthly_price_cents: monthlyPriceCents,
    });
  }

  unpublishListing(listingId: string): Promise<void> {
    return this.req(
      'POST',
      `/v1/marketplace/listings/${encodeURIComponent(listingId)}/unpublish`
    );
  }

  botPerformance(id: string, days = 30): Promise<PerformanceMetrics> {
    return this.req('GET', `/v1/bots/${encodeURIComponent(id)}/performance?days=${days}`);
  }

  generateStrategy(prompt: string): Promise<AiGenerateResponse> {
    return this.req('POST', '/v1/ai/generate-strategy', { prompt });
  }
}

export interface AiGenerateResponse {
  strategy: Strategy;
  asset_filter: AssetFilter;
  source_template_id: string;
  stub: boolean;
}

/// The surface both the real HTTP client and the in-memory demo mock
/// implement. Stores depend on this interface, not the concrete class,
/// so swapping in the mock is a one-line change in `configure`.
export interface BotService {
  listMarketplace(): Promise<Listing[]>;
  getListing(id: string): Promise<Listing>;
  listingPerformance(id: string, days?: number): Promise<PerformanceMetrics>;
  subscribe(listingId: string, allocatedCapitalCents: number): Promise<Subscription>;
  cancelSubscription(subId: string): Promise<void>;
  mySubscriptions(): Promise<Subscription[]>;
  listTemplates(): Promise<TemplateSummary[]>;
  getTemplate(id: string): Promise<Template>;
  listMyBots(status?: BotStatus): Promise<BotConfigSummary[]>;
  getMyBot(id: string): Promise<BotConfig>;
  createFromTemplate(templateId: string, name: string, description?: string): Promise<BotConfig>;
  createBot(payload: {
    name: string;
    description?: string;
    strategy: Strategy;
    asset_filter: AssetFilter;
    source?: string;
  }): Promise<BotConfig>;
  updateBot(
    id: string,
    patch: { name?: string; description?: string; strategy?: Strategy; asset_filter?: AssetFilter }
  ): Promise<BotConfig>;
  deleteBot(id: string): Promise<void>;
  setBotStatus(id: string, status: BotStatus): Promise<BotConfig>;
  publishBot(
    id: string,
    title: string,
    summary: string,
    monthlyPriceCents: number
  ): Promise<Listing>;
  unpublishListing(listingId: string): Promise<void>;
  botPerformance(id: string, days?: number): Promise<PerformanceMetrics>;
  generateStrategy(prompt: string): Promise<AiGenerateResponse>;
}
