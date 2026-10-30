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
成员级已核 196 接口。同日晚刷新（M3 事件线+M4 刀1 几何族落表）：缺口 163→142 接口、2631→2471 成员（M4 刀2 再收：2471→1043；刀3 继收：1043→992；刀4 继收：992→652；刀5 继收：652→575；刀6 继收：575→478；刀7 继收：478→370；刀8 继收：370→321；刀9 继收：321→275；刀10 继收：275→262；刀11 继收：262→233；刀12 继收：233→224；刀13 继收：224→208；刀14 继收：208→188；刀15 继收：188→187；刀16 继收：187→180；刀17 继收：180→**158**，见文末）；出表的=M3 原生事件核的 cancelBubble/returnValue 等全线 + 几何六件（见差集表）。已确认的真缺口样例：
~~`DOMRectReadOnly`/`DOMMatrixReadOnly` 仅空壳 constructor~~（2026-10-29 M4 刀1 全族补齐）、
`Element.clientLeft/clientTop` 缺、`ClipboardChangeEvent` 等未继承 Event 成员。

## 文件清单

| 文件 | 职责 |
|---|---|
| bootstrap.js | 根装配 + 大部分 API 族（指针捕获、PointerEvent/Touch、hover 链、elementFromPoint、cookie、credentials…） |
| indexeddb.js | IndexedDB 全家（open/upgrade 语义、cursor、proto 转发、arity） |
| cookie_gate.js | document.cookie 访问门 __cookieGate（遗留层；随 Document.cookie 族迁走）；cookie 读写本体仍在 bootstrap 的 document 字面量里 |
| platform_bundle.js | **生成件**：platform/core.js（模块注册表）+ 全部平台模块，按 requires-load 拓扑序（tools/webplatform/bundle.py） |
| platform/gen/storage.gen.js | **生成件**：Storage、StorageEvent 的 IDL 骨架（tools/webplatform/gen/gen_iface.py，规范 webref 6ac6259，成员顺序取 Chrome 真值 overlay） |
| platform/storage.impl.js | 手写：Storage 的 backing 行为（惰性加载、防抖写盘、opaque origin 仅内存） |
| platform/gen/geometry.gen.js | **生成件**：几何七件套的 IDL 骨架（gen_iface.py value 策略：类/描述符/安装序/statics 全生成，行为全部走 B.* 委托） |
| platform/geometry.impl.js | 手写：几何族行为（m_{CR} 存储、CSS transform-list 解析、WebKitCSSMatrix 旧脸接线 ready()） |
| platform/gen/htmlelement.gen.js | **生成件**：HTMLElement/Element 通用成员层（gen_iface.py patch-common 策略：按 Chrome truth 自有属性序发射搬位/反射/委托/staged，生成期对账断言） |
| platform/htmlelement.impl.js | 手写：非均匀成员语义（contentEditable/popover/nonce 内槽/outerText/outerHTML 报文/classList 转发/hasAttributeNS 等） |
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
| HTMLElement/Element 通用成员层（on* 搬位+反射器+读写升级） | **truth 生成**（patch-common：DATA 表+P.reflector 唯一运行时） | htmlelement_members_match_chrome_truth_except_known_gaps（54 探针对 Chrome 145 真值；刀4 后 staged 滤表清空、fn.length 缺口出册） | 2026-10-29 M4 刀2：详见文末 |
| 几何族 DOMPoint/Rect/Quad/Matrix*+WebKitCSSMatrix | **IDL 生成** | geometry_stack_matches_chrome_truth_except_known_geometry_gaps（156 探针对 Chrome 145 真值，155 全等+1 已档 ulp 缺口） | 2026-10-29 M4 刀1：bootstrap 玩具删除；shape 全生成+行为全委托；WebKitCSSMatrix=旧厂函数共 DOMMatrix.prototype；缺口=rotate 非 90° 整数倍角的 Blink SIMD sincos 末位 ulp（见测试注释） |
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

## 原生后端：声明层 + DOMPoint + structuredClone 钩子（2026-10-29）

接口 = 静态描述符（`native/declare.rs` + `native/geometry.rs`），一套通用回调（ctor/get/set/toJSON/factory）服务全部接口，
external_references 恒为 5 条，与接口数无关。DOMRect*/DOMPoint* 四个接口，探针 63 项：裸 V8（快照恢复/新建）全一致，
完整引擎 62/63（缺 `DOMPointReadOnly.matrixTransform`，待 DOMMatrix 原生化）。跨族品牌（DOMRect getter 作用于 DOMPoint）
走 family 标签拒绝，变异验证过。structuredClone：原生实例由 `client_rects.js` 的 `__diting_clone_hooks` 重建（按 @@toStringTag）。

## M3 步骤1：DOMException 原生化（2026-10-29）

`native/exceptions.rs`：描述符声明（字符串槽 message/name，`code` 由 name 派生，25 个遗留常量同时挂构造函数与原型，原型链接 Error.prototype）。
bootstrap.js 里的 JS 版已删除（−34 行）。探针 36 项对 Chrome 145：裸 V8（快照恢复/新建）与完整引擎全一致；Event 族探针 31→38/71。
声明层扩展：`Def/Val`（数值/字符串槽）、`consts`、可选 `factory`、`error_like`；全部接口共用一张表（`native/mod.rs::IFACES`）。
回归修复：JS structuredClone 的 Error 分支会对副本赋 `name`（原生 DOMException 是只读访问器）——钩子查找提前到 Error 分支之前，
新增 `native_clone.js` 统一登记原生接口的克隆钩子（几何 + DOMException）；测试 test_structured_clone_of_native_dom_exception_and_geometry（变异验证过）。

## 实验：V8 私有槽作 EventTarget 监听器存储（2026-10-29，已拆除，结论留档）

在任意对象上用 `v8::Private`（`for_api` 具名符号）存「事件类型→函数数组」：
- 烤进快照、恢复后监听器仍能触发——普通对象、**全局对象**、遗留 JS 类实例（未经原生构造函数创建）三种都过；
- 恢复后新增监听器并入同一表；`for_api` 符号身份跨快照保持；
- 页面反射不可见（`Reflect.ownKeys`/`getOwnPropertySymbols` 都为空）；冻结对象上也能挂监听器（Chrome 同）；
- 代价：粗实现 291ns/次派发（3 监听器），JS WeakMap 版 29ns；主要是每次重建键字符串，正式实现要缓存键。
设计含义：EventTarget 原生化**不需要**内部字段/原生构造函数创建的对象——遗留节点、window 照样能用私有槽，迁移可渐进。
未验证：监听器抛异常时的 TryCatch 上报、嵌套派发、中途增删监听器的快照语义（正式实现里用探针覆盖）。

## M3 步骤2：EventTarget 原生化 + 单一派发核心（2026-10-29）

`native/event_target.rs`：DOM §2.9 派发算法一份（路径 → 捕获 → 目标[捕获/冒泡两趟] → 冒泡；once/passive/signal/去重/监听器快照/handleEvent 对象/
异常上报后继续/再入 InvalidStateError/`when()`）；监听器表在私有槽（实验已验证）；宿主相关的一切走 `event_hooks.js` 注册的 hooks
（enter/exit=_mtWindow 括号、report、parent、isEvent、handler=on* 属性）。`__diting_event_hooks_set` 一次性注册后即删，页面不可见。
探针（event-probe.js 71 项）38 → 51 一致，EventTarget 部分全绿；剩余 20 项全是 Event 类（见 tests.rs 的 KNOWN_EVENT_GAPS，棘轮式只减不增）。
副作用发现：audio.js 把普通对象 `{type:'complete'}` 交给 dispatchEvent（Chrome 会抛 "not of type 'Event'"，旧 JS 版放过）——已改成真 Event。
未迁（步骤 B）：Node/Document/window/XHR/MSE/AbortSignal 各自的 dispatchEvent 仍是遗留 JS（AbortSignal 派发的 `{type:'abort'}` 也是普通对象）。

## M3 步骤C：Event/CustomEvent 原生化 + 20 项棘轮清零（2026-10-29）

`native/event.rs`：Event/CustomEvent 描述符（成员按 Chrome 原型序），状态=固定布局 17 槽 JS 数组存 `diting#ev-state` 私有槽
（页面子类 `extends Event` 分配普通对象，内部字段装不上——私有槽即品牌，this 上任何 shape 都能盖）。
`isTrusted` 是 [LegacyUnforgeable]：实例唯一 own 属性（getter-only、不可配置），构造时用捕获的 intrinsic `Object.defineProperty`
（`diting#ev-dp`）盖共享描述符（`diting#ev-itdesc`）——页面覆写 defineProperty 也偷不走。
要点：S_DISPATCHED 一个位兼管再入 InvalidStateError 与 initEvent 派发中失效；composedPath 派发中返回存路径、派发后 S_PATH_DONE 永远 `[]`、
未派发 target=null 也 `[]`；cancelBubble set-false 不清旗；字典成员按字典序读（bubbles/cancelable/composed[/detail]）。
子类状态迁出实例：`_evx` WeakMap + `_evxProps` 在子类原型装 accessor 对（Chrome 子类 own keys=['isTrusted']）；
引擎内部写腿=原生 facade `__diting_event_state`（bootstrap 顶部捕获成 `_evS` 即删全局；无品牌对象退化为普通属性——XHR/MSE 手搓事件照旧容忍）。
全部 ~50 个内部写点迁 `_evS`（window/Node/Document/XHR/MSE/window on* 包装）。
**修出一个严重泄漏**：EventView 重写时 invoke 把 state 数组本体当 event 传给监听器（Node 走 JS 旧派发没露，普通对象走原生核心全露）——
EventView 带回 obj 句柄，监听器/on* 钩子收事件本体。
验收：探针 71→**79 项对 Chrome 145 全一致**（新增 cancelBubble.setfalse、et.passive、isTrusted 六键）；tests.rs KNOWN_EVENT_GAPS 20→**0**
（棘轮清空，新差异只许真回归）；surface fixture 刷新（Event 原型 12 访问器+4 常量+5 方法，子类原型现出各自成员）；
变异三刀全咬（cancelBubble 清旗 / preventDefault 弃 passive / isTrusted 不装配——最后一刀 11 键齐响）。
遗留 JS 派发器（window/Node/Document/XHR/MSE）收尾盖章 `pathDone`（composedPath.after 真值），迁原生核心仍是步骤 B。

## M3 步骤B1：window 派发并轨原生核心（2026-10-29）

