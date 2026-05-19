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

export class BotServiceError extends Error {
  constructor(
    public readonly status: number,
    public readonly code: string | undefined,
    message: string
  ) {
    super(message);
  }
}

export class BotServiceClient {
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
}
