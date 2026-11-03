# llms-install.md — Installing Diting

Diting is an agent-first browser server: one Rust binary exposing an HTTP API and a Firecrawl-compatible `/v1/scrape`. No Chromium, no Docker. (The MCP and CDP faces were removed; HTTP is the only protocol surface.)

## Option A — Hosted instance (zero install)

The public instance is already running. Point any HTTP client at it:

```bash
curl -sS https://browser.aginx.net/health
# → {"status":"ok","screenshot":true,"stealth":true,...}
```

## Option B — Self-host from a release binary

1. Run the installer (macOS / Linux / Windows git-bash; it picks the platform, verifies the SHA-256 and runs `diting doctor`):

```bash
curl -fsSL https://raw.githubusercontent.com/yinnho/diting/main/install.sh -o install.sh && sh install.sh
```

   Or download `diting-<version>-<target>.tar.gz` for your platform from https://github.com/yinnho/diting/releases/latest
   (targets: `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-musl`,
   `x86_64-pc-windows-msvc`) and unpack it:

```bash
tar xzf diting-*.tar.gz && cd diting-*/
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



## Surfaces

fetch, search, download, screenshot, eval, click, flow replay, and the stateful-session family (create/navigate/state/click/input/scroll/eval/screenshot/export/close). Route guide: [SKILL.md](SKILL.md); a running engine describes itself at `GET /doctor` and `GET /flow/info?name=<flow>`.
