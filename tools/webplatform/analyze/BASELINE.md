# JS 平台层依赖基线（生成件 · `tools/webplatform/analyze`）

> 静态分析：acorn + eslint-scope。**近似**：加载期 = 顶层执行路径（含 IIFE 与同步回调 forEach/map 等）；函数体内引用记为调用期。动态写入（`globalThis[expr] =`）、`eval`、PRELUDE 注入名分析不到，归入 external。

## 总览

| 指标 | 值 |
|---|---|
| 快照脚本数 | 38 |
| 总行数 | 26129 |
| bootstrap.js 行数 | 17078 |
| 顶层绑定数 | 570 |
| 跨文件·加载期依赖（文件×名字） | 46 |
| 跨文件·调用期依赖（文件×名字） | 129 |
| 无任何跨文件加载期依赖的文件 | 15 / 38 |
| **加载序违规**（加载期引用较晚文件的绑定） | 1 |
| **同文件 TDZ** | 0 |
| hide list 泄漏候选（bootstrap 之后的 `_`/diting 函数声明或 var） | 0 |
| 无提供者的外部名 | 2 |
| **跨文件原型改写**（点数 / 文件×目标对） | 16 / 10 |
| 平台模块数 / **模块契约违规** | 2 / 0 |

## 枢纽绑定（被最多引用点依赖）

| 名字 | 引用点数 |
|---|---|
| `_markNative` | 77 |
| `_OPS` | 44 |
| `document` | 32 |
| `Document` | 28 |
| `Element` | 23 |
| `__hideOwn` | 20 |
| `_markNativeProto` | 16 |
| `__evtStore` | 15 |
| `_fpRand` | 12 |
| `Range` | 12 |
| `__recordFetchTiming` | 9 |
| `_fetchStage` | 8 |
| `_invokeListener` | 8 |
| `_fp` | 7 |
| `__lmap` | 7 |
| `_domRaw` | 6 |
| `__listenerGate` | 6 |
| `HTMLFormElement` | 5 |
| `_bytesToBase64` | 5 |
| `_dom` | 5 |
| `HTMLCanvasElement` | 5 |
| `HTMLOListElement` | 4 |
| `_domParse` | 4 |
| `__def` | 4 |
| `_initiatorHint` | 4 |

## 每文件

| 文件 | 行 | 顶层绑定 | 全局属性写 | 加载期依赖（来源文件:名字数） | 调用期依赖文件数 |
|---|---|---|---|---|---|
| bootstrap.js | 17078 | 441 | 341 | — | 0 |
| cssom_sheets.js | 141 | 7 | 2 | bootstrap.js:1 | 1 |
| pointer_touch.js | 93 | 0 | 6 | bootstrap.js:4 | 0 |
| webrtc.js | 231 | 2 | 18 | bootstrap.js:3 | 1 |
| location_nav.js | 65 | 0 | 2 | bootstrap.js:1 | 1 |
| list_reflect.js | 83 | 2 | 0 | bootstrap.js:3 | 0 |
| details_summary.js | 74 | 0 | 2 | bootstrap.js:1 | 1 |
| css_register.js | 97 | 0 | 1 | bootstrap.js:2 | 1 |
| css_decl_door.js | 73 | 8 | 0 | — | 0 |
| raf.js | 30 | 0 | 2 | — | 0 |
| computed_size.js | 36 | 0 | 1 | — | 1 |
| form_validation.js | 252 | 0 | 2 | bootstrap.js:6 | 1 |
| fetch.js | 282 | 4 | 1 | bootstrap.js:1 | 1 |
| audio.js | 363 | 7 | 5 | bootstrap.js:2 webrtc.js:1 | 1 |
| worker.js | 415 | 16 | 2 | — | 1 |
| media_source.js | 494 | 12 | 2 | bootstrap.js:2 | 1 |
| iframe_realm.js | 398 | 5 | 2 | — | 1 |
| range_selection.js | 216 | 8 | 3 | bootstrap.js:1 | 1 |
| indexeddb.js | 591 | 33 | 11 | bootstrap.js:1 | 1 |
| cookie_gate.js | 26 | 1 | 0 | — | 1 |
| platform_bundle.js | 279 | 1 | 0 | — | 1 |
| crypto_subtle.js | 816 | 0 | 5 | — | 1 |
| ed25519.js | 143 | 0 | 0 | — | 1 |
| xhr.js | 645 | 1 | 2 | bootstrap.js:2 | 2 |
| domparser.js | 197 | 0 | 1 | — | 2 |
| performance.js | 539 | 9 | 11 | bootstrap.js:1 | 1 |
| window_faces.js | 26 | 0 | 4 | — | 0 |
| message_port.js | 235 | 8 | 4 | bootstrap.js:1 | 1 |
| fetch_probe.js | 57 | 5 | 1 | — | 1 |
| global_surface.js | 807 | 0 | 4 | iframe_realm.js:1 | 1 |
| media_late.js | 55 | 0 | 0 | bootstrap.js:1 | 1 |
| canvas_face.js | 115 | 0 | 0 | bootstrap.js:1 | 0 |
| interface_sweep.js | 328 | 0 | 3 | bootstrap.js:1 | 1 |
| css_rules.js | 304 | 0 | 2 | — | 1 |
| adopted_sheets.js | 51 | 0 | 2 | cssom_sheets.js:1 bootstrap.js:1 | 2 |
| web_api_gaps.js | 168 | 0 | 0 | bootstrap.js:5 | 1 |
| navigation.js | 236 | 0 | 2 | — | 1 |
| client_rects.js | 90 | 0 | 0 | bootstrap.js:2 | 2 |

## 违规明细

| 类型 | 文件:行 | 名字 | 说明 |
|---|---|---|---|
| forward-load-ref | bootstrap.js:17025 | `_IframeWindow` | 提供者在较晚文件 iframe_realm.js |

## 跨文件原型改写（自由变量图看不见的隐式耦合）

| 文件 | 目标（定义于） | 改写点数 |
|---|---|---|
| pointer_touch.js | `Element.prototype`（bootstrap.js） | 3 |
| form_validation.js | `HTMLFormElement.prototype`（bootstrap.js） | 3 |
| webrtc.js | `Element.prototype`（bootstrap.js） | 2 |
| canvas_face.js | `HTMLCanvasElement.prototype`（bootstrap.js） | 2 |
| cssom_sheets.js | `Document.prototype`（bootstrap.js） | 1 |
| interface_sweep.js | `Document.prototype`（bootstrap.js） | 1 |
| adopted_sheets.js | `CSSStyleSheet.prototype`（cssom_sheets.js） | 1 |
| web_api_gaps.js | `Element.prototype`（bootstrap.js） | 1 |
| client_rects.js | `Element.prototype`（bootstrap.js） | 1 |
| client_rects.js | `Range.prototype`（range_selection.js） | 1 |

## 无提供者的外部名（2）

主机/PRELUDE 注入或动态定义，需要人工归类：

`c`(1)、`HTMLAllCollection`(1)
