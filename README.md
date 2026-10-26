# Diting

**The browser engine built for agents.** One Rust binary, zero Chromium, zero Docker. See, read, find, act, remember.

```bash
curl -fsSL https://browser.aginx.net/install.sh | sh
diting  # → HTTP on :8089, everything below is plain REST
```

<video src="https://github.com/yinnho/diting/releases/download/v0.7.0/diting-vs-competitors-2026-09.mp4" controls muted width="720"></video>

## Why agents need their own browser

Existing "browser automation" was built for humans or one-shot scraping. Agents need five things at once — **see, read, find, act, remember** — and they need them cheap:

- **7.6× faster** to agent-usable text than headless Chrome (same 20 pages, same network, p50 532 ms vs 4 053 ms)
- **~25 MB cold start, <40 MB in everyday static use** (measured whole-process); even a heavy JS page runs ~150 MB against Chrome's ~2 GB **per page**
- **Zero hard failures** where Chrome's dump-DOM produced nothing

An agent's total cost is browser efficiency × model efficiency. This is the browser half.

## What it does

- **Full browser, no Chromium** — V8 + an owned Rust HTTP/CSS/layout/paint stack in one binary. Screenshots and rendering are built in; there is nothing to download and nothing to sandbox
- **Real TLS fingerprints** — production-grade impersonation that passes edge checks where fingerprint-less engines eat 403s; CAPTCHA and challenge pages are detected, waited on, and reported instead of handed over as content
- **Stateful sessions** — login state injects, exports, and survives restarts. The same session id comes back logged in
- **Tiered rendering** — static pages answer in ~100 ms; V8 spins up only when a page actually needs JS. Every response says which tier served it
- **Meta-search** — 20 engines across 7 categories (including the Chinese web), merged and deduplicated; reads land with section-level snippets
- **Flows** — a multi-step interaction an agent figured out becomes a replayable JSON document: reruns cost zero model tokens
- **Documents** — deterministic markdown → HTML + diagrams, byte-identical by construction; page sets export to PDF / PPTX / DOCX
- **Receipts** — every fetch reports its tier, redirect trail, and content hash. Agents act on what actually happened
- **Local memory** — fetch/search cache (SQLite FTS5) with CJK matching, so re-reads don't re-pay

## Quick start

```bash
# Homebrew (macOS 7+ needs the trust line first)
brew trust yinnho/diting && brew install yinnho/diting/diting

# Or build from source (full features)
cargo build --release --features stealth,screenshot

diting &  # listening on :8089
curl :8089/health          # → {"status":"ok","engine":"diting"}
curl -X POST :8089/fetch -d '{"url":"https://example.com"}' | jq -r .markdown
curl -X POST :8089/search -d '{"query":"rust browser engine"}' | jq '.results[:3]'
curl -X POST :8089/session/create -d '{"url":"https://example.com"}'  # → session_id
```

## Where it sits

Computer-use agents come in two layers: GUI-layer stacks drive a whole desktop; **engine-layer** browsers make the page itself the machine. Diting is the engine layer — DOM actions in, structured state out, one process per session instead of a VM. It is a real-time retrieval instrument, not a crawler: no link-following recursion, built-in per-domain budgets, robots.txt opt-in.

Self-host under Apache-2.0, or use the hosted instance at [browser.aginx.net](https://browser.aginx.net).

## Documentation

- [API reference](docs/API.md) · [中文](docs/API.zh-CN.md) · [install for LLM agents](llms-install.md)
- Works with existing Firecrawl clients (`/v1/scrape`) and any HTTP client — no SDK required

## Star History

[![Star History Chart](https://api.star-history.com/svg?repos=yinnho/diting&type=Date)](https://star-history.com/#yinnho/diting&Date)

## License

Apache-2.0
