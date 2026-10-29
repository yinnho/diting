<p align="center"><img src="web/brand/icon.svg" width="160" alt="Diting"></p>

# Diting

[English](README.md) | 中文

**为 agent 而生的浏览器引擎。** 一个 Rust 二进制，零 Chromium、零 Docker。看、读、搜、操作、记忆。

[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Release](https://img.shields.io/github/v/release/yinnho/diting)](https://github.com/yinnho/diting/releases/latest)
[![skills.sh](https://skills.sh/b/yinnho/diting)](https://skills.sh/yinnho/diting)
[![X](https://img.shields.io/badge/X-%40diting-black?logo=x)](https://x.com/diting)

> 人有 Chrome，Agent 有 Diting。

```bash
curl -fsSL https://browser.aginx.net/install.sh | sh
diting  # → HTTP 服务在 :8089，以下全部是普通 REST
```

<video src="https://github.com/yinnho/diting/releases/download/v0.7.0/diting-vs-competitors-2026-09.mp4" controls muted width="720"></video>

*历史片段，素材来自 [v0.7.0](https://github.com/yinnho/diting/releases/tag/v0.7.0) 发布资产。与同类产品的实测对比（2026-09）：出文本快 7.6 倍、内存省约 10 倍。*

## 为什么 agent 需要自己的浏览器

现有的"浏览器自动化"是给人调试或一次性抓取用的。agent 需要的是五件事一体——**看、读、搜、操作、记忆**——而且要便宜：

- 出可用文本比 headless Chrome **快 7.6 倍**（同 20 页同网络，p50 532 ms vs 4 053 ms）
- **冷启动 ~25 MB、日常静态使用全程 <40 MB**（整进程实测）；重 JS 页也只要 ~150 MB，而 Chrome 是**每页** ~2 GB
- Chrome 的 dump-DOM 有 5/40 次连 DOM 都出不来，我们 **0 次硬失败**

Agent 的总成本 = 浏览器效率 × 模型效率。这里是浏览器那一半。

## 它能做什么

- **完整浏览器，不依赖 Chromium**——V8 + 自有 Rust HTTP/CSS/布局/绘制栈装进一个二进制。截图与渲染内置，无需下载任何东西、无需沙箱
- **真实 TLS 指纹**——生产级伪装，通过边缘检查；无指纹引擎吃 403 的地方我们过得去。CAPTCHA 与挑战页被检测、等待并如实上报，不会把挑战页当内容交出去
- **有状态会话**——登录态可注入、可导出、跨重启存活。同一个会话 id 回来时还是登录的
- **分层渲染**——静态页 ~100 ms 直出；只有页面真需要 JS 时才起 V8。每个响应都标明是哪一层服务的
- **聚合搜索**——7 类 20 引擎（含中文互联网），并发查询、合并去重；命中带章节级摘要
- **Flow**——agent 摸出来的多步操作沉淀为可重放的 JSON 文档：重跑零模型 token
- **文档生成**——markdown → 确定性 HTML + 图表（同输入同字节）；页面集导出 PDF / PPTX / DOCX
- **回执文化**——每次 fetch 报告服务层级、重定向链、内容哈希。agent 依据"实际发生了什么"行动
- **本地记忆**——fetch/搜索缓存（SQLite FTS5），中文子串匹配，读过的不重付网络费

## 快速开始

```bash
# Homebrew（macOS 7+ 需先信任）
brew trust yinnho/diting && brew install yinnho/diting/diting

# 或源码构建（全特性）
cargo build --release --features stealth,screenshot

diting &  # 监听 :8089
curl :8089/health          # → {"status":"ok","engine":"diting"}
curl -X POST :8089/fetch -d '{"url":"https://example.com"}' | jq -r .markdown
curl -X POST :8089/search -d '{"query":"rust 浏览器引擎"}' | jq '.results[:3]'
curl -X POST :8089/session/create -d '{"url":"https://example.com"}'  # → session_id
```

## 它的位置

Computer-use agent 有两层：GUI 层驱动整个桌面；**引擎层**让页面本身成为机器。Diting 是引擎层——DOM 动作进、结构化状态出，每会话一个进程而不是一台虚拟机。它是实时检索仪器，不是爬虫：没有跟随链接的递归、内置每域名预算、robots.txt 默认不拦（运营者可开）。

Apache-2.0 自托管，或直接用 [browser.aginx.net](https://browser.aginx.net) 托管实例。

## 文档

- [API 参考](docs/API.zh-CN.md) · [English](docs/API.md) · [LLM agent 安装指引](llms-install.md)
- 兼容 Firecrawl 客户端（`/v1/scrape`），任何 HTTP 客户端即接即用——无需 SDK

## Star History

[![Star History Chart](https://api.star-history.com/svg?repos=yinnho/diting&type=Date)](https://star-history.com/#yinnho/diting&Date)

## 致谢

diting 的 JavaScript 运行时层建立在 [Obscura](https://github.com/h4ckf0r0day/obscura)（Apache-2.0）的工作之上。
许多兼容性修复跟随 Obscura 的改动，并在源码注释中以 `upstream <commit>` 标注来源。详见 [`NOTICE`](NOTICE)。

## 许可

Apache-2.0，详见 [`LICENSE`](LICENSE) 与 [`NOTICE`](NOTICE)。
