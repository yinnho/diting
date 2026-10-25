# OpenCarrier web_fetch 接入 Diting（外挂定位）

> 本文是 OpenCarrier `web_fetch` 接入 Diting 的实际落地方案。Diting 侧无需改动。

## 定位

Diting 是**纯外挂基础设施**——像真实浏览器一样挂在系统里，`web_fetch` 在内部按需调用它，对**所有 agent 完全透明**。agent 还是调 `web_fetch(url)`，参数、返回、语义都不变；`web_fetch` 自己决定走 reqwest 直连还是走 Diting。

**核心原则**：

- **零配置暴露**：不给 `WebFetchConfig` 加字段、不进 `config.toml`、不给 `web_fetch` 加 `use_browser` 参数
- **唯一开关**：环境变量 `DITING_URL`。不设 = 不启用 = 纯 reqwest（行为等同改造前，完全可回退）
- **路由是私有实现**：哪些 URL 走浏览器，由 `web_fetch` 内部的域名常量决定，不暴露成配置
- **失败降级**：Diting 调用失败/空内容时回退 reqwest，不报错

## Diting 是什么

独立 HTTP 服务（systemd 守护 `diting.service`），默认 `127.0.0.1:8089`（仅本机可达，公网被防火墙拦）。提供：

- `POST /fetch` — 抓页面，支持 `format`(markdown/html/text)、`selector`、`wait_secs`(等 JS 渲染)、`use_proxy`。内置 stealth（Chrome145 TLS 指纹）、SSRF 防护、进程内缓存
- `POST /eval` — 执行 JS
- `POST /click` — JS 点击

源码与文档：https://github.com/yinnho/diting

## 实现（`crates/runtime/src/web_fetch.rs`）

### 开关 + 路由判断（私有，不进配置）

```rust
/// 外挂 Diting 需要兜底抓取的站点（JS 渲染或风控）。
const DITING_HOSTS: &[&str] = &[
    "mp.weixin.qq.com",   // 微信公众号文章（风控）
    "zhuanlan.zhihu.com", // 知乎专栏（JS 渲染）
    "search.jd.com",      // 京东搜索（动态）
    "github.com",         // GitHub（动态 + 需代理）
];

/// 读 DITING_URL。未设/空 → None（不启用，纯 reqwest）。
/// 在 fetch 调用时读取（非构造时），环境变量可动态生效，无需进 WebFetchConfig。
fn diting_url() -> Option<String> {
    std::env::var("DITING_URL").ok().filter(|s| !s.is_empty())
}

fn should_use_diting(url: &str) -> bool {
    let host = ssrf::extract_host(url); // "host:port"
    DITING_HOSTS.iter().any(|h| host.contains(h))
}
```

### pipeline 插入点（cache-miss 后、reqwest 前）

```rust
// Step 2: Cache lookup (only for GET) ...
// ↓ 插入：
if method_upper == "GET" && should_use_diting(url) && diting_url().is_some() {
    if let Ok(content) = self.fetch_via_diting(url).await {
        // 复用原有 truncate + wrap_external + cache pipeline
        let result = format!("HTTP 200 (via Diting)\n\n{}",
            wrap_external_content(url, &truncated));
        self.cache.put(cache_key.clone(), result.clone());
        return Ok(result);
    }
    // 失败 → 不 return，继续走下方 reqwest（降级）
}
// Step 3: Build reqwest request ...
```

### 调 Diting（请求格式对齐 `browser.rs` 的 `do_fetch_request`）

```rust
async fn fetch_via_diting(&self, url: &str) -> Result<String, String> {
    let base = diting_url().unwrap();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(self.config.timeout_secs.max(30)))
        .build()?;
    let body = serde_json::json!({ "url": url, "format": "markdown", "wait_secs": 4 });
    let resp: serde_json::Value = client
        .post(format!("{}/fetch", base.trim_end_matches('/')))
        .json(&body).send().await?.json().await?;
    resp.get("content").and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| "Diting response missing/empty content".into())
}
```

### SSRF 说明

`web_fetch.rs` 第一步的 `ssrf::check_ssrf(url)` 校验的是**目标 URL**（如微信公网域名），合法通过。Diting 跑在 `127.0.0.1:8089`，`web_fetch` 对它发的 plain reqwest POST 是固定可信服务地址，不经 SSRF check（与连任何外部 API 同理）。Diting 服务端自身也有 SSRF 防护，双层保险。