bootstrap 顶部的遗留 window 三件（`__windowListeners` 基表 + `_mtDepth` 包装 + `_windowDispatch` on* 包装，三层）删除；
`:96-98` 直接把 `EventTarget.prototype` 三件装成 window own 属性，`global_surface.js` relocate() 照旧搬到 P1 门层——
window own-prop 脸不变。window 专属 on* 腿（`__windowOnHandlers` store、`<body onX>` 内联回退 `_bodyWindowInline`、
'error' 只走管线不上双报）整体住进 `event_hooks.js` 的 `handler` 钩子；handler 在监听器循环**之前**跑（指派在前的常见页形与 Chrome 同序；
已知分歧：监听器在前、on* 后指派时我们仍 handler 先）。
`_mtDepth` IIFE 的 window 腿随之删除（Node/Element/Document 腿不动，B3/B4 再并）。
**修出一处核心缺口**：`invoke()` 在监听表为空时提前 return，把 handler 腿也跳了——DOM §2.9 的 inner invoke 与监听器数量无关
（`<body onload>` 页零监听器照样触发 onload；Chrome 探针 `win.on.nolistener`=yes 坐实）。空表现在只清空快照，on* 照跑。
**反向缺口一并修**：通用腿 `target[key]` 对非 IDL 处理器槽的 on* 数据属性（`window.onzzzprobe=f`、裸 EventTarget 上的 `et.onfoo=f`）
会过度触发——Chrome 只认 accessor 槽（`isHandlerSlot` 沿原型链找第一个描述符，data 即非处理器）。
#227 斩首形改写：native 核心的 enter/exit 钩子跑在 Rust 侧，监听器被脚本看门狗砍掉后括号照样闭合（`_mtDepth` 立即归 0）——
旧测试断言「必须泄漏≥1」的前提不复存在，改为钉「砍的是活派发 + 砍完即回 0」。
验收：探针 79→**85 项对 Chrome 145 全一致**（新增 win.et/win.dispatch.state/win.on.order/win.on.nolistener/win.on.unknown/et.on.unknown）；
known 分歧（不修，留档）：合成 `dispatchEvent(new Event('error'))` 不触发 onerror（管线已叫过，避免双报；Chrome 会触发）。
未迁（B2 起）：XHR/MSE/Worker/FileReader/iframe 派发器 + AbortSignal 的 `{type:'abort'}` 普通对象；B3/B4：Document、Element/Node。

## M3 步骤B2a：XHR 家族+AbortSignal+BatteryManager 派发并轨原生核心（2026-10-29）

B1（window）之后的第二刀：七套同形手搓派发器里的第一组并轨。XHRET 类体 34 行三件删净，
`class XMLHttpRequestEventTarget extends EventTarget {}` ——三件经链解析到原生核心
（XHRET.prototype → EventTarget.prototype，Chrome 同构）；XHR 类体自有三件（含 zone.js 注释那份
loose dispatchEvent）同样删净。发火点全部换真事件：`_fireEvent` 现产
`new ProgressEvent(type)`（传输族）/`new Event('readystatechange')`，`_setReadyState` 的
`this.onreadystatechange()` 直调删除（#248 的「故意跳过」被核心的 handler 腿取代——
一次派发同时喂监听器+handler，旧直调会双发）；handler 现在收到 event 参数、this=xhr（Chrome 同）。
on* 迁 #31 `__acc` 原型 accessor（isHandlerSlot 只认 accessor 槽）：七个传输事件在 XHRET.prototype，
onreadystatechange 在 XHR.prototype（Chrome 的分家位置），ctor 的 `this.onload=null` 初始化经
setter 落隐藏 `_onload`。upload 由内联桩对象换 `Object.create(XMLHttpRequestUpload.prototype)`
（链式真身；引擎无增量网络腿，不发事件，原型形状即全部契约）。AbortSignal 两修：extends 直指
EventTarget（Chrome 145 真链不过 XHRET——探针抓出的旧简化）、`_signalAbort` 的
`{type:'abort'}` 普通对象换 `new Event('abort')`。BatteryManager 桩三件删除+接链
（getBattery 的实例=Object.create 其原型）。NetworkInformation 验明早已接链（#27 时代），不动。
XMLHttpRequestUpload 由生成件 global_surface 定义，其链布线落在它后面的 interface_sweep.js
（生成件禁手改）。夹具面 diff=声明的三张原型面（XHRET −三件+七 accessor；XHR −三件
+onreadystatechange；BatteryManager −三桩）；棘轮 47→48（xhr.js 顶层加载期引用 bootstrap 的
`__acc`）。已知分歧沿 B1 档：handler 先于监听器（Chrome=注册序，本步探针模式两说一致）；
#31 accessor 的 JS toString vs Chrome native getter（模式级既有面，探针键已避开）。
验收：探针 85→**96 项对 Chrome 145 全一致**（xhr.chain/instanceof/on.desc/on.default/upload/
dispatch.shape/dispatch.on/dispatch.string、abort.event/abort.dispatch.state、battery.et）；
tests 1432/0；check_fresh+棘轮绿。步骤 B2b=MSE/Worker/FileReader（同形）；iframe 两腿因
receiver() 判链（et-proto 私槽等值比较）与已立案的 _IframeWindow 链缺陷纠缠，并入该专项。

## M3 步骤B2b：Worker/MSE/FileReader 派发并轨原生核心（2026-10-29）

七套手搓派发器第二组收编，`__evtStore` 只剩 Document（B3）与 iframe 两腿（B2c，并入
_IframeWindow 链缺陷专项）。**Worker**：`class Worker extends EventTarget`，自有
add/remove 二件删净；发火点全换真事件——workerSelf.postMessage 的页面腿换成
`dispatchEvent(new MessageEvent('message', {data}))`（在 `_unmaskMainThreadGlobals` 窗内，
MessageEvent 可解析），`_fireWorkerError` 换 `new ErrorEvent('error', {...})`（原生 handler 腿
先 onerror 后监听器=旧 unshift 序）；on* 迁 `__acc` 原型 accessor——**Chrome 145 实测
Worker.prototype 无 onmessageerror**（成员=constructor/onerror/onmessage/postMessage/terminate，
引擎迁移后成员集与 Chrome 完全一致）。**MSE**：`_MseEventTarget` 三件删净换 `extends EventTarget`
（旧注释担心的 tree 路径/nid 问题不存在——核心 parent 钩子对非 Node 返 null）；发火点本就真
`new Event`；关键补丁=`ms.onsourceopen = fn` 原落实例数据属性会被 isHandlerSlot 拒——补
`__acc(MediaSource, [onsourceopen/onsourceended/onsourceclose])` 与
`__acc(SourceBuffer, [onupdatestart/onupdate/onupdateend/onerror/onabort])`（Chrome 分家位置；
播放器在 onsourceopen 里建 SourceBuffer，不补即哑）。**FileReader**：`extends EventTarget`，
桩 dispatchEvent（恒 true 的空壳）删净，`_fire` 换 `new ProgressEvent(type)`（ctor 默认
lengthComputable:false/loaded:0/total:0 与旧普通对象字段全等）。ctor 的死 `__lmap(this)`
清除（XHR/Worker/FileReader 三处）。夹具面 diff=四张原型面（上述+FileReader）；棘轮 48→50
（worker.js/media_source.js 加载期引 `__acc`）。验收：探针 96→**102 项对 Chrome 145 全一致**
（worker.et/mse.et/mse.on/sb.on.desc/fr.et/fr.dispatch）；tests 1432/0。探针自课：裸
getOwnPropertyDescriptor 描述符上误用 `desc()` 紧凑键 `.g`（真键 `.get`）——第一轮 worker.et
假分歧即此。

## M3 步骤B3+B4：Document/Element/Node 派发并轨原生核心（2026-10-29，一刀）

七套手搓派发器的终刀（B2c iframe 两腿并入 _IframeWindow 链缺陷专项）：bootstrap 的
Document 与 Element 自有三件套（合计 ~200 行）连同注册机器（`_eventRegistry` 常量组）
全部删除，元素/文档的一切派发落到原生 EventTarget 核心。元素专属语义不走类体、全部
进 `event_hooks.js` 的宿主钩子（Rust 侧四新钩：`origin`（offsetX/Y 逆映射一次/派发）、
`activationPre/Post`（checkbox/radio 预翻转+提交/取消+label 转发/表单提交）、
`registered`（animation* 注册即排动画扫描）；handler 腿补 return-false→preventDefault）。
`parent` 钩子补 document→window 边（document.parentNode 为 null 但 Chrome 传播路径续进
Window——元素冒泡事件从此达 window 监听器，旧实现停在 document）。连带修法：
`_windowScroll` 的 scrollend 双发（doc+window 各一）撤窗腿补发（doc 单发即传播到窗，
旧双发会双响）；`_fireScroll` 陈注「dispatch 不传播」随实况改写。Node.prototype 上
一对来历不明的惰性三件（add/remove len=0、dispatch=name_sweep 分离壳形）——陷阱实证
为 fill-if-missing（预设 accessor 即挡装；赋值从不穿 setter、也从不替换 accessor），
 installer 身份穷尽搜索未钉（全部 js、生成件、Rust、`__diting_init` 均无）；旧世界被
Element/Document 真三件遮蔽无害，本刀删净上层后它成为解析目标、全元素监听器变哑——
在 event_hooks.js（最后一个快照脚本，早于它的一切 fill 已落）尾段 delete 中和，解析
落穿到原生核心。**form 代理腿**：表单元素骑 `_formNamedProxy`（命名访问 JSProxy），
V8 C++ get_prototype 对 JSProxy 返 null→receiver() 拒收（"Illegal invocation"）——
补 JS 级 `Object.getPrototypeOf` 兜底（安装期捕 Object 入 `diting#object` 私槽，页面
覆写不能改道；私符号穿 proxy trap，监听器存储不走代理）。**DocC 教训**：钩子里
`target === DocC` 的 DocC 是快照 realm 的 document——类跨 realm 存活（instanceof 活）
但 document 实例每 realm 新铸，恒不等（composedPath 真值键当场抓获：Chrome 尾带 W、
引擎停在 D）；parent 钩子补认活绑定 `globalThis.document`，detached 元素（同为
null-parent）仍不误入窗。夹具面 diff=声明的三张原型面（Document/Element/Node 各
−addEventListener/removeEventListener/dispatchEvent——与 Chrome 同形，三件套解析落
EventTarget.prototype）。验收：探针 102→**106 项对 Chrome 145 全一致**（新四键
et.el.window.reach/et.el.onclick.false/et.el.handler.order/et.el.dedup——Chrome 实测
dedup=2：第四行 (f,false) 与首行塌缩，非预想的 3，探针说了算）；truth 空棘轮零豁免；
tests 1432/0；check_fresh+棘轮绿（bootstrapLines 净减 ~230，forwardLoadRefs 仍 1）。
外引擎探针踩坑：`cargo build --release` 只重编 lib 不重链根包 bin（root package=
diting-browser，bin=根 src/main.rs）——health 报旧 commit+旧行为即此，touch 根
src/main.rs 强制重链后 106/106。已知分歧沿档：handler 先于监听器（Chrome=注册序）。

## M3 步骤B2c：iframe 两腿派发并轨原生核心 + _IframeWindow 链缺陷修（2026-10-29）

