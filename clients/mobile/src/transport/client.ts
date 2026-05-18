// WebSocket transport: auto-reconnects with exponential backoff, queues
// outbound frames until the socket is open, and dispatches every inbound
// binary frame to a single sink callback (the store handles fan-out).

import { CancelOrder, ExecutionReport, NewOrder } from '../protocol/types';
import { decodeFrame, encodeCancelOrder, encodeNewOrder } from '../protocol/qftx';

export type ConnState =
  | { kind: 'idle' }
  | { kind: 'connecting'; url: string }
  | { kind: 'open'; url: string; since: number }
  | { kind: 'closed'; url: string; code?: number; reason?: string }
  | { kind: 'error'; url: string; message: string };

export interface TradingClientCallbacks {
  onExec: (r: ExecutionReport) => void;
  onState: (s: ConnState) => void;
  onParseError?: (e: Error) => void;
}

const MAX_BACKOFF_MS = 30_000;
const SUBPROTOCOL = 'qftx-binary.v1';

export class TradingClient {
  private ws: WebSocket | null = null;
  private url: string | null = null;
  private outbox: ArrayBuffer[] = [];
  private reconnectAttempt = 0;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private explicitlyClosed = false;

  constructor(private readonly cb: TradingClientCallbacks) {}

  connect(url: string): void {
    this.disconnect();
    this.explicitlyClosed = false;
    this.url = url;
    this.openSocket();
  }

  disconnect(): void {
    this.explicitlyClosed = true;
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    if (this.ws) {
      try {
        this.ws.close();
      } catch {
        /* ignore */
      }
      this.ws = null;
    }
    if (this.url) this.cb.onState({ kind: 'closed', url: this.url });
  }

  isOpen(): boolean {
    return this.ws !== null && this.ws.readyState === WebSocket.OPEN;
  }

  placeOrder(o: NewOrder): void {
    this.send(encodeNewOrder(o));
  }

  cancelOrder(c: CancelOrder): void {
    this.send(encodeCancelOrder(c));
  }

  private send(buf: ArrayBuffer) {
    if (this.isOpen()) {
      this.ws!.send(buf);
    } else {
      this.outbox.push(buf);
    }
  }

  private openSocket() {
    if (!this.url) return;
    const url = this.url;
    this.cb.onState({ kind: 'connecting', url });
    let ws: WebSocket;
    try {
      // React Native and browsers both accept this constructor signature.
      ws = new WebSocket(url, SUBPROTOCOL);
    } catch (e) {
      this.cb.onState({ kind: 'error', url, message: (e as Error).message });
      this.scheduleReconnect();
      return;
    }
    ws.binaryType = 'arraybuffer';
    this.ws = ws;

    ws.onopen = () => {
      this.reconnectAttempt = 0;
      this.cb.onState({ kind: 'open', url, since: Date.now() });
      // Drain queued frames.
      for (const frame of this.outbox) ws.send(frame);
      this.outbox = [];
    };
    ws.onmessage = (ev: WebSocketMessageEvent) => {
      const data = ev.data;
      if (!(data instanceof ArrayBuffer)) {
        this.cb.onParseError?.(new Error('expected ArrayBuffer, got text frame'));
        return;
      }
      try {
        const report = decodeFrame(data);
        this.cb.onExec(report);
      } catch (e) {
        this.cb.onParseError?.(e as Error);
      }
    };
    ws.onerror = (ev: Event) => {
      const message =
        (ev as { message?: string }).message ?? 'websocket error (no message available)';
      this.cb.onState({ kind: 'error', url, message });
    };
    ws.onclose = (ev: WebSocketCloseEvent) => {
      this.ws = null;
      this.cb.onState({
        kind: 'closed',
        url,
        code: ev.code,
        reason: ev.reason,
      });
      if (!this.explicitlyClosed) this.scheduleReconnect();
    };
  }

  private scheduleReconnect() {
    if (this.explicitlyClosed) return;
    this.reconnectAttempt += 1;
    const backoff = Math.min(
      MAX_BACKOFF_MS,
      Math.round(500 * Math.pow(1.7, this.reconnectAttempt - 1))
    );
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      this.openSocket();
    }, backoff);
  }
}

// React Native's lib.dom types for WebSocket events.
interface WebSocketMessageEvent {
  data: unknown;
}
interface WebSocketCloseEvent {
  code: number;
  reason: string;
  wasClean?: boolean;
}
