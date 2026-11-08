# What driving a Chrome-based agent browser for a day taught us about not using Chrome

*2026-11-08, macOS arm64. We spent a day driving the most popular Chrome-based
agent browser — [vercel-labs/agent-browser](https://github.com/vercel-labs/agent-browser)
(0.41.2, a Rust CLI that drives real Chrome over the DevTools Protocol) — on
the same pages we run diting on, in the same network windows, and wrote down
what a Chromium-free engine actually buys and what it costs. Numbers below are
first-hand; where a claim isn't ours, we link the maintainer's own issue.*

## What Chrome-based gets right

Credit first, because it's real:

- **A warm browser is unbeatable for interaction.** Once the daemon and Chrome
  are up, `read` on an already-open page returned in **10 ms**. The
  accessibility-tree snapshot with `@e1`-style refs is a genuinely good agent
  loop, and clicking a ref on react.dev just worked.
- **Ecosystem breadth**: cloud browser providers, session persistence, pluggable
  engines (they already ship `--engine lightpanda` and `--engine obscura`).

## Where driving real Chrome hurts — first hand

Same six pages (example.com, react.dev, nextjs.org, bun.sh, blog.rust-lang.org,
nodejs.org), one session, timed CLI calls, peak process-tree RSS sampled at
0.2 s:

- **Cold start is a browser launch.** `open https://example.com` from a dead
  daemon: **6.4–7.1 s** before text was readable. That's the daemon spawn plus
  a full Chrome process tree. The project's own tracker corroborates the shape
  of this pain on CI and Windows: a 30 s cold first open while Chrome is still
  launching ([#2369](https://github.com/vercel-labs/agent-browser/issues/2369)),
  ~6 s to connect when tabs were discarded
  ([#2431](https://github.com/vercel-labs/agent-browser/issues/2431)), and
  stuck tabs surfacing as raw `os error 35`
  ([#2279](https://github.com/vercel-labs/agent-browser/issues/2279)).
- **Memory is a browser tree.** ~830 MB with one page open, **2.0 GB after six
  pages** in one session. Every page is a tab in a real Chrome; the tree only
  grows.
- **You maintain a second browser.** The CLI drives whatever Chrome it can
  find — on this machine it grabbed the real installed Chrome 157 (version
  coupling: what your automation tests today, a Chrome auto-update changes
  tomorrow). With no system Chrome, it pulls Chrome for Testing — this very
  machine carries a **549 MB** Puppeteer cache from exactly that path. Two
  browsers to install, patch, and keep behavior-compatible.
- **The automation shape is detectable.** A real Chrome with DevTools attached
  (Runtime.enable, CDP sockets) is a fingerprint of its own; agent-browser
  ships `--no-webmcp` flags to shave it off. Chromium-fork agent browsers hit
  the same wall — sites that detect DevTools and blank out
  ([BrowserOS #3146](https://github.com/browseros-ai/BrowserOS/issues/3146)).
- **Version skew bites the toolchain, not just the page.** npm-shipped CLI
  binaries vs a moving Chrome: their own regression report — npm binaries
  missing a fix, macOS press flood reappearing
  ([#2299](https://github.com/vercel-labs/agent-browser/issues/2299)).

## What no-Chromium buys — the same afternoon, same pages

[diting](https://github.com/yinnho/diting) (0.10.0, own V8 + own
CSS/layout/paint/network, one binary):

- **Cold start is a process start, not a browser launch**: server ready in
  **0.27 s**, first JS-rendered fetch of example.com at **2.39 s** — **2.4×
  faster end-to-end** than the Chrome-based cold path on the same box.
- **One dependency, zero second browser**: a single ~180 MB binary (release
  build; ~396 MB with screenshot features). No Chrome to have, update, or
  fear an auto-update on. The engine *is* the version — behavior can't drift
  under you between Tuesday and Wednesday.
- **Flat memory curve**: ~1.0 GB after the first page, ~1.25 GB after six —
  the server holds one engine, not a growing tab forest.
- **Every response is a receipt** — tier hit, redirect trail, content hash —
  so a pipeline can act on what actually happened instead of hoping a tab
  rendered.
- **"But compatibility?" is a measured question now**: diting's generated
  WebIDL diff covers 428 standard Window interfaces with **zero missing among
  everything Chrome has actually shipped** (the remaining 71 are
  experimental/unreleased faces). Not using Chrome no longer means guessing
  at the web platform.

## What no-Chromium costs — said plainly

- **Heavy SPA walls in a bad network moment**: on our run, nextjs.org took
  20.3 s via diting vs 7.1 s via Chrome-based earlier in the day (the network
  window flapped between runs; treat the SPA deltas as directional, the cold
  starts as solid). Chrome's parallel load pipeline is genuinely good.
- **You give up the mature interaction layer.** Refs, actuation quality,
  a11y-tree ergonomics — that's years of Chrome-ecosystem work. A no-Chromium
  engine has to earn that surface itself.

## The honest takeaway

Chrome-based agent browsers are **interaction-first**: a big, warm runtime
that is superb once hot and heavy always. No-Chromium engines are
**pipeline-first**: cheap to start, cheap to ship, uniform behavior, receipts
over hope — the shape agent fleets want when there are fifty tasks, not one,
and the runtime lives in CI or a container where "install Chrome" is the
tax. Neither is free; now there are first-hand numbers for both sides of the
ledger.

*Reproduce: `bench/five_engine.py` in this repo drives the same comparison;
the page set is the robots-clean subset in `bench/pages.txt`.*