M3 终收：iframe-doc 与 content-window 两腿的手搓三件套删净（`__lmap`/`__evtStore`
随之成死码一并删），帧面的一切监听/派发落原生 EventTarget 核心。**链缺陷修**（M1
立案的加载期前向引用）：bootstrap 尾部 `setPrototypeOf(_IframeWindow.prototype,
Window.prototype)` 在加载期引用后件 `iframe_realm.js` 的类、TDZ ReferenceError 被
catch 吞——帧窗原型实为 Object.prototype 已数月。修法=链接搬进 iframe_realm.js 类
定义之后，并给死对象 `Window.prototype` 补 Chrome 亲子：帧链=「_IframeDocument.
prototype → Document.prototype → Node.prototype → ET.prototype」与「_IframeWindow.
prototype → Window.prototype → ET.prototype」（**主窗链不含 Window.prototype**——
probe 实证 globalThis 第一层是另一 1263 键对象，主窗靠全局门过 receiver；故给
Window.prototype 接 ET 只影响帧、零存量位移）。顺带成真：contentDocument
instanceof Document/Node、contentWindow instanceof Window/EventTarget、
addEventListener === EventTarget.prototype.addEventListener。**parent 钩子补帧内两跳**
：帧根（_isIframeDocRoot）→帧 doc→帧窗（帧版 document→window 边）——帧内元素冒泡
从此达帧 doc/帧窗监听器且止于此（不漏入宿主 doc/主窗，回归测试钉死）；此前旧腿
两代实现都停在帧根。**假 doc 两坑**：①构造器裸赋值撞链上 Document.prototype 的
只读 accessor（nodeType 等 7 项改 `__def`）；②链让 Node.prototype 的原生背书
`parentNode` getter（走 `_dom(_nid)`）对无 nid 的假 doc 抛 Illegal invocation——
DOM 规范 Document.parentNode 本就=null，补 own getter 双愈（事件路径走查+
summaryActivation 走查）。已知留档：postMessage 投递仍落主窗（Chrome=目标窗，
路由缺陷另案）；帧 doc 的 Node.prototype 其余原生背书成员（firstChild 等）对假
this 仍会抛——面已收窄到事件腿所需。验收：tests 1432→**1433/0**（新回归测试
test_iframe_realm_dispatch_rides_native_core：链形/冒泡序 [el,doc,win]/on* 腿/
去重/宿主零漏）；探针 106 项不变全绿（主 realm 面零位移）；外引擎 srcdoc 实战
烟测全过；夹具面零 diff；棘轮 forwardLoadRefs **1→0**（TDZ 前向引用死绝）、
crossFileLoadTimeNames 50→51（iframe_realm 加载期引 Document/EventTarget 的
合法序边，--write-baseline）。iframe 回归测试载体=Rust 测试而非 Chrome 探针：
帧域是单 realm 壳（cross-realm identity 在 Chrome 为假、在本引擎为真），探针键
会造假分歧。

## M4 刀1：几何族（DOMPoint/DOMRect/DOMQuad/DOMMatrix×2 + WebKitCSSMatrix）（2026-10-29）

一族一刀收口：`gen/geometry.gen.js`（gen_iface.py 新 **value 策略**——类壳/描述
符/安装序/@@toStringTag/statics 全部生成，值行为经 `B.ctor/get/set/call/stat`
字符串键委托）+ `platform/geometry.impl.js`（全部行为）。bootstrap 的几何玩具
删除（冻结律只缩）。真源=Chrome 145 实捕 `geometry-truth-145.json`（156 键）：
**155 键逐位相等 + 1 键已档缺口**。

生成器一并长出（storage.gen.js 同步重生成过门）：webref 的 `special:'static'`
识别、stringifier 归入 ops、接口拓扑排序（父先子后）、expect 拆
members/statics（verify() 各自核挂点）、value 模块尾 `if (B.ready) B.ready()`
——impl 借此在类就位后接线旧脸。

行为钉子（全部 truth 实证）：m_{CR} 列主存储、直角快路（90° 整数倍出干净
0/±1）、rotate 单参=Z 轴保 is2D / 三参=Rx·Ry·Rz、DOMMatrixInit 三维判定按
**字典默认值**（m33/m44 默认 1——按 0 比会把活体 2D 矩阵读成 3D）、CSS
transform-list 的 is2D=无三维函数出现、toJSON 键序=字典序、DOMRect 四边
NORMALIZE、WebKitCSSMatrix=旧厂函数共享 DOMMatrix.prototype（name 仍
'DOMMatrix'，三个自有 statics）、`scale(2, undefined, 3)` 的 NaN **不跨列扩散**
——Blink 原位乘列，全矩阵乘的 0·NaN 会毒化整阵（postTranslate 同理只动末列）。

已知缺口（ratchet，见 tests.rs 注释）：`ro.rotate45` 差 1 ulp——Blink rotate
走 Chromium 自家 SIMD sincos（third_party/simd），45°/60° 处与 V8 Math.sin 末
位不同（0.7071067811865475 vs …476）；本引擎 Math.sin 与真 Chrome 的
Math.sin **逐位相同**（专项实捕验证），故此缺口是 C++ 库末位舍入，非我们错。

验收：tests **1434/0**（新回归=geometry 探针测试）；node 直跑差分 156 键
155 等；夹具面 diff=几何族换血（WebKitCSSMatrix constructor 壳→全脸）；
check_fresh 4 生成件全鲜；analyze 棘轮过（moduleContractViolations=0）。
SURFACE-WEBIDL.md 差集表已随刀刷新：几何六件出表，缺口 163→142 接口、2631→2471 成员。

## M4 刀2：HTMLElement/Element 通用成员层（2026-10-29）

一刀两层收口：**成员搬位 + 反射生成化**。真值=`chrome/htmlelement-truth-145.json`
（54 键实捕 Chrome 145：HT/EL order+kinds、on.levels、反射语义全键、fn.length）。

**生成线推进**（回应「别手写规则」）：均匀反射从手写规则变为 **DATA 表 +
唯一运行时**——`P.reflector(proto, name, spec)`（core.js，五个 kind：
string/presence/truefalse/string-known/known-fallback）+ gen_iface.py 新
**patch-common 策略**：读 truth 的 HT.order **按 Chrome 自有属性序逐名发射**
（constructor 在 139 位重坐、onpointerrawupdate 垫尾），每名四选一：
搬位（seat：Element.prototype 描述符整体迁 HT/SVG 并删源）、反射安装、
impl 委托（非均匀语义）、staged 注释。生成器内置断言：表与 truth 的
fn.length/on.levels/HT.order 全对账，未入账成员即 build fail。

**搬位**：98 个 on* Element.prototype→HTMLElement.prototype（+SVGElement
按规范共享子集 dataset/style/tabIndex/focus/blur）+ 16 个真成员
（title/lang/dir/hidden/accessKey/offset 五件/innerText/popover/dataset/
autofocus/tabIndex/style/attachInternals/blur/click/focus/popover 三方法）；
Element 留 8 个 on*（beforecopy/cut/paste、fullscreen×2、search、
webkitfullscreen×2）；Document.prototype 补齐 110 个 on*；
**删 7 个引擎多出**（ondragexit/onfocusin/onfocusout/ontouch×4——
Chrome 145 任何层都不持有）；搬迁原生函数 arity 对齐 truth（focus
1→0、togglePopover 1→0）。

**新装**：HT 13 个反射器（draggable true/false 写串、autocapitalize
known-fallback 'sentences'、enterKeyHint/inputMode 大写直写 attr、
writingSuggestions 等）+ nonce 内槽不反射 + contentEditable 大小写不敏感
（'TRUE'→attr 'true'）+ outerText 替换语义 + attachInternals 精确报文；
EL：hasAttributeNS/scrollIntoViewIfNeeded/webkitMatchesSelector/
clientLeft/clientTop/currentCSSZoom/prefix/assignedSlot/elementTiming/
7×aria*Elements；classList/outerHTML 只读→读写升级（原 getter 保留，
setter 走 impl：classList=转发 className、outerHTML 无父抛
NoModificationAllowedError 精确报文）。

**staged（账面在册，后续家族刀）**：attributeStyleMap、computedStyleMap
（Typed OM 族）、part（DOMTokenList）、requestFullscreen×3 + requestPointerLock
（fullscreen 族）、moveBefore、ariaNotify。

验收：tests **1435/0**（新回归=htmlelement 探针测试，54 键 51 全等）；
夹具面 diff=五条原型行（Document/Element 换 on* 集、HTMLElement 裸→141、
Option/SVGElement 继承层）；check_fresh 4 生成件全鲜；analyze 棘轮过
（moduleContractViolations=0）。SURFACE-WEBIDL 差集表随刀刷新：
缺口 2471→**1043 成员**（-58%），行均缺口 66→约 7——大头已是 9 个
staged 名的重复出现；已档缺口=hasAttributeNS.cases（引擎属性存储丢
namespaceURI，跨层 DOM-core 债）+ fn.length 两项 staged。

教训：①`cargo build -p diting` 不重链根包 bin——js 改动要
`cargo build --bin diting` 才进 `target/debug/diting`（坑④复发，浪费
一轮差分）。②diting serve 无 --port，绑址走 `DITING_BIND=127.0.0.1:18199`。
③探针分析脚本剥 `DOC.` 前缀要 4 字符——3 字符剥出假零（.URL 类），
on.levels 的 startswith("DOC") 同病（值形如 "HT+SVG+DOC"），须
split("+") 含判。④spec 直觉再输一次：accessKeyLabel 在真 Chrome 145
不存在（Blink 未实现），truth 定谳。

## M4 刀3：EventTarget 继承 sweep（2026-10-30）

真值 `chrome/eventtarget-chain-truth-145.json`（43 键，探针
`eventtarget-chain-probe.js` 两端跑）：Chrome 145 非 DOM 侧共 **147 个**
EventTarget 家族接口，每个的**全链**（各级 constructor 名）、叶级不自有
三件（ownTrio）、可构造叶的监听/派发往返（rt.*）、20 个单例的
instanceof 面（sin.*）。修前引擎：129 件链断（含 Sensor/OrientationSensor/
AudioNode/AudioScheduledSourceNode/BaseAudioContext/Screen/MediaStreamTrack
等全部抽象中间层）+2 件 MSE 带私有中间层+16 个单例裸对象。

**修法（生成件 DATA 化）**：`gen_globals_surface.py` 读同一 truth 文件，
emit 三张表到 `global_surface.js`（该件在一切真类之后、sweep 之前跑）：
- `ET_CHAIN` 143 行：逐名 `setPrototypeOf(X.prototype, 真值父.prototype)`
  ——三件/when 经 M3 原生核继承解析；
- `ET_ALIAS` 3 行：webkitMediaStream/webkitSpeechRecognition/
  webkitRTCPeerConnection 是**同一接口对象**（[LegacyAlias]），从 OK 表
  除名换真身别名；
- `ET_SINGLETON` 20 行：裸对象单例（performance/speechSynthesis/
  cookieStore/screen.orientation/navigator 十一件…）补挂类原型。

**配套三处 legacy 收缩**（freeze 律最小缺陷修正，记档）：
- `media_source.js`：删 `_MseEventTarget` 空中间层，MediaSource/
  SourceBuffer 直接 extends EventTarget（Chrome 无此层）；
- `bootstrap.js`：`_MediaQueryList` 正名 MediaQueryList（constructor
  名脸）+ VisualViewport 删两行**空桩三件**（原监听被吞，现在真活）；
- `performance.js`：`_Performance` 正名 Performance + 构造守卫
  （`new Performance()` 照抛 Illegal constructor，内部旗标放行单例铸造）
  ——公开真类原型载成员、THROW 桩退役，chain/instanceof/方法三者同真。

**教训（当场抓获）**：盲 setPrototypeOf 单例会拆**拆体类**——performance
  的方法原在私有 `_Performance.prototype`，换链到 THROW 桩原型即孤儿化，
  fetch 全线死（`performance.now is not a function`），page 测试 172 件
  PoisonError 连坐（真根一件）。单例挂链前必须核方法的宿主原型。

