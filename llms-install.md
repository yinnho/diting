# llms-install.md — Installing Diting

Diting is an agent-first browser server: one Rust binary exposing an HTTP API and a Firecrawl-compatible `/v1/scrape`. No Chromium, no Docker. (The MCP and CDP faces were removed; HTTP is the only protocol surface.)

## Option A — Hosted instance (zero install)

The public instance is already running. Point any HTTP client at it:

```bash
curl -sS https://browser.aginx.net/health
# → {"status":"ok","screenshot":true,"stealth":true,...}
```

## Option B — Self-host from a release binary

1. Download the archive for your platform from https://github.com/yinnho/diting/releases/latest (`diting-<os>-<arch>.tar.gz`), then:

```bash
tar xzf diting-*.tar.gz && chmod +x diting
```

2. Run it:

```bash
./diting          # HTTP server on 0.0.0.0:8089 (REST + /v1/scrape)
```

Verify:

```bash
curl -sS http://127.0.0.1:8089/health   # → {"status":"ok",...}
```

## First calls

```bash
# read a page as markdown
curl -sS -X POST http://127.0.0.1:8089/fetch \
  -H 'Content-Type: application/json' \
  -d '{"url":"https://example.com","format":"markdown"}'

# aggregated search
curl -sS -X POST http://127.0.0.1:8089/search \
  -H 'Content-Type: application/json' \
  -d '{"q":"rust browser engine","max_results":5}'

# interactive session
curl -sS -X POST http://127.0.0.1:8089/session/create \
  -H 'Content-Type: application/json' \
  -d '{"url":"https://example.com"}'
```

## The market (v0.7.1+)

`POST /activation/activate {"code": "oc_..."}` (key from duphub.com Keys page) connects the capability market: `flow/run {"name": ...}` auto-prices and buys hosted flows (paid ones execute in memory), `/activation/quota` checks the wallet, and low balance enters the recharge loop (`/activation/topup` returns a WeChat QR the agent shows the owner). See [docs/API.md](docs/API.md) § DupHub Market.

## Surfaces

fetch, search, download, screenshot, eval, click, flow replay, and the stateful-session family (create/navigate/state/click/input/scroll/eval/screenshot/export/close). Full reference: [docs/API.md](docs/API.md).
