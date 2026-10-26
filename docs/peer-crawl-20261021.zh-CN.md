# Amiibo 爬取，2026-10-21

[English](peer-crawl-20261021.md) | [中文](peer-crawl-20261021.zh-CN.md)

Pierre Tachoire 2026-10-19 文章 [Benchmarking AI-native headless browsers built from scratch](https://lightpanda.io/blog/posts/benchmarking-ai-native-headless-browsers) 里那次爬取，在这台机器上重跑了一遍。原始回执：[`bench/peer-crawl-20261021/`](../bench/peer-crawl-20261021/)。

本页不替换 [2026-10-19 记分板](scoreboard.zh-CN.md)。那一页的 Lightpanda / Obscura 格子仍然是 **未跑**。这次没有跑 Web Platform Tests，也没有跑 Moli 和 Kitesurf。

## 文章里的数字

文章比较的是 Lightpanda 1.0.1、Obscura 0.2.5、Moli 1.1.18 和 Kitesurf。AginxBrowser 不在其中。性能部分是爬 [Amiibo 演示站](https://demo-browser.lightpanda.io/amiibo/)：顺着同站链接走完 940 页，并发池 25。方法页是 [crawler benchmark](https://github.com/lightpanda-io/demo/blob/main/BENCHMARKS.md)。那一页写的机器是 AWS m5.xlarge，内存是 Linux `smem` 的 PSS。同一页里较旧的表是 933 个 URL；2026-10-21 的线上站点，以及这篇文章，是 940。

他们公布的 25 路成绩，是他们机器上的声称：

| 引擎 | 墙钟 | CPU | 峰值内存 |
|------|------|-----|----------|
| Lightpanda 1.0.1 | 4.5s | 12s | 109MB |
| Obscura 0.2.5 | 11.5s | 29s | 201MB |
| Moli 1.1.18 | 17.4s | 40s | 230MB |

这些格子是他们的数字。下面的表里没有这一列。

## 我们量到的

macOS 27.0.1，arm64，2026-10-21 Asia/Shanghai。引擎一次只跑一个。种子 `https://demo-browser.lightpanda.io/amiibo/`。索引页是一小段 HTML 再加 `load.js`；只读原始 HTML 的爬虫看不到链接。每一页等到至少出现一个 `a[href]`（上限 30 秒），只留同主机的 http(s) 链接，按不带 fragment 的绝对 URL 去重。第一页出错就停掉整个池，和官方爬虫一样。

内存是 `ps` 的 RSS，大约每 0.1 秒采一次，把匹配到的进程加总。macOS 没有 `smem`。RSS 不是 PSS；25 个进程相加时，共享页会被算不止一次。Lightpanda 和 Obscura 启动时清掉了 `http_proxy`、`https_proxy`、`HTTP_PROXY`、`HTTPS_PROXY`、`ALL_PROXY`、`all_proxy`。Aginx 忽略 `http_proxy`，除非设置 `AGINXBROWSER_PROXY`；这次没有设。用户自己的 `127.0.0.1:8089` 一直开着，采样按 pid 排除了它。

被测二进制是 `target/release/aginxbrowser`，版本 **0.6.0**，`/health` 提交 **`3dd366b`**，TLS 预设 `chrome145`。写这份回执时的 git HEAD 是 `153c2af`（`release: v0.6.1`）。这次 bench 没有用那个提交。

Aginx 没有 CDP 监听（[`cdp-serve-design.md`](cdp-serve-design.md)），Playwright 和官方 chromedp 爬虫驱动不了它。Aginx 的客户端是 [`bench/peer-crawl/crawl_http.py`](../bench/peer-crawl/crawl_http.py)：`POST /session/create`、`/navigate`、`/eval`。Lightpanda 和 Obscura 用的是官方爬虫的本机 darwin/arm64 构建（chromedp v0.15.1，和他们 demo 的 pin 一致）。本地改了两处，都在取页逻辑之外：fork 模式会等到端口能连上（最多 15 秒；上游只睡 100 毫秒，这台机器上 25 个进程那时还没在听）；`CRAWL_LINKS=eval` 改为轮询 `Runtime.evaluate`，不再走 `chromedp.Nodes`。Lightpanda 的两次都没有设置 `CRAWL_LINKS`。

| 引擎 | 形态 | 怎么取链接 | 墙钟 | 峰值 RSS | 采样 CPU |
|------|------|------------|------|----------|----------|
| Aginx 0.6.0 `3dd366b` | 1 个进程，25 个 HTTP 会话，限速和页数上限都关掉 | HTTP `/eval` | **88.425s** | 414912 KB（405 MiB） | 17.03s |
| Lightpanda 1.0.1 | 25 个进程，`--fork` | `chromedp.Nodes` | **59.01s**，再跑 **67.76s** | 861184 KB，再跑 861712 KB（841 MiB） | 第二次 11.62s |
| Obscura 0.2.5 | 1 个进程，25 个 CDP target | `Runtime.evaluate` | **54.83s** | 203424 KB（199 MiB） | 11.35s |

Lightpanda 第一次的 CPU 采样写成了 `0`，因为发信号时 25 个进程已经退出。这个 0 不是测量值。第二次记的是进程还活着时见过的最大 cputime 合计，所以 11.62 秒是下界。爬虫自己的 `user`/`sys`（Lightpanda 大约 11 秒 + 12 秒，Obscura 0.81 秒 + 0.89 秒）是 Go 客户端，不是浏览器。

在这台 Mac 上，跑完的墙钟顺序是 Obscura 54.83 秒、Lightpanda 59.01 秒和 67.76 秒、Aginx 88.425 秒。峰值 RSS 从小到大是 Obscura、Aginx、Lightpanda。Obscura 的完成数字和 Lightpanda 用的不是同一种取链接方式。文章里的 1 路和 5 路这次没跑。同一晚早些时候，Aginx 6 页的冒烟花了 14.6 秒，所以 940 页的 1 路大约要半小时，留着没跑。

## Aginx 的默认服务器

`AGINXBROWSER_DOMAIN_RATE_PER_MIN` 默认每个可注册域名每分钟 20 页。`AGINXBROWSER_SESSION_PAGE_LIMIT` 默认每个会话 200 页。两个都留在默认值时，25 路在 **20 页**（已知 193 个 URL）、**13.069 秒**处停住，HTTP 429。峰值 RSS 369808 KB，采样 CPU 0.82 秒。88.425 秒那一行是同一只二进制，两个旋钮都设成 `0`。429 的正文就是让操作员这么设。bench 服务器还用了 `/tmp/peer-crawl/aginx-data` 里的独立存储、`AGINXBROWSER_RESTORE_SESSIONS=0`、`AGINXBROWSER_CACHE_TTL_SECS=0`、`AGINXBROWSER_DISABLE_SITE_PRELOAD=1`，以及 `AGINXBROWSER_BIND=127.0.0.1:18092`。

## Aginx 那次停住的尝试

第一次关掉限速的运行，在 **73 页**（已知 548）、**77.877 秒**处停住，`https://demo-browser.lightpanda.io/amiibo/04d81d02.html` 返回 HTTP 500。响应体没有留下来。服务器还活着。后来在新会话上重放这个 URL，返回 HTTP 200，稍等之后有 11 个链接。那次尝试的服务器日志（`/tmp/peer-crawl/aginx.log`，没有放进仓库）里有 `execute_scripts: fetch deadline reached after 0/1 script bodies arrived` 和 `loadAmiibo is not defined`，还有 `#66 busy window: 0% duty`。脚本拉取期限默认 10 秒（`AGINXBROWSER_SCRIPT_DEADLINE_MS`）。0% duty 是每 30 秒打一次的心跳，空闲窗口也打。跑完那次的日志（[`aginx-p25b.log`](../bench/peer-crawl-20261021/aginx-p25b.log)）里这两类都没有。88.425 秒那次没有调高期限。500 没有追到根因，后来也没有再出现。

## Obscura 和 `chromedp.Nodes`

官方的 `chromedp.Nodes("a[href]", AtLeast(1))` 在这台机器上对 Obscura 0.2.5 没有跑完。1 路（limit 3）在 30 秒时报 `get links: context deadline exceeded`，`urls=1`（[`ob-p1.log`](../bench/peer-crawl-20261021/ob-p1.log)）。25 路同样失败；那份日志被后来的 eval 运行盖掉了。chromedp 还打了 `executor for "page-1-session" doesn't exist`。`obscura fetch --dump html` 拉索引页得到 1085 字节、里面有 href 的渲染后 HTML，所以页面本身能跑。54.83 秒那一行设置了 `CRAWL_LINKS=eval`：每 100 毫秒轮询 `querySelectorAll('a[href]')`。这条路径的冒烟，1 路 limit 4，4 个 URL，15.68 秒。跑完的 25 路日志末尾仍有那行 executor，退出码 0，`urls=940`，`fetcher err` 为零。

Lightpanda 的两行用的是 `chromedp.Nodes`。Obscura 跑完的那一行不是。

## 复现

```bash
# Aginx。要爬完全集，两个上限都必须是 0。不要把 STORE_PATH 指到正在用的库。
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

Lightpanda：`lightpanda version` 打印 `1.0.1`（`--version` 是未知命令）。Obscura：`obscura --version` 打印 `obscura 0.2.5`。跑完后对端进程已停。`127.0.0.1:18092` 没有留着监听。

同机差距的下一刀写在 [peer-crawl-20261021-优化交接.md](peer-crawl-20261021-优化交接.md)。2026-10-22 用 `v0.6.2` / `2426b37` 冷启动重跑了两遍，940/940，墙钟 **65.827 秒**和 **62.349 秒**，峰值 RSS 大约 406–408 MiB。回执在 [bench/peer-crawl-20261022/](../bench/peer-crawl-20261022/)。40.7 秒和大约 44 秒都没有在这只发布构建上复现。默认限速仍在 20 页处 HTTP 429。