## 行为矩阵

| `DITING_URL` | URL 域名 | 走哪 | 行为 |
|-------------------|----------|------|------|
| 设了 | mp.weixin.qq.com 等 | Diting | ✅ 拿到正文 |
| 设了 | api.xxx.com 等 | reqwest | 不变（快） |
| 没设 | 任意 | reqwest | 完全不变（可回退） |
| 设了但服务挂了 | 风控站 | 回退 reqwest | 降级，不瘫 |

## 与 browser_* 工具的关系

两者**并存、互不影响**：

- `web_fetch` 内嵌 Diting —— 增强抓取，对 agent 透明（agent 不知道浏览器存在）
- `browser_*` 工具（`crates/runtime/src/tools/browser.rs`）—— 浏览器自动化（click/eval/scroll/交互），agent 显式调用

env var 差异：`browser.rs` 未设 `DITING_URL` 时用默认 `127.0.0.1:8089`（默认启用）；`web_fetch` 未设时为 None（默认禁用，保回归）。各自语义合理。

## 搜索能力下沉：删除 SearXNG MCP，改用 Diting /search

### 背景

OpenCarrier 原本通过 `mcp_searxng_web_search` 工具（SearXNG MCP）提供搜索。这把
"搜索基础设施"塞进了 Agent 运行时，违背"OpenCarrier 回归 Agent 本身"的原则。

决策：**搜索下沉到 Diting**。OpenCarrier 删除 SearXNG MCP，Agent 需要搜索时
调 Diting 的 `/search`。SearXNG 从 OpenCarrier 的依赖变成 Diting 的依赖
（物理上仍是同机同一个进程，只是归属转移）。

### OpenCarrier 侧改动

1. **删除** `config.toml` 里的 searxng MCP 条目：
   ```toml
   # 删除这一段
   [[mcp_servers]]
   name = "searxng"
   transport = { type = "stdio", command = "/Users/.../searxng-mcp" }
   ```
2. **删除** `mcp_searxng_web_search` 工具的注册/分发代码。
3. **新增** 一个调 Diting `/search` 的内置工具（或复用 web_fetch 的 Diting
   路由），例如 `web_search`：
   - `web_search(q, fetch_top=0)` → POST `http://<DITING_URL>/search`
   - 复用 `DITING_URL` 环境变量（和 web_fetch 同一个开关）

### 关键设计：不重写 SearXNG

Diting 的 `/search` **调用**本机 SearXNG（`SEARXNG_URL`，默认
`http://127.0.0.1:8888`）做聚合，自己只做"抓正文"。SearXNG 的几百个引擎解析器
（百度/Google 改版维护）是社区十年积累，重写是自杀——白嫖它。

`/search` 的两种模式：
- `fetch_top=0`：纯搜索（等价原 SearXNG MCP），毫秒级
- `fetch_top>0`：搜完对前 N 条抓正文（复用 Diting 的 stealth+JS），Agent 一步拿到"结果+正文"

### 部署

SearXNG 仍同机部署（`127.0.0.1:8888`），只是它的"上层调用方"从 OpenCarrier 换成
Diting。SearXNG 不可用时，`/search` 返回 503，不影响 Diting 的其他端点
（`/fetch` `/eval` `/click` 都不依赖 SearXNG）。

详见 [`docs/search-design.md`](search-design.md)。


## 部署

1. Diting 与 OpenCarrier 同机部署（`127.0.0.1:8089`）
2. OpenCarrier 启动环境设 `export DITING_URL=http://127.0.0.1:8089`
3. Diting 已 systemd 守护，开机自启 + 崩溃重启

## 验证

- 不设 `DITING_URL`：所有 `web_fetch` 行为与改造前一致（回归）
- 设了：抓微信文章 → 走 Diting 拿到正文；抓 JSON API → 走 reqwest，速度正常
- 停掉 Diting：风控站请求回退 reqwest，不报错
- 同一微信 URL 第二次抓 → 走 cache

## 不采用的替代方案

- **全量替换**（所有 web_fetch 走 Diting）：API/JSON 抓取变慢，服务挂了全瘫
- **config.toml 配置域名白名单**：把外挂实现细节暴露成配置，违反"外挂定位"
- **包装成 MCP**：web_fetch 已是内置工具，绕 MCP 徒增协议层
