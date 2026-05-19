# Deploying to Vercel

The web export of QFTX Trader is a static SPA — Vercel serves it as plain static files. There are two paths.

## Option 1 — Vercel CLI (one-shot)

From your laptop, where you can authenticate interactively:

```bash
cd clients/mobile
npx vercel login           # first time only — opens browser
npx vercel deploy --prod   # builds with `npm run vercel-build`, uploads dist/
```

Vercel reads `vercel.json` here, runs `npm run vercel-build` (which is `expo export --platform web --output-dir dist`), and serves `dist/`. You get a `*.vercel.app` URL printed in the terminal.

## Option 2 — Git integration (recommended, auto-deploys on push)

1. Push the branch (already done — `claude/trading-server-rust-HUjf5` on `samirovsky/quickfix`).
2. Open <https://vercel.com/new> and import the repo.
3. **Important — set Root Directory to `clients/mobile`** under "Project Settings → Root Directory" during import. Without this, Vercel will try to build from the repo root and fail because there's no `package.json` there.
4. Framework preset: **Other** (Vercel will detect `vercel.json` and skip auto-config).
5. Click **Deploy**.

Every future push to the branch deploys automatically. Pull requests get preview URLs.

## What you get

A public `https://<project-name>-<hash>.vercel.app` URL serving the QFTX Trader UI on iOS Safari, Android Chrome, and desktop.

## The mixed-content gotcha

The deployed site is served over `https://`. Browsers **block** `ws://` connections from `https://` pages (mixed content). With the current setup the live demo on Vercel will **not** be able to connect to a local trading server at `ws://localhost:9002/ws`.

Three ways to actually trade through the deployed UI:

1. **Run the UI locally instead.** `npm start` in `clients/mobile/` gets you the same app and lets you point at `ws://localhost:9002` without restriction. Use the Vercel deploy as a public demo, run locally to actually trade.

2. **Expose the trading server with TLS.** Run nginx/caddy in front of the trading server on a public host, terminate TLS there, and point the app at `wss://your-domain.com/ws`. The server's current `tokio-tungstenite` setup accepts the upgrade unchanged once TLS termination lives in front of it.

3. **Cloudflare Tunnel / ngrok.** `cloudflared tunnel --url http://localhost:9002` or `ngrok http 9002` gives you a `wss://*.trycloudflare.com` URL pointing at your local server with no public IP or DNS work. The app just needs that URL in the Connect tab.

There is no auth on the WS endpoint today — only expose the trading server publicly if you understand that anyone with the URL can place orders.

## Build verification

The build was verified from this branch:

```text
$ npm run vercel-build
Web Bundled 10576ms index.ts (449 modules)
Exporting 1 bundle for web:
  _expo/static/js/web/index-<hash>.js (700 kB)
Exporting 4 files:
  index.html  (1.18 kB)
  metadata.json + 2 PNG assets
App exported to: dist
```

So Vercel's build step (`npm run vercel-build`) is known-good in a clean environment.