**残留（棘轮在案）**：ownTrio 七件遮蔽——WebSocket/EventSource/
BroadcastChannel（`__lboxProto` 注册表，WS message 直读 _lbox）、
MessagePort（message-only 队列）、IDB 三件（on-handler 映射）、
MediaQueryList（mqFlush 注册表）。各为独立家族刀，不并刀。

验收：探针 43 键全等（147/147 链+无引擎多链）；tests **1436/0**；快照
漂移恰三行（Performance 真成员/Viewport 删三件/webkitMediaStream 真身）
已重生成；check_fresh 4 件鲜；analyze ratchet 过。差集表 1043→**992**
（OffscreenCanvas/SharedWorker 整行出表；CloseWatcher 6→3、
IdleDetector 6→3、MediaStream 4→1、SpeechSynthesisUtterance 4→1、
Animation 22→19）。下一刀顺位：Element staged 家族（Typed OM×2/part/
fullscreen）或真家族（Animation/Notification/传感器/EditContext）。

## M4 刀4：Element staged 家族收官（2026-10-30）

九件 staged 全数落地，刀2 的 staged 滤表就此清空。真值
`chrome/element-staged-truth-145.json`（38 键，真 Chrome 145 实捕）：

- **part**（EL rw 存取器）：缓存 `DOMTokenList(el, 'part')`（identity 真），
  setter 直写属性；SVG 元素同享（装在 Element.prototype）。
- **attributeStyleMap**（HT ro 存取器）→ **StylePropertyMap**、
  **computedStyleMap()**（EL 方法）→ **StylePropertyMapReadOnly**：新
  `platform/stylemap.impl.js` 模块**拥有两接口**（Performance 式移交：
  global_surface THROW 表 SKIP 我们的非枚举 expose；ctor 照抛 Chrome 原
  文 "Failed to construct … Illegal constructor"）。原型成员按 Chrome
  自有序（RO：size/get/getAll/has/entries/forEach/keys/values；SPM：
  append/clear/delete/set），`@@iterator = entries`（spm.symbols 真），
  SPM 原型链挂 RO。attributeStyleMap 解析 style 属性成有序 Map，
  `get('color')` 回私有 CSSStyleValue（constructor.name/toString 真；
  window 的 CSSStyleValue 桩不动）；computedStyleMap 缓存
  getComputedStyle(el) 全表，**size ≡ gcs.length 不变量**（Chrome
  413=413；引擎 444=444——计数差属 CSSStyleDeclaration 计算清单家族，
  已档棘轮 KNOWN_GAPS）。
- **fullscreen 三件**：requestFullscreen 回真 Promise 拒 TypeError
  "Permissions check failed"；webkitRequestFullScreen/webkitRequestFullscreen
  返回 **undefined**（真值证明：then 读取即 TypeError）。
- **requestPointerLock**：真 Promise（constructor.name "Promise"）拒
  DOMException WrongDocumentError "The root document of this element is
  not valid for pointer lock."。
- **moveBefore**（EL len2 + **Document/DocumentFragment 双宿主**，报文
  前缀随宿主接口名）：参数非 Node 抛 TypeError "parameter 1 is not of
  type 'Node'."；非己子抛 HierarchyRequestError "State-preserving atomic
  move…"；ref 非己子抛 NotFoundError "The node before which…"；合法即
  insertBefore 语义。
- **ariaNotify**（EL，len **1**，Chrome 实测归 Element 非 HTMLElement）：
  返回 undefined。

生成器长法：patch-common 新 `IMPL_ACCESSOR_EL_RW`（part 真读写腿，别于
aria 的 null+swallow）与 `MOVE_BEFORE_HOSTS`（ctx 前缀随宿主）；STAGED
表清空成未来预留。行为腿全在 htmlelement.impl.js（GET/SET/CALL 委托），
Typed OM 实例铸在 stylemap 模块。

**坑**：`call_function_on_remote` 自做 `__fn.call`——传裸 IIFE 会
"__fn.call is not a function"；异步探针测试一律包 `() => (probe)` 箭头
+await_promise。真 Chrome 里 `computedStyleMap().size ===
getComputedStyle(el).length` 两边同值（413/413），引擎侧同构成立
（444/444）——探针只钉绝对值会误报，测试改验**不变量**。

验收：探针 38 键，36 全等+2 已档（gcs.len 及 csm.face 的 size 位，不变量
真）；htmlelement 刀2 测试 fn.length 缺口出册（staged 九件装齐后 dict 全
等）；tests **1437/0**；快照漂移恰七行（Document/Fragment +moveBefore、
EL/HT/Option 九件、SPM 双原型真身）已重生成；check_fresh 4 件鲜；analyze
ratchet 过（模块契约零违）。差集表 992→**652**（-340，恰=5 名×68 接口的
预测账；行 140→118）。下一刀顺位：真家族（Animation 19/Notification 14/
EditContext 11/传感器×6/SpeechRecognition 8）或 HTML 元素成员家族。

## M4 刀5：HTMLMedia/Video/Image 媒体图像家族（2026-10-30）

缺口行分解干净的一刀：HTMLAudioElement 22 行=HTMLMediaElement 级、
HTMLVideoElement 31 行中 video 独有 9、HTMLImageElement 24——55 名关
**77 行**。Chrome 在三原型上是**满员**（53/16/30 own 名，引擎基线
25/5/3、零多余），故这刀不是补洞而是**整原型对齐**：缺件安装+三处
kind 修偏+reseat() 重排到 Chrome 精确 own 序。

- **生成器**：`patch-media` 策略（gen_iface.py）。DATA 表三张
  （REFLECT/CONST_GETTER/IMPL_*）+PURE_MOVE+ON_INSTALL；reseat() 尾走
  Chrome 序 delete+redefine（幂等）。
- **P.reflector 长三 kind**（core.js，真值钉死）：
  `unsigned-long`（双向 ToUint：set 'abc'→0/attr '0'，真值 vid.wh/
  img.wh）；`crossorigin`（'' 与 anonymous→'anonymous'，
  use-credentials，未知→null，真值 med.cross）；`url`（读值对
  document.baseURI 解析——poster='' 解析回页面本身，真值 vid.poster；
  写值原样进 attr）。
- **行为模块** htmlmedia.impl.js：volume 出 [0,1] 抛 IndexSizeError
  （真值逐字）；muted 纯状态不反射；defaultMuted=反射 'muted' 属性
  （attr 改动不动运行时）；preload 缺/坏皆读 'metadata'；textTracks
  铸 TextTrackList（stub 原型直接用，ctor 消息本就逐字同）；addTextTrack
  铸 TextTrack{mode:'hidden'} 且入列；captureStream=真 new MediaStream；
  setSinkId 拒 NotFoundError；setMediaKeys(null) resolve；decode 无源拒
  EncodingError；rvcb 计数 1,2；gvpq 铸 VideoPlaybackQuality 空实例。
  **VideoPlaybackQuality 类接管**（stub ctor 消息缺 Chrome 前缀，真值
  faces 六中五的唯一差）。
- **三处 kind 修偏**：MED.duration rw→ro（剥 setter）；IMG.sizes ro→rw
  （补 attr 写腿，原生 getter 留守）；VID onenter/onleave 数据属性→
  installOn 访问器。
- **链委托**（新类目）：med `src` 与 img `name` Chrome 归接口层、引擎在
  Element.prototype——且引擎 src setter 是**承重腿**（MSE blob 握手/
  script 预备/iframe 装载）。装同名访问器两腿转调 Element 描述符：归属
  层归 Chrome，引擎语义原封（一刀两得，不搬家）。
- **media_late.js 收缩**：VID PiP 四件块删除——它在 platform_bundle
  **之后**跑，会把模块装的 on* 访问器盖回数据属性（对拍当场抓获）；
  四件全部平台化（requestPictureInPicture 行为原样搬进 impl）。
- **bootstrap Image 模拟改槽**：complete/naturalWidth/naturalHeight 装
  成 getter-only 后，模拟内部的直赋（9 处）改 `__def` 隐藏槽（同
  _playing 通道）——own 槽遮原型 getter，读写两全。实例直赋在 Chrome
  同样抛 TypeError（readonly accessor），页面侧行为=Chrome。

**坑**（两新）：①九个就绪/网络常量（NETWORK_*/HAVE_*）bootstrap 以
`{value}` 装=**不可配置**，reseat 搬不动（delete 即 TypeError，快照期
当场抓获）；且引擎侧不可枚举而 Chrome 可枚举——**bootstrap 层既定
事实**，测试 MED.order 按剥常量后精确等序断言。②SURFACE-WEBIDL 行探
针走实例链，Element 级 src/name 早被探到=**从来不是缺口行**，装它们是
归属层洁癖——装错（裸反射器）当场拆了 MSE。

验收：探针 49 键全等（MED.order 常量归一后等序，VID/IMG.order 逐字
等）；tests **1438/0**（新增 htmlmedia_members_match_chrome_truth）；
快照漂移恰三原型+Image 别名行已重生成；check_fresh 4 件鲜；analyze
ratchet 过（模块契约零违）。差集表 652→**575**（-77，恰=预测账；行
118→115）。下一刀顺位：HTMLInputElement 23 / HTMLObjectElement 21 /
HTMLIFrameElement 16（元素家族续刀）或 Animation 19 / Notification 14
（真家族）。

## M4 刀6：HTML 反射类元素族——指纹实测出表（2026-10-30）

27 个元素接口（Anchor/Area/Base/BR/DList/Directory/Embed/Font/Form/Frame/
HR/IFrame/Legend/Link/Menu/Meta/Mod/OptGroup/Param/Pre/Quote/Script/Source/
Style/TableCaption/Time/Title）约 97 个缺口名，绝大多数是纯属性反射。这刀
不再手写每接口的表，而是**把反射语义当作实测量**：

- **行为指纹探针** `htmlreflect-probe.js`（真 Chrome 145 实捕，266KB 真值
  `htmlreflect-truth-145.json`）：对每个 rw 成员记默认读值、11 种赋值（含
  null/undefined/越界/浮点）后的回读与**创建出的内容属性名**（getAttributeNames
  差分，属性名是实测不是猜）、15 种原始 setAttribute 值后的回读、枚举成员的
  关键字词表扫描；另有 DOMTokenList 的 `supports()` 实测、实例路由
  （`createElement(tag) instanceof Iface`）、刀2 字符串转换键。页面 URL 归一为
  `#B`、origin 为 `#O`，使 Chrome 临时服务器与测试页（example.com/test）同形。
- **镜像分类器** `reflect_classify.py`：Python 镜像 core.js 的
  `P.reflector` 规则，逐种候选 kind 试跑，**只有复现指纹全部面**的才接受；
  都不复现的返回 None，进 impl 表或 STAGED，绝不猜 kind。结果：string 97 /
  boolean-presence 30 / enum 24 / url 17 / tokenlist 10 / string-nte 6 /
  unsigned-long 6 / crossorigin 4 / unsigned-long-dim 2 / long 1 /
  boolean-truefalse 1。生成器与 core.js 漂移时 gen 当场失败。
