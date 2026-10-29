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
| cookie_gate.js | document.cookie 访问门 __cookieGate（遗留层；随 Document.cookie 族迁走）；cookie 读写本体仍在 bootstrap 的 document 字面量里 |
| platform_bundle.js | **生成件**：platform/core.js（模块注册表）+ 全部平台模块，按 requires-load 拓扑序（tools/webplatform/bundle.py） |
| platform/gen/storage.gen.js | **生成件**：Storage、StorageEvent 的 IDL 骨架（tools/webplatform/gen/gen_iface.py，规范 webref 6ac6259，成员顺序取 Chrome 真值 overlay） |
| platform/storage.impl.js | 手写：Storage 的 backing 行为（惰性加载、防抖写盘、opaque origin 仅内存） |
| worker.js | Worker（含 blob URL Worker） |
| client_rects.js / computed_size.js | 几何查询 |
| pointer_touch.js | PointerEvent、指针捕获注册表（interact.rs 的会话/CDP 鼠标链经 __diting_pointerCaptureFor 改道）、Touch、TouchEvent（2026-10-29 棘轮搬出，紧随 cssom_sheets 加载） |
| css_*.js | CSSOM（decl/rules/register/adopted sheets） |
| cssom_sheets.js | StyleSheet/CSSStyleSheet、document.styleSheets 背后（外链体缓存/元素 sheet 恒等/StyleSheetList）、Document.adoptedStyleSheets（2026-10-29 棘轮搬出，紧随 bootstrap 加载） |
| audio.js / canvas_face.js / crypto_subtle.js / image_info.js | 媒体与加密面 |

## API 族状态（随修随记）

| 族 | 状态 | 测试锚点 | 备注 |
|---|---|---|---|
| IndexedDB cursor 守卫/arity | shim | idb_cursor_guard_and_arity_follow_chrome | 2026-10-28 与 Moli 同日修复（双重 continue 曾静默吞迭代） |
| blob URL | native | （session E2E） | 真 UUID + 同步 XHR 可读；fake 随机串是死代码 |
| summary→details 激活 | native | （session E2E） | Obscura 10-27 修的坑我们已有 |
| document 只读赋值 | native | （session E2E） | 同上 |
| elementFromPoint 冷路径/double 参数 | shim | hit_test_cold_path_and_webidl_double_args, mouse_chain_before_first_layout_does_not_fault | 2026-10-29 对照 blitz#1304 |
| Storage / StorageEvent | **IDL 生成** | storage_matches_chrome_truth（53 探针对 Chrome 156 真值）, platform_generated_descriptors_conform, platform_errors_match_chrome_truth（17 个报错形状对 Chrome）, storage_family_proxy_semantics_and_gate | 2026-10-29 M2 竖切：旧 storage.js 删除；行为对齐 Chrome（名字在原型链上则仍为普通属性；StorageEvent 属性改原型访问器）；已知差异：枚举序（实现定义）、Event 基类实例自有属性（Event 族缺口） |
| CSSOM sheet 对象 | shim | cssom_sheets_identity_list_and_adoption | 2026-10-29 自 bootstrap 搬出（纯搬移）；helper 改词法 const |
| 指针捕获 setPointerCapture | shim | drag_follows_pointer_capture_slider, pointer_touch_family_capture_and_hidden_registry | 2026-10-28 修（滑块拖不动根因） |

## Event 族缺口（M2 探针暴露）

遗留 `Event` 基类把 type/bubbles/cancelable/… 当实例自有属性（`Object.keys(new Event('x'))` 为 9 项），Chrome 只有 `isTrusted`。
子类（含已生成的 StorageEvent）继承该差异；待 Event 族迁 IDL 时一并纠正（chrome-storage-truth.json 的 ev.ownkeys 即回归锚）。

## 竞品缺口日志

| 日期 | 来源 | 项 | 我们的结论 |
|---|---|---|---|
| 2026-10-28 | Moli | IndexedDB cursor/参数个数/upgrade 等待 | 已修 + 回归测试 |
| 2026-10-27 | Obscura | blob URL 同步 store、summary 激活、document 只读 | 探针验证已覆盖 |
| 2026-10-27 | Obscura | CDP 响应体懒解码（性能） | 不适用——我们无 CDP 面 |
| 2026-10-27 | Blitz | 无 computed style 元素的命中测试（blitz#1304） | 已对照：不同病——命中测试是 JS 遍历矩形、布局按需解出，首次布局前即可命中，display:none/template/head/脱离节点不入栈；回归 mouse_chain_before_first_layout_does_not_fault。顺带修：elementFromPoint/elementsFromPoint 对非有限坐标与缺参抛 TypeError（Chrome 行为），内部 __diting_hitTarget 对非有限坐标仍返回 null，测试 hit_test_cold_path_and_webidl_double_args（2026-10-29） |

## 二进制保真（PDD invalid wire type，2026-10-29）

PDD 商家后台 sharedWorker 用 `binaryType='arraybuffer'` 收 WebSocket 帧，再 `new DataView(buf,0,n)` / `new Uint8Array(buf,16)` 拆 16 字节头后交 protobuf 解码。
我们 `event.data` 恒为 `Uint8Array`（Chrome：arraybuffer→ArrayBuffer，blob/默认→Blob），偏移参数被静默忽略 → 负载错位 → "invalid wire type"。
同批修：`Request` 的 text/arrayBuffer/blob 对二进制 body 走字节（原先 String() 破坏）、补 `Request/Response.bytes()`、`ws.send(Blob)`、
`TextDecoder` UTF-8 非法序列按 WHATWG 出 U+FFFD。回归锚：`tools/webplatform/chrome/binary-probe.js` + `binary-truth-145.json`（Chrome 145 真值，69 项，现 66 同）；
Rust 测试 `test_request_and_response_keep_binary_bodies_intact` / `test_text_decoder_malformed_utf8_gets_replacement_chars`。
未修（与 PDD 无关）：transfer 列表不 detach、`CompressionStream` 构造。**真站验证待运营机复测**（本机无商家登录）。

## Rust 原生后端试点：DOMRect / DOMRectReadOnly（2026-10-29）

`crates/diting_runtime/src/native/`：FunctionTemplate + 内部字段（字段0=接口标签，1–4=x/y/w/h）+ 原始回调登记进 external_references。
`install_faces` 里装到全局，bootstrap 里原 JS 版因 `typeof DOMRect === 'undefined'` 守卫自动让位。
验收：`native-pilot-probe.js` 42 项对 Chrome 145 真值（`native-pilot-truth-145.json`）——裸 V8 快照恢复 / 新建 VM 全一致，完整引擎 41/42
（唯一差异 structuredClone：它是 JS 实现，不认原生实例，需每接口序列化钩子）。
