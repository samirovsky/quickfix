# Deploying QFTX (mobile web + bot-service)

A working live URL needs **two** deploys: the mobile web bundle (static SPA) and the bot-service API (Rust container with persistent volume for sqlite). The Trade tab also needs the trading-server's WebSocket, but the BUILD / MARKET / SUBS flows work standalone with just the bot-service.

The repository is shaped so each half can be deployed independently. Everything below runs from your laptop; I cannot deploy from the sandbox this repository is being developed in (its egress proxy blocks `vercel.com`, `fly.io`, `render.com`, etc. with `host_not_allowed`).

---

## 1. Mobile web → Vercel

The web export of QFTX Trader is a static SPA — Vercel serves it as plain static files. There are two paths.

### Option A — Vercel CLI (one-shot)

```bash
cd clients/mobile
npx vercel login           # first time only — opens browser
npx vercel deploy --prod   # builds with `npm run vercel-build`, uploads dist/
```

`vercel.json` is already in place; it runs `npm run vercel-build` (which is `expo export --platform web --output-dir dist`) and serves `dist/`. You get a `*.vercel.app` URL printed in the terminal.

### Option B — Git integration (recommended, auto-deploys on push)

1. Open <https://vercel.com/new> and import the repo.
2. **Set Root Directory to `clients/mobile`** under "Project Settings → Root Directory". Without this, Vercel will try to build from the repo root and fail.
3. Framework preset: **Other** (Vercel detects `vercel.json` and skips auto-config).
4. Click **Deploy**. Every future push to the branch deploys; PRs get preview URLs.

### Build verification (run on the latest branch)

```text
$ npm run vercel-build
Web Bundled, 463 modules
_expo/static/js/web/index-<hash>.js  1.8 MB
index.html  1.18 kB
App exported to: dist
```

---

## 2. bot-service → Fly.io / Render / your own host

The bot-service is shipped with a multi-stage `Dockerfile` and reads everything from env vars. Pick whichever host fits.

### Fly.io (recommended — has free persistent volumes)

```bash
fly launch --no-deploy --copy-config --dockerfile rust/bot-service/Dockerfile \
           --name qftx-bot-service
fly volumes create bot_data --size 1 --region <region>
fly secrets set \
   BOT_SERVICE_SEED_DEMO=1 \
   BOT_SERVICE_CORS_ORIGINS=https://<your-vercel-domain>.vercel.app
# Edit fly.toml to mount the volume at /data — see Fly's docs.
fly deploy
```

### Render

Create a Web Service from the GitHub repo, Dockerfile path `rust/bot-service/Dockerfile`, set the env vars listed below. Add a disk mounted at `/data` (1 GB is plenty).

### Required env vars

| Var                          | Required for deploy? | Example                                                       |
| ---------------------------- | -------------------- | ------------------------------------------------------------- |
| `BOT_SERVICE_BIND`           | yes                  | `0.0.0.0:9100` (Dockerfile sets this by default)              |
| `BOT_SERVICE_DB`             | yes                  | `sqlite:///data/bot.sqlite` (uses the mounted volume)         |
| `BOT_SERVICE_CORS_ORIGINS`   | **yes** for the Vercel UI | `https://<your-vercel-domain>.vercel.app` (or `*` for demo)   |
| `BOT_SERVICE_SEED_DEMO`      | optional             | `1` to populate the demo bots + 30 days of synthetic trades   |
| `BOT_SERVICE_KEYS`           | optional             | Path to a mounted JSON file of `[{name, api_key}]`            |
| `RUST_LOG`                   | optional             | `info`, `debug`, etc.                                          |

### Build verification (local)

```bash
docker build -t bot-service -f rust/bot-service/Dockerfile .
docker run --rm -p 9100:9100 \
  -e BOT_SERVICE_BIND=0.0.0.0:9100 \
  -e BOT_SERVICE_DB=sqlite:///tmp/bot.sqlite \
  -e BOT_SERVICE_SEED_DEMO=1 \
  -e BOT_SERVICE_CORS_ORIGINS='*' \
  bot-service
# curl http://127.0.0.1:9100/healthz   → {"status":"ok"}
```

---

## 3. Point the deployed UI at the deployed bot-service

Once both are live, open the deployed Vercel URL on a phone, go to **Connect**, and enter:

* **Bot service URL**: `https://<your-bot-service-host>` (the Fly/Render URL).
* **API key**: `demo-alice-please-rotate` (or `demo-bob-please-rotate`) if `BOT_SERVICE_SEED_DEMO=1` was set. Otherwise the key you provisioned via `BOT_SERVICE_KEYS`.

The BUILD / MARKET / SUBS tabs work entirely against the bot-service. No trading-server needed yet — that's for the TRADE tab.

### Browser limitation to know about

The deployed site is `https://`. Browsers **block** `http://` API calls from `https://` pages (mixed content). Your bot-service URL **must** be `https://` for the deployed UI to reach it. Fly.io, Render, Railway and similar terminate TLS for you automatically; that's why they're the path of least resistance here. Plain `http://your-vm-ip:9100` will not work from a Vercel-served page.

---

## 4. (Later) trading-server for the Trade tab

The Trade tab needs the matching engine's WebSocket. Same browser rule applies: it has to be `wss://` from an `https://` page.

Two paths when you're ready:

* **Reverse-proxy with TLS.** nginx/caddy in front of `rust/trading-server`, TLS terminated by the proxy, app points at `wss://your-domain/ws`. Tungstenite accepts the upgrade unchanged.
* **Cloudflare Tunnel.** `cloudflared tunnel --url http://localhost:9002` from any host running the trading server gives you an instant `wss://*.trycloudflare.com` URL with zero infra work.

There is no auth on the WS endpoint today — only expose the trading server publicly if you understand that anyone with the URL can place orders against the same shared book.
