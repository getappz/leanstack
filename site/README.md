# agentflare.dev

The landing site for agentflare, deployed to **Cloudflare Workers** with static
assets — the same setup pattern as the sarvo apps (`apps/web`: a Worker with an
`assets` binding + a custom-domain route).

```
site/
├── wrangler.jsonc     # Worker config — assets binding + agentflare.dev custom domain
├── package.json       # dev / deploy / tail scripts (wrangler only, no build step)
├── src/
│   └── worker.ts      # serves ./public from ASSETS; proxies /install.sh + /install.ps1 → raw GitHub
└── public/
    ├── index.html     # the site — self-contained (inline CSS/JS, SVG favicon, system fonts)
    └── 404.html       # on-brand not-found page
```

There is **no build step** — `public/index.html` is hand-authored and shipped as-is.

## Deploy

```bash
cd site
npm install          # or: aube install  — pulls wrangler
npm run deploy       # wrangler deploy
```

`wrangler deploy` uploads `public/` to the `ASSETS` binding, bundles `src/worker.ts`,
and (first deploy only) provisions the `agentflare.dev` + `www.agentflare.dev` custom
domains declared in `wrangler.jsonc`. The apex zone must already be on this Cloudflare
account; Wrangler creates the custom-domain records.

Local preview: `npm run dev` (serves on `localhost:8787`). Live logs: `npm run tail`.

## What the Worker does

- **Everything** → served from `public/` via the `ASSETS` binding.
- **`/install.sh`, `/install.ps1`** → proxied from
  `raw.githubusercontent.com/getappz/agentflare/master`, edge-cached 5 min, so
  `curl -fsSL https://agentflare.dev/install.sh | sh` works and the hero command
  resolves on the domain instead of exposing a raw GitHub URL.
- **Unknown paths** → `public/404.html` (`not_found_handling: "404-page"`).

## Editing

All page content lives in `public/index.html` — copy, the benchmark numbers, and the
animated hero terminal are inline and commented. Keep the numbers in sync with the
repo's root `README.md` metrics table; the site deliberately mirrors its
"attributed, not blended" framing.

## Optional polish (free tiers, none required)
- **Fonts** — the page uses a system monospace/sans stack, so it needs no network. To
  push the display type further, self-host a face (Commit Mono, Geist Mono, Departure
  Mono) under `public/fonts/` and add a `@font-face` — keep it self-hosted so the page
  stays request-free.
- **Analytics** — Cloudflare Web Analytics (free, cookieless) via the dashboard, or a
  `<script>` beacon.
- **Social image** — add a real `og:image` PNG (1200×630) under `public/` and reference
  it in `<head>` for richer link unfurls on X / Slack / Discord.
