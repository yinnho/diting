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
成员级已核 196 接口。同日晚刷新（M3 事件线+M4 刀1 几何族落表）：缺口 163→**142 接口**、
2631→**2471 成员**；出表的=M3 原生事件核的 cancelBubble/returnValue 等全线 + 几何六件（见差集表）。已确认的真缺口样例：
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