- **P.reflector 新 kind**（core.js）：`enum`（实测 map/missing/invalid/empty，
  旧 string-known/known-fallback 折成它）、`string-nte`（LegacyNullToEmptyString）、
  `unsigned-long-dim`/`long`、`nullRemoves` 开关；`string` 修正为 DOMString 转换
  （`null`→`'null'`，Symbol 抛 Chrome 原文 TypeError），`crossorigin` 未知值读
  `'anonymous'`（刀5 误写 null）、set(null/undefined) 移除属性。
- **patch-reflect 生成策略**：Chrome 自有序逐成员出「**安装或委托**」——引擎链
  上已有的成员（href/src/hash…引擎把它们通用地放在 Element.prototype）在接口层
  装同名访问器两腿转调（归属归 Chrome，语义不动；刀5 src 的教训制度化）；
  委托的字符串 setter 在接口边界先做 WebIDL 转换（`iframe.src = 5` 之前直接把
  引擎装载器打崩）；接口自身 own 的天真 setter 用 `wrapSet` 原位升级；
  FORCE 表收「按标签门控/存 expando/返回原始属性」的链上错误实现。
  账：reflect 135 / tokenlist 9 / impl 14 / const 2 / delegate 21 / left 10 /
  staged 7。
- **结构性修复**：15 个标签（area/base/dl/dir/embed/font/frame/menu/ins+del/
  param/blockquote+q/source/time/title）在引擎里 `createElement` 出来是**裸
  HTMLElement**，`el instanceof HTMLSourceElement` 为 false（Chrome true）——
  接口只是 global_surface 的抛异常桩。新模块 `htmlifaces` 用引擎自己的
  `_htmlInterface`（`_tagInterfaces` 路由+hasInstance 兜底）在 global_surface
  之前拥有这 14 个接口（Performance 式移交，THROW 表 SKIP）。
- **DOMTokenList 总用实测 supported 集**：引擎链上 Anchor/Area 的 relList
  没带集合（`supports()` 抛 TypeError，Chrome 不抛：noopener/noreferrer/
  opener），Link 的集合与实测不符；tokenAccessor 现在统一走 impl 缓存列表+
  实测集合+PutForwards setter。
- **刀2/刀5 已落地件的纠偏**（回归行抓获）：`translate` 是 boolean（写
  `'yes'/'no'`，刀2 把 `set.translate.bad` 读成 no-op 是读错了）；
  `contentEditable` 空属性读 `'true'` 且报错值是 `String(v)`；`autocapitalize`
  空串读 `''`、`off`→`none`；`preload` 空串读 `'auto'`；`crossOrigin` 未知值
  `'anonymous'`；video.width/height 是普通 unsigned long（刀5 误作维度类）；
  `img.longDesc/lowsrc` 是 URL；`img.border` null→`''`；`img.sizes` 引擎 getter
  读 undefined（刀5 只补了 setter）→整件替换；`img.referrerPolicy` 只认 5 关键字；
  媒体 `volume`/`defaultPlaybackRate`/`srcObject` 补 double 非有限 TypeError、
  [0,16] 区间、类型检查（`2.14748e+9` 的 %g 格式报错）；维度解析补
  `'9px'→9`/`'-3'→0`/溢出→0。
- **两条旧测试按实测 Chrome 修订**：`iframe.srcdoc = null` 写 `'null'`（旧测试
  按「可空」读成移除属性，错）；a/area 的 `relList.supports('preload')` 返回
  false 不抛（旧测试断言抛，错）。
- **新鲜度门扩容**：`check_fresh.sh` 现把 patch-common/media/reflect 三个生成件
  也纳入「生成件 vs 生成器」比对（4→7 件）；`MANIFEST.sha256` 补登记之前漏
  登的 6 个真值文件（含刀4/刀5）。

**KNOWN 入账**（测试 `htmlreflect_members_match_chrome_truth` 的棘轮，转绿即强制
摘除）：a/area `protocol` 无 href 时 Chrome 读 `':'`（URL 分解族）；HTMLElement
`editContext/innerText/outerText/style`；`Form.elements` 默认集合；媒体
`buffered/played/seekable`（TimeRanges）与 `currentTime/playbackRate`（double
转换+NotSupportedError 区间）；`Script.innerText` undefined（Trusted Types 联合）；
title/lang/accessKey 的 Symbol 赋值（刀2 搬位的原生 setter 不抛）；
`offsetHeight/Width` 默认值仅带布局的服务进程里有差，单测运行时一致。
STAGED（Chrome 145 实验/试用/旗标，不装）：interestForElement×2、
IFrame.featurePolicy/privateToken/adAuctionHeaders/browsingTopics/
sharedStorageWritable。

**坑**（三新）：①**探针页 URL 差异**：Chrome 临时服务器端口每次不同，第一版
src 的「差异」全是端口噪声——指纹必须归一化页面 URL/origin 才可比；②
**zsh 不对 `$var` 分词**：`for g in "..."; do cmd $g` 的参数被当成一个词，gen
报 IndexError，用显式逐条命令；③**raw 字符串里的 `\"`**：生成器里嵌 JS 引号
时 raw 与非 raw 三引号的转义层数不同，写错则 gen 语法错但旧输出仍在，易误判
「已生成」——生成后必看 stdout 的 wrote 行。

验收：指纹对拍 31 接口 22 项已入账差异外全等（instance routing/tokens/extra
全等）；tests **1439/0**（新增 htmlreflect_members_match_chrome_truth，修订
两条旧测试）；快照漂移恰 27 个原型行已重生成；check_fresh 7 件鲜（26 真值
全登记）；analyze ratchet 零违（平台模块 9→12）。差集表 575→**478**（-97，恰=
27 接口缺口名合计；行 115→89，仅余 IFrame.privateToken 为旗标特性）。下一刀
顺位：表单关联元素族（Input 23/Object 21/Output 7/FieldSet 7/Button 6/Select 6/
TextArea 6：约束验证 API+状态）或真家族（Animation 19/Notification 14）。

## M4 刀7：表单关联元素族（2026-10-30）

刀6 的指纹流水线延伸到 15 个表单/状态类元素接口（Input/Object/Output/FieldSet/
Button/Select/TextArea/Track/Template/Meter/Option/Marquee/Map/Progress/Slot），
关 **108** 个缺口名（478→370；行 89→74）。至此 HTML 元素接口的缺口只剩
Canvas（captureStream/transferControlToOffscreen）与 IFrame.privateToken（旗标）。

- **探针扩容**：htmlreflect-probe 的 TAGS 增 15 个接口（词表补 track kind、
  template shadowrootmode、button type；`'0'`/`zero` 取值）；新增**行为探针**
  `htmlform-probe.js`（36 键，真 Chrome 145）覆盖指纹分不了类的状态/方法成员，
  页面 URL/origin 归一为 `#B/#O`。
- **P.reflector 新 kind `ulong-def`**（textarea cols/rows、marquee
  scrollAmount/scrollDelay：带默认值的限定 unsigned long，非法/负/溢出/低于 min
  读默认值，set 越界写默认值）；分类器自动从指纹取默认值并试 min 0/1；`url` 增
  `fallbackDoc`（formAction：属性缺省时读 document.URL）。
- **生成器兜底改委托**：未归账成员不再让 gen 中止，而是把引擎链上已有的
  描述符**搬归属到 Chrome 接口**（`delegate`/`delegateFn`），缺的由对拍报
  missing；impl 表键改 `接口.成员` 命名空间（Meter.value/Progress.value/
  Output.value 同名）；FORCE 现在先 `delete` 引擎自有错实现再装；IDL 常量
  （Track.NONE…）同时定义在原型与构造器上。
- **又一批结构性路由修复**：Map/Marquee/Meter/Object/Output/Track 的
  `createElement` 实例是裸 HTMLElement，`htmlifaces` 增这 6 个（共 20）。
- **Object/Output/FieldSet 的约束验证面**：form_validation.js 比平台包先加载，
  那时这三个接口不存在；`htmlifaces` 把 `<button>` 上 form_validation 已装的
  描述符借过去（`this` 泛型；willValidate false、checkValidity true）。
- **引擎里真实存在的错实现（对拍抓获）**：`meter.min/max/value` 返回**字符串**
  且不夹逼；`progress.value/max` 同样；`select.type`/`textarea.type`/
  `fieldset.type` 把 type 属性当反射（Chrome 为只读常量）；`button.value`
  存 expando 不写属性；`select.size` 默认 1（Chrome 0）；`input.multiple`
  读 undefined；`track.kind('CAPTIONS')` 读 `'metadata'`；`output` 无
  defaultValue 语义；`popovertarget` 移除不清属性。全部按实测实现：Meter/
  Progress 为夹逼 double（`'9px'` 整串无效→默认，非有限 TypeError）、output 的
  default-value-override 标志、元素反射（置 null 移除属性，非 Element 抛
  `Failed to convert value to 'Element'.`）、marquee.loop 的 IndexSizeError 文案。
- 新增 `Select.length/item/namedItem/showPicker`、`Input.list/webkitEntries/
  showPicker/width/height(type=image)`、`Option.index`、`TextArea.defaultValue/
  textLength`、`FieldSet.elements`、`Map.areas`（`HTMLCollection._from` 快照，
  非实时，入账）、`Slot.assign`、`Track.readyState/track/常量`。

**KNOWN 入账**（棘轮转绿强制摘除）：Input 的 `files/selectionStart/End/type/
value/valueAsDate`、TextArea 的 `selectionStart/End/value`（值/选区状态机，
属输入状态族）；`Select.options` 默认集合、`Template.content` 的 set 面；
`FieldSet.elements`/`Map.areas` 为快照集合非实时；`Button.interestForElement`
为实验特性。

**坑**（一新）：**重启引擎的竞态**——`kill; sleep 1; start; probe` 当引擎还没监听
时探针 `Connection refused`，而对拍读的是**上一轮的旧 JSON**，会得出「diff 没变」
的假象。启动后必 `curl /health` 确认，且看探针的 `captured` 行。

验收：行为探针 36/36 全等；指纹对拍 46 接口除入账项外全等（routing/tokens/
extra 同）；tests **1440/0**（新增 htmlform_behaviors_match_chrome_truth）；
快照漂移恰 15 个原型行已重生成；check_fresh 7 件鲜（27 真值全登记）；analyze
零违。差集 478→**370**。下一刀顺位：Canvas 方法+真家族（Animation 19/
Notification 14/EditContext 11/传感器×6/SpeechRecognition 8/PointerEvent 16…）。

## M4 刀8：UI 事件族整体接管（2026-10-30）

UIEvent / MouseEvent / WheelEvent / KeyboardEvent / InputEvent / FocusEvent /
CompositionEvent / PointerEvent。这不是补洞：bootstrap 的旧类构造器只挑几个
字段（`o.screenX||0`），init 字典里没列的成员（movementX、keyCode、tiltX…）连
存的地方都没有，**行为探针 111 键里 92 键与 Chrome 不同**。新模块
`platform/uievent.impl.js` 用「每类字典成员表（类型+默认值）+ 一个共享运行时」
造出 8 个接口，探针 112 键 0 差异。差集 370→**321**（-49；行 74→65，连未直接
改的 TouchEvent 也因继承 UIEvent 而关行）。

- **真值** `uievent-truth-145.json`（真 Chrome 145，`uievent-probe.js` 112 键）：
  形状（自有成员序/种类/arity/常量/原型链）、构造器错误（缺参/无 new/非对象
  init）、**每个字典成员的类型转换**、派生成员、getModifierState、init* 旧式
  初始化器。
