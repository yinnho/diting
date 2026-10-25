# Using Diting as an agent-browser-style engine (HTTP)

> Status: **shipped via HTTP** (`/session` + `/snapshot` + `ref`). The CDP face was evaluated and retired — HTTP is the only protocol surface; upstream `--engine aginx` adapters would speak HTTP.

[中文](agent-browser-engine.zh-CN.md)

## Why HTTP first

[vercel-labs/agent-browser](https://github.com/vercel-labs/agent-browser) attaches Lightpanda and Obscura by spawning a **CDP** process (`obscura serve --host 127.0.0.1 --port …`). Diting today exposes a **local HTTP API** with the same agent loop:

1. create session → navigate
2. **snapshot** interactive elements (`ref=eN`)
3. **click** / **input** by `ref`
4. close

With CDP retired, coding agents should call HTTP directly (or use the thin shim below). That is copy-paste usable today; an upstream `--engine aginx` PR is retired with the CDP face.

## Prerequisites

```bash
# Install (Homebrew or release binary — see docs/install.md)
brew install yinnho/diting/diting   # or curl install.sh

# Start the HTTP server (default 0.0.0.0:8089)
diting &
curl -sS http://127.0.0.1:8089/health
```

Override the base URL with `DITING_URL` (shim + docs examples use this).

## Three-command smoke test

```bash
export BASE="${DITING_URL:-http://127.0.0.1:8089}"

SID=$(curl -sS -X POST "$BASE/session/create" \
  -H 'content-type: application/json' \
  -d '{"url":"https://example.com"}' | jq -r .session_id)

curl -sS "$BASE/session/$SID/snapshot?format=text"
# → lines like: - link "More information..." [ref=e0]

curl -sS -X POST "$BASE/session/$SID/click" \
  -H 'content-type: application/json' \
  -d '{"ref":"e0"}' | jq .
```

Or with the in-repo shim (agent-browser-shaped verbs over HTTP):

```bash
./scripts/aginx-agent open https://example.com
./scripts/aginx-agent snapshot
./scripts/aginx-agent click e0
```

## HTTP map (vs agent-browser verbs)

| agent-browser-ish verb | Diting HTTP |
|------------------------|-------------------|
| open / navigate | `POST /session/create` `{url}` or `POST /session/{id}/navigate` |
| snapshot | `GET\|POST /session/{id}/snapshot` (`?format=json\|text`) |
| click `@eN` | `POST /session/{id}/click` `{"ref":"eN"}` |
| fill / type | `POST /session/{id}/input` `{"ref":"eN","text":"..."}` |
| eval | `POST /session/{id}/eval` `{"script":"..."}` |
| screenshot | `POST /session/{id}/screenshot` (needs `--features screenshot` build) |
| close | `POST /session/{id}/close` |

Refs are `e` + the interactive index from the **current** snapshot. After navigate or DOM mutation, call `/snapshot` again — stale refs fail.

Full reference: [`API.md`](API.md) (`/session/{id}/snapshot`, `/click`, `/input`).

## Agent skill path

`SKILL.md` at the repo root is the Cursor / Claude Code skill. Prefer `/snapshot` + `ref` for multi-step UI work (same element map as `/state`, structured for agents). One-shot page reads still go through `/fetch`.

## Pointing vercel agent-browser at Aginx (today)

There is **no** `--engine aginx` in upstream yet. Practical options:

1. **Preferred for agents**: call Aginx HTTP (this doc + `scripts/aginx-agent`) and skip agent-browser for that session.
2. **Side-by-side**: keep `agent-browser --engine chrome|lightpanda|obscura` for CDP engines; use Aginx for stealth/TLS-sensitive Chinese merchant flows where HTTP session + cookie import already work (`/import/curl`, account sessions).
3. **Future**: after the retired CDP design note lands (`diting serve --cdp`), upstream can mirror the Obscura adapter (`cli/src/native/cdp/obscura.rs`: spawn → discover `webSocketDebuggerUrl` → shared CDP command path). Until then, a stub PR would be rejected — **upstream PR pending CDP**.

Example config you can keep locally for documentation (not honored by stock agent-browser):

```json
{
  "comment": "Proposed once CDP serve ships — not valid in agent-browser today",
  "engine": "aginx",
  "executablePath": "/opt/homebrew/bin/diting"
}
```

## Differences from Chrome / Obscura via agent-browser

| Feature | Aginx HTTP today | agent-browser + Chrome/Obscura |
|---------|------------------|--------------------------------|
| Protocol | HTTP JSON | CDP WebSocket |
| Snapshot refs | `eN` on `/snapshot` | accessibility snapshot refs |
| Stealth / TLS presets | First-class (`tls_fingerprint`, fleet UA Chrome/154) | Engine-dependent |
| Extensions / headed | No | Chrome yes; Obscura limited |
| MCP | Not offered by Aginx | agent-browser has MCP (out of scope here) |

## Related

- Snapshot feature commit lineage: session `/snapshot` with actionable refs
- Stealth/TLS public matrix: [`stealth-tls-comparison.md`](stealth-tls-comparison.md)
- CDP follow-up: the retired CDP design note
