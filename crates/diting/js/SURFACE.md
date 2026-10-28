# JS Web-API 表面台账（Web-API Surface Ledger）

> 目的：让"引擎有什么/缺什么"从偶然发现变成可查清单。每加一族 API
> 或修一个兼容缺口，在此登记一行；竞品修复（Obscura/Moli/Blitz 的
> commit）先对照此表再决定跟进与否——2026-10-28 起的纪律。
>
> 状态：native（引擎原生）/ shim（JS 层实现）/ partial（有坑，注明）/
> missing。测试锚点写测试函数名。

## 表面快照门

`runtime/surface_snapshot.txt` 冻结 `getOwnPropertyNames(globalThis)` + 每个构造器的静态/原型成员；
测试 `js_surface_snapshot_matches_fixture` 逐行比对，漂移即红（NEW=泄漏嫌疑，GONE=兼容破坏）。
有意改表面：`UPDATE_SURFACE_SNAPSHOT=1 cargo test -p diting js_surface_snapshot` 重生夹具并在下表登一行。
注：`__` 前缀内部名已被 `__diting_hide_list` 从页面可见面隐去，不进快照。

## WebIDL 差集（全量表）

`SURFACE-WEBIDL.md`（生成件，脚本 `scripts/surface_webidl_diff.py`）：有/缺/多三表 + 成员级缺口（Chrome 已发布判据）。
2026-10-29 首版：标准 Window 接口 428，有 357，缺 71（全部是 Chrome 未发布/实验接口，已发布者零缺）；
成员级已核 196 接口、163 个有缺口、合计 2631 成员——补齐路线图的输入。已确认的真缺口样例：
`DOMRectReadOnly`/`DOMMatrixReadOnly` 仅空壳 constructor、`Element.clientLeft/clientTop` 缺、`ClipboardChangeEvent` 等未继承 Event 成员。

## 文件清单

| 文件 | 职责 |
|---|---|
| bootstrap.js | 根装配 + 大部分 API 族（指针捕获、PointerEvent/Touch、hover 链、elementFromPoint、cookie、credentials…） |
| indexeddb.js | IndexedDB 全家（open/upgrade 语义、cursor、proto 转发、arity） |
| worker.js | Worker（含 blob URL Worker） |
| client_rects.js / computed_size.js | 几何查询 |
| css_*.js | CSSOM（decl/rules/register/adopted sheets） |
| audio.js / canvas_face.js / crypto_subtle.js / image_info.js | 媒体与加密面 |

## API 族状态（随修随记）

| 族 | 状态 | 测试锚点 | 备注 |
|---|---|---|---|
| IndexedDB cursor 守卫/arity | shim | idb_cursor_guard_and_arity_follow_chrome | 2026-10-28 与 Moli 同日修复（双重 continue 曾静默吞迭代） |
| blob URL | native | （session E2E） | 真 UUID + 同步 XHR 可读；fake 随机串是死代码 |
| summary→details 激活 | native | （session E2E） | Obscura 10-27 修的坑我们已有 |
| document 只读赋值 | native | （session E2E） | 同上 |
| 指针捕获 setPointerCapture | shim | drag_follows_pointer_capture_slider | 2026-10-28 修（滑块拖不动根因） |

## 竞品缺口日志

| 日期 | 来源 | 项 | 我们的结论 |
|---|---|---|---|
| 2026-10-28 | Moli | IndexedDB cursor/参数个数/upgrade 等待 | 已修 + 回归测试 |
| 2026-10-27 | Obscura | blob URL 同步 store、summary 激活、document 只读 | 探针验证已覆盖 |
| 2026-10-27 | Obscura | CDP 响应体懒解码（性能） | 不适用——我们无 CDP 面 |
| 2026-10-27 | Blitz | 无 computed style 元素的命中测试 | 待对照（hit-test 冷路径） |