- **WebIDL 字典读取序**：祖先字典先、同字典内字典序；报错文案
  `Failed to construct 'MouseEvent': Failed to read the 'clientY' property from
  'MouseEventInit': The provided double value is non-finite.`（view 属
  UIEventInit，relatedTarget 属各自 Init，转 EventTarget/Window 的文案各异）。
- **转换类型（实测）**：`long`(ToInt32，detail/movementX/charCode/keyCode)、
  `ulong`(location/deltaMode)、`short`(button)、`ushort`(buttons，-3→65533)、
  **截断 double**(MouseEvent 的 screenX/clientX…：非有限抛 TypeError、2.7→2、
  4294967301 不回绕)、PointerEvent 的坐标保留小数、`float32`(pressure，
  2.7→2.700000047683716)、`DOMString?`(InputEvent.data：null/undefined 皆 null)。
- **怪癖逐项实测**：`which` 只在 UIEvent 原型上（UIEvent 自身恒 0；鼠标族
  =button+1；键盘=keyCode，与 charCode 无关）；`InputEvent.inputType` 只认
  Input Events 标准词表否则读 `''`；`modifierOS`/'OS' 恒 false；
  `initKeyboardEvent` 的 key 标识实参被忽略；`wheelDelta*` 为 delta 的截断而非
  ×-3；offsetX/layerX 在未派发事件上读 pageX。
- **PointerEvent 倾角↔球面角**：规格公式 + 位级对齐。V8 的 `Math.tan` 与 Chrome
  C++(libm) 在末位 ulp 不同（tan 30°），**tan 用 sin/cos、高度角用
  `atan2(1, sqrt(x²+y²))`、轴向用 `π/2-|tilt·rad|`** 才与 10 组真值逐位一致。
- **宿主集成**：旧子类（TouchEvent 等）运行期按原型链重新挂到新原型（其
  `super()` 经 [[GetPrototypeOf]] 动态解析，挂一次即可）；派发枢纽
  `event_hooks.js` 对 `offsetX/Y` 的赋值改走隐藏钩子 `__diting_uiOffset`
  （成员现为只读 getter，赋值会静默失败而退回 0——#663 的回归）。

**坑**：①V8 `Math.tan` 与 libm 差末位——凡要「位级一致」的三角计算先用 Chrome
真值反推公式变体，不要凭规格推导；②`P.expose` 的名字要登记进模块
`exposes:` 清单，否则 analyze 报 expose-not-listed（模块不得直接写全局）；
③macOS `sed -i` 语法与 GNU 不同，改文件一律用 python。

验收：探针 112 键 0 差异；tests **1441/0**（新增
uievent_family_matches_chrome_truth）；快照漂移 8 个原型行（旧类把 view/detail
错放在 Composition/Keyboard/Mouse 原型上，现回 UIEvent）；check_fresh 7 件鲜
（28 真值）；analyze 零违。下一刀顺位：真家族（Animation 19/Notification 14/
EditContext 11/传感器×6/SpeechRecognition 8）、ClipboardChangeEvent 18。

## M4 刀9：Generic Sensor 家族（2026-10-30）

Sensor / Accelerometer / LinearAccelerationSensor / GravitySensor / Gyroscope /
OrientationSensor / AbsoluteOrientationSensor / RelativeOrientationSensor /
SensorErrorEvent（9 接口）。差集 321→**275**（-46；行 65→59）。新模块
`platform/sensor.impl.js`，探针 `sensor-probe.js` 43 键 0 差异。

- **Chrome 145（无传感器硬件）的真面**：所有传感器可构造；`start()` **异步**触发
  **trusted** 的 `SensorErrorEvent`（`DOMException NotReadableError`，
  "Could not connect to a sensor"，`error.code` 0，不冒泡不可取消）；永不
  `activated`、永无读数（x/y/z/quaternion/timestamp 为 null）。**Magnetometer 与
  AmbientLightSensor 在 Chrome 145 不存在**——引擎也不定义（'多'表不变）。
- 行为：二次 `start()` 只出一个 error；`stop()` 取消待发的 error；error 之后可
  重新 start；`populateMatrix` 参数检查（缺参/类型联合/Float 数组长度 <16 抛
  `Target buffer must have at least 16 elements.`，合法则抛 NotReadableError
  `Sensor data is not available.`）；构造选项字典 `SensorOptions{frequency:
  double}`←`SpatialSensorOptions{referenceFrame: LocalCoordinateSystem}`，祖先先读，
  报错文案逐字（枚举错 `... is not a valid enum value of type
  LocalCoordinateSystem.`）；`SensorErrorEvent` 需 2 参、`error` 必填且须
  DOMException（`Required member is undefined.`）。
- 抽象类 `Sensor`/`OrientationSensor` 构造抛 `Illegal constructor`；x/y/z 只属
  Accelerometer/Gyroscope 家族、quaternion/populateMatrix 只属 OrientationSensor
  家族（跨类 getter 调用抛 Illegal invocation）。

**宿主前置（最小例外）**：UA 事件的 `isTrusted=true` 只能经原生 Event 状态
facade `__diting_event_state` 设置，而 bootstrap 在烘焙时把它从全局删掉。现在
在删除前一行存入**词法**常量 `_eventState`（仍不是全局属性，页面 JS 看不见），
平台模块经 `legacy:_eventState` 使用——这是之后所有 UA 派发事件（SpeechRecognition、
Notification…）的通路。bootstrap 因此 +4 行（含注释），属「既有缺陷最小例外」。

**已知核心级偏差（不在本刀）**：IDL `on*` 处理器由引擎派发核心**先于**监听器触发
（event_hooks 注释已记「handler 后赋值仍先触发」），Chrome 按注册序。探针把
处理器触发与监听器日志分开记录，两者顺序不比对。

验收：探针 43 键 0 差异；tests **1442/0**（新增 sensor_family_matches_chrome_truth）；
快照漂移 6 个原型行（Accelerometer/Gyroscope 获 x/y/z，Sensor/OrientationSensor/
SensorErrorEvent 由空桩变真身）；check_fresh 7 件鲜（29 真值）；analyze 零违。下一刀
顺位：Notification 14、Animation 19、EditContext 11、SpeechRecognition 8、
ClipboardChangeEvent 18。

## M4 刀10：Web Notifications（2026-10-30）

`Notification`（单接口，旧实现是空壳：只有静态 permission/requestPermission 和一个假
`image` getter）。差集 275→**262**（-13；行数不变——`Notification.image` 一行见下）。
新模块 `platform/notification.impl.js`，探针 `notification-probe.js` 42 键全等。

- **Chrome 145 headless 真面**：`Notification.permission` 起始 `default`；**首次
  `requestPermission()` 结算 `denied` 且从此固定**（无提示 UI；回调与 Promise 都给
  `denied`，非函数实参抛 `parameter 1 is not of type 'Function'.`）；没有授权时
  `new Notification()` 在**后一个任务**触发 **trusted** 的普通 `error` Event（不冒泡、
  不可取消、target=该通知），**即便期间已 `close()` 也照发**，永不 show。
- **NotificationOptions 字典**：按 WebIDL 字典序读取；`title` 走 DOMString（null→'null'）；
  `icon/badge` 为 USVString 并**解析成绝对 URL**（空串保持空）；`timestamp` 是
  `unsigned long long`（负数/溢出按 2^64 回绕：-5→18446744073709552000，1e20→
  7766279631452242000，NaN/null→0，默认 Date.now()）；`vibrate` 是冻结数组且缓存
  （数字→[n]，非法→[0]）；`data` 在构造时**结构化克隆并缓存**（`data !== 原对象`，重复读
  同一个；函数/Symbol 抛 `DataCloneError`，文案以被拒值命名：
  `Failed to construct 'Notification': function () {} could not be cloned.`）；`dir` 枚举
  区分大小写；`actions` 非空抛"仅支持 ServiceWorkerRegistration.showNotification()"；
  `silent`+`vibrate`、`renotify` 无 tag 各有专门 TypeError。
- 形状：原型自有序 `onclick,onshow,onerror,onclose,title,dir,lang,body,tag,icon,badge,
  vibrate,timestamp,renotify,silent,requireInteraction,data,actions,close,constructor`；
  静态 `permission`、`maxActions`(=2)、`requestPermission` 依序；实例无自有属性；
  **`image` 不是实例成员**（规格与 BCD 都列了它，但 Chrome 145 实测没有——本表该行即此
  假阳性，以真 Chrome 为准不装）；`NotificationEvent` 在 window 上不存在。
- 第二个使用 `_eventState` 的模块（trusted `error` 事件）。

**坑**：①**活引擎的 /eval 会在同一运行时把脚本执行两遍**，带副作用的状态（这里是
requestPermission 把权限固定成 denied）在第二遍已变——对拍到 `permission.initial` 读
`denied` 是探测环境假象，**有状态探针以 Rust 单测（新运行时、单次执行）为准**；②Chrome
的 DataCloneError 文案命名被拒值，而引擎 structuredClone 的是通用文案——本模块在边界处
重建（沿对象/数组/Map/Set 找第一个函数/Symbol）；structuredClone 本身的文案属另一刀。

验收：探针 42 键全等（单测）；tests **1443/0**（新增 notification_matches_chrome_truth）；
快照漂移 1 行（Notification 空桩→真身，并去掉引擎多出的假 `image`）；check_fresh 7 件鲜
（30 真值）；analyze 零违。下一刀顺位：Animation 19、EditContext 11、SpeechRecognition 8、
ClipboardChangeEvent 18、RTCIceCandidate 16、VideoFrame 10、URLPattern 9。

## M4 刀11：Web Speech 识别族 + ClipboardChangeEvent（2026-10-30）

SpeechRecognition / SpeechRecognitionEvent / SpeechRecognitionErrorEvent /
SpeechGrammarList / SpeechGrammar / SpeechRecognitionPhrase + 5 个 `webkit*` 别名 +
ClipboardChangeEvent。差集 262→**233**（-29；行 59→56；已核接口 196→199——新装的
接口进入了探测范围）。新模块 `platform/speech.impl.js`，探针 `speech-probe.js` 74 键
全等（单测）。

- **Chrome 145 headless 真面**：无识别器无权限 UI。`start()` 在后一任务触发 **trusted**
  的 `SpeechRecognitionErrorEvent{error:'not-allowed'}` 再触发 trusted 的普通 `end`
  （**没有 `start` 事件**）；第二次 `start()` 抛 `InvalidStateError ... recognition has
  already started.`；`stop()`/`abort()` 在已 start 时取消待发的 error、只剩 `end`，未
  start 时什么都不发；error 后可重新 start。
- **`webkit*` 是同一个对象**（`webkitSpeechRecognition === SpeechRecognition`，
  `.name` 仍为 'SpeechRecognition'，非枚举）；`SpeechRecognitionResult/ResultList/
  Alternative` 在 window 上**不暴露**，所以 `SpeechRecognitionEvent.results` 只可能是
  null（传非 null 抛 `Failed to convert value to 'SpeechRecognitionResultList'.`）。
