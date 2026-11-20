# 移交：LexBench Web Fetch 动态列和静态列

日期：2026-11-20。`docs/` 被 gitignore，这份是单独强制加入的。
这一轮只做 LexBench-Web-Fetch 的动态列和静态列。热门列、WebMainBench、WPT、搜索、反爬都没动。

下面的数字都是本机逐页抓取，不是官方榜。官方动态列不要拿这里的 95/100 去报。

## 现在停在哪

动态列 100 条里，95 条锚点对齐。5 条单独记账，不再改引擎：

| 页面 | 原因 |
| --- | --- |
| Falabella 音响分类 | 读到的是安全拦截页，归反爬，不在动态列里绕过 |
| RSuite 表格 | 源站 402，部署已停。站点恢复后再抓一次 |
| Redoc `redocly.github.io/redoc/` | Cafe 规格已画出，操作标题在 `+ Show` 后面，DOM 里没有 `Create order` 这类句子。抓取不去逐个点开 |
| USGS 地震图 | 列表和 Sort 控件在。`Newest First`、`Only List Earthquakes Shown on Map` 不在默认视图里 |
| Vant 英文 README | 产品默认 `zh-CN`。不把全局语言改成英文来过这一条 |

静态列用当前二进制重测了全部 100 条：87 条通过，13 条不通过。13 条都不是空壳。3 条是禁用词打在正常正文上（WCAG 里的 `CAPTCHA`、RFC 9110 里的 `404 Not Found`、Browser Use README 里的 captcha solving）。另外 10 条是页面改写了原句，附近内容还在，包括 Anthropologie 退货、Scrapy、Open WebUI、React Router、PostgreSQL 教程顶上的版本号、Tavily 未授权 JSON、Fetch 规范的 `In only one current engine.`、飞书帮助里的一句统计、NASA Science 三段旧介绍、OpenAI API 平台四句介绍。PostgreSQL 源站 HTML 里已经没有 `PostgreSQL 18.6, 17.11, ...` 这行。

## 这一段已推到 `diting/main` 的提交

远程是 `diting`（`https://github.com/yinnho/diting.git`）。`origin` 是退役仓库，不要推那里。

| 提交 | 说明 |
| --- | --- |
| `c6dd2720` | 大文档 Mock 级联只算一次。PrimeNG 菜单靠这个 |
| `726d5fdf` | 样式表按 `<base href>` 解析。提交时的样式表、级联查找、动态 link、CSSOM href 都走文档 base |
| `997c8597` | `/fetch` 导航时限 60 秒。交互会话和 CDP 仍是 30 秒。显式 `DITING_NAV_TIMEOUT_MS` 优先 |
| `ebfa5011` | 动态 `import()` 还在飞时结算继续泵。空的 `router-outlet` 不算正文。普通脚本和样式表仍按 3 秒停 |
| `12bba7dc` | `<script type=module>` 先并行下载，再按文档顺序执行。执行阶段重新计算 10 秒 |
| `88711c8b` | 同步 XHR 看 `stale_termination` 退回。不到 10KB 的经典脚本也套 5 秒。任天堂 Switch 2 页脚靠这个 |
| `608cb0eb` | GET/HEAD 正文读超时后再请求一次。Kabum 电脑区和外设区靠网络好的那次过了，重试是给中途停住的连接留的 |

再往前、这一列也用得上的：`e0cc6ed7` 中文导航壳不算正文、`67130b2e` TrustedHTML 策略回调、`651e6303` 收起的侧栏和页脚、`e488d956` 文档站正文在 article 里。

## 本地怎么再测

数据集和评分脚本在 `/tmp/webfetch/`，不在仓库里。

- 动态任务：`/tmp/webfetch/dynamic-sites.jsonl`
- 静态任务：`/tmp/webfetch/general-web.jsonl`
- 旧结果：`dynamic-results.json`、`static-results.json` 都过时了，不要引用里面的通过数
- 这一轮静态后 80 条的记录：`/tmp/webfetch/static-rest.json`

评分请求和脚本里的一致：`POST /fetch`，`format=markdown`，`max_chars=0`，`sanitize=false`，`render_tier=browser`，`auto_bypass_challenge=true`，`wait_secs=1`。通过条件是每条 `required_facts` / `stable_text_anchors` 的任一替代句能在正文里 casefold 命中，没有 `forbidden_any` 的精确子串，长度不低于下限，请求本身没有 HTTP 错误。

不要用 `/tmp/webfetch/run_dynamic.py` 或 `run_static.py` 去打已经开着的探针。它们会自己再拉起一个 diting，结束时把它杀掉。进程内的抓取缓存按 URL 和 `wait_secs` 等字段做键，不含 `js_extract`。同一进程里要再抓同一页，改 `wait_secs` 才能绕开缓存。

评测二进制是 `./target/debug/diting`，必须带 `--features screenshot`。`target/release/diting` 是旧的，不能用来跑这列。编译时设置：

```sh
export RUSTY_V8_ARCHIVE=$HOME/.cargo/rusty-v8-152/librusty_v8_release_aarch64-apple-darwin.a
```

代理：`DITING_PROXY=http://127.0.0.1:8118`（本机 privoxy）。GitHub 和部分外站不走代理会超时。

探针习惯绑在 `127.0.0.1:8098`。产品服务在 `127.0.0.1:8148`，不要停它，也不要停 privoxy。`/health` 里的 git 哈希是编译时写进去的，二进制若在提交前编出来，哈希会停在上一个提交，源码仍可能已经包含这次改动。

每次提交前跑门禁（`Agents.md`）：

```sh
./scripts/gate_fmt.sh
python3 scripts/audit_layers.py
tools/webplatform/check_fresh.sh
cargo test --workspace --features screenshot
```

提交说明用中文，带 `Co-Authored-By: Claude Code <noreply@anthropic.com>`。只暂存相关文件。`docs/` 在 `.gitignore` 里，普通提交带不走。不要提交 `target/`、`/tmp`，也不要提交仓库根上那些 `_github*`、`_hook*`、`_nurture*`、`bench/results/`。

## 不要再做的事

- 不要把 `navigator.language` 改成 `en-US` 去过 Vant。
- 不要为了 Redoc、USGS 去自动点开每一个 `+ Show` 或设置面板。
- 不要把 Falabella 的拦截页当成动态列的渲染缺陷来修。
- 不要把 Quasar 虚拟滚动的 10 秒模块图预算再抬高。那一页在现在的二进制上已经 33/33 通过，靠的是 60 秒读取时限、`import()` 还在飞时继续泵，以及模块并行下载。
- 不要改字体表 145、chrome145 TLS、DOMParser 门面、WebGL null、window 原型、htmlColl。这些是故意钉住的。
- `cssom_value.rs` 卡在 1500 行，不要再加。`domcore.js` 的祖父上限是 4836，只能缩不能涨。
- 格式改动单独提交。神文件棘轮失败时把代码拆出去，不要把记录的行数调高。

## 接手时如果还要看引擎

动态列和静态列都没有剩下的空壳页等着修。下一件若还做评测，先决定是不是要动热门列。动之前先把上面 5 条动态和 13 条静态的记账看完，避免把站点改版或禁用词误判成引擎回归。
