# Amiibo crawl, 2026-10-21

[English](peer-crawl-20261021.md) | [中文](peer-crawl-20261021.zh-CN.md)

Same-machine rerun of the crawl in Pierre Tachoire's post, [Benchmarking AI-native headless browsers built from scratch](https://lightpanda.io/blog/posts/benchmarking-ai-native-headless-browsers) (2026-10-19). Raw receipts: [`bench/peer-crawl-20261021/`](../bench/peer-crawl-20261021/).

This page does not replace the [2026-10-19 scoreboard](scoreboard.md). That page's Lightpanda and Obscura cells stay **not run**. Web Platform Tests were not run here. Moli and Kitesurf were not run here.

## What the post claims

The post compares Lightpanda 1.0.1, Obscura 0.2.5, Moli 1.1.18, and Kitesurf. AginxBrowser is not in that set. The performance piece is a crawl of the [Amiibo demo](https://demo-browser.lightpanda.io/amiibo/): follow every same-host link, 940 pages, a pool of 25. The method is the [crawler benchmark](https://github.com/lightpanda-io/demo/blob/main/BENCHMARKS.md). That page says the machine is an AWS m5.xlarge and that memory is Linux `smem` PSS. An older table on the same page counts 933 URLs; the live site on 2026-10-21, and the post, are 940.

Their published pool-25 figures, on their machine, in their words:

| Engine | Wall | CPU | Peak memory |
|--------|------|-----|-------------|
| Lightpanda 1.0.1 | 4.5s | 12s | 109MB |
| Obscura 0.2.5 | 11.5s | 29s | 201MB |
| Moli 1.1.18 | 17.4s | 40s | 230MB |

Those cells are their claim. They are not a column in the table below.

## What we measured

macOS 27.0.1, arm64, 2026-10-21 Asia/Shanghai. One engine at a time. Seed `https://demo-browser.lightpanda.io/amiibo/`. The index is a short HTML page plus `load.js`; a crawler that only reads the raw HTML sees no anchors. Each page waits until at least one `a[href]` exists (30s cap), keeps same-host http(s) links, and dedupes by absolute URL. The first page error stops the pool, matching the official crawler.

Memory is `ps` RSS, sampled about every 0.1s, summed across the matched processes. macOS has no `smem`. RSS is not PSS, and a 25-process sum counts shared pages more than once. Lightpanda and Obscura were started with `http_proxy`, `https_proxy`, `HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`, and `all_proxy` unset. Aginx ignores `http_proxy` unless `AGINXBROWSER_PROXY` is set; it was not set. The user's server on `127.0.0.1:8089` was left running and was excluded from the RSS sample by pid.

The release binary is `target/release/aginxbrowser`, version **0.6.0**, `/health` commit **`3dd366b`**, TLS preset `chrome145`. Git HEAD when this was written is `153c2af` (`release: v0.6.1`). The bench did not use that commit.

Aginx has no CDP listener ([`cdp-serve-design.md`](cdp-serve-design.md)), so Playwright and the official chromedp crawler cannot drive it. The Aginx client is [`bench/peer-crawl/crawl_http.py`](../bench/peer-crawl/crawl_http.py): `POST /session/create`, `/navigate`, `/eval`. Lightpanda and Obscura used a local darwin/arm64 build of the official crawler (chromedp v0.15.1, the pin in their demo). Two local changes, both outside the page-work path: the fork mode waits until the port accepts (up to 15s; upstream sleeps 100ms, and 25 processes were not listening yet on this machine); `CRAWL_LINKS=eval` polls `Runtime.evaluate` instead of `chromedp.Nodes`. Lightpanda runs did not set `CRAWL_LINKS`.

| Engine | Shape | How links were read | Wall | Peak RSS | Sampled CPU |
|--------|--------|---------------------|------|----------|-------------|
| Aginx 0.6.0 `3dd366b` | 1 process, 25 HTTP sessions, rate limit and page limit off | HTTP `/eval` | **88.425s** | 414912 KB (405 MiB) | 17.03s |
| Lightpanda 1.0.1 | 25 processes, `--fork` | `chromedp.Nodes` | **59.01s**, then **67.76s** | 861184 KB, then 861712 KB (841 MiB) | 11.62s on the second run |
| Obscura 0.2.5 | 1 process, 25 CDP targets | `Runtime.evaluate` | **54.83s** | 203424 KB (199 MiB) | 11.35s |

Lightpanda's first CPU sample wrote `0` because the 25 processes had already exited when the sampler was signalled. That zero is not a measurement. The second run records the highest cputime sum seen while the processes were still alive, so 11.62s is a lower bound. Crawler `user`/`sys` (about 11s + 12s on the Lightpanda runs, 0.81s + 0.89s on Obscura) is the Go client, not the browser.

On this Mac the wall order of the finished runs is Obscura 54.83s, Lightpanda 59.01s and 67.76s, Aginx 88.425s. Peak RSS order is Obscura, Aginx, Lightpanda. Obscura's finished number uses a different link query from Lightpanda's. Pool widths 1 and 5 from the post were not run. A 6-page Aginx smoke earlier the same evening took 14.6s, so a 940-page pool of 1 is on the order of half an hour and was left out.

## Aginx, default server

`AGINXBROWSER_DOMAIN_RATE_PER_MIN` defaults to 20 pages per minute per registrable domain. `AGINXBROWSER_SESSION_PAGE_LIMIT` defaults to 200. With both left at the default, pool 25 stopped at **20 pages** (193 URLs known) in **13.069s** with HTTP 429. Peak RSS 369808 KB, sampled CPU 0.82s. The 88.425s row is the same binary with both knobs set to `0`, which is what the 429 text tells the operator to do. The bench server also used a private store under `/tmp/peer-crawl/aginx-data`, `AGINXBROWSER_RESTORE_SESSIONS=0`, `AGINXBROWSER_CACHE_TTL_SECS=0`, `AGINXBROWSER_DISABLE_SITE_PRELOAD=1`, and `AGINXBROWSER_BIND=127.0.0.1:18092`.

## Aginx, the attempt that stopped

The first limits-off run stopped at **73 pages** (548 known) in **77.877s** with HTTP 500 on `https://demo-browser.lightpanda.io/amiibo/04d81d02.html`. The response body was not captured. The server stayed up. A later replay of that URL on a fresh session returned HTTP 200 and, after a short wait, 11 anchors. The server log of that attempt (`/tmp/peer-crawl/aginx.log`, not committed) contains `execute_scripts: fetch deadline reached after 0/1 script bodies arrived` and `loadAmiibo is not defined`, plus `#66 busy window: 0% duty` lines. The script-fetch deadline defaults to 10s (`AGINXBROWSER_SCRIPT_DEADLINE_MS`). A 0% duty line is the 30s heartbeat, including idle windows. The completed run's log ([`aginx-p25b.log`](../bench/peer-crawl-20261021/aginx-p25b.log)) has neither warning. The deadline was not raised for the 88.425s run. The 500 was not root-caused and did not recur.

## Obscura and `chromedp.Nodes`

Official `chromedp.Nodes("a[href]", AtLeast(1))` did not finish against Obscura 0.2.5 on this machine. Pool 1 (limit 3) returned `get links: context deadline exceeded` at 30s with `urls=1` ([`ob-p1.log`](../bench/peer-crawl-20261021/ob-p1.log)). Pool 25 failed the same way; that log was overwritten by the eval run. The chromedp client also logged `executor for "page-1-session" doesn't exist`. `obscura fetch --dump html` of the index returned 1085 bytes of rendered HTML that does contain the hrefs, so the page runs. The 54.83s row set `CRAWL_LINKS=eval`: poll `querySelectorAll('a[href]')` every 100ms. A smoke of that path, pool 1 limit 4, crawled 4 URLs in 15.68s. The finished pool-25 log still prints the executor line and exits 0 with `urls=940` and zero `fetcher err` lines.

Lightpanda's two rows used `chromedp.Nodes`. Obscura's finished row did not.

## Reproduce

```bash
# Aginx. Both limits at 0 are required for a full crawl. Do not point
# STORE_PATH at a store you use for anything else.
AGINXBROWSER_BIND=127.0.0.1:18092 \
AGINXBROWSER_STORE_PATH=/tmp/peer-crawl/aginx-data/cache.db \
AGINXBROWSER_COOKIE_STORE_DIR=/tmp/peer-crawl/aginx-data/cookies \
AGINXBROWSER_RESTORE_SESSIONS=0 \
AGINXBROWSER_CACHE_TTL_SECS=0 \
AGINXBROWSER_DISABLE_SITE_PRELOAD=1 \
AGINXBROWSER_DOMAIN_RATE_PER_MIN=0 \
AGINXBROWSER_SESSION_PAGE_LIMIT=0 \
  ./target/release/aginxbrowser

python3 bench/peer-crawl/crawl_http.py --pool 25 \
  --out bench/peer-crawl-20261021/aginx-p25.json
```

Lightpanda: `lightpanda version` prints `1.0.1` (`--version` is an unknown command). Obscura: `obscura --version` prints `obscura 0.2.5`. Peer serves were stopped after the runs. `127.0.0.1:18092` was not left listening.

Next step for the same-machine gap: [peer-crawl-20261021-优化交接.md](peer-crawl-20261021-优化交接.md). On 2026-10-22 a fresh `v0.6.2` / `2426b37` release binary finished the same crawl twice, 940/940, at **65.827s** and **62.349s**, peak RSS about 406–408 MiB. Receipts: [bench/peer-crawl-20261022/](../bench/peer-crawl-20261022/). Neither 40.7s nor the stated ~44s reproduced on that binary. The default rate limit still returns HTTP 429 at 20 pages.