- 实例/静态面：原型自有序含 `onaudiostart…onend`(11 个 handler)、`abort,start,stop,
  processLocally,phrases`；静态 `available`/`install`（on-device 语音包）：`available`
  需 `{langs}`（缺/空/非序列各有专门文案），`processLocally:false`→`'available'`，
  本地且是常见语言→`'downloadable'`，未知语言→`'unavailable'`；`install` 对
  downloadable 抛 `NotAllowedError ... Requires handling a user gesture`，其余 resolve false。
- **SpeechGrammarList**：自有可枚举下标属性（`Object.keys` 为 "0","1"…，空表 `0 in g`
  false）；`addFromString` 把文本编成 `data:application/xml,<encodeURIComponent>`，
  `addFromUri`（Chrome 145 拼写是 `Uri`，无 `URI`）把 src 解析为绝对 URL；weight 是
  **float32**（0.2→0.20000000298023224；非有限抛 float TypeError；不夹逼：3/-1 保持）。
- **SpeechRecognitionPhrase(phrase, boost=1)**：boost 范围 [0,10] 外抛 `SyntaxError
  ... must be inside the range [0, 10].`。**SpeechRecognitionErrorEvent.error 是自由
  DOMString**（'zzz'、5→'5'、null→'null' 都原样，缺省 ''，不校验枚举）。
- **ClipboardChangeEvent 的怪构造器**：签名 `(optional init)`，**没有 type 参数**（type
  恒 `clipboardchange`，`length` 0——传字符串当第一个参数会被当字典而抛）；`changeId`
  是 **BigInt**（有符号不回绕：-1n、2n**70n 原样；字符串 '7'→7n；数字抛
  `Cannot convert 5 to a BigInt`）；`types` 是 `sequence<DOMString>`，**每次读返回新的
  冻结数组**（`e.types !== e.types`），构造时拷贝（改原数组不影响）。

**坑**：①ClipboardChangeEvent 第一轮探针全部抛"not of type Init"——因为我按 Event 惯例
传了 type 字符串；构造器签名是真值的一部分，**先 `length`、再试无参，别默认 `(type, init)`**；
②探针 JSON 序列化遇 BigInt 会抛，需 replacer 标记（'BIGINT:n'）；③本刀的 on-device 语言表
是**近似**（实测只有 en-US/zh-CN→downloadable、xx-ZZ→unavailable 三点，其余按常见语言
前缀猜），属已知近似。

验收：探针 74 键全等；tests **1444/0**（新增 speech_family_matches_chrome_truth）；快照漂移
18 行（旧空桩→真身，含 5 个 webkit 别名）；check_fresh 7 件鲜（31 真值）；analyze 零违。
下一刀顺位：URLPattern（旧实现是 test()恒false 的桩，真实现是完整 URL Pattern 标准，体量
一整刀）、Animation 19、EditContext 11、RTCIceCandidate 16、VideoFrame 10。

## M4 刀12：URLPattern 真实现（2026-10-30）

旧 bootstrap 类是惰性桩（`test()` 恒 false、`exec()` 恒 null、没有任何组件 getter）。新模块
`platform/urlpattern.impl.js` 按 WHATWG URL Pattern 标准实现全套：分词器、模式串解析器、各组件
规范化、正则与模式串生成、构造串解析器（状态机）、匹配。差集 233→**224**（-9），探针
`urlpattern-probe.js` **900 键 0 差异**。

- **语料驱动对拍**：约 150 个构造用例（字符串/字典/baseURL/选项/各组件错误输入）+ 741 个
  「模式 × 输入」匹配矩阵（test() 与完整 exec() 结果：inputs、各组件 groups 与 input、
  字典序成员序）。真值取自真 Chrome 145，报错文案是 liburlpattern 风格逐字
  （`Invalid pathname pattern '/('. Unbalanced regex at index 1.`、`Unexpected '}' '}' at
  index 2, expected end of pattern.`、`Duplicate group name 'a' at index 6.` 等）。
- **构造器重载与转换**：`null` 输入视为空字典；三参选 (input, baseURL, options) 重载，第二参
  **恒转字符串**（`undefined`→`'undefined'`，报 `Invalid baseURL 'undefined'.`）；字典输入
  配字符串 baseURL 抛 `Invalid second argument baseURL ... Use the URLPatternInit.baseURL
  property instead.`；字典成员按字典序读取并转 USVString。
- **标准里的易错点（对拍抓获）**：`*` 是修饰符还是通配符取决于前面有没有名字 token
  （`:a*` = 名字 + 修饰符，`(.*)` 才是匿名通配）；hostname 规范化在 `/ ? # \` 处截断
  （引擎 URL 的 hostname setter 不截断，须预处理）；`isSearchPrefix` 在 token 0 时前一个
  token 不存在应为 true；`exec().inputs[0]` 是字典**副本**不是原对象；协议大小写不敏感地判
  默认端口（HTTPS+443 → 端口 ''）。
- 引擎 URL 的依赖面：组件规范化复用 `new URL()` 与其 setter（username/password/hostname/
  pathname/search/hash），pathname 用 '/-' 前缀技巧避免相对路径被解析；IPv6 主机名、opaque
  路径、port 数字前缀语义单独实现。

**坑**：①单测页的 origin（`http://example.com`）恰与语料里的输入字面量重名，探针对页面 URL
的归一化把输入回显改成了 `#O`——**探针语料的域名不得与测试页 origin 重名**，单测改用
`probe.test`；②WebIDL 字典→JS 对象的成员序是字典序（hash,hostname,inputs,password,…），
自建结果对象必须按该序；③分词器宽松模式（构造串）与严格模式（模式串）的报错路径不同，
宽松模式把非法字符变成 invalid-char token 继续走状态机。

验收：探针 900 键 0 差异（单测）；tests **1445/0**（新增 urlpattern_matches_chrome_truth）；
快照漂移 1 行（URLPattern 桩→真身）；check_fresh 7 件鲜（32 真值）；analyze 零违。下一刀顺位：
Animation 19、EditContext 11、RTCIceCandidate 16、VideoFrame 10、XSLTProcessor 8。

## M4 刀13：RTCIceCandidate（2026-10-30）

旧 bootstrap 类只存了 `candidate` 一个字段。Chrome 把 candidate-attribute 串解析成
foundation/component/priority/address/protocol/port/type/tcpType/relatedAddress/relatedPort，
**解析是全有或全无**——任何一个 token 不合法，所有解析字段全为 null。新模块
`platform/rtcice.impl.js`，探针 `rtcice-probe.js` 72 键 0 差异（43 条 candidate 语料 + 构造器转换/
校验 + toJSON）。差集 224→**208**（-16）。

- **解析规则（语料实测）**：可带 `a=` 前缀与结尾 CRLF；**只容单空格**（`candidate: 8` 与多空格整体
  失败）；`typ` 与类型关键字必须小写（`TYP HOST` 整体失败）、类型限 host/srflx/prflx/relay；传输层
  大小写不敏感且**归一为小写**，仅 udp/tcp（sctp 整体失败）；端口 ≤65535、优先级 ≤4294967295 且
  纯数字；`component` 1→'rtp'、2→'rtcp'、其它数字→null（但其余字段仍解析成功）；裸 IPv6 地址
  **自动加方括号**；扩展属性（ufrag、generation、network-id/cost、未知词、`relay-protocol`、`url`）
  **全部忽略**；`tcptype` 仅 active/passive/so；`raddr`/`rport` 给一个时另一个取 `''`/`0`，都没给
  则两者为 null。
- **`usernameFragment` 永远只来自 init**（候选串里的 `ufrag` 不解析）；`relayProtocol`、`url` 实例上
  恒为 null。
- **构造器**：`init` 缺省/null 为空字典，非对象抛 `not of type 'RTCIceCandidateInit'`；字典成员按
  字典序读取；`candidate` 为 DOMString（null→'null'）；`sdpMLineIndex` 为可空 unsigned short
  （-1→65535、70000→4464、1.9→1、NaN/对象→0）；`sdpMid`/`usernameFragment` 为可空 DOMString；
  `sdpMid` 与 `sdpMLineIndex` 同时为 null 抛 `sdpMid and sdpMLineIndex are both null.`。
- **toJSON**：`{candidate, sdpMid(null→''), [sdpMLineIndex 仅非 null 时存在], usernameFragment(null→'')}`，
  键序为声明序；`JSON.stringify` 同。
- webrtc.js 的 `__emitCand` 内部构造（`sdpMid:'0', sdpMLineIndex:0`）与新类兼容。

**坑**：①Chrome 的候选串解析是**严格的全有或全无**——凭"能解析多少解析多少"的直觉实现会处处与真值
不符，必须以失败语料（空格、大小写、范围、缺 token）为准；②`relayProtocol`/`url` 看似该从串里取，
实测恒 null（它们只在本地候选上有值）——**别按规格字段名推断来源，按语料证据**。

验收：探针 72 键 0 差异（单测）；tests **1446/0**（新增 rtcice_matches_chrome_truth）；快照漂移 1 行；
check_fresh 7 件鲜（33 真值）；analyze 零违。下一刀顺位：Animation 19、EditContext 11、VideoFrame 10、
XSLTProcessor 8、RTCPeerConnection 15。

## M4 刀14：DOM/事件小成员扫尾（2026-10-30）

差集里一批 1–4 个名的缺口，接口形状已有、缺行为：差集 208→**188**（-20；行 54→42）。
新模块 `platform/domsmall.impl.js`，探针 `smallmembers-probe.js` 51 键 0 差异（形状按「Chrome 的名字
都在 + 共有名相对序一致 + 父类一致」比较）。

- **节点 mixin**：`CharacterData.nextElementSibling/previousElementSibling`（Chrome 里在
  CharacterData 上，Text/Comment/PI 继承）、`Text.assignedSlot`(null)、
  `ProcessingInstruction.sheet`(null)、`DocumentFragment.childElementCount`。
- **四个空桩事件换真身**：`HashChangeEvent{oldURL,newURL}`（USVString，null→'null'）、
  `SubmitEvent{submitter}`（只收 HTML 命名空间的 HTMLElement——引擎的 SVG 元素
  `instanceof HTMLElement` 为真，须按 namespaceURI 判；转换失败文案 `Failed to convert value
  to 'HTMLElement'.`）、`PopStateEvent{state, hasUAVisualTransition}`、
  `ClipboardEvent{clipboardData: DataTransfer|null}`；共用一个小工厂（字典序读取+Chrome 文案）。
- **真实表单提交**：`requestSubmit()`/按钮点击派发的 `submit` 以前是普通 `Event`；现在是
  **`SubmitEvent`，带 submitter、isTrusted=true**（bootstrap 一处最小改，用 `_eventState`）。
- **MessageEvent**：补 `initMessageEvent`(长度 1，参数默认 data null/origin ''/source null/
  ports [])与 `userActivation`(null)，按 Chrome 序重排。
- **DataTransfer**：补 `setData/getData/clearData`（格式小写；'text'→text/plain、'url'→
  text/uri-list；同类型 set 替换；`getData('url')` 返回 uri-list 首个**合法 URL 的序列化**
  （'http://x'→'http://x/'）；`types` 为冻结数组且内容不变时**同一对象**），挂在旧类的
  `_dtItems` 存储上并按 Chrome 序重排。
- **RTCSessionDescription**：旧类只是两个赋值；新类 `type`/`sdp` **可读写**（type 默认 null，
  非枚举值的 setter 忽略；构造器枚举错抛 `... valid enum value of type RTCSdpType.`）、`toJSON()`。
- **InputDeviceCapabilities**：`{firesTouchEvents}` 默认 false；让 UIEvent 的 `sourceCapabilities`
  终于能装真对象。

**本刀入账的 KNOWN（节点层 placement 族）**：Text/Comment/ProcessingInstruction/DocumentFragment
实例原型上，引擎多挂了 `nodeName/nodeType/cloneNode/innerHTML/nodeValue`（Chrome 在
Node.prototype）；`DocumentFragment`/`ProcessingInstruction` 构造器 `length` 不同。
**本刀有意不做**（各自成族，留后续）：`Headers.getSetCookie`（旧 Headers 是个会覆盖重复值的
对象表，须整族重写含排序/校验）、`ImageData`（原型上无 width/height/data）、`Highlight`、
`FontFace` 覆盖项、`TaskController`/`TaskSignal`、`CloseWatcher`、`QuotaExceededError`、
`MediaStream` 缺 id/active/getTrackById、`Path2D.roundRect`。

**坑**：①**改了 impl 必须重跑 bundle.py**（Rust 测试嵌入的是 platform_bundle.js，不是 impl
源文件——忘了会拿旧代码对拍并得到「修了没生效」的假象）；②按 Chrome 序重排某原型时，**目标成员
若由后加载的模块再安装就会落到末尾**（moveBefore 在 gen/htmlelement 里装）——用
`requires-load` 让重排模块排在安装者之后；③引擎的 SVG 元素继承 HTMLElement，凡是「须为 HTML 元素」
的转换都要看命名空间。

验收：探针 51 键 0 差异；tests **1447/0**（新增 smallmembers_match_chrome_truth）；快照漂移 13 行；
check_fresh 7 件鲜（34 真值）；analyze 零违。下一刀顺位：Headers/Request/Response 整族、
Animation 19、EditContext 11、ImageData/Highlight/FontFace、VideoFrame 10。

## M4 刀15：Headers（2026-10-30）

`platform/headers.impl.js`（模块 `headers`，L2）整类换真身。旧 Headers 是以小写名为键的对象表，
`append` 会**覆盖**重复值、不校验名/值、迭代按插入序、没有 `getSetCookie`。探针
`headers-probe.js`（75 键，真 Chrome 145，`headers-truth-145.json`）：构造语料（record/序列/
Headers-like/坏形状及逐字措辞）、名/值合法性与归一化、同名合并、排序且**活**迭代、
set-cookie 分列 + `getSetCookie`、迭代器协议、JSON/own 形状。

- 头列表按插入序存 `{name(小写), value}`；`get` 同名以 `', '` 合并；迭代走「排序后合并」视图，
  set-cookie 条目永不合并；迭代器/`forEach` 每步重算视图并按下标取（迭代中追加的头会被访问、
  删除会让后续前移——与 Chrome 一致）。
- ByteString 转换拒 >255 字符（`String contains non ISO-8859-1 code point.`）；名须匹配 token
  （`Invalid name`）；值先裁 HTTP 空白，再遇 NUL/CR/LF 抛 `Invalid value`。
- 迭代器原型 toStringTag=`Headers Iterator`，只有 `next`，接在 %IteratorPrototype% 上。

**本刀入账的 KNOWN（Request/Response 族，下一刀）**：`headers.from.request`、
`headers.from.response`、`headers.response.ct`、`sc.response` 四键——Request/Response 把
`headers` 做成**自有属性**而 Chrome 是原型 getter；Response 对 string/Blob/URLSearchParams
体不补默认 content-type；Response 守卫不滤 Set-Cookie。另 Request 缺 7 个成员：bodyUsed、
destination、duplex、integrity、isHistoryNavigation、keepalive、targetAddressSpace。

验收：探针 75 键中 71 键 0 差异、4 键 KNOWN；新增 headers_match_chrome_truth 过；缺口 188→187
（Headers.getSetCookie 出表）；check_fresh 7 件鲜；analyze 零违。**环境备注**：本机 diting lib
全量 908 过/110 败，失败集与干净 HEAD（36bfac4）逐条相同——根因是 font_size_zero_text_occupies_nothing
与 viewport_override_drives_media_queries_and_survives_navigation 两个页面测试先败后毒化共享锁
（PoisonError 连带），与本刀无关；待单独查。下一刀顺位：Request/Response 整族、Animation 19、
EditContext 11、ImageData/Highlight/FontFace、VideoFrame 10。

## M4 刀16：Request / Response（2026-10-30）

`platform/fetchapi.impl.js`（模块 `fetchapi`，L2）整族换真身，旧的 bootstrap 两个类连同
`_requestBodyBytes`/`_formDataFromString` 一并删除（bootstrap 净减 ~200 行）。探针
`reqres-probe.js`（96 键，真 Chrome 145，`reqres-truth-145.json`）0 差异：原型形状与成员序、
`headers` 改**原型 getter**（旧：自有数据属性）、构造校验与逐字措辞（URL 解析/带凭据、method token 与
CONNECT/TRACE/TRACK、枚举成员按字典序读取、GET/HEAD 带 body、mode:navigate、referrer 跨源回
`about:client`、signal 须为 AbortSignal 且 Request.signal 是**跟随**输入信号的新对象、status 取
unsigned short 后须在 [200,599]、204/205/304 不能带 body、statusText 字节串+字符集校验）、体推导
Content-Type（string→text/plain;charset=UTF-8、Blob→其 type、URLSearchParams→urlencoded、FormData→
multipart、ArrayBuffer/流→无）、`bodyUsed`/locked/disturbed 语义（读取 body 流或任一 reader 方法即置位，
null 体永不置位）、clone（字节体复制、流体 tee、已用抛 `... body is already used`）、
Response.error/redirect/json 全套、`bytes()`。**text() 不再按 Content-Type charset 解码**（Chrome
恒为 UTF-8；探针 `res.text.gbk` 为证）。

- **头守卫**落在 Headers 模块：`request`（丢弃禁用请求头名与 `proxy-`/`sec-` 前缀）、
  `request-no-cors`（只放 CORS 安全清单名/值）、`response`（丢弃 Set-Cookie/Set-Cookie2）、
  `immutable`（Response.error()/redirect() 抛 `Headers are immutable`）。Headers 因此导出
  内部面 make/has/add/copy/guard/addLenient 给 fetchapi 用。
- **fetch.js 两个隐藏钩子**（入 hide list）：`__diting_mkResponse(body, {status,statusText,headers,
  type,url,redirected})` 造网络 Response（公共构造器按 Chrome 忽略 type/url/redirected）；
  `__diting_reqTake(req)` 异步取 Request 体字节并置已用（fetch(request) 复用已用 Request 现按 Chrome 抛
  TypeError）。fetch(request) 现在也带上 Request.headers（旧版丢弃，含体推导的 content-type）。

**坑**：①Response 头守卫会吃掉 Set-Cookie，网络造的头里非法名要**跳过不抛**（addLenient），否则一个怪头杀整个响应；
②Request 体来自输入 Request 时要把输入置已用；③`Promise` 返回方法的坏接收者应返回 rejected promise，不是同步抛。

验收：探针 96 键 0 差；新增 reqres_match_chrome_truth；headers 测试的四个 KNOWN 全部转绿；缺口 187→180
（Request 的 7 个缺失成员出表）；check_fresh 7 件鲜（36 真值）；analyze 零违。全量（跳过两个先败毒化共享锁的页面
测试后）987 过/30 败，失败集与干净 HEAD 逐条相同（环境类：CSS/布局/计算样式），`test_local_fetch_resolves_on_task_boundary`
为时序型，偶发于满载全量、单跑 3/3 过。下一刀候选：Animation 19、EditContext 11、VideoFrame 10、XSLTProcessor 8、
RTCPeerConnection 15、ImageData/Highlight/FontFace 等。

## M4 刀17：Web Animations（2026-10-30）

`platform/animation.impl.js`（模块 `animation`，L2）一次落六个接口：Animation / AnimationEffect /
KeyframeEffect / AnimationTimeline / DocumentTimeline / AnimationPlaybackEvent，并让 `document.timeline`
有真对象。此前这些全是构造器桩，`Element.animate` 返回 web_api_gaps.js 里手写的对象字面量（不是 Animation
实例、没有 pending、没有 KeyframeEffect）。探针 `anim-probe.js`（89 键，真 Chrome 145，
`anim-truth-145.json`）0 差异：六接口形状；EffectTiming 校验与逐字措辞（`duration must be non-negative or
auto` / `iterationCount must be non-negative` / `iterationStart must be non-negative` / easing 与枚举错误
的字典名 `EffectTiming`）；关键帧规整（数组式与属性索引式、computedOffset 补全、offset 区间与单调检查、
easing/composite 校验）；`getComputedTiming` 按 Chrome 以**秒**做加法再乘 1000（endTime 215.00000000000003）；
play/pause/finish/cancel/reverse/updatePlaybackRate 状态机与 `pending`、ready/finished promise 的同一性
与重建时机、finish/cancel/remove 事件、replaceState、`getAnimations`、`commitStyles`。

- **状态机**：startTime/holdTime/playbackRate/pendingPlaybackRate + 待决任务（play|pause|rate）；任务由约
  16ms 的“帧”定时器完成（Chrome 在下一帧 resolve ready）。currentTime 与 playState 皆派生。自然结束只
  用一个按剩余时间武装的定时器；finish/cancel/remove 事件同样在帧上派发。`playState` 的 finished 判断用
  **有效速率**（pending rate 优先）——`reverse()` 后速率读数仍是旧值而状态已按新方向算，正是 Chrome 的行为。
- **胶水**：`Element.animate`/`getAnimations`（及 Document/ShadowRoot）在 web_api_gaps.js 里只剩两行
  钩子调用（`__diting_animate` / `__diting_getAnimations`，已入 hide list），并继续调用原生 animate 以保留
  既有 animationstart/animationend 事件（test_element_animate_lifecycle_events 仍过）；旧的 ~60 行手写
  对象删除。on<finish|cancel|remove> 只存函数，派发由引擎的 on* 属性机制完成（再挂监听会双触发）。

**坑**：①引擎 EventTarget 派发本身会调 `on<type>` 属性——属性访问器里再 addEventListener 会让处理器触发两次；
②`performance.now()` 在满载下偶尔读到略小于 0，timeline.currentTime 须钳到 ≥0；③探针里凡与帧定时器赛跑的等待
（事件、remove）要给 ≥300ms 余量，否则满载全量偶发误报。

验收：探针 89 键 0 差；新增 animation_matches_chrome_truth；缺口 180→158（Animation 19、DocumentTimeline 2、
Document.timeline 1 出表）；check_fresh 7 件鲜（37 真值）；analyze 零违。全量（跳两个毒化页面测试）988 过/30 败，
失败集与干净 HEAD 逐条相同；连跑 4 次动画测试 0 误报。**留后**：ScrollTimeline/ViewTimeline、
rangeStart/rangeEnd 真值（现为恒 null）、复合效果（add/accumulate 的真实叠加）、CSSAnimation/CSSTransition
真对象。下一刀候选：EditContext 11、VideoFrame 10、XSLTProcessor 8、RTCPeerConnection 15、ImageData/Highlight/FontFace。
