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
成员级已核 196 接口。同日晚刷新（M3 事件线+M4 刀1 几何族落表）：缺口 163→142 接口、2631→2471 成员（M4 刀2 再收：2471→1043；刀3 继收：1043→992；刀4 继收：992→652；刀5 继收：652→575；刀6 继收：575→478；刀7 继收：478→370；刀8 继收：370→321；刀9 继收：321→275；刀10 继收：275→262；刀11 继收：262→233；刀12 继收：233→224；刀13 继收：224→208；刀14 继收：208→188；刀15 继收：188→187；刀16 继收：187→180；刀17 继收：180→158；刀18 继收：158→143；刀19 继收：143→135；刀20 继收：135→107；刀21 继收：107→**102**，见文末）；出表的=M3 原生事件核的 cancelBubble/returnValue 等全线 + 几何六件（见差集表）。已确认的真缺口样例：
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

## M4 刀18：EditContext 族（2026-10-30）

`platform/editcontext.impl.js`（模块 `editcontext`，L2）一次落五个接口：EditContext / TextFormat /
TextUpdateEvent / TextFormatUpdateEvent / CharacterBoundsUpdateEvent，外加 `HTMLElement.editContext`。此前全是
无成员的构造器桩。探针 `editctx-probe.js`（49 键，真 Chrome 145，`editctx-truth-145.json`）首轮 0 差异：
EditContextInit 转换与措辞；`updateText` 反向区间交换、两端夹到缓冲长度；`updateSelection` 夹取但保序；
text/selection 只读；`characterBounds()` 每次返回新数组与新 DOMRect；`updateControlBounds`/`updateSelectionBounds`/
`updateCharacterBounds` 的 DOMRect 与参数个数校验；五个 on* 访问器（非函数落 null）；`element.editContext` 的
挂接规则（一个上下文只挂一个元素→NotSupportedError、不支持的元素→NotSupportedError、非 EditContext→
TypeError、赋 null 脱挂）；TextFormat 的枚举（UnderlineStyle/UnderlineThickness）与只读成员；三个事件的
init 字典（字典序读取、unsigned long 回绕）与 `getTextFormats()` 每次新数组。

**范围**：这是**对象面**，不含「输入法/软键盘驱动 EditContext 派发 textupdate」那条路径——引擎无输入法，
所以事件只能由页面自己 dispatch；真输入链路属于输入栈，另议。

验收：探针 49 键 0 差；新增 editcontext_matches_chrome_truth；缺口 158→143（EditContext 11、TextFormat 4 出表）；
check_fresh 7 件鲜（38 真值）；analyze 零违。全量（跳两个毒化页面测试）989 过/30 败，失败集与干净 HEAD 逐条相同。
下一刀候选：VideoFrame 10 + VideoColorSpace 5、XSLTProcessor 8、IntersectionObserverEntry 8、ViewTimeline/
ScrollTimeline 11、RTCPeerConnection 15。

## M4 刀19：IntersectionObserver + IntersectionObserverEntry（2026-10-30）

`platform/intersection.impl.js`（模块 `intersection`，L2）整体接管：IntersectionObserver 连同其调度逻辑从
bootstrap.js 迁出（bootstrap 净减 ~110 行），IntersectionObserverEntry 由空类换成真接口。旧面：原型里带
`_fireFor`、构造器 length 2、观察者回调收到**无原型的对象字面量**、rect 不是 DOMRectReadOnly、entry 类是空壳。
探针 `ioentry-probe.js`（31 键，真 Chrome 145）0 差异：**Chrome 145 的 entry 构造器是非法的**（`new` 与直接调用
措辞不同：前者 `Failed to construct '…': Illegal constructor`，后者裸 `Illegal constructor`），八个成员是原型
getter、无 own 属性、无 toJSON；观察者投递的 entry 是真实例，rect 皆 DOMRectReadOnly，rootBounds 为视口矩形；
观察者校验：回调须为 Function、init 类型、root 须 Element/Document、rootMargin/scrollMargin 展开为四段并校验
（无单位 0 抛 SyntaxError `rootMargin must be specified in pixels or percent.`）、threshold 范围检查
（RangeError）与非有限数（TypeError）、thresholds 存 float32 升序且每次读取返回**新的冻结数组**、回调
`(entries, observer)` 且 this===observer、observe/unobserve 的 Element 与参数个数校验。

**行为策略不变**（只是搬家）：无布局引擎，被观察目标一律视为完全在视口内（isIntersecting true、ratio 1），
observe 后微任务触发一次 + 指数退避 120/500/1500/3500/7000ms 再触发，DOM 变动（防抖）重触发全部存活观察者，
单观察者 256 次封顶。几何值（intersectionRect 等）仍是目标自身 rect——**不是**真交叠计算，真交叠属后续布局相关刀。

**坑**：①用 Python 做字符串替换时 `s.index(a):s.index(b)` 若 b 先于 a 出现会得空串，`replace('', new, 1)` 会把新文本
塞到文件开头（本刀踩过，ResizeObserver 里有同名片段）——替换前断言 old 非空；②迁移类后要清掉 bootstrap 里对该名字的
加载期引用（`_markNative` 列表），否则快照构建期 ReferenceError。

验收：探针 31 键 0 差；新增 intersection_matches_chrome_truth；缺口 143→135（IntersectionObserverEntry 8 出表）；
check_fresh 7 件鲜（39 真值）；analyze 零违。全量（跳两个毒化页面测试）990 过/30 败，失败集与干净 HEAD 逐条相同。
下一刀候选：ScrollTimeline/ViewTimeline 11（复用 animation 模块）、VideoFrame 10 + VideoColorSpace 5、XSLTProcessor 8、RTCPeerConnection 15。

## M4 刀20：Document 遗留/杂项成员（2026-10-30）

`platform/docmisc.impl.js`（模块 `docmisc`，L2）把 Document.prototype 上缺的 28 个成员按 Chrome 的相对顺序装齐，
并新增 CaretPosition、FragmentDirective 两个接口。**此前最隐蔽的后果**：这些成员不存在时，页面赋值
（`document.bgColor = 'red'`、`document.fullscreen = true`）会在 document 上凭空长出自有数据属性——可被指纹探测，
读回的也是页面自己写的值。探针 `docmisc-probe.js`（36 键，真 Chrome 145）0 差异：

- 五个颜色属性（fg/link/vlink/alink/bgColor）反射 body 的 text/link/vlink/alink/bgcolor 遗留属性，null→''，无 body 时读 ''、写静默；
- `anchors`（`a[name]`，按属性存在计）/`embeds`/`plugins` 是**活**集合且身份稳定（embeds===plugins，anchors===anchors），`applets` 恒空；
- `designMode` 大小写不敏感地归一为 on/off，其余值（含 null/undefined 字符串化）忽略；
- fullscreen/fullscreenElement/fullscreenEnabled 在 Chrome 里是 **rw 描述符但写入被忽略**，pointerLockElement/
  prerendering/wasDiscarded/rootElement 只读；`exitFullscreen()` 返回拒绝的 promise（`Document not active`）；
- `lastModified` 为本地 `MM/DD/YYYY HH:MM:SS`；`captureEvents/releaseEvents/clear` 是 length 0 的空操作；
- `caretPositionFromPoint(x,y)`（length 2，参数个数检查）经 elementFromPoint 命中后返回 CaretPosition（引擎无行内光标模型，
  位置取命中元素起点，getClientRect 为元素 rect）；
- `hasPrivateToken/hasRedemptionRecord`（length 1，缺参数返回拒绝的 promise，否则 false）、`hasUnpartitionedCookieAccess`（true）；
- `fragmentDirective` 是单例 FragmentDirective 对象（构造器非法）。

**本刀不含**：`createExpression/createNSResolver`（XPath 族，缺口表里还剩，与 XPathEvaluator 3 项一并另做）。
**已知差异**：anchors/embeds/plugins 的「活」是每次访问重新查询（Proxy），不是增量维护；forms/images/links/scripts 仍是旧的
每次新建快照（与 Chrome 的身份稳定不符，属另一刀）。

验收：探针 36 键 0 差；新增 docmisc_matches_chrome_truth；缺口 135→107（Document 30→2）；check_fresh 7 件鲜（40 真值）；analyze 零违。
全量（跳两个毒化页面测试）991 过/30 败，失败集与干净 HEAD 逐条相同。下一刀候选：XPath 族（Document 2+XPathEvaluator 3+XSLTProcessor 8）、
VideoFrame 10 + VideoColorSpace 5、RTCPeerConnection 15、Typed OM（CSSUnitValue 等，ScrollTimeline/ViewTimeline 的前置）。

## M4 刀21：XPath 1.0 全量（2026-10-30）

`platform/xpath.impl.js`（模块 `xpath`，L2）取代 bootstrap 里的「紧凑子集」（仅 child/descendant 步、@attr、几种谓词形状，无联合、
无轴、无函数、无运算符、无命名空间；约 420 行，连同 Document.evaluate 一并删除，bootstrap 净减 ~280 行）。新实现是完整的
XPath 1.0 求值器：词法（按规范的运算符/名字消歧）、递归下降解析、十三条轴（namespace:: 恒空，同 Chrome）、核心函数库
（含元数检查）、四种值的转换与比较规则、命名空间解析、XPathResult / XPathExpression / XPathEvaluator /
`createExpression`/`createNSResolver`。对抓取/代理型页面脚本这是高价值面（轴、count()、normalize-space()、联合、算术）。
探针 `xpath-probe.js`（583 键，真 Chrome 145）0 差异：~120 条路径（全部轴/节点测试/谓词）、~370 条标量表达式与函数、结果类型与
访问器报错、~130 条畸形表达式的逐字措辞、命名空间、顺序、迭代器失效、XPathExpression/XPathEvaluator。

**从语料里挖出的 WebKit/Blink 特性**（均已复刻）：
- 数字转字符串取 **6 位有效数字**：指数在 [-6,5] 用定点（`0.000001`、`12345.7`、`999999`），否则 `d.ddddde+X` 且尾数保留 5 位小数
  （`1000000`→`1.00000e+6`、`0.0000001`→`1.00000e-7`、`5 div 3`→`1.66667`）；
- **无序结果类型保持求值顺序**（联合是左右拼接而非排序；`//b | //li` 的 ANY_UNORDERED 首个是 B），有序类型才排成文档序；
- 变量不受支持：`$x` 求值为空串、`$x + 1` 为 NaN；`count(1)` 抛 SyntaxError「Type conversion failed…」，`sum(1)` 为 0，`local-name(1)` 为 ''；
- `lang()` 只看 `xml:lang`（`<p lang="en-US">` 对 `lang("en")` 为假）；`name()` 对 HTML 文档里的 HTML 元素回小写；
- 无前缀名字测试只匹配无命名空间元素，HTML 文档里还匹配 XHTML 命名空间（大小写不敏感），SVG `circle` 须经已解析前缀；
  无法解析的前缀在**解析期**抛 NamespaceError；
- 游离树的绝对路径根是其最顶祖先；DocumentFragment/DocumentType 作上下文抛 NotSupportedError；
- 迭代器结果在**任何**文档变动后失效（InvalidStateError），经一个共享 MutationObserver + 同步 takeRecords 实现；
- `ANY_TYPE` 的节点集结果是迭代器（对它读 stringValue 抛 TypeError）——旧测试 document_evaluate_xpath_subset 的断言据此改为 Chrome 行为，联合也不再抛。

**坑**：①**Chrome 自己的 XPath 解析器遇 `a[$b]` 会死循环**（探针语料里踩到，已剔除；写语料前先分段二分）；②引擎的探针页面跨次 /eval 持久，往
document 里追加的节点会累积（`//li` 翻倍），探针必须开头清理、结尾还原，并把宿主页的 head/body 子节点临时摘下以免计数依赖宿主页；
③单元测试宿主页没有 doctype/meta/pre，别用 `document.doctype`，用 createDocumentType 造游离节点。

**已知差异（引擎，非 XPath）**：DOMParser/createHTMLDocument/createDocument 造出的次级文档在引擎里不是 Document 实例（原型链只有 Object），
没有 evaluate()；**XSLTProcessor 仍是桩**（8 个成员，另需一整套 XSLT 引擎）。

验收：探针 583 键 0 差；新增 xpath_matches_chrome_truth；缺口 107→102（XPathEvaluator 3、Document 2 出表）；check_fresh 7 件鲜（41 真值）；analyze 零违。
全量（跳两个毒化页面测试）992 过/30 败，失败集与干净 HEAD 逐条相同。下一刀候选：次级文档升格为真 Document（解锁 XML 上的 XPath 与大量 DOMParser 用法）、
VideoFrame 10 + VideoColorSpace 5、RTCPeerConnection 15、XSLTProcessor 8。

## M4 刀22：整面形状审计 + 内部辅助成员不再外泄（2026-10-30）

**新工具**：`chrome/shape-audit-probe.js` 抓 Chrome 145 的全部 936 个接口对象的形状（原型自有成员序与种类 `ro/rw/function:len` + e/c/w 标志、
构造器 length、静态成员、父原型、Symbol 成员），真值 `chrome/shape-audit-truth-145.json`；`tools/webplatform/shape_audit.py <engine.json>`
做记分牌（absent/missing/extra/order/kind/len/statics/parent/syms 分类，`--iface NAME` 看单个接口）。基线（刀22前）：936 个里**只有 115 个完全一致**；
7 缺失、622 缺成员、53 带多余成员、155 序不同、220 种类不同、124 构造器 length 不同、195 父原型不同、82 个 Symbol 成员不同。缺口表（SURFACE-WEBIDL）只量
「实例上有无成员」，这份才是真实待办规模。引擎侧用 `probe_run.py engine tools/webplatform/chrome/shape-audit-probe.js out.json` 抓取。

**修复**：平台接口对象及其原型上的 `_` 前缀引擎辅助成员（`_signalAbort`、`BroadcastChannel._fire`、`MessagePort._pump`、`XMLHttpRequest._setReadyState`、
`Performance._navTiming` …）会被 `Object.getOwnPropertyNames`/`Reflect.ownKeys`/`Object.keys`/`getOwnPropertyDescriptors` 列出，Chrome 没有——指纹可见。
bootstrap 里原有的「只过滤 globalThis」枚举补丁扩展为：receiver 是**平台接口构造器或其 prototype**（init 时按大写全局建 WeakSet，排除 JS 内建如 Object/Array/Error）时同样过滤 `_` 开头名字；
页面后来定义的类不在集合里，其 `_x` 保持可见。带多余成员的接口 53→38（余下是 Element 的按标签成员、Option 继承的 HTMLElement 成员、Attr/Text/Comment 的节点级成员、
EventTarget 方法自带副本等，属**placement 族**，须逐类搬到 Chrome 的原型层）。

验收：全量（跳两个毒化页面测试）992 过/30 败，失败集与干净 HEAD 逐条相同；快照重生（22 行漂移，均为隐藏的 `_` 成员）。
**下一步建议顺序**：①placement 族（节点级 nodeName/nodeType/cloneNode 搬回 Node.prototype；Range/StaticRange 的 start*/end* 搬到 AbstractRange；EventTarget 副本去重）；
②155 个「序不同」与 220 个「种类不同」多半可由生成器/重排一把清；③195 个「父原型不同」查链；④再回到按接口的功能刀（RTCPeerConnection、VideoFrame、XSLTProcessor）。

## M4 刀23：节点级成员搬回 Node.prototype（2026-10-30）

`platform/nodelevel.impl.js`：Chrome 的 nodeType/nodeName/nodeValue/cloneNode 只在 Node.prototype 上，各节点接口继承。引擎里 Text/Comment/CDATASection/
ProcessingInstruction/Document/DocumentFragment/DocumentType/ShadowRoot 各自挂了一份自有访问器/方法。模块把这些描述符摘下存入「以原型为键」的表，
Node.prototype 的四个成员改为分派器（按接收者原型链查表，按原型缓存命中；元素这条热路径只在每个原型第一次 miss，其后走原 Node 基础实现）。
行为不变，形状向 Chrome 靠拢：带多余成员的接口 38→32，完全一致的接口 115→118。**Attr 不在其中**——引擎里 Attr 不继承 Node，其自有 nodeType 等必须保留
（第一次把它也搬走，XPath 以属性节点作上下文即告失败，已回退）。
验收：全量（跳两个毒化页面测试）992 过/30 败，失败集与干净 HEAD 逐条相同；analyze 零违；快照已重生。
余下 placement：Range/StaticRange 的 start*/end*/collapsed 应在 AbstractRange.prototype；EventTarget 方法的自带副本（BroadcastChannel/EventSource/IDB*/MediaQueryList/MessagePort/WebSocket/Audio）；
Element 上的按标签成员与 Option 继承的 HTMLElement 成员。

## M4 刀24：父原型链按审计批量修复（2026-10-30）

形状审计的「父原型不同」195 项里，绝大多数是引擎类缺省挂在 Object.prototype 下而 Chrome 的父是 Event/SVGElement/WebGLObject/CSSRule/CSSMathValue/CSSStyleValue/
CSSTransformComponent/Credential/DOMException/HTMLElement… 。`global_surface.js` 已有 ET_CHAIN（只含 EventTarget 家族）；新增 `AUDIT_CHAIN`（由审计直接生成的 [子, 父] 表，
共 ~190 项，一次 `setPrototypeOf(C.prototype, P.prototype)`）。完全一致的接口 118→146，父原型不同 195→9（余下为 Performance* 条目与 SVGElement 家族）。

**有意排除**：①`Performance*` → PerformanceEntry/PerformanceResourceTiming（9 项）——这些真类在 performance.js 里晚于本表定义，改父会让 5 个性能测试失败（桩的 PerformanceEntry 访问器遮住真成员），
须先把 PerformanceEntry 做成真类再接；②SVGElement/SVGClipPathElement/SVGSymbolElement/SVGFEDropShadowElement——引擎里 SVGElement 继承 HTMLElement（已知怪癖，改成 Element
会丢掉 style/dataset 等依赖 HTMLElement 的行为），须先重做 SVG 基类。③**构造器自身的 [[Prototype]]**（`Object.getPrototypeOf(Derived) === Parent`，审计的 statics 项）本刀未动：
试过与原型链一并重挂，会破坏 Performance* 构造路径，应作为独立一步并带测试。
验收：全量（跳两个毒化页面测试）992 过/30 败，失败集与干净 HEAD 逐条相同；快照重生。

## M4 刀25：描述符级一致性表 shape_fix.js（2026-10-30）

审计里最大的两类——方法 `length`（397+124 项）和 e/c/w 标志（类方法在 JS 里天生不可枚举，而 WebIDL 的操作/属性可枚举；~300 项）——是纯数据问题，不必逐类改代码。
`tools/webplatform/gen/gen_shape_fix.py <engine.json>` 把引擎抓取与 Chrome 真值的差生成 `crates/diting/js/shape_fix.js`（数据表 + 一个小应用器），`build.rs` 把它排在**最后**
（所有脚本之后，类已定稿）。应用器只改「两边都有的成员」：函数 length、构造器 length、e/c/w 标志（data 与 accessor 区别对待）；ro/rw（setter 有无）有意不碰，因为引擎内部代码可能经 setter 写入。
效果：完全一致的接口 146→194，构造器/方法 length 差异清零（124→0），种类差异 220→40（余下是 ro/rw 与不可重定义的非 configurable 常量）。
**再生成注意**：要在**未应用** shape_fix.js 的引擎上抓取（暂时把它从 build.rs 注释掉），否则表会是空的。
**顺带纠正一个旧测试**：canvas_blank_readout_face_p3 曾断言 `toDataURL.length === 2`、`toBlob.length === 3`，真 Chrome 145 是 0 和 1（IDL 里 toDataURL 两个参数皆可选，toBlob 只有 callback 必填），测试已改为 Chrome 值。
验收：全量（跳两个毒化页面测试）992 过/30 败，失败集与干净 HEAD 逐条相同；快照重生。余下审计项：缺成员 622、顺序 155、种类 40、多余 33、statics 59、父原型 15、Symbol 82。

## M4 刀26：自有成员顺序（2026-10-30）

shape_fix.js 增加 ORDER 表（审计里「序不同」的接口，只含两边共有成员的 Chrome 序）。应用方式：引擎独有成员保持原相对序排在最前，其后按 Chrome 序，
每个 configurable 键 delete 再按序 defineProperty（JS 自有键序即创建序）。不可配置键无法移动，尽力而为。序不同 155→10，完全一致接口 194→236（共 936）。
余下审计项：缺成员 622、种类 40、多余 33、statics 59、父原型 15、Symbol 82、序 10。验收：全量（跳两个毒化页面测试）991–992 过/30 败（test_local_fetch_resolves_on_task_boundary 为已知时序型偶发，单跑 2/2 过），
失败集与干净 HEAD 相同；快照重生。

## M4 刀27：纯桩接口的成员外壳 stub_members.js（2026-10-30）

审计里「缺成员」的大头是**纯桩接口**：USB*/Bluetooth*/XR*/GPU*/多数 Audio 节点/CSS Typed OM…，引擎里只有 global_surface.js 造的惰性构造器（mkThrow/mkOk），原型除 `constructor` 外空空。
`tools/webplatform/gen/gen_stub_members.py` 由 Chrome 真值为它们生成 `crates/diting/js/stub_members.js`（482 个接口、2548 个成员，含访问器 ro/rw、方法的 name+length+e/c/w、常量及其真值、静态成员），
`build.rs` 排在 shape_fix.js 之前。行为契约：在该桩的**真实例**（原型链匹配）上，getter/方法返回 undefined；在**任何其他接收者**上抛 `TypeError: Illegal invocation`（Chrome 的品牌检查——指纹脚本会对 `{}` 调
描述符 getter 来识破造假）。
**关键约束**：只给**真桩**加外壳——global_surface.js 现把每个惰性构造器登记进 `__diting_stubs`（隐藏全局 WeakSet），审计探针据此给引擎侧打 `stub` 标记，生成器只取标记为真桩的接口。
第一版按「原型只有 constructor」判定，误伤了 File/SharedWorker 等**把成员当自有实例属性**的真类（构造器里 `this.name = …` 遇到继承的只读访问器在严格模式下抛错，连带 ~90 个测试失败）。
结果：缺成员 622→140，完全一致接口 236→659（共 936）。ORDER 表须在**应用桩之后**重新生成（桩成员追加在 constructor 之后，需按 Chrome 序重排）：先注释掉 shape_fix.js，抓取，再运行 gen_shape_fix.py。
再生成流程（两步，均需暂时从 build.rs 注释掉被生成的脚本）：①注释 stub_members.js 与 shape_fix.js → 抓取 → gen_stub_members.py；②启用 stub_members.js、注释 shape_fix.js → 抓取 → gen_shape_fix.py。
验收：全量（跳两个毒化页面测试）992 过/30 败，失败集与干净 HEAD 逐条相同；快照重生。余下审计项：缺成员 140、序 27、种类 40、多余 33、statics 15、父原型 15、Symbol 82。

## M4 刀28：Symbol 成员与缺失接口（2026-10-30）

**sym_fix.js**（`gen_sym_fix.py` 生成）：审计里 82 个接口的 well-known Symbol 成员不同。
- **Symbol.iterator 缺失（49 个）**：CSSRuleList/NodeList/CSSStyleDeclaration/…这些索引集合在 Chrome 里 `[Symbol.iterator] === Array.prototype.values`（同一函数对象），maplike/setlike
  的则别名到自己的 entries/values（AudioParamMap、FormData、CustomStateSet…）。补上后 `for (const r of sheet.cssRules)` 等真正可用（已与 Chrome 逐项对拍）；
- **多余的 toStringTag（19 个）**：JS 内建 Error 家族与 BigInt64Array/BigUint64Array 的 prototype 上被引擎误加了 `Symbol.toStringTag`，Chrome 没有——已删除；
- **Symbol.unscopables（5 个）**：Element/CharacterData/Document/DocumentFragment/DocumentType 补上空原型对象（键集合取自真值，例如 Element 的
  after/append/before/prepend/remove/replaceChildren/replaceWith/slot）；
- **asyncIterator**（ReadableStream、FileSystemDirectoryHandle）与 4 个接口的 Symbol 相对次序，一并按 Chrome 重建。
**缺失的 7 个接口**（SharedStorage 及其 Append/Clear/Delete/Modifier/Set 方法类与 SharedStorageWorklet）：stub_members.js 生成器新增 CREATE 段，铸造惰性构造器（`Illegal constructor`、登记 `__diting_stubs`、
接上 Chrome 的父原型）再装成员。
再生成顺序（依赖链）：①注释 stub_members/sym_fix/shape_fix → 抓取 → gen_stub_members + gen_sym_fix；②启用这两个、注释 shape_fix → 抓取 → gen_shape_fix（顺序表须在桩与符号之后）。
结果：Symbol 差异 82→1，缺失接口 7→0，完全一致接口 659→728（共 936）。验收：全量（跳两个毒化页面测试）992 过/30 败，失败集与干净 HEAD 逐条相同；快照重生。
余下审计项：缺成员 140、序 27、种类 40、多余 33、statics 15、父原型 15、Symbol 1。

## M4 刀29：MathMLElement 与「真类缺成员」的剩余盘点（2026-10-30）

`mixin_fix.js`（手写后冻结，排在 stub_members 之前）：Chrome 的 MathMLElement 带有 HTMLOrSVGElement/GlobalEventHandlers 的 110 个成员（全部 on* 处理器属性 + dataset/style/attributeStyleMap/tabIndex/autofocus/nonce/blur/focus），
引擎的 MathMLElement 是空壳。HTMLElement.prototype 已经按节点 id 实现了这些成员，缺的直接按**描述符共享**（同一 getter/setter/方法函数对象）。MathMLElement 完全一致。完全一致接口 728→729。
**再生成链现为三级**：mixin_fix → stub_members → sym_fix（均先于 shape_fix 生效，shape_fix 的顺序表须在它们之后的抓取上生成）。

**剩余缺成员的分布**（139 个接口、约 860 个成员）已无法靠数据表补齐，原因统一：这些是**有真实现的类**，其成员当前是**实例自有属性**（构造器里 `this.x = …`），Chrome 里是原型上的访问器。
典型：CanvasRenderingContext2D 73、ElementInternals 51、RTCPeerConnection 37、SVGSVGElement 35、GPUSupportedLimits 31、Document 24、PerformanceResourceTiming 19、XMLHttpRequest 18、MediaStreamTrack 17、
SVGMarkerElement 17、RTCDataChannel 16、PerformanceNavigationTiming 15、SpeechSynthesisUtterance 13、VideoFrame 13、SubtleCrypto 12、Touch 12、TextMetrics 10、Attr 7…
第 27 刀已证明：给这类类直接加原型只读访问器会让构造器里的 `this.x = v` 在严格模式下抛错。正确做法是**逐类**把「自有数据属性」转成「原型访问器 + 私有槽」（WeakMap/私有字段），并同时保证
`JSON.stringify`/`toJSON`/`Object.keys` 等依赖自有枚举属性的路径（Touch、TextMetrics、PerformanceEntry 家族）仍按 Chrome 行为工作——这是每类一刀、带探针的工作，不是批处理。
这同时会清掉另一个指纹面：**实例上不该有的自有属性**（Chrome 里这些接口的实例没有自有属性），当前审计只量原型，尚未量实例。建议下一步先给审计加「实例自有键」维度，再按「引擎自有键数 × 被探测频率」排序逐类转换。
验收：全量（跳两个毒化页面测试）992 过/30 败，失败集与干净 HEAD 逐条相同；快照重生。

## M4 刀30：实例自有键审计 + 引擎内部状态不再外泄到实例/全局（2026-10-30）

**新审计维度**：`chrome/instance-audit-probe.js`（127 种常见对象的 `Object.getOwnPropertyNames(instance)`）+ `tools/webplatform/instance_audit.py <engine.json> <chrome.json>`。Chrome 的 WebIDL 实例几乎没有自有属性，
引擎的真类常把成员存成自有数据属性，并带 `_nid`/`_style`/`_peName`… 引擎内部状态。基线：127 个里 51 个干净、76 个带泄漏。
**修复**（bootstrap 枚举补丁再扩展）：①平台接口**实例**（直接原型是平台原型）上，引擎自己脚本用过的 `_x` 名字不再出现在 getOwnPropertyNames/ownKeys/keys/getOwnPropertyDescriptors 中——名字集合由
`gen_internal_names.py` 扫描 `js/*.js` 的 `this._x` 生成 `internal_names.js`（350 个），**不是**一刀切隐藏所有 `_`：页面给 `document`/`performance` 挂的 `_docMarker` 必须可见（旧测试
key_enumeration_keeps_page_underscore_keys… 钉着这条；第一版一刀切被它抓出）。页面子类的实例（直接原型是页面原型）本来就不受影响；②crypto_subtle.js/ed25519.js 的 25 个 helper 函数声明
（makeKey/keyBytes/runOp/…，块内函数声明被提升成全局且两文件共享，无法改成局部）加入全局枚举过滤的显式名单。结果：带泄漏的对象 76→42，Window 里 25 个 helper 泄漏消失。
**未做、需裁决**：Window 还多出 ~29 个 Chrome 145 CfT 抓取里没有的接口名（AnimationTrigger/LanguageModel/Sanitizer/XSLTProcessor/PermissionsPolicy/XR*Layer/requestResize…）。
旧的 window 键真值（fp-chrome-winkeys*.json，测试 window_own_prop_face_p2 钉着）**包含**这些名字，而本次 CfT 145 无头抓取没有——两份真值不同源（版本或 headless 差异）。
我先按「全部删掉」试过，立刻与旧测试冲突；在弄清哪份才代表目标 Chrome 145 稳定版（有头）之前不动，**已回退**。建议：用有头的 Chrome 145 重抓 window 键作仲裁。
仍泄漏的真类（成员是实例自有属性）：CSSStyleDeclaration(383, 属性很多但 cssFloat 多余)、PerformanceNavigationTiming、XMLHttpRequest、TreeWalker、CanvasRenderingContext2D、Touch、SubtleCrypto、RTCPeerConnection…
需逐类转原型访问器（见刀29 说明）。
验收：全量（跳两个毒化页面测试）991–992 过/30 败（test_local_fetch… 为已知时序偶发），失败集与干净 HEAD 相同；快照重生。

## M4 刀31：实例自有数据属性 → 原型访问器（lift_own.js）（2026-10-30）

`gen_lift_own.py <instance-audit.json> Class,…` 为指定类在原型上定义「getter/setter + 私有槽」访问器（只提升 Chrome 原型上本来就是 ro/rw 访问器的名字）。构造器里的 `this.readyState = 0`
和内部读取原样继续工作——赋值落到继承的 setter，实例不再有这些自有键。首批：XMLHttpRequest(11)、CanvasRenderingContext2D(9)、SpeechSynthesisUtterance(5)、TextDecoder(3)、MediaQueryList(3)、
TreeWalker(4)、NodeIterator(5)、Touch(12)、TextMetrics(3)、MediaStream(2)。XHR 的 UNSENT…DONE 原是**类字段**（`UNSENT = 0;` 用 [[Define]] 语义直接在实例上建属性，绕过 setter），
已删字段并在原型上定义为只读常量（Chrome 的形状）。实例上引擎存放的 `_on*` 处理器槽（XHR 的 `_onload` 等）并入隐藏规则（`/^_on[a-z]+$/`）。
**已接受并记录的权衡**：提升后的访问器**带 setter**（Chrome 的只读属性没有），因此审计里这些属性的 kind 从 ro 变 rw，页面对只读属性的写入会成功而不是被忽略。选这个是因为实例自有键泄漏
（`Object.keys(xhr)`/`hasOwnProperty('readyState')`）比描述符 setter 更常被探测，且无需改动引擎里成百上千处内部写入。
**此机制的局限**：用 `Object.defineProperty(this, …)` 在构造器里建的自有属性（TreeWalker/NodeIterator 的 root、CanvasRenderingContext2D 的 fillStyle 等）和类字段不被 setter 拦截，仍是自有键，需逐类改源码。
**再生成链**（脚本 `scratchpad/regen_shape.sh` 的做法，须固化为工具）：lift_own → internal_names → mixin_fix → stub_members → sym_fix → [抓取，shape_fix 关] → gen_shape_fix → shape_fix。
结果：带泄漏对象 76→38；完全一致接口 729→728（个别接口的 ro 变 rw）。验收：全量（跳两个毒化页面测试）992 过/30 败，失败集与干净 HEAD 逐条相同；快照重生。

## M4 刀32：版本一致性——Chrome 155 遗留名字清出 window（2026-10-30）

**上一刀留下的仲裁问题查清了**：旧 window 键真值（`chrome/fp-chrome*.json`，global_surface 的各表由它生成）里 `fp-chrome.json` 带的 UA 是 **Chrome/155**，而 chrome-target.toml 钉死的目标是
**145**（"all layers the SAME real version"）。所以那些名字不是 headless/headful 之分，而是**抓自更新的版本**。Chrome for Testing 145.0.7632.117（审计真值）没有它们，Window 的名字集合因此多出：
AnimationTrigger/AudioPlaybackStats/HTMLCameraElement/HTMLMicrophoneElement/HTMLUserMediaElement/InteractionContentfulPaint/LanguageModel/NodeRange/OpaqueRange/PerformanceSoftNavigation/
PermissionsPolicy/TimelineTrigger*/XR*Layer/XRPlane*/XRSubImage/XRWebGLSubImage/Sanitizer/XSLTProcessor（XSLT 在 Chrome 142 起已移除）/requestResize。`version_prune.js`（最后加载）把这 28 个 `delete`。
**有意保留**：FontFaceSet、CSSPseudoElement——同样不在 145，但引擎自己的 document.fonts / 伪元素代码按名字用它们，待这些调用方改造后再删。
**测试同步**：window_own_prop_face_p2 原先钉着 PermissionsPolicy/XSLTProcessor 存在（据旧 155 真值），改为断言它们（及 Sanitizer）在 145 下 `undefined`，并改用 145 确有的接口（IDBCursor、XMLSerializer）验证「惰性构造器 / 可构造」两种形状。
这条的更大含义：**global_surface.js 的各生成表来自 155 抓取，仍可能含有别的 155-only 的细节**（成员、顺序、枚举）；已有的 CfT 145 审计（形状/实例/XPath/…）才是权威，后续对 global_surface 的修补应以 145 真值为准。
验收：全量（跳两个毒化页面测试）992 过/30 败，失败集与干净 HEAD 逐条相同；快照重生；实例审计带泄漏对象 38/127。

## M4 刀33：window 自有名字的顺序（2026-10-30）

指纹收集器（Radar 的 windowFeatures 等）对 `Object.getOwnPropertyNames(window)` 的**有序**列表取哈希。引擎按脚本加载顺序建全局，与 Chrome 145 的顺序在 1212 个位置里有 1151 个不同。
`gen_window_order.py <instance-audit-chrome.json>` 把 Chrome 145 的 window 名字序（1213 个）生成 `window_order.js`：对每个 configurable 且 Chrome 也有的全局 `delete` 再按序 `defineProperty`（引擎独有的 configurable 名字先放，它们本就被枚举补丁隐藏）。
**两个坑**：①循环会 `delete` 掉 `Object`/`Reflect`/`Set` 本身——所有原语（getOwnPropertyDescriptor、defineProperty、Reflect.deleteProperty、hasOwnProperty）必须先捕获进局部，第一版直接引用 `Object` 导致引擎启动即 `Object is not defined`；
②`window_faces.js`/`__diting_init` **每个页面**会重装若干 window 访问器（innerWidth、scrollX…），重装的属性被追加到键序**末尾**，所以排序函数导出为隐藏全局 `__diting_reorder_window`，在快照末尾跑一次、并在每页 init 末尾再跑一次。
**结果**：其余 1204 个名字的**相对次序与 Chrome 完全一致（0 处不同）**。**做不到的部分**：8 个不可配置名字（Infinity/NaN/undefined/window/document/location/top，加引擎多出的 MediaQueryList）无法移动，仍排在最前，
而 Chrome 里它们散布在 6/7/8/…/690 位置——所以**绝对位置哈希仍与 Chrome 不同**，只有「相对序」对齐。这需要在 V8 建 realm 的层面解决（Infinity 等由 V8 在内建序列中间创建），脚本层无解；
`MediaQueryList` 在引擎里是不可配置的全局（Chrome 里是普通可配置接口），另可查成因。
验收：全量（跳两个毒化页面测试）992 过/30 败，失败集与干净 HEAD 逐条相同；快照重生。

## M4 刀34：不可配置的 window 属性集合对齐 Chrome（2026-10-30）

Chrome 145 里 window 上**不可配置**的自有属性恰好 8 个：Infinity/NaN/undefined/window/document/location/top/chrome。引擎有两处偏差：①`MediaQueryList` 是 bootstrap.js 里的顶层 `function` 声明，
顶层函数声明会生成**不可配置**的全局属性——改成 `const MediaQueryList = function MediaQueryList(q) {…}`（词法绑定不产生全局属性，再由原有 `globalThis.MediaQueryList = …` 显式装成普通可配置接口全局）；
②`window.chrome` 在引擎里可配置，Chrome 里不可——window_order.js 的排序函数末尾在它就位后把它改为不可配置（排序对不可配置项只跳过，所以位置保持）。
结果：不可配置集合与 Chrome 逐项相同（8/8）。验收：全量（跳两个毒化页面测试）992 过/30 败，失败集与干净 HEAD 相同；快照重生。
经验：**bootstrap/各脚本里的顶层 `function` 声明都会泄漏成不可配置全局**（本次 crypto helper 与 MediaQueryList 同源），新代码应放进 IIFE 或用 `const` 函数表达式。

## M4 刀35：单例对象的自有成员下沉到原型（demote_own.js）（2026-10-30）

`history`、`crypto`、`crypto.subtle`、`navigator.geolocation`、`navigator.serviceWorker`、`indexedDB` 在引擎里把成员（pushState/getRandomValues/getCurrentPosition/open…）存成**实例自有属性**；Chrome 的这些实例没有任何自有属性，成员都在原型上。
`demote_own.js`：把实例上的自有方法/访问器按**同一函数对象**搬到对应接口原型（History/Crypto/SubtleCrypto/Geolocation/ServiceWorkerContainer/IDBFactory），自有**数据**属性（`history.length` 等）改成原型访问器+私有槽，
使引擎内部的赋值照常工作；实例自带的 `Symbol.toStringTag` 若原型已有则删除以免遮蔽。**EventTarget 三件套不搬**（可能由对象自己的监听表支撑，且搬到子接口原型会违反 EventTarget 链真值——被 eventtarget_chains 测试抓出过）。
下沉函数在两个时点运行：快照末尾，以及**每页 init 末尾**（`__diting_demote_own`，因为 indexedDB / serviceWorker 这类单例是每页新建的）。
结果：History/Crypto/SubtleCrypto/Geolocation 干净；带泄漏对象 38→34。**仍剩**：IDBFactory（open/deleteDatabase/databases/cmp）与 ServiceWorkerContainer（register/getRegistrations）——它们的对象在 demote 之后才创建或被每次访问重建，需在其创建处处理。
验收：全量（跳两个毒化页面测试）992 过/30 败，失败集与干净 HEAD 逐条相同；快照重生。

## M4 刀36：IDBFactory 干净 + window.sharedStorage + 窗口原型链的发现（2026-10-30）

- **IDBFactory**：`indexedDB` 对象的自有方法 open/deleteDatabase/databases/cmp 此前被 demote 跳过——因为 IDBFactory.prototype 上**已有同名占位**（类体里的桩）。改为：原型上已有的是占位，**实例自有的才是可用实现，替换占位**后删除自有。实例审计 IDBFactory 干净；ServiceWorkerContainer 只剩 EventTarget 三件套（有意不搬）。
- **window.sharedStorage**：新增 `window_extras.js`，在 window 上装 getter（enumerable/configurable，返回单例 SharedStorage 实例），补上 Chrome 145 有而引擎没有的名字。Window 的「缺失名字」清零。
- **窗口原型链（发现，未修）**：Chrome 里 `Object.getPrototypeOf(window) === Window.prototype` 且链为 window → Window.prototype → WindowProperties → EventTarget.prototype，`window instanceof EventTarget` 为真；
  引擎里全局对象的**直接原型不是** Window.prototype（`window instanceof Window` 靠链上某处成立），`window instanceof EventTarget` 为**假**。窗口自有的 `Symbol.toStringTag` 因此不能删（删了 `Object.prototype.toString.call(window)` 变成 `[object Object]`，已试过并回退）。
  这是 V8 全局模板（Rust 侧）的问题，脚本层无解；会让 `window instanceof EventTarget` 这类检查与 Chrome 不同，值得单列一刀（改全局对象模板的原型）。
验收：全量（跳两个毒化页面测试）992 过/30 败，失败集与干净 HEAD 相同；快照重生；实例审计带泄漏对象 33/127。

## M4 刀37：主窗口原型链接上 Window.prototype / EventTarget.prototype（2026-10-30）

上一刀的发现（`window instanceof EventTarget` 为假）**不需要改 Rust**——我之前判断成「V8 全局模板问题」是错的：`v8::Context::new` 用的是默认全局，原型链全是 JS 在搭。真正的结构是
window → gate(Proxy，WindowProperties 具名访问层) → 一个只有 `constructor` 的裸层 → Object.prototype；iframe_realm.js 的注释还写着「主窗口的链从不包含 Window.prototype，所以现有东西不动」。
修法（window_extras.js 末尾）：把 gate 的**原始目标** `_namedHolder`（bootstrap 顶层绑定，跨脚本可见）的原型改指真正的 `Window.prototype`（它已链到 EventTarget.prototype）。gate 仍在最前，具名访问解析与 holder 上的 EventTarget 三件套不变；
结果 `window instanceof EventTarget` / `instanceof Window` 皆真，链 window → gate → Window.prototype → EventTarget.prototype → Object.prototype。新增回归测试 window_chain_reaches_event_target（含 add/removeEventListener + dispatchEvent 实测）。
**仍与 Chrome 不同**：Chrome 的**直接**原型就是 Window.prototype（window → Window.prototype → WindowProperties → EventTarget），引擎的直接原型是 gate；因此 `Object.getPrototypeOf(window) === Window.prototype` 仍为假，window 自有的
toStringTag 仍不能删。要对齐须给每个 realm 一份自己的 Window.prototype（现在主窗口与 iframe 窗口共用一个），属较大的结构改动，暂不做。
验收：全量（跳两个毒化页面测试）992 过/30 败，失败集与干净 HEAD 逐条相同；快照重生。

## M4 刀38：TreeWalker / NodeIterator / NodeFilter（2026-10-30）

`platform/treewalker.impl.js`（模块 `treewalker`）。此前 `document.createTreeWalker/createNodeIterator` 返回**对象字面量**（不是 TreeWalker/NodeIterator 实例，`[object Object]`，实例上泄漏 13 个自有键：`_filter`/`_accept`/
`_traverseChildren`/… 与所有移动方法）。Chrome 145：构造器非法、`root/whatToShow/filter` 只读、`currentNode` 可读写的原型访问器、原型上的移动方法。探针 `treewalker-probe.js`（真 Chrome 145，63 键）0 差异：形状、
参数校验措辞（`1 argument required` / `parameter 1 is not of type 'Node'.` / `parameter 3 is not of type 'Object'.`）、whatToShow 的 unsigned long 转换、currentNode 赋值的 `Failed to convert value to 'Node'.`、
**过滤器返回值按 unsigned short 取模，==1 接受、==2 拒绝（剪枝），其余一律跳过**（旧实现用严格相等且把非数字当作接受/拒绝以外的混合情形）、过滤器异常原样传播、全方向遍历语料、NodeIterator 指针语义。遍历算法本身
（#432/#461/#462/#467/#469/#475 一整套 DOM 6.1/6.2 规则）原样搬到原型方法，状态放私有槽。
**两处与旧引擎行为不同、以 Chrome 为准**：①`previousNode()` 爬到**被接受的根**时**返回根**（规范与 Chrome 一致；旧实现里「后退遍历永不返回根」的注释与测试 test_treewalker_previousnode_reverse_order 是错的，已改为 `["A","DIV"]`）；
②`NodeFilter` 在 Chrome 是**没有 prototype 的函数**（回调接口：常量挂在函数上，`new NodeFilter()` 报 not a constructor），引擎里原来是普通对象（`typeof` 为 object）——用方法简写函数实现。
bootstrap.js 删除了两个对象字面量工厂与 TreeWalker 桩类。TreeWalker/NodeIterator 实例自有键泄漏消失；实例审计带泄漏对象 33→32。
验收：全量（跳两个毒化页面测试）994 过/30 败，失败集与干净 HEAD 逐条相同；analyze 零违；check_fresh 7 件鲜（43 真值）；快照与形状表按再生成链重生。

## M4 刀38 补记：依赖棘轮（2026-10-30）

**我自己的流程漏洞**：从刀30 起我每次跑 `analyze.mjs --check baseline.json` 都只看 `tail -1`，而棘轮失败的那一行（`FAIL dependency ratchet: crossFileLoadTimeNames: 55 > baseline 51`）在倒数第二行，
被「module contract violations」这一行盖住了——棘轮实际从 lift_own/stub_members/window_extras 三个平铺脚本在加载期直接引用 `_markNative`（+3）加上 treewalker 模块的 Document/_markNative（+1）起就是红的，却被我当成绿的提交了几刀。
修复（不动基线）：①bootstrap 在 `_markNative` 定义处导出隐藏全局 `__diting_markNative`，在具名访问 holder 创建处导出 `__diting_namedHolder`，平铺脚本经 `globalThis.…` 取用（属性访问不是自由变量引用，不计入棘轮）；
②treewalker 模块改 `globalThis.Document.prototype` 并用 `P.method({}, 'NodeFilter', 0, fn)` 标记原生，去掉 load-time 依赖。结果 crossFileLoadTimeNames 51（=基线），`dependency ratchet OK`。
以后的检查口径：**分别**看 `ratchet` 行与 `module contract violations` 行，脚本里用 `grep -E "FAIL|OK"`。
验收：全量（跳两个毒化页面测试）994 过/30 败，失败集与干净 HEAD 逐条相同；check_fresh 7 件鲜（43 真值）；analyze 棘轮 OK 且契约零违。

## M4 刀39：Performance* 条目换成原型访问器 + 追回被「新鲜度更新」抹掉的 global_surface 手改（2026-10-30）

**Performance 家族**（performance.js）：①`PerformanceNavigationTiming` 现在**继承 PerformanceResourceTiming**（Chrome 即如此；之前继承 PerformanceEntry），其「Level-2 字段」不再用 `Object.assign(this, extra)` 灌成 25 个实例自有属性，
也不再有实例自有的 `toJSON` 覆盖——导航阶段时间戳放进构造时的记录 `_prt`，Resource Timing 的 getter 与导航专有 getter（unloadEventStart…loadEventEnd、type、redirectCount、criticalCHRestart、activationStart、confidence、notRestoredReasons）
共同读它，`toJSON` 在原型上；②`PerformanceResourceTiming` 补齐 Chrome 的完整时间面（workerStart、redirectStart/End、fetchStart、domainLookup*、connect*、requestStart、responseStart/End、serverTiming、finalResponseHeadersStart、
firstInterimResponseStart、worker* 、contentEncoding…），有真实测量就用，没有按规范给 0 / 空；③`detail` 只留在 PerformanceMark/Measure（Chrome 的 PerformanceEntry 上没有）；
④`PerformanceServerTiming` 不再继承 PerformanceEntry（Chrome 里是独立接口：name/duration/description/toJSON）。性能相关 9 个测试全过。

**一次事故（已修）**：`global_surface.js` 是**生成件**（头注释写明由 gen_globals_surface.py 生成），而我在刀24/刀27 **手改**了它（AUDIT_CHAIN、`__diting_stubs` 登记）。刀38 里我跑了两次 `check_fresh.sh --update`，
它按生成器重写了该文件，把我的两处手改**整段抹掉**（-204 行）——提交 eb37f00 因此静默丢了父原型修复（父原型不同 8→193）和真桩登记，而我当时只看了「全量失败集不变」，没复核审计。刀39 复跑审计才发现。
**修法是改生成器而不是再手改**：`gen_globals_surface.py` 现在直接产出 `__diting_stubs` 登记与 `AUDIT_CHAIN`（由 shape-audit-truth-145.json 直接算出：Chrome 145 父原型不是 Object 且不在 ET_CHAIN 的全部接口；排除 SVGElement 家族与真 Performance* 类）。
**新规矩**：①改了生成件的来源（生成器/真值）后必须 `check_fresh.sh --update` 并**再跑一遍审计**确认没被抹；②任何「手改」前先看文件头是不是 GENERATED；③形状/实例审计的数字应作为每刀的固定回归检查，而不只看测试通过。
结果：完全一致接口 741/936（含追回的父原型修复），父原型不同 8，实例审计带泄漏对象 30/127（PerformanceNavigationTiming 的 25 个自有键清零）。
验收：全量（跳两个毒化页面测试）994 过/30 败，失败集与干净 HEAD 逐条相同；check_fresh 7 件鲜（43 真值）；ratchet OK。

## M4 刀40：lift/demote 扩面（2026-10-30）

lift_own 增 AbortController(signal)、MessageChannel(port1/2)、TransformStream(readable/writable)、Blob(size/type)、File(name/lastModified)、ImageData(width/height)、Image(6)；`gen_lift_own.py` 新增第 4 个参数
（Chrome 侧实例审计）：**Chrome 实例自己也持有的名字不提升**（ImageData.data 在 Chrome 里就是实例自有，第一版提升后审计立刻报 `missing=['data']`）。demote_own 增 navigator.clipboard(Clipboard)、navigator.permissions(Permissions)。
gen_internal_names 的赋值正则放宽到任意接收者（`x._bufferedAmount = …`）。实例审计带泄漏对象 30→22/127。
**留作决策而非机械修复**：CSSStyleDeclaration 的 383 个缺失属性名（Chrome 的 `el.style` 实例上有全部 CSS 属性的 camelCase 自有名，含 anchorName 等引擎并未实现的特性）——补上会让 `'anchorName' in el.style`
这类特性检测为真，而引擎并不实现，页面可能据此走不通的分支；是「对外自称」与「真实能力」的取舍，需要产品裁决（建议：只补引擎真实支持的，其余维持缺失）。
还剩：CanvasRenderingContext2D(defineProperty 自建)、RTCPeerConnection、Image(DOM 类自有)、Attr(不继承 Node)、NamedNodeMap、CanvasGradient、WebSocket(动态 `_` 名)。
验收：全量（跳两个毒化页面测试）994 过/30 败，失败集与干净 HEAD 相同；ratchet OK；一致接口 741/936。

## M4 刀41：CanvasRenderingContext2D 成为真接口 + 状态属性（2026-10-30）

**可见度很高的缺陷**：`canvas.getContext('2d')` 返回的是 bootstrap 里私有类 `_Canvas2D` 的实例，而全局 `CanvasRenderingContext2D` 是个空桩类、实例从不继承它：`ctx instanceof CanvasRenderingContext2D` 为**假**，
`ctx.constructor.name` 是 **`_Canvas2D`**（泄漏内部类名），`Object.prototype.toString.call(ctx)` 是 `[object Object]`，实例上 15 个自有键。Canvas 指纹脚本最爱检查这几项。
**修法**：让实现类**就是**接口——把 `_Canvas2D` 改名为 `CanvasRenderingContext2D` 并直接装成全局（原桩删除）；构造器对非对象 canvas 抛 `Failed to construct 'CanvasRenderingContext2D': Illegal constructor`。
lift_own 已把它的 9 个赋值型属性提升为原型访问器，内部 `_w/_h/_buf/...` 因实例的直接原型已是平台原型而被隐藏。
**状态属性**（`platform/canvasstate.impl.js`，探针 `cvstate-probe.js`，32 键 0 差）：此前 lang/direction/fontKerning/fontStretch/fontVariantCaps/letterSpacing/textRendering/wordSpacing/filter/imageSmoothingQuality/imageSmoothingEnabled/
shadowColor/shadowOffsetX/Y/shadowBlur/lineCap/lineJoin/miterLimit/lineDashOffset 读出 `undefined`/`null`、且接受任何值。现按 Chrome：默认值逐项相同；无效枚举值**静默忽略**（含大小写敏感：`'LTR'`、`'BUTT'` 无效）、
`direction='inherit'` 解析为 `'ltr'`、非有限数/负 shadowBlur/非正 miterLimit 忽略、letterSpacing/wordSpacing 只收**无空白、带单位**的 CSS 长度（`'0'` 与 `'5'` 无单位被拒、`' 4px '` 被拒）；
`setLineDash`（奇数长度自动复制、含负数/NaN 整体忽略、迭代器可用）/`getLineDash`（每次新数组）、`reset()`、`roundRect()`（半径数量与负值的 RangeError 措辞、对象半径、真实光栅化）、`createConicGradient`、
`getContextAttributes`（含 colorType/toneMapping）、`isContextLost`、`drawFocusIfNeeded`。这些属性**无渲染效果**（软件光栅器没有阴影/滤镜管线，与此前一致），只是状态面。
**仍未处理**：`createLinearGradient` 等返回的是对象字面量而不是 CanvasGradient 实例（实例审计 CanvasGradient 5 个自有键），下一刀。
验收：全量（跳两个毒化页面测试）995 过/30 败，失败集与干净 HEAD 相同；ratchet OK；analyze 契约零违；实例审计带泄漏对象 21/127。

## M4 刀42：CanvasGradient / CanvasPattern 真接口（2026-10-30）

`platform/canvasobjects.impl.js`。此前 `createLinearGradient` 返回 `{ addColorStop(){}, _x0, _y0, _x1, _y1 }`、`createRadialGradient` 返回 `{ addColorStop(){} }`、`createPattern` 返回 `{}`：不是
CanvasGradient/CanvasPattern 实例、无任何参数校验、`_x0` 等键外泄。Chrome 145：非法构造器接口；工厂的参数个数（`4 arguments required`/`6`/`3`/`2`）、非有限数（`The provided double value is non-finite.`）、
**负半径为 IndexSizeError**（`The r0 provided is less than 0.`）；`addColorStop` 的 `(0.0, 1.0)` 范围检查（IndexSizeError，措辞里带实际值）与颜色解析失败（SyntaxError `The value provided ('zz') could not be parsed as a color.`，
颜色经 `CSS.supports('color', v)` 判定，`currentcolor`/`transparent` 放行，空串拒绝）；`createPattern` 的图像类型联合措辞、repetition 枚举（`''`/null 等同 repeat，其余无效为 SyntaxError）、
0 尺寸 canvas 为 InvalidStateError、未加载完的 img 返回 null；`CanvasPattern.setTransform`；`fillStyle = gradient` 往返身份保持。探针 `cvobj-probe.js`（16 键）0 差异。
同样没有渲染效果（软件光栅器没有渐变管线），状态被记录供将来使用。canvasstate 里临时的 createConicGradient 移入本模块。实例审计 CanvasGradient 清零（带泄漏对象 21→20/127）。
**教训（已第二次踩）**：平台模块里写 `CSS`/`HTMLCanvasElement` 这类**自由变量**会触发契约违规与棘轮上涨——一律用 `globalThis.X`（属性访问不计入）。
验收：全量（跳两个毒化页面测试）996 过/30 败，失败集与干净 HEAD 相同；ratchet OK；契约零违；check_fresh 7 件鲜。

## M4 刀43：Attr 成为 Node（2026-10-30）

Chrome 的 `Attr` 是 `Node` 的子类；引擎的 Attr 是个不挂 DOM 节点 id 的普通 JS 类（`attr instanceof Node` 为假、7 个实例自有键、公开构造器可用）。本刀：
①bootstrap 里把 `Attr.prototype` 接到 `Node.prototype` 之下（**必须写在 bootstrap 同文件**：放进平台模块会算「跨文件原型改写」让依赖棘轮 16→17 变红，刚好被「棘轮行单独看」的新规矩拦下）；
②Node 上基于 id 的成员对 Attr 会抛 Illegal invocation，故在 Attr 上覆盖为属性节点的正确答案：parentNode/parentElement/firstChild/lastChild/previousSibling/nextSibling 为 null、childNodes 为空、hasChildNodes 假、isConnected 假、textContent=value、ownerDocument=document；
nodeType/nodeName/nodeValue 并入刀23 的 Node.prototype 分派表（`nodelevel.impl.js`，Attr 终于能加入）；③公开构造器非法（`Failed to construct 'Attr': Illegal constructor`），引擎内部经隐藏工厂 `__diting_mkAttr` 创建（6 处调用点已改）；
④`value` 现在**与宿主元素联动**：附着时读写都经 ownerElement（`attr.value = 'v2'` 会真改元素的属性，此前只改 Attr 对象自己；Chrome 里 Attr 与元素属性是同一件事）；⑤其余 7 个字段由 lift_own 提升为原型访问器，实例无自有键。
探针 `attr-probe.js`（真 Chrome 145，11 键）与引擎一致，除两项**已知差异**（测试里显式豁免并说明）：
- **身份**：`el.getAttributeNode('id') === el.getAttributeNode('id')` 在引擎为假（每次新对象），且 `el.attributes` 仍是「普通对象数组」而非 NamedNodeMap——属于下一刀（NamedNodeMap 接口 + 按元素缓存的活 Attr 节点）；
- 提升字段与 Node 成员覆盖在原型上显示为 rw/多出成员（Chrome 里只读 / 在 Node.prototype）。
验收：全量（跳两个毒化页面测试）997 过/30 败，失败集与干净 HEAD 相同；ratchet OK；契约零违；check_fresh 7 件鲜（46 真值）；实例审计带泄漏对象 19/127。XPath 以属性节点作上下文的用例仍全绿。

## M4 刀44：NamedNodeMap + 活的 Attr 节点（2026-10-30）

`platform/attributes.impl.js`（模块 `attributes`，探针 `nnm-probe.js` 37 键 0 差异）。此前每次访问 `element.attributes` 都新建一个「普通对象的数组」（`el.attributes !== el.attributes`，`getNamedItem` 等作为数组自有属性），
`getAttributeNode` 每次返回新对象且值不跟随元素。现在：
- **每元素一个 NamedNodeMap**（Proxy 实现 legacy platform object：索引与属性名两类属性；索引可枚举、名字不可枚举，`Object.getOwnPropertyNames` 为 `['0','1','2','id','class','data-x']`，`Object.keys` 只有索引；`for-in` 含原型上的可枚举方法；
  `length` 实时；`Symbol.iterator === Array.prototype.values`；构造器非法）；`item/getNamedItem(NS)/setNamedItem(NS)/removeNamedItem(NS)` 的参数个数、`InUseAttributeError`、`NotFoundError`（措辞含 `No item with name 'urn:x::zz' was found.`）逐字对拍。
- **每个属性一个 Attr 节点**：属性存在期间身份不变（`getAttributeNode === attributes[i] === getNamedItem`）；`value` 双向跟随元素（刀43 已接）；**移除时脱离**（`ownerElement` 变 null，保留最后一个值），**重新添加得到一个新 Attr**；
  `setAttributeNode` 返回被替换的旧节点（并使其脱离）、属于其他元素时抛 InUseAttributeError、重复放同一个返回自身。
- 移除路径拦截：`removeAttribute`/`removeAttributeNS`/`toggleAttribute`/`removeAttributeNode`/`removeNamedItem` 在 DOM 真移除前把缓存的 Attr 脱离（先取走值）。
- 命名空间：引擎 DOM 只按限定名存属性、丢命名空间，故模块**记录** `setAttributeNS` 的命名空间，`getAttributeNS`/`hasAttributeNS`/`getNamedItemNS`/`removeNamedItemNS` 按（命名空间，localName）查找；`createAttributeNS` 现在把 `localName` 设为冒号后部分。
**引擎自用方也受益**：`isEqualNode`、outerHTML 等读 `node.attributes` 的内部代码照常工作（NamedNodeMap 支持索引与 length）；XPath 模块自带的属性节点缓存现在是冗余的（保留，无害）。
**已知局限**：HTML 文档的属性 DOM 不记录命名空间，所以由解析器产生的 `xlink:href` 之类属性的 `namespaceURI` 仍为 null；内部 JS 之外（Rust 侧解析/innerHTML 重建）造成的属性移除不会让已缓存的 Attr 脱离（元素被替换，旧 Attr 持有旧元素）。
验收：全量（跳两个毒化页面测试）998 过/30 败，失败集与干净 HEAD 相同；ratchet OK；契约零违；check_fresh 7 件鲜（47 真值）；实例审计带泄漏对象 18/127。


## M4 刀45：实例自有键泄漏清扫（2026-10-30）

实例审计带泄漏对象 18→14（127 个对象）。`TextDecoder` 不再用 `defineProperty` 在实例上造 `encoding/fatal/ignoreBOM`，改普通赋值，由 `lift_own.js` 提升到原型存取器；
`measureText` 返回真正的 `TextMetrics` 实例（原先是普通对象字面量）；`lift_own.js` 纳入 `RTCPeerConnection`（10 个状态/回调属性）、`CSSStyleSheet`（`cssRules/ownerRule`）、
`MediaQueryList.media`、`TextMetrics.actualBoundingBoxLeft/Right`。`gen_lift_own.py` 现在重跑时并入输出文件里已有的 DATA（审计看到的是已提升状态，只会列出新增残余，覆盖会丢旧项）。
仍泄漏（需逐类处理）：`CSSStyleDeclaration`（383 个属性名，策略：只补引擎真支持的）、`Image`（`defineProperty` 造的属性）、`PluginArray/MimeTypeArray`（具名属性）、`WebSocket`（动态 `_` 名）、
`Location`、`Window`、`ServiceWorkerContainer`、`FileReader`（`_result/_error`）、`NodeList/HTMLCollection.length`、`HTMLOptionElement.text`、`Document.location`。
验收：全量 998 过/30 败，失败集与干净 HEAD 相同；surface 快照 fixture 按预期更新（四个类的原型成员增加）。

**工具坑**：探针服务器 `target/debug/diting` 属于包 `diting-browser`，重建必须 `cargo build -p diting-browser --bin diting`（`-p diting` 只建库，二进制不更新，审计会读到旧代码）。

## M4 刀46：Image 与 __acc 私有槽（2026-10-30）

泄漏对象 14→11。`new Image()` 不再在实例上造 `complete/naturalWidth/naturalHeight/width/height/src` 自有属性与 `__onHandlers`：每图状态进 bootstrap 持有的 WeakMap（隐藏全局 `__diting_imgState`），
新模块 `platform/imagestate.impl.js` 包装 `HTMLImageElement.prototype` 上的这六个访问器——有状态的 Image() 元素先读状态，其余 `<img>` 回落原访问器；给 Image() 元素设 `src` 时启动加载模拟（`state.load`）。
页面自己在元素上定义的 `src` 自然遮蔽原型访问器，模拟不再运行（原先要显式探测 ownSrc）。`__acc`（WebSocket/FileReader/EventSource/BroadcastChannel 的原型访问器）
登记它造出的 `_x` 私有槽名到隐藏集 `__diting_accPrivs`，实例枚举补丁一并隐藏 → `WebSocket._bufferedAmount…`、`FileReader._result/_error` 不再外泄。
剩余：CSSStyleDeclaration(383)、PluginArray/MimeTypeArray 具名属性、Location、Window、ServiceWorkerContainer、MediaQueryList(matches/media 由 matchMedia 返回物 defineProperty 造)、NodeList/HTMLCollection.length、HTMLOptionElement.text、Document.location。
验收：全量 998 过/30 败（失败集同干净 HEAD）；ratchet/契约/check_fresh 过。

## M4 刀47：MediaQueryList / Option / NodeList（2026-10-30）

泄漏对象 11→8。`MediaQueryList.matches/media` 由实例 `defineProperties` 改为原型只读访问器（无 setter，与 Chrome 一致）；`Option.prototype` 改为 `HTMLOptionElement.prototype`（原为 `HTMLElement.prototype`，
`new Option()` 的原型链错），并删掉实例上的 `text` 自有访问器（原型上本就有）；`NodeList.length` 改为原型访问器 + 隐藏 `_len` 槽（`__accPrivs` 登记后实例枚举隐藏）。
未动：`HTMLCollection.length`（继承 Array，自有 `length` 不可配置，无法隐藏——要改需脱离 Array 子类，另立刀）。
surface 快照按预期更新（`NodeList` 多 `length`，`Option` 原型改指）。验收：全量 998 过/30 败同干净 HEAD；ratchet/check_fresh 过。

## M4 刀48：PluginArray / MimeTypeArray 具名属性与 Location 自有布局（2026-10-30）

泄漏对象 8→5。`navigator.plugins['PDF Viewer']`、`navigator.mimeTypes['application/pdf']` 原先返回 undefined；现在按 legacy platform object 加不可枚举的自有具名属性（同名先到先得）。
`location` 对齐 Chrome 的 [LegacyUnforgeable] 布局（`window_extras.js`）：原型改为 `Location.prototype`（原为 Object.prototype；toStringTag 来自原型而非实例自有），自有成员按
`valueOf, ancestorOrigins, href, origin, protocol, host, hostname, port, pathname, search, hash, assign, reload, replace, toString` 重排，`valueOf` 为 `Object.prototype.valueOf` 的同一函数（不可写不可配），
自有 `Symbol.toPrimitive`（undefined，冻结），去掉实例上的 `fragmentDirective`，补 `ancestorOrigins`（空 DOMStringList）。对拍探针 `q2`：descriptor/名序/同一性与 Chrome 一致。
**坑**：模块里用 `{}` 当描述符表时 `D.valueOf` 继承自 Object.prototype，`if (!D.valueOf)` 恒假——键名可能撞 Object.prototype 时用 `Object.create(null)`。
剩余：CSSStyleDeclaration(383)、Window(FontFaceSet/CSSPseudoElement)、ServiceWorkerContainer、HTMLCollection.length、Document.location。验收：全量 998 过/30 败同干净 HEAD。

## M4 刀49：Document.location 自有化与 ServiceWorkerContainer 去空桩（2026-10-30）

泄漏对象 5→3（127 个）。`document.location` 现在是主文档实例上的自有不可配置访问器（Chrome：[LegacyUnforgeable]），`Document.prototype.location` 删除；描述符由隐藏全局 `__diting_docLocDesc` 保存，
`document` 全局 getter 首次包装主文档时挂到实例上。`navigator.serviceWorker` 去掉三个空操作的自有 `addEventListener/removeEventListener/dispatchEvent`，改走 EventTarget 原型链（真实现）：
`dispatchEvent(new Event('x'))` 现在返回 `true`（Chrome 行为；旧桩返回 false），`test_issue_81_service_worker_event_target` 的期望按 Chrome 真值改为 true。
剩余泄漏：`CSSStyleDeclaration`(383 属性名)、`Window`(FontFaceSet/CSSPseudoElement 引擎内部使用)、`HTMLCollection.length`(继承 Array)。
注意：iframe 等非主文档的 Document 包装对象不再有 `location`（它们原先靠原型）；全量测试无回归。验收：全量 998 过/30 败同干净 HEAD。

## M4 刀50：CSSStyleDeclaration 属性名枚举（2026-10-30）

泄漏对象 3→2（127 个）。样式对象是 Proxy，自有键来自 bootstrap 里的名单，只有 309/690 个 Chrome 145 的 camelCase 属性名。新模块 `platform/cssnames.impl.js`
通过隐藏全局 `__diting_cssNames`（`{list, set}`，bootstrap 只多一行）把名单原地替换成 Chrome 的 690 个名字（字母序，与实例枚举一致）。
**策略澄清**：未知属性本来就能被 `setProperty` 和 `style.xxx = v` 存储、读回并参与 `cssText` 序列化（`_cssDeclAccepted` 只对长度类属性做 token 校验），缺的只是枚举与 `in`；
所以补全名单并不新增任何「声称支持」的能力，布局仍只实现它实现的那部分。`cssFloat` 不再枚举（Chrome 145 实例上没有），但保留 get/set/has 识别。
剩余泄漏：`Window`(FontFaceSet/CSSPseudoElement，引擎内部使用)、`HTMLCollection.length`(继承 Array)。验收：全量 998 过/30 败同干净 HEAD；ratchet/契约/check_fresh 过。

## M4 刀51：HTMLCollection 家族重做 + getElementById 脱离节点修复（2026-10-30）

泄漏对象 2→1。`HTMLCollection` 原为 `class extends Array` + Proxy：`Array.isArray` 为真、带全套 Array 方法、自有不可配置 `length`、公开静态 `_from`。
新模块 `platform/collections.impl.js`（不做兼容，旧实现整段删除）：
- **HTMLCollection** = legacy platform object：Proxy over `Object.create(HTMLCollection.prototype)`，后备快照数组在 WeakMap。索引属性可枚举只读（`writable:false`），具名属性（id / HTML `name`，先 id 后 name，去重）为不可枚举自有，
  原型成员遮蔽具名（`<p id="item">` 不盖 `item`），`length` 是原型只读访问器，只有 `item/namedItem/@@iterator(=Array.prototype.values)`，构造器非法，`Object.defineProperty` 索引键抛 Chrome 原文 TypeError。
- **HTMLOptionsCollection**（`select.options`）：`length` 可写（截断/补空 option）、`selectedIndex`、`add`、`remove`，原先是给数组实例硬塞的自有属性。
- **HTMLFormControlsCollection**（`form.elements`）+ **RadioNodeList**：同名多匹配返回 `RadioNodeList`（NodeList 子类，`value` 取选中单选项/按值勾选）。
- 工厂改为隐藏全局 `__diting_mkCollection(items, kind, owner)`，不再有 `HTMLCollection._from`；全部内部调用点（bootstrap、domparser、docmisc、htmlreflect）已改。
- 探针 `coll-probe.js` 25 键对 Chrome 145 0 差异（`coll-truth-145.json`，测试 `htmlcollection_family_matches_chrome_truth`）。严格模式下 set/delete 的报错措辞无法在 Proxy trap 内区分（不知调用方是否严格），探针不含该项。

**顺手修的引擎 bug**：`set_inner_html` 只 `detach` 旧子节点，`id_index`（保留首个）仍指向旧节点且永远学不到同 id 的新元素 → 同容器重复 `innerHTML = 同样 id 的标记` 后 `getElementById` 返回已脱离的旧节点、`isConnected` 为假
（Chrome 只返回文档树内的节点）。`get_element_by_id` 现在要求索引命中必须在文档里，否则重新扫描并刷新索引；脱离节点按 Chrome 不再被找到。测试 `get_element_by_id_ignores_detached_nodes_after_inner_html`。

**发现（下一刀）**：`JSON.stringify(element)`、`for-in`、`Object.assign`/展开会看到引擎内部的 `_nid/_style/_templateContent`（Node 构造器与懒建字段为可枚举自有属性）；文本/注释/片段/Range/DocumentType/Blob/File/FormData/MutationObserver/XHR/WebSocket/ReadableStream 等同样。
此前实例审计走打过补丁的 `getOwnPropertyNames`，看不到这一面。
验收：engine 全量 1000 过/30 败（同干净 HEAD）；diting-browser 464 过/46 败（同干净 HEAD）；ratchet/契约/check_fresh 过。

## M4 刀52：document.fonts 做成真 FontFaceSet、FontFace 重做、Window 额外全局清零（2026-10-30）

**实例审计 127/127 全部干净**（leaky 0；整面审计 936 接口：同 744、缺 118、多 29）。
`document.fonts` 原先是一个**数组**（`Array.isArray` 为真，`size` 为 undefined，`forEach/has/check/...` 作为自有属性嫁接，里面是伪造的"常见字体" FontFace 标签对象，`check()` 对未知字体返回 false），
`FontFaceSet`/`CSSPseudoElement` 这两个 Chrome 145 并不暴露的接口站在 window 上。新模块 `platform/fontfaceset.impl.js`（旧实现整段删除，bootstrap 少约 150 行）：
- **FontFaceSet**（接口对象不暴露、原型无自有 `constructor`，继承 EventTarget）：`size/status/ready/check/load/add/clear/delete/entries/forEach/has/keys/values/@@iterator`，成员顺序与 Chrome 一致；
  插入序去重；默认**空集**（Chrome 不预装字体）；`check()` 对没有 FontFace 声明的家族一律返回 true，`font` 简写必须可解析否则 `SyntaxError: Could not resolve '…' as a font.`；`load()` 返回 Promise，无参/非法走 rejected；`keys !== values`、`@@iterator === values`；
  非法 this 的 promise 属性返回 rejected promise 而不是同步抛。
- **FontFace** 移入模块：完整 Chrome 145 描述符面（补 `ascentOverride/descentOverride/lineGapOverride/sizeAdjust/variationSettings`，去掉非标准的 `source` 访问器，默认值 `weight:'normal'`、`sizeAdjust:'100%'`），
  setter/构造器按各描述符语法校验（非法 setter 抛 `SyntaxError: Failed to set '…' as a property value.`；构造器里非法描述符/来源不抛，状态变 `error`），`unicodeRange` 规范化（`U+0041`→`U+41`），`loaded` 稳定同一 Promise，`load()` 幂等。
- `Document.prototype.fonts` 现在是模块里的原型 getter，每个文档一个稳定实例。
- `FontFaceSet`/`CSSPseudoElement` 加入 `version_prune.js`；window 上多余的自有 `Symbol.toStringTag` 删除（原型链已到 Window.prototype）。
- 探针 `fonts-probe.js` 27 键对 Chrome 145 0 差（`fonts-truth-145.json`，测试 `fonts_matches_chrome_truth`）。`fonts_for_each_iterates_without_recursing` 改为先 `add` 一个 FontFace（Chrome 的集合默认为空）。
- `shape_fix.js`/`internal_names.js` 按新引擎重新生成（`regen_shape.sh`、`gen_internal_names.py`；FontFace 的成员顺序原先按旧引擎写死）；`check_fresh --update` 把 coll/fonts 真值登记进 MANIFEST。
**行为变化（有意，对齐 Chrome）**：`document.fonts.check('12px "不存在的字体"')` 现在是 true；`document.fonts.size` 为 0；遍历默认无元素。依赖"字体探测"的指纹脚本在 Chrome 上读到的就是这些值。
验收：engine 全量 1001 过/30 败（同干净 HEAD）；diting-browser 464 过/46 败（同干净 HEAD）；ratchet/契约/check_fresh 过。

## M4 刀53a：window 的可枚举自有键对齐 Chrome（300 → 0）（2026-10-30）

新审计维度 `chrome/enum-probe.js` + `tools/webplatform/enum_audit.py`：每个对象的**自有可枚举键**（`for-in`+`hasOwnProperty`，即 `for-in`、`JSON.stringify`、`Object.assign`、展开看到的集合）。
引擎对 `Object.keys/getOwnPropertyNames/Reflect.ownKeys` 打了补丁来藏内部名字，**补丁遮住了真实泄漏**——此前"实例审计 127/127 干净"只对补丁后的视图成立。
`window` 原本有 537 个自有可枚举键（Chrome 237），多出 300 个：顶层 `function`/`var` 声明（`_wrap`、`_idbStore`、`makeKey`…）是**不可配置、可枚举**的 window 属性，`defineProperty` 改不了。
根治：**每个脚本的顶层都放进函数作用域**，只把别的脚本/Rust 真正用到的名字重新导出到 globalThis（不可枚举、可配置、登记进隐藏名单）：
- 17 个脚本（bootstrap + 16 个有顶层 function/var 的）以 `(function () { // @@SCOPE` … `}).call(globalThis); // @@SCOPE-END` 包起，尾部有生成的 `// @@EXPORTS-BEGIN … // @@EXPORTS-END` 块。
- 块由 `node tools/webplatform/analyze/analyze.mjs --gen-exports` 生成：名字 = 依赖分析里别的文件对它的 load/call/**前向 call** 引用（分析工具原先漏掉了「早加载脚本调用时引用晚加载脚本」，已补 `forwardCallRequires`）∪ `exports-extra.json`（Rust 字符串里引用的名字，解析器看不到）。
  function/const/class → 数据属性；let/var → 访问器（Rust 的 `globalThis._fpCache = null` 要写穿到局部变量）；已存在的 window 属性不动（Chrome 可见全局保持原值）；`--check` 在块过期时失败。
- Rust PRELUDE 的 `function __diting_register` 改 `const`（脚本级词法绑定，所有后续脚本可见但不是 window 属性）。
- 运行时才赋值的内部状态（`__diting_objects/oid/hw/mem/ua/lang…`、`__parserScript*`、`__currentScriptNid`、`__diting_click_target`、`__documentReadyState__`）由 `window_order.js` 预声明为隐藏占位，后续赋值保留不可枚举；window 上 Chrome 没有的可配置键在重排时一律置为不可枚举。
- 枚举补丁的 `_isInternal` 现在对所有名字查隐藏名单（导出的非下划线内部名如 `REFERRER_POLICIES`）。
测试 `window_enumerable_own_keys_match_chrome_truth`（`enum-truth-145.json`，237 键）。验收：engine 1001 过/30 败、diting-browser 464/46 败、diting_runtime 全过（均同干净 HEAD）；ratchet/契约/check_fresh 过。
**仍待做（刀53b）**：实例上的 `_x` 私有字段（`_nid/_style/_templateContent`、Range/Blob/XHR/WebSocket/ReadableStream/…）、单例对象的自有方法（speechSynthesis/mediaDevices/caches/rs reader/AudioNode…）、DOMRect/PerformanceTiming 自有数据属性、getComputedStyle 的索引键。

## M4 刀53b：实例的自有可枚举键与类身份（泄漏对象 127 → 11 的审计口径）（2026-10-31）

审计口径（刀53a 起）= `for-in`+`hasOwnProperty`，覆盖 211 个对象（每个元素标签、各类节点、Range、事件、XHR/WebSocket/流、单例、画布、观察者、URL 家族……）。
本刀处理 **实例上的 `_x` 私有字段** 与 **类身份错误的对象**：
- **隐藏型访问器 `hide_priv.js`（生成，`gen_hide_priv.py`）**：engine 的类把私有状态当普通自有属性（`this._nid = nid`），for-in/JSON/spread 全看得见。不改几十个构造器，而是给每个这样的字段名在**最近的公共原型**上放一对访问器：
  setter 把第一次赋值变成实例上**不可枚举**的自有数据属性，之后读写直接命中自有属性；getter 返回 undefined（与字段缺失时一致）。名单来自 `enum-probe.js` 抓取的 `#chains`（每个对象的原型链）取最深公共祖先；
  `hide_priv_extra.json` 补探针走不到的路径（`Node._ownerDocument`）；生成器合并已有名单（隐藏后的抓取已看不到被隐藏的字段）。
- **单例并入类原型**（`demote_own.js` TARGETS）：`navigator.storage`→StorageManager、`mediaSession`、`mediaDevices`、`speechSynthesis`、`caches`→CacheStorage、`document.implementation`→DOMImplementation（现在每文档一个缓存实例，原先每次读都是新对象字面量）、
  `performance.timing`→PerformanceTiming；自有 `_x` 在单例上置为不可枚举；mediaDevices/speechSynthesis 的空操作 EventTarget 三件套删除，走真 EventTarget（voiceschanged 用 `dispatchEvent`）。
- **类身份**：`getBoundingClientRect()` 返回真 `DOMRect`（原为带 `toJSON` 自有属性的字面量，`[object Object]`）；`getClientRects()` 返回 `DOMRectList`（`domrectlist.impl.js`，原为数组+自有 `item`）；
  `document.styleSheets` 返回 `StyleSheetList`（`stylesheetlist.impl.js`）；MutationObserver 记录是 `MutationRecord` 实例、`addedNodes/removedNodes` 是 NodeList（`mutationrecord.impl.js`）；
  ReadableStream 的 reader/controller 成员移到各自原型（状态在 WeakMap）；`new Audio(url)` 是真 `<audio>` 元素且 `Audio.prototype === HTMLAudioElement.prototype`（原来直接抛 Illegal invocation）；`DragEvent` 可构造（`dragevent.impl.js`，原为忽略参数的桩）；
  `OffscreenCanvas.width/height` 提升到原型；`console` 按 Chrome 顺序补齐 `dirxml/profile/profileEnd/timeStamp/memory`，`@@toStringTag` 为 `'console'`；样式对象的 `epub*` 名字不可枚举。
- `performance.measure` 解析 PerformanceTiming 名字改用 `name in timing`（成员已在原型上）。
测试 `own_enumerable_keys_match_chrome_truth`（`enum-truth-145.json`）：~200 个对象逐个对拍，**已知缺口显式列出且只能缩不能涨**：
`computed`（getComputedStyle 枚举 690 个名字，Chrome 枚举索引）、`newdoc`（createHTMLDocument/DOMParser 文档是普通对象）、`gain/ctxDest`（Web Audio 节点是普通对象）、`gl`（无头 Chrome 的 WebGL 是 null，引擎给假上下文）。
验收：engine 1003 过/30 败、diting-browser 464/46 败、diting_runtime 全过（均同干净 HEAD）；ratchet/契约/check_fresh 过。

## M4 刀54：getComputedStyle 的枚举形状对齐 Chrome（2026-10-31）

`getComputedStyle(el)` 原来：`length` 444（含简写如 `margin`/`border`/`font`），没有任何索引键，名字只枚举 444 个。Chrome 145：`length` **413**（只有长声明 longhand，顺序即 Chrome 的），
自有可枚举键 = **413 个索引 + 681 个 camelCase 名**（`epub*` 9 个不可枚举，共 690 个名，与 `element.style` 同一份名单，见 `platform/cssnames.impl.js`），`getOwnPropertyNames` 共 1103 个。
改动（均在 `getComputedStyle` 的 Proxy 里）：`_COMPUTED_PROPS_KEBAB` 换成 Chrome 的 413 个 longhand（bootstrap 少 ~30 行）；`ownKeys` = 索引 + `__diting_cssNames.list`；`getOwnPropertyDescriptor`/`has` 认索引与全部 690 名
（索引为只读可枚举值，名字为带实时 `lookup` 值的可写数据属性）。简写（`margin`…）仍可读取，只是不再是 computed 声明的「项」。
`element_staged_members_match_chrome_truth` 的 KNOWN_GAPS 清空（`gcs.len` 现在等于 Chrome 的 413）；`own_enumerable_keys_match_chrome_truth` 不再跳过 `computed`。
枚举审计：211 个对象 204 个干净；剩 `newdoc`（createHTMLDocument/DOMParser 文档是普通对象）、`gain`/`ctxDest`（Web Audio 节点）、`gl`（WebGL，待拍板）和三个依赖宿主页面计数的噪声键。
验收：engine 1002 过/31 败中，唯一多出的 `docmisc_matches_chrome_truth` 是 `lastModified` 两次读取跨秒的时间型偶发（单独跑三次全过）；其余失败集同干净 HEAD；diting-browser 同基线；ratchet/契约/check_fresh 过。

## M4 刀55：Element 厨房水槽搬离（94 个鬼成员清零）（2026-10-31）

整面形状审计（`shape_audit.py` + `shape-audit-truth-145.json`）的口径从「缺」转「多」：**Element.prototype 上有 94 个 Chrome 在任何接口上都没有的成员**——
引擎历史设计把全部 HTML/SVG 反射成员带 localName 守卫装在 Element.prototype（#27「厨房水槽」，再镜像到子接口），于是 `<div>.value` 答空串、`<div>.rows` 答 undefined 之外的鬼、
for-in 一览无余、`getOwnPropertyDescriptor(Element.prototype,'requestSubmit')` 撒谎。本刀把 #27 的 MIRROR 翻成 **MOVE**：
- **`_interfaceProps` 扩表**（bootstrap）：镜像表补方法家族——dialog（close/requestClose/show/showModal）、表格十二件（create*/delete*/insertRow…落 HTMLTableElement/Row/Section）、label.control、datalist.options；同名描述符定义到子接口（守卫体不变，行为保持），随后 `element_sink.js` 把 94 个名字从 Element.prototype **delete**。
- **`element_sink.js`（新，挂在 platform_bundle 之后）**：SVG 落家（SVGAElement: href/rel/relList/type，SVGUse/Image/Pattern/Gradient: href，SVGStyle: sheet/disabled/type，SVGSVG/Marker/Pattern: viewBox——描述符守卫本来就认这些 localName，搬=行为保持）+ 94 删单（冻结自审计，防后来者回填）。SVG 成员的 Chrome 真形（SVGAnimatedString 等）归 SVG 家族刀。
- **selection 家族改挂真家**：selectionStart/End/Direction、setSelectionRange/setRangeText/select 从 Element.prototype 改装 HTMLInputElement+HTMLTextAreaElement（Chrome 真位）；`setSelectionRange.length`=2、`setRangeText.length`=1 对齐（可选参数走 `arguments`）。坑：中途把 `direction` 声明成 `const` 又重赋——运行时才炸「Assignment to constant variable」，改 `let`。
- `assignedSlots`（复数）Chrome 145 全表面无此成员（只有 `assignedSlot` 单数），直接删除；`isConnected` Node 已有，删 Element 冗余副本。
- shape_fix.js 再生循环收敛搬迁后的 order/kind 噪声。

数字：整面 extra 成员 152 → **58**（Element 94 清零），missing 637 → 603（SVG/表格落家补缺 34），same 接口 740 → 748。表格式枚举审计 204/211 干净（7 个已知缺口不变）。
表格/dialog/label/datalist 家族成员集与 Chrome **逐名全等**（对拍脚本验证）。`div.value`/`div.rows`/`div.type` 现答 undefined（Chrome 真）。
测试改动：`element_prototype_members_match_chrome_truth`（新，钉死 147 名单=Chrome）；4 个钉旧发明的测试改钉真值（slot 的 assignedSlots、div.labels=undefined、section/row/table 面名单）。
验收：`cargo test -p diting` 失败集与干净 HEAD **逐名全等**（110 个既有环境败）；ratchet（baseline 51→52，element_sink→bootstrap:Element 一条合法新边）/契约/check_fresh 过。
**教训**：取基线用 `git stash` 前先想清楚未跟踪文件——`stash push` 带不存在路径会失败，随后误弹上一会话的旧 stash；已原样存回。

## M4 刀56：挂错原型层家族折翼——Attr 11 + Range 5×2 + EventTarget 三件套×7（2026-10-31）

58 个 extra 拆族后的「挂错原型层」一族：成员本身 Chrome 有，但挂在 Chrome 不放它的原型上。三摊：
- **Attr +11**（childNodes/firstChild/lastChild/hasChildNodes/isConnected/nextSibling/ownerDocument/parentElement/parentNode/previousSibling/textContent）：Attr 链上 Node.prototype 后，其 id 基成员会对 Attr 炸，类体只好手搓 11 个。正解=**nodelevel 派发表扩容**（刀23 只收 nodeType/nodeName/nodeValue/cloneNode 四名）：15 名按 getter/accessor/method 三组批量收表、批量装派发器，Attr 类体的 11 个被吊走后 Attr.prototype 恰剩 Chrome 的 8 名。热路径元素走 WeakMap 缓存 miss-once 后落 base getter（刀23 已验证同型）。坑：NP.isConnected 原是 defineProperty 装的**非配置**属性，派发器 redefine 直接炸快照——补 `configurable:true`（Chrome 真形 ro:ec）。
- **Range/StaticRange 各 +5**（startContainer/startOffset/endContainer/endOffset/collapsed）：AbstractRange.prototype 早有五件、双原型链也对，纯冗余 own 副本。新脚本 **`range_lift.js`**（挂 global_surface 之后、stub_members 之前——AbstractRange 要等 global_surface 铸、stub 的 fill-if-missing 要让位真体）：五 getter（体=range_selection 原文，`_rngSame` 因文件词法不可达而内联）装上 AbstractRange（ro:ec、`__diting_markNative` 防 toString 漏源码），两叶类 delete。
- **EventTarget 三件套×7**（BroadcastChannel/EventSource/MediaQueryList/WebSocket 各 3，IDBDatabase/IDBTransaction/MessagePort 各 2）：七家早已链到 EventTarget.prototype（native 核心，M3 B1），own 拷贝全是语义更弱的影子（无 options/once/capture、手搓 Map）。本刀全数拆除、统一继承：
  - `__lboxProto` 退役（函数+三调用点+三个 `_lbox` 字段）；ES/WS/BC 发火路径删手动 on* 腿——native dispatch 的 handler 钩子（isHandlerSlot 查原型 accessor）本来就会发，双发才错。BC._fire 换真 `MessageEvent`（原假事件对象连 instanceof 都过不了）。
  - MessagePort：类体三件套删，`_fire` 走 native dispatch；**#237 防丢件账本不能瞎**——native 注册表 JS 不可见，故 Rust 侧 `remove_entry` 补 **`unregistered(target,type)` 钩子**（显式移除/once 排干/abort 清理三条路都过这），event_hooks 维护 WeakMap 镜像 + 隐藏口 `__diting_etHasListeners(t,ty)`；孤儿判定与 sweep 剪枝改查镜像；「'message' 监听即 start()」搬进 registered 钩子腿。
  - MediaQueryList：三件套删（addListener/removeListener 保留，Chrome 有）；registry 剪枝同查镜像；`_change` 数组随三件套死。
  - IDB：`_idbProtoOps` 两名单摘掉三件套转发件（实例字面量自持 on* 路由方法，转发件纯为形状）。

数字：整面 extra 成员 58 → **17**（余下全为单件族=刀57），missing 603 不变，same 748 → 751；order 28 不变、kind 66→65（无新噪声，shape_fix 未再生）。枚举审计 204/211 干净（7 已知缺口不变）。ratchet 过（crossFileLoadTimeNames 52 不变；exports 块因 `__lboxProto` 退役刷新）。
行为验证（活引擎探针）：WS 真回声往返 open/message/close 各 1×1×1；BC onmessage+listener 各 1×、真 MessageEvent、target 正确；MQL 翻转 listener 与 onchange 同次各发；MessagePort 纯监听端口自启两投两达；IDB open→onsuccess 活；attr 树答（null/空 NodeList/value 直通/写穿元素）+ 元素 isConnected/childNodes 回退全对。
测试：`eventtarget_chains_match_chrome_truth_except_known_gaps` 的 SHADOWED_TRIO 棘轮例外表**清空**（七家下榜）；新增 `knife56_wrong_prototype_levels_fold_to_chrome`（三摊形状+行为钉死）与 `knife56_native_core_delivery_single_fire`（单发/真事件/注册可见性）。surface_snapshot 重生成。
验收：`cargo test -p diting` 失败集与干净 HEAD **逐名全等**（110 既有环境败，928 过=924+4）。
**教训**：`function get name(){}` 不是合法 JS（getter 名推导只在对象字面量/类体里成立），独立函数表达式要用 `function name(){}`；测 MQL 翻转先归一视口再翻，别赌默认视口宽度。

## M4 刀57：单件族清尾——extra 类清零 152→58→17→**0**（2026-10-31）

17 件散兵（16 接口各 1–2 件）按真源普查分三判：
- **DELETE 纯发明 ×11**：AudioContext.playbackStats、BaseAudioContext.renderQuantumSize、CSSCounterStyleRule.style、CSSPropertyRule.style、**CSSScopeRule.conditionText**（真源反转：Chrome 里 ScopeRule 挂 CSSGroupingRule 下、答 start/end，从无 conditionText）、Document.referrerPolicy（Chrome 的 Document.prototype 没有，只在元素/Request 上）、GPUAdapter.requestAdapterInfo、LaunchQueue.setTargetURL、Navigator.cpuPerformance、ResizeObserver.takeRecords（Chrome RO=observe/unobserve/disconnect 三件，takeRecords 属 Mutation/Intersection/Performance 家）、WebTransport.congestionControl。
- **MOVE→CSSConditionRule ×3**：Media/Container/Supports 的 conditionText 副本删，真 getter 装上 CSSConditionRule（原先是 stub_members 的 undefined 桩——**先验桩再删副本**，否则行为退化）；三子类沿链继承真值，CSSScopeRule 直接失去。
- **MOVE→Credential/ShadowRoot ×3**：PasswordCredential.id/type 走继承（Credential.prototype 换真 getter 读 `_id`/`_type`，ctor 补 `this._type="password"`，web_api_gaps 装桩替换位）；DocumentFragment 的 innerHTML 对（get+set）**删**、ShadowRoot 类体自有一对——Chrome 145 真探：plain fragment 与 template.content 读 innerHTML 皆 **undefined**、赋值=无解析裸 expando（firstChild 不变 null）、只有 ShadowRoot 真往返。

**断腿发现**：fragment 面（innerHTML 对）有三个内部用户原靠它解析——Element.innerHTML 的 template 特例、DocumentFragment.cloneNode(deep)、Range.createContextualFragment——删面后 template 解析当场死（kids=0）。三处全改 `_dom("set_inner_html", frag._nid, …)` 直调 op，template/深克隆/上下文档段全活。

数字：整面 **extra=0**（刀55 起 152→58→17→0），missing 111 接口/603 成员不变（ShadowRoot 仍缺 5：onslotchange/pointerLockElement/fullscreenElement/pictureInPictureElement/getSelection），same 751→**759**；order 28（ShadowRoot 因 innerHTML 入类一度 29，shape_fix 正规再生循环收敛回 28——ORDER.ShadowRoot 补 innerHTML、PROTO.Node 甩掉刀56 已原生正确的 isConnected）；kind 65 不变。枚举审计 204/211（7 已知缺口不变）；ratchet 过（crossFileLoadTimeNames 52 不变）。
shape_fix 再生：禁用→重建→裸捕获→gen→恢复→重建（生成器头部规定的舞步）；语义 diff 恰两条（见上）。internal_names 再生（`_congestionControl` 出列）。
测试：`knife57_singleton_extras_fold_to_chrome_truth`（新，16 接口形状+9 组行为钉死）；两个钉发明的测试改钉真值（observer 族 RO=undefined、Navigator 枚举面 83→82）；音频形状清单两处摘名。surface_snapshot 重生成（恰 17 行）。
验收：`cargo test -p diting` 失败集与基线**逐名全等**（110 既有环境败，929 过=925+4）；活引擎行为探针复验（真值 conditionText 三例、scope undefined、pc.id/type/name/password 继承答、sr 往返、df 裸 expando、template kids=2、clone/上下文档段解析）。
**教训**：删成员前先查真源该成员在 Chrome 的**全部宿主**（conditionText 的宿主是 CSSConditionRule 而非四个子类——判 MOVE 还是 DELETE 全靠它）；「桩在、副本真」的格局里删副本必先换桩为真（Credential 同理）；改原型面后必跑内部用户 grep——`content.innerHTML`/`frag.innerHTML` 这类顺手写法就是隐性依赖。

## M4 刀58：ElementInternals ARIAMixin 五十一件（missing 方向第一刀）（2026-10-31）

extra 清零后转 missing 方向，最大户 ElementInternals：Chrome 145 有而引擎无的 **51 件全是 ARIAMixin 反射族**（role + 43 nullable-DOMString aria* + 7 element-ref aria*，全 rw:ec），引擎只有表单验证 12 件。

**Chrome 真义三探钉死（三轮探针互证）**：ElementInternals 的 aria 存储**纯内部、两向都不同步**——宿主的 aria-*/role 内容属性喂不进 getter；setter 不碰属性；element 自己的 ARIAMixin 面（真反射属性）与 internals 面互相独立（`el.ariaLabel='A'` 与 `inr.ariaLabel='B'` 共存互不覆盖）。element-ref 七件（ariaActiveDescendantElement/Controls/DescribedBy/Details/ErrorMessage/FlowTo/LabelledBy Elements）同样零属性同步：存数组原样返回、null 清、[] 留空表。**假象警**：第一轮探针的「setter 写属性」实为手动 setAttribute 的残留值——对拍必须逐步归因，别让前一腿的副作用冒充本腿结论。

实现：bootstrap 的 ElementInternals 类后装机（44 字串件 + 7 element-ref 件，`this._aria`/`this._refs` 隐藏槽，internal_names 再生收名）；`v==null` 清、`String(v)` 强转（5→'5'）。

数字：missing 接口 111→**110**（ElementInternals 毕业），same 759→**760**，extra 0/order 28/kind 65 不变；枚举审计 204/211（7 缺口不变）；ratchet 过（crossFileLoadTimeNames 52）。shape_fix 正规再生恰一条 diff（ORDER.ElementInternals 全 Chrome 序）。surface_snapshot 重生成。
测试：`knife58_element_internals_aria_reflection_family`（新，63 面+13 组断言：缺省 null/属性不喂/setter 不碰/双面独立/强转/null 清/role/ref 三态/rw:ec/隐藏槽/toStringTag）。验收：`cargo test -p diting` 失败集与基线**逐名全等**（110 环境败，930 过=929+1）。
**教训**：对拍探针的每一步都可能被前一步污染——同页连续赋值/设属性后，下一腿的读数要先排除前腿残留；customElements 名字在同一 /eval 页里跨调用持久（脚本双注入即撞名），探针一律 `if (!customElements.get(...))` 守卫。

## M4 刀59：SVGSVGElement 三十七件 + SVG 值类型真存储（missing 方向第二刀）（2026-10-31）

missing 方向第二刀，最大户 SVGSVGElement：**34 原型件 + 3 zoom 静态**（37 mismatch），顺带 SVGPoint 的 x/y 两件（引擎原型只有 matrixTransform）。刀前该接口只有 viewBox——还是 element_sink 从 Element 厨房水槽搬来的守卫版。

**刀芯不止装面——面够到的存储全是空的**：八个工厂方法返回的 SVGAngle/SVGLength/SVGMatrix/SVGNumber/SVGPoint/SVGRect/SVGTransform 与 x/y/width/height 的 SVGAnimatedLength、preserveAspectRatio 的 SVGAnimatedPreserveAspectRatio，全是 global_surface 铸壳+stub_members 桩（成员在、返回 undefined）。新文件 **`svg_root.js`**（挂 web_api_gaps 之后、stub_members 之前——壳已铸、fill-if-missing 让位真体）一揽子装真：
- **SVGLength 属性视图模型**：挂靠长度（svg 的 x/y/width/height）是属性的 VIEW——读=parse 属性、写=序列化回写（探针：`baseVal.value=42`→属性 "42"；属性 '5pt'→值 fround(20/3)）。x/y 缺省 0-number；width/height 缺省 '100%' 对 **300×150 固有视口**解析（探针：width '100%'→300、height→150）。animVal 与 baseVal **分体对象**（`baseVal!==animVal`）但写一律 NoModificationAllowedError。脱挂长度纯存储；**脱挂 % 读 value 抛 NotSupportedError**（"Could not resolve relative length"，探针）。**float32 存储**：Chrome 存 f32——5pt=6.666666507720947、10mm=37.7952766418457，引擎 `Math.fround` 补齐。单位换算 96dpi 表（pt=4/3、pc=16、mm=96/25.4…）；convertToSpecifiedUnits 保 px 值换单位重排串（72pt→96px→'6pc'，探针）。
- **SVGMatrix 全纯函数**：探针 `m.a=2; m.translate(1,2)` 后原矩阵 e/f 仍 0——Chrome 145 的 translate/scale/rotate/inverse 等全返**新矩阵**不动自身（mutation 版当场翻案）。inverse 奇异抛 InvalidStateError。
- **SVGTransform**：type/matrix/angle 三 ro + set 族；matrix 每 transform 缓存恒等（`t.matrix===t.matrix`）；setMatrix/createSVGTransformFromMatrix **复制**（探针：源后改不影响）；setRotate(90,0,0) 答 Chrome 精确 cos90=6.123233995736766e-17。
- **SVGPreserveAspectRatio 属性视图**：align/meetOrSlice 双 rw，写回**全关键字串**（align=7→"xMaxYMid meet"，且**保留当前 meetOrSlice**——先设过 slice 再改 align 写回 "xMaxYMid slice"，探针定案）。缺省 6/1（xMidYMid meet）。
- **SVGPoint**：bootstrap 手搓类存自有数据属性 x/y 改隐藏 `_ps` 槽+原型 accessor（Chrome rw:ec）；matrixTransform 从「返 this」改真算 `a·x+c·y+e`；currentTranslate 每读**新包装共享存储**（探针：恒 false 身份、写跨读持久）。
- **SVGSVGElement 34 面**：几何四件（缓存 SVGAnimatedLength 活包装）、currentScale rw 缺省 1、currentTranslate、preserveAspectRatio、**zoomAndPan 缺省 2**（DISABLE=1/MAGNIFY=2/UNKNOWN=0，静态 proto+ctor 双挂 e--）——set 5 落 **0**、**不同步属性**（探针）；SMIL/redraw 全 no-op 但形对：getCurrentTime 恒 0（setCurrentTime 后仍 0，Chrome 探针同答）、animationsPaused 恒 false、**suspendRedraw 恒返 1**（两次调用都 1，探针——非自增）、unsuspend*/forceRedraw/deselectAll undefined；getElementById 子树走 id 属性；命中测试（getIntersectionList/getEnclosureList/check×2）走**属性几何 bbox**（circle/ellipse/rect/image/line/polyline/polygon 精确、path 取 d 坐标对近似、text/use 零盒），空结果=空 NodeList 非 null（探针）、ref 元素收窄子树。

**shape_fix 新雷（本刀最值钱的一坑）**：装齐后 order 28→29。根因链——类语法把 `constructor` 铸在原型**首位**而 Chrome 序在**末位**→gen_shape_fix 因此发 ORDER 条目→shape_fix 的重排循环只搬得动 configurable 成员，而 zoom 常量按 truth e-- 是**不可配置**的搬不动→重排把一切可搬的挪到常量后面、constructor 又被搬到尾，序反而全错。正解=**svg_root 自己把 constructor 挪到末位**：raw 序=truth 序→gen 不发 ORDER 条目→不重排→常量保住 e-- 真形。ORDER 表 649→648 条。

数字：missing 接口 110→**108**（SVGSVGElement+SVGPoint 毕业，SVGSVGElement 37→0），same 760→**762**，extra 0/order 28（收雷后归位）/kind 65/len 0；statics 15→**14**。枚举审计 204/211（7 已知缺口不变）。ratchet **52→54**（刀55 先例：新文件入链的合法新边——svg_root→bootstrap 的 `_markNative` 与 `SVGPoint`，同提交顶基线）。
行为验证：三轮 Chrome 对拍探针 **72/72 腿逐位全等** + 两轮补探（写回单位保留/序列依赖）10/10 全等；唯一记档分歧=**百分写回基数**（Chrome 用布局宽、随探针窗口变——本例 121.61528015136719/'40.5405%'；引擎恒用固有 300/150 基数、环境无关），不入测不入账为缺陷。
测试：`knife59_svg_root_face_and_value_types`（新，36 面全序+静态+13 组行为断言）。surface_snapshot 重生成（+3 行）。验收：`cargo test -p diting` 失败集与基线**逐名全等**（110 环境败，931 过=930+1）。
**教训**：①测试断言两次想当然被 Chrome 打脸（attrOut 以为写 '3' 实为保单位 '2.25pt'；parOut 以为重置 meet 实为保留 slice）——**探针要探测试的精确序列**，不是探相似序列；②`pkill -f` 看不见环境变量（DITING_BIND 只在 env）——杀服务用 `lsof -ti :端口` 拿 PID；③shape_fix 的重排对不可配置成员全盲——新接口带 e-- 常量时，要么让 raw 序天生=truth 序（本刀正解），要么别让它进 ORDER 表。

## M4 刀60：GPUSupportedLimits 三十一限位读面（2026-10-31）

missing 方向第三刀，小刀清户：GPUSupportedLimits 原型 **31 个 ro:ec 限位 getter**（maxTextureDimension1D…maxComputeWorkgroupsPerDimension），引擎铸壳只有 `constructor`。

**纯原型形状刀，零发明**：两个世界都拿不到实例——Chrome for Testing 145 headless `requestAdapter()` 返 **null**（探针 k60-chrome0），引擎同答 null。所以 getter 只读内部 `_lim` 槽、裸对象上读答 undefined（与未开槽读数一致），别的不装（无 statics、无 toStringTag 改动——审计只缺这 31 件）。ctor 挪尾配方（刀59）直接复用：raw 序=truth 序→gen 不发 ORDER 条目。

ratchet **54→55** 第三次垫（刀55/刀59 先例）：新边=web_api_gaps→bootstrap 的 `GPUSupportedLimits` 一名。解剖：该类在 bootstrap 是**直写** `globalThis.GPUSupportedLimits = class`（静态可见 provider）→裸引用即成边；同文件 Credential 不成边因其为动态铸名（分析器归 external）、Blob 不成边因 node 全局表滤掉。同形边（Document/Element/XMLSerializer/_markNative）本就挂在该文件 loadRequires 里——合法、同构、照旧例顶基线，不玩 `globalThis.X` 语法躲表。

数字：missing 接口 108→**107**（GPUSupportedLimits 毕业），same 762→**763**，extra 0/order 28/kind 65/len 0/statics 14 全不动。枚举审计 204/211（7 已知缺口不变）。internal_names 356→**357**（`_lim`）。surface_snapshot 重生成（GPUSupportedLimits 一行 1→32 名）。
测试：`knife60_gpu_supported_limits_face`（新：32 名全序 ctor 末位、ro:ec 四旗、裸读 undefined、`_lim` 管道 8192、31 getter 各佩己名、ctor 无 statics）。验收：`cargo test -p diting` 失败集与基线**逐名全等**（110 环境败，932 过=931+1）。

## M4 刀61：RTCPeerConnection 四十六件全面 + 七接口 kind 修（missing 方向第四刀）（2026-10-31）

missing 方向第四刀，最大户 RTCPeerConnection：**46 原型件**（12 状态 getter + 11 on* 处理器 + 23 方法）+ **generateCertificate static**（ctor 面长 0、四 static 名）。顺刀清 kind 差七接口（RTCRtpSender.track/transport、RTCRtpReceiver.track、RTCDTMFSender 三件、MediaStreamTrack.kind、RTCDataChannel.label、RTCCertificate.expires）与 RTCRtpTransceiver 四缺件。kind/statics/missing 计**接口**不计成员——本刀七个 kind 差 + 28 缺成员各只折 1。

**构造器重写为「一切状态进隐藏槽」**：Chrome 新鲜 pc `Object.keys(pc)===[]`、全部状态是原型 getter。`__rtc` 单槽装 desc×6/signaling/ice×2/conn/sctp/canTrickle/收发器五数组/cfg，`__on` 装 on* 处理器，close 后 iceGatheringState 停在 **'new'**（探针）。on* 从 own 数据属性改**原型 accessor**——这不只是形状：event_hooks 的 isHandlerSlot 只对 accessor 描述符点火，旧 `this.onicecandidate=null` 数据属性会影子杀掉处理器，本刀后 ontrack 真火、null 真清（探针 fire=1）。

**两颗雷**：①**加载序陷阱**——webrtc.js 是第 5 个脚本，global_surface.js（铸 RTCRtpSender/Receiver/DTMFSender/Certificate 壳）第 32 个，face 块裸引用那些 prototype 即载入时 TypeError；解=lazy `fillSats()` 挂在各 mint 工厂顶。②**死桩陷阱**——stub_members 已给七接口装了**形对读死**的桩（audit missing:0 但 mint 实例读垃圾）；解=`force()`（delete+define）强换 mint 关键读，RTCRtpTransceiver 四件才用 fill-if-missing。

**行为=三轮 Chrome 对拍 50/50 腿零差额**（k61-chrome/2/4/5 ↔ eng/eng2/eng5），全套报文逐字：WebIDL 参数类型 TypeError 先于一切（addTrack/removeTrack/createDTMFSender 非 track/sender、addTransceiver 非 'audio'/'video'——含 `{}` 也拒，dict overload 不存在）；close 门五处同款 InvalidStateError「The RTCPeerConnection's signalingState is 'closed'.」（addTrack/addTransceiver/removeTrack/createDTMFSender/setConfiguration）；**setConfiguration 可变性三分**——bundlePolicy/rtcpMuxPolicy/encodedInsertableStreams 构造时定格（InvalidModificationError 同文案），iceServers/iceCandidatePoolSize/iceTransportPolicy 可改且**整字典替换**（连改三键后前两键归零——探针 mutIce 逐位同答）；iceServers 条目归一化 **credential/urls/username 恒在场**（数字强转 "7"/"8"）且键序字母序；`'stun:'` 空主机与 'not a url' 同文案 SyntaxError「The given configuration has a syntax error.」；removeTrack 外来 sender=InvalidAccessError。mint 家族（sender/receiver/transceiver/DTMF/datachannel）+ legacy addStream/removeStream/getLocalStreams + getConfiguration 六键字典 + ctor 满参全反照（junkKey 丢弃）。

数字：missing 接口 107→**106**（RTCPeerConnection 毕业），same 763→**764**，extra 0/order 28/kind 65→**64**/len 0/statics 14→**13**/parent 6。枚举审计 204/211（7 已知缺口不变）。ratchet **55 平**（同文件面零新边——刀60 起同类同家策略见效）。internal_names 357→**365**（+__rtc/__on/__closed/__ufrag/__pwd/__host/__port/__cfg 收编）。shape_fix 648→**647** 条（RTCPeerConnection ORDER 条目消失——ctor 挪尾配方第三用：raw 序=truth 序，gen 不发条目）。surface_snapshot 重生成。hide_priv 重生成（RTCPeerConnection 槽清单换血）。
行为验证另有记档分歧三条：①gOPN(pc) 见 8 个不可枚举 `__` 槽 vs Chrome 空全量（引擎级模式，较旧版 16 个可枚举已收敛）；②fireOrder 引擎 handler 先于 listener（['H','L'] vs ['L','H']，event_hooks 全局语义，另行立案）；③createOffer on closed 异步拒绝形状未探（两侧均不同步抛）。
测试：`knife61_rtc_peer_connection_face`（新，20 组断言：desc×6/states/cfg 六键/keys 空/品牌/afterAdd/tr1/senderTrack 管道/recvTrack/15 连发错误分类学/mutEntry 归一化/mutReplace/dtmf/genCert/ctor/fire/proto=46/roFlags/closed 四态）。验收：`cargo test -p diting` 失败集与基线**逐名全等**（110 环境败，933 过=932+1）。
**教训**：①`MediaStreamTrack.kind` 两侧都是只读 getter——探针别想赋值绕行，要用被测面自己的 mint 工厂取真轨道；②Chrome 探针没跑过的腿（setCfgGood）不能拿引擎答案对旧 Chrome 腿充数——**补探再对**，本刀 InvalidModificationError 三锁全靠补探翻案（初版引擎 ok 是错的）；③enum/instance 两审计各有专属探针（enum-probe.js/instance-audit-probe.js）与真源（enum-truth-145.json），别拿 shape-audit 捕获喂错工具。

## M4 刀62：RTC 家族清尾五面——MST/DC/transceiver 全面 + 两事件类重缚（missing 方向第五刀）（2026-10-31）

missing 方向第五刀，RTC 线收尾：**MediaStreamTrack 17 件全面**（EventTarget 父级、Illegal constructor、id 每轨唯一随机）、**RTCDataChannel 16 件全面**（21 原型名全序、选项进 `__dc` 隐藏槽）、**RTCRtpTransceiver 8 缺件**（direction rw + stop 锁 + 头扩展协商双表）、**RTCTrackEvent 4 件重缚**（receiver/track/transceiver 必填 TypeError + streams 冻结）、**RTCPeerConnectionIceErrorEvent 3 件重缚**（errorCode 必填 + 默认 address/port null、url/errorText/hostCandidate ''）——五接口一次毕业，顺收 transceiver `direction` kind 差。

**Chrome 真源两轮 36 腿**（k62-chrome/2）：MST `getCapabilities` 在**原型**（statics []）、`new`=TypeError Illegal constructor、clone 保 readyState/label 但**铸新 id**、停轨 clone 仍 'ended'、getConstraints {}、getCaptureHandle null、stats null；DC 新鲜= arraybuffer/0/0/null/connecting/''、**非法 binaryType 静默忽略**、非 open send=InvalidStateError、connecting 上 close()→'closing'；transceiver direction 非法值**静默忽略**、stop 后读 'stopped' 且 setter=InvalidStateError、currentDirection 恒 null、头扩展表 audio **5**/video **16** 条、negotiated 视图全 'stopped'、`setHeaderExtensionsToNegotiate([])`=InvalidModificationError、非法条目=TypeError；TrackEvent 校验序 receiver→track→transceiver、streams 默认**冻结空数组**；IceErrorEvent 默认空串三件+null 两件。

**母题三则**：①**函数铸类 constructor 位置**——函数原型自带 constructor 居首且 configurable（非 class 语法的不可配置），`delete P.constructor` 后再装 getter 才能挪尾（原位重定义不挪位）；②**persona 种子的确定性陷阱**——`__rtcToken` 是 `_fpRand` 人格种子（ufrag/pwd 故意稳定），轨道 id 用它=**每轨同 id**，Chrome 每轨唯一→新铸 `__rtcUid`（Math.random 64 字符，长度记档分歧）；③**重缚必接父链**——global_surface 壳类被换下时原型父链不会自来，`setPrototypeOf(P, Event.prototype)` 显式接回（parent 审计当场抓获 6→8→修回 6）。

数字：missing 接口 106→**101**（MST/DC/transceiver/TrackEvent/IceErrorEvent 五排毕业），same 764→**769**，extra 0/order 28/kind 64→**63**（direction）/len 0/statics 13/parent 6。枚举审计 204/211（7 已知缺口不变）；实例审计 **127/127 全净**（Chrome 侧重捕）。ratchet **55→57** 第四次垫（刀55/59/60 先例）：新边=web_api_gaps→webrtc 的 `RTCDataChannel`/`RTCRtpTransceiver` 两名（面块裸引用 webrtc 铸的原型，加载序 37>7 合法同构）。internal_names 365→**385**（+__dc/__xkind/__rtcMintTrackImpl/__rtcUid/__te*/__iee* 等 20 名）。shape_fix 重生成（647 条，五排 ORDER 消失=ctor 挪尾配方第四用）。surface_snapshot 重生成（五排行换血）。
行为验证另有记档分歧：①getCapabilities/getSettings 返 {}（Chrome 真字典——无真源面，getConstraints {} 是 Chrome 精确）；②label ''（Chrome canvas 轨有长 label——源特定）；③id 64 字符随机 vs Chrome 88 字符 GUID 形（唯一性同、形状异）；④direction stop 后 setter 与 setHeaderExtensions 空表/非法条目的**文案**未逐字探（错误名确定）；fireOrder 引擎级模式（已另案）。
测试：`knife62_rtc_family_stragglers`（新，26 组断言：五接口全序/父级/Illegal ctor/DC 12 默认+选项+binaryType 忽略+send/close/方向静默+stop 锁/hdr 5-16-5/hdrSet 五腿分类学/TE 冻结+必填/IEE 值+默认+必填）。验收：`cargo test -p diting` 失败集与基线**逐名全等**（110 环境败，934 过=933+1）。
**教训**：①**`cargo build -p diting` 不重编 bin**——`diting` 二进制在工作区根包（src/main.rs），`-p diting` 只编 lib，bin 停吃旧快照照常服务（本刀两修「不生效」半小时的真相）；探针循环一律 `cargo build --bin diting`；②快照 include_bytes!(env!(DITING_SNAPSHOT_PATH)) 链上任何一环指纹没踩脏都会静默吃旧件——疑旧先 `strings 二进制 | grep 新符号`；③zsh 里 `echo ===X===` 会被当 `=cmd` 展开炸行——裸 echo 少用 `=` 开头记号；④满载时序测试会抖（test_local_fetch_resolves_on_task_boundary 单跑 3/3 过）——全量新败先单验再定罪。

## M4 刀63：SVG 家族五面——marker/animation/text/feDropShadow/pattern + 值类型内件全活（missing 方向第六刀）（2026-10-31）

missing 方向第六刀，SVG 族首批大户：**SVGMarkerElement 16 件**（refX/refY/markerUnits/markerWidth/markerHeight/orientType/orientAngle/viewBox/PAR + 常量×6 + setOrientToAngle/setOrientToAuto）、**SVGAnimationElement 13 件**（targetElement + on*×3 + requiredExtensions/systemLanguage + begin/end×4 + getCurrentTime/getSimpleDuration/getStartTime）、**SVGTextContentElement 11 件**（textLength/lengthAdjust + 常量×3 + per-char API×9）、**SVGFEDropShadowElement 11 件**（in1/dx/dy/stdX/stdY/x/y/width/height/result/setStdDeviation）、**SVGPatternElement 10 件**（patternUnits/patternContentUnits/patternTransform/x/y/width/height/PAR/href/requiredExtensions/systemLanguage）——五接口一次毕业，顺收 pattern `href` kind 差（rw→ro）与 FE 父链空中间层（parent 6→5）。

**本刀的真活是值类型内件**：SVGAnimatedNumber/Enumeration/String/Angle/TransformList 与 SVGTransformList/SVGStringList 原是死壳（baseVal/animVal undefined），全部就地重定义为**属性视图**——附着读每次解析属性、写序列化回写；SVGAngle 重写为附着感知（orient 属性视图，detached 语义不变）；SVGLength 增 `_def`（按附着点覆写默认：marker 宽高 3、FE 区 [0,%]、pattern 全 0——压过刀59 的 width/height→[100,2] 启发）与 `_noPct`（FE 区百分永不解析→NotSupportedError，probed）。值类型行原本 same，就地重定义不动位不动序。

**Chrome 真源三轮全解码**（k63-chrome/2/3）：marker 默认宽高 **3**、orientType 默认 **2 ANGLE**（SVG1.1 缺省 '0'，非 auto）、'auto-start-reverse'→**0**、junk→[2,0]、setOrientToAngle 吃 **SVGAngle 对象**（数字参 TypeError）且按 value+unit 序列化（'45'/'30deg'）、orientAngle.baseVal.value=20→attr '20'；animation targetElement 裸/在树/悬空 href 全 null、on* 非函数强转 null、四方法 no-op、getCurrentTime 0、getStartTime InvalidStateError 文案逐字、getSimpleDuration '5s'→5/'250ms'→0.25/缺→NotSupportedError；text 无布局 nchars 恒 0（textContent 'abc' 两侧同答 0——引擎无排版=天然同构）、getSubStringLength(0,0)→0 但 (0,5) 抛、selectSubString 连 (0,0) 都抛、textLength '70px'→attr 原样/单位 5；feDropShadow dx/dy/std 默认 2、junk→2、stdDeviation 对解析（'4 6'→4/6，单值双取）、setStdDeviation(x,y) 存**内部 override 不序列化**（净元素 attr 读 null）、restricted float 非数/缺参 TypeError、x/y/w/h 缺省 **[0,%]**、'%' unresolvable→.value NotSupportedError、in1/result=SVGAnimatedString 且 animVal 镜像；pattern units 双枚举（默认 2/1、junk→默认）、patternTransform 'rotate(30)'→1 项 type 4、空表 getItem(0) IndexSizeError、baseVal!==animVal。

**stale shape_fix 反噬（本刀最值钱的一坑）**：面装齐后序全乱——常量挤首位、saved viewBox/href 垫尾。根因：shape_fix 的 ORDER 表是**旧审计的陈货**（marker 条目=["viewBox","constructor"]），其重排循环围着**搬不动的 e-- 常量**把一切可搬成员挪去后面。教训：装新面当轮 raw 序=truth 序**不够**——陈 ORDER 条目当天就打你；重生 shape_fix（注释 include→裸编→raw 捕获→gen→还原）才是止血正法，重生后五面 ORDER 条目全消、raw 序即终序。另验证 `.prototype` 在 class 铸壳上**不可写**——整换 prototype 死路，序只能原生装对+重生表。

数字：missing 接口 101→**96**（五排毕业），same 769→**774**，extra 0/order 28/kind 63→**62**（pattern href）/len 0/statics 13→**11**/parent 6→**5**。枚举审计 204/211（7 已知缺口不变）；实例审计 **127/127 全净**。ratchet **57→61** 第五次垫（刀55/59/60/62 先例）：新边=svg_root 裸引用五个类名（加载序合法同构）。internal_names 385→**396**（+_a63/_arr/_custom/_def/_kw/_lastS/_noPct/_ov/_ovr/_pair/_sdOv 十一槽）。shape_fix 重生成（ORDER 643→**640**：五面陈条目消失，另收 getCharNumAtPosition arity 钉 0）。surface_snapshot 重生成。
行为验证记档分歧（GUESS 标注处）：TransformList/StringList **写路径序列化格式**未探（读侧全探）——ser1 按型拼接（'rotate(30)' 形）；SVGAnimatedEnumeration.baseVal **写回**反查关键词序列化未探；targetElement **活 #id 解析**未探（三态 null 全探）；IndexSizeError 文案格式未逐字（错误名确定）。
测试：`knife63_svg_family_five_faces`（新，39 组断言：五面 truth 全序/marker 默认+orient 三态+setOrient 双腿+常量/animation on* 强转+no-op+dur 解析/text 零布局+per-char 七腿/FE 默认+对解析+override+restricted float+区百分/pattern 枚举+transform 视图+href ro:ec）。验收：`cargo test -p diting` 失败集与基线**逐名全等**（110 环境败，935 过=934+1）。
**教训**：①**陈 ORDER 表反噬**——装面当轮必须重生 shape_fix，别信「raw 序对=完事」；②accessor 装键漏前缀（onacc(n) 装 `n` 忘 `'on'+n`）——冒烟当场抓获（onend=5 读回 5=data prop 回退，还会铸出 begin/end/repeat 三个 EXTRA）；③tri-state 别用 boolean 双值——pair X/Y/非对 用 null/0/1 三态，false 会吞进 null 分支（'4 6' 读成 2 6）；④列表视图缓存必须**按属性串脏检**重解析（_lastS），直接 setAttribute 不走方法时旧项滞留；⑤测试序列≠取证序列时期望要按**同构行为**推（feSet 前 attr 已是 '5'，override 不清 attr——Chrome 同构，净元素才 null）。

## M4 刀64：SVG 几何反射族 13 面——rect/circle/ellipse/line/FO/双梯度/stop/symbol/clipPath/image/use/textPositioning + 三列表内件全活（missing 方向第七刀）（2026-10-31）

missing 方向第七刀，SVG 几何反射全家：**rect 6**（x/y/width/height/rx/ry）、**circle 3**（cx/cy/r）、**ellipse 4**、**line 4**（x1..y2）、**linearGradient 4**（x1/y1/x2/y2）、**radialGradient 6**（cx/cy/r/fx/fy/fr）、**foreignObject 4**、**image 8 面**（几何 4+PAR+decoding/crossOrigin/href/decode）、**use 5**（几何 4+href）、**stop 1**（offset）、**symbol 2**（viewBox+PAR）、**clipPath 2**（clipPathUnits+transform）、**textPositioning 5**（x/y/dx/dy/rotate）——13 接口一次毕业（53 件）；顺收 image/use `href` kind 差（rw 壳→ro 视图）。**SVGTextPathElement 维持 order-only**：TEXTPATH_* 常量是 mint 层 e-- 不可配置且物理居首，真源要 5–10 位——本层搬不动，记为 mint 层遗留。

**本刀的内件活是三列表+Rect**：SVGLengthList/SVGNumberList（列表视图：token 解析、_lastS 脏检重解析、项=SVGLength/SVGNumber 附着槽 `_lstO/_lstI` 写穿透整表序列化）、SVGAnimatedLengthList/NumberList（baseVal/animVal 双对象接线）、SVGAnimatedRect+SVGRect 附着模式（viewBox 'min-x min-y w h' 四 token 视图，缺/junk→0，写四数重序列化）；SVGLength 六写路径挂列表分支（value/valueAsString/valueInSpecifiedUnits/newValueSpecifiedUnits/convertToSpecifiedUnits/write）；SVGAnimatedNumber 增 `_pctDiv`（stop offset '50%'→÷100）。

**Chrome 真源两轮全解码**（k64-chrome/2）：几何族缺省全 **[0, number]**（rect 本尊 width 也 0——刀59 的 width/height→[100,2] 启发只在 svg 根）且 **'50%' .value 全族 NotSupportedError**（百分只在根 svg 解析）；linear x1/y1/y2 缺省 0%、**x2 独缺省 100%**；radial cx/cy/r/fx/fy 缺省 **50% 且 fx 独立于 cx**（cx=30% 后 fx 仍 50）、fr 0%；梯度 % .value 同抛、'7' 无单位直读 7；stop offset '0.5'→0.5、**'50%'→0.5**、junk→0、写 0.75→attr '0.75'、**不钳位**（-0.5/1.5 原样）；symbol viewBox 缺→0、'0 0 10 20'→10/20、PAR 6；clipPathUnits 缺省 **1**（userSpaceOnUse——pattern 的 patternUnits 缺省 2，**此家不同**）、transform→SVGAnimatedTransformList；textPositioning x/y/dx/dy=**SVGAnimatedLengthList**、rotate=**SVGAnimatedNumberList**、'1 2 3'→3 项、'10px' 项带单位、rotate 项 SVGNumber 且 **item.value=90→attr '90 60' 整表序列化**、空表 getItem(0) IndexSizeError；image decoding 缺省 'auto'、**大小写不敏感**（attr 'ASYNC'→'async'）、junk→'auto'、属性写→attr；crossOrigin=映射属性（'crossorigin' 小写名）缺省 null、**''→null**、attr 'USE-CREDENTIALS'→'use-credentials' 小写读、写 'anonymous'→attr；href→SVGAnimatedString（baseVal 写序列化、animVal 镜像）；decode()→Promise；use 同 href 视图。

**陈 ORDER 表反噬第二例+假裸烤（本刀最值钱的坑）**：image 面装好后 own 序 decode 在 href 前——旧 ORDER 表的 image 条目围着旧成员集重排（刀63 教训复发，正路=当轮重生）。但重生环第一遍翻车：gen_shape_fix 吐出**近空表**（order 29/proto 33/ctor 0）——因为裸烤后的探针服务没换成（pkill 没杀净旧进程，端口被占新进程静默死，health 打到旧服务=带 shape_fix 的引擎），生成器 docstring 明言「capture WITHOUT shape_fix applied, otherwise the table comes out empty」，拿固定引擎当裸引擎→表空→复烤后全局 order 635/kind 231/len 123 大塌方。**止血三律**：①git checkout 恢复好表再入环；②换服务**按 PID 杀**（lsof -tnP -iTCP:18199）非 pkill -f；③裸烤后先跑 bare-check（Attr.length===1 且 constructor 居首=真裸）再抓。第二遍真裸抓取→重生大表（order 638/proto 819/ctor 123）→复验全绿。另记：shape_fix 四张表各是**一整行 minified**，git diff --stat 的「4 行」是四张表不是四个条目——判表大小看 gen 的计数输出别看 diff 行数。

数字：missing 接口 96→**83**（13 排毕业），same 774→**785**，extra 0/order 28/kind 62→**60**（image+use href）/len 0/statics 11/parent 5/syms 1。枚举审计 204/211（7 已知缺口不变）；实例审计 **127/127 全净**。ratchet **61→64** 第六次垫：新边=svg_root 新工厂名（__diting_svgLengthList/__diting_svgNumberList/__diting_svgRectAttach 跨文件加载期引用）。internal_names 396→**403**（+_lstO/_lstI/_tA/_itA/_pctDiv/_toks/_setTok 七槽）。shape_fix 重生成（ORDER 640→**638**：image/use 旧条目退役，新面原生序即终序）。surface_snapshot 重生成（13 行换血）。
行为验证记档分歧（GUESS 标注处）：列表**写路径**除 number 表（'90 60' probed）外未探——length 表 ser 按 value+suffix 拼、appendItem/insert/replace 按 valueAsString 取值；SVGRect 写路径四数重序列化未探（读侧全探）；crossOrigin 写 null→removeAttribute 未探（读侧 ''→null 全探）；junk token 项→0 未探。
测试：`knife64_svg_geometry_reflector_family`（新，37 组断言：13 面 truth 全序/rect 默认[0,1]+%抛+em 读+写穿透+恒等/族缺省/linear x2=100%+radial fx 独立/stop 六态含不钳位/symbol viewBox 视图+品牌/clipPath 三态+transform/text 五列表腿含整表序列化+清空重解析/image decoding 四腿+crossOrigin 四腿+href 视图+decode/use href/三列表品牌）。验收：`cargo test -p diting` 失败集与基线**逐名全等**（110 环境败，935+1=936 过）。
**教训**：①**裸烤必验裸**——pkill -f 杀不净时新进程静默死在 bind 上，health 照样绿（旧进程应答）；重生环三律=checkout 好表/PID 杀/bare-check 先行；②**minified 单行表骗 diff --stat**——shape_fix 判大小用 gen 计数不用行数；③探针腿读 `.value` 前先想清该面 % 是否解析（梯度/几何缺省即 %——真源录的是 unitType+valueInSpecifiedUnits 正因为 .value 必抛）；④Edit 收尾删换行会把注释行和下一行焊死——node --check 先过再 build。

## M4 刀65：Document 24 件大户毕业——XML 三件/on* 七件/webkit 八面/FeaturePolicy/activeViewTransition/caretRangeFromPoint/rsafFor/browsingTopics/ariaNotify + body rw + adoptedStyleSheets ec（missing 方向第八刀）（2026-10-31）

missing 方向第八刀，**missing 账上最大单排**：Document 24 件一次毕业——`xmlEncoding`（ro 恒 null）/`xmlVersion`（'1.0' 落存，其余含 null 抛 NotSupportedError 且报文点名尝试版本、null 串成 ''）/`xmlStandalone`（布尔强转）；on* 七件（onreadystatechange/onpointerlockchange/onpointerlockerror/onfreeze/onprerenderingchange/onresume/onvisibilitychange——installOn 同款 `__onHandlers` 存储、get/set 命名、ec）；webkit 八面（webkitVisibilityState/webkitHidden 活镜像 + webkitIsFullScreen=false/webkitCurrentFullScreenElement=null/webkitFullscreenEnabled/webkitFullscreenElement 镜像）；`featurePolicy`（**缓存单例** Object.create(FeaturePolicy.prototype)、same-ref、零自有键）；`activeViewTransition`（静 null、进行中===startViewTransition 返回物、finished 落定清槽——包装 web_api_gaps 原实现）；`caretRangeFromPoint`（elementFromPoint 锚定、collapsed Range(el,0)、OOR null、**arity 0**）；`requestStorageAccessFor`（无参 TypeError/非 URL 同步 `Invalid origin`/合法→rejected NotAllowedError）；`webkitCancelFullScreen`/`webkitExitFullscreen`（void，转发 exitFullscreen GUESS）；`browsingTopics`（→`Promise.resolve([])`）；`ariaNotify`（签名=（DOMString message, optional AriaNotificationOptions）→void，消息逐字）。顺收两件 kind 差：**body ro→rw**（setter 三档报文逐字：null/div/str；BODY 与 FRAMESET 经 documentElement.replaceChild 换装读穿，getter 补 frameset 回退——换 frameset 后 document.body 读回 frameset）、**adoptedStyleSheets `--`→ec**（源头在 cssom_sheets.js 尾部裸 defineProperty——非配置故 shape_fix 修不动，源头补 enumerable/configurable，shape_fix 的 [adoptedStyleSheets,-1,ec] 行随之退役）。

**FeaturePolicy 桩夺位**：FeaturePolicy 原是纯桩（四方法对任何 proto 匹配接收者返回 undefined）。document_gaps.js 载于 stub_members **之前**，先手把四方法重定义为 Chrome 145 真值——allowedFeatures()=80 项默认允许表（每次新数组）、features()=81 项（多出 'unload'）、allowsFeature 缺参 TypeError/命中布尔/getAllowlistForFeature 允许→[location.origin] 否则→[]、裸接收者 `TypeError: Illegal invocation`（与桩惯例同款裸文案）——stub_members 的 fill-if-missing（hasOwnProperty→continue）让位，面形状不变（同 ecw 同 arity）。清单为 Chrome 145 缺省允许表（probed 80/81，烤死在文件里）。

**Chrome 真源四轮全解码**（k65-chrome/1-4）：xmlVersion '1.0' 落存且 xmlEncoding/xmlStandalone 不受牵连；browsingTopics **返回 Promise**（resolvee=[]——第一轮 JSON 化成 '{}' 差点误判成对象，原型是 Promise）；requestStorageAccessFor 合法 origin→rejected `requestStorageAccessFor not allowed`；ariaNotify ('a','b') 第二参字符串 TypeError、单串参合法（签名=消息+可选字典非双字典）；body null 有专属报文（「1st argument provided is either null...」）与 str 的转换报文分开；caretRangeFromPoint 命中文本区也锚**元素**（nodeType 1、offset 0）；startViewTransition 进行中 same-ref、finished 后回 null；featurePolicy === featurePolicy（缓存）、getOwnPropertyNames=[]。

**ghost hunt 一课（本刀最耗的坑）**：adoptedStyleSheets `rw:--` 追凶——grep 全 js 零命中（bootstrap/platform_bundle/lift_own/stub_members 全排除），descriptor 是 `[native code]` 的 get/set、idx 0 物理居首（被 ORDER 重排挤到头）。真凶 **cssom_sheets.js 尾部的 defineProperty**——该文件内嵌 `\x00style\x00`/`\x00link\x00` 缓存键哨兵 NUL 字节，git 视作 binary、**grep 静默不匹配、Edit 工具拒写**（10-29 棘轮搬出时的提交也是 Binary files differ）。破案靠 sed 直读文件尾。改法=python 字节级替换（Edit 永远进不去这文件）。另：探针 `t()` 包 async 腿连坑两次（存的是 Promise、JSON 吞成 '{}'）——async 腿必须 `await ta()`；ariaNotify/caretRangeFromPoint 的 truth arity（1/0）与形参解耦（options 走 arguments[1]、坐标走 arguments[0..1]）。

数字：missing 接口 83→**82**（Document 毕业大排），same 785，extra 0/order 28/kind 60→**59**（Document 行整行退出）/len 0/statics 11/parent 5/syms 1。枚举审计 204/211（7 已知缺口不变）；实例审计 **127/127 全净**。ratchet **64→65** 第七次垫：新边=document_gaps 加载期对 `_markNative` 中枢的引用（各面文件同款）。internal_names 403→**404**（+`__onHandlers`——document 实例新态）。shape_fix 重生成（ORDER 638 持平/proto 819→**818**：adoptedStyleSheets 行源头治愈退役）。surface_snapshot 重生成。
行为验证记档分歧（GUESS 标注处）：webkit 两动词的转发行为未探（只探返回 undefined）；caretRangeFromPoint 文本内景锚定按元素同构（探腿只命中元素）；browsingTopics 恒 resolvee=[]（无 topics 上下文的同构截断）；body setter 对非元素对象（如 document）走 TypeError 转换档未逐字探；FeaturePolicy 清单烤死 145 缺省表（页面无 permissions-policy 头的同构）。
测试：`knife65_document_family`（新，23 组断言：24 件全在/xml 三态+两份逐字报文/on* 往返+描述符四元组/webkit 六值/fp 单例+80+81+四方法+双 TypeError/avt 静-动/crfp 七元组含 OOR 与 arity 0/rsaf 四元组含逐字报文/两动词六元组/双 promise 面/ariaNotify 五元组/body 描述符+三份逐字报文+四段换装含 frameset 读穿与还原/adopted ec）。验收：`cargo test -p diting` 失败集与基线**逐名全等**（110 环境败，936 过=935+1）。
**教训**：①**NUL 哨兵文件是 grep 盲区**——cssom_sheets.js 带刻意 `\x00` 缓存键，binary 旗一下 grep/Edit/全失效，追凶别信 grep 零命中，sed/python 直读；②async 探针腿必须 await 进 t()（Promise 被 JSON.stringify 吞成 '{}'，两连坑）；③truth 的 arity 与代码形参解耦——Chrome 常是 function:0/function:1 而实现惯用命名形参，签名面（ariaNotify 1、caretRangeFromPoint 0）用 arguments 走参保真；④桩接口的方法可在装桩**之前**夺位（fill-if-missing 让位律），形状不动行为升级——以后纯桩要行为化走这条路。

## M4 刀66：CSSCounterStyleRule 11 面 + ShadowRoot 5 面——描述符语法校验/规范序串行化/双行毕业（missing 方向第九刀）（2026-10-31）

missing 方向第九刀，合刀两面：**CSSCounterStyleRule**（11 个 rw 描述符一次齐——name 升 rw + system/symbols/additiveSymbols/negative/prefix/suffix/range/pad/fallback/speakAs 十新面）与 **ShadowRoot**（fullscreenElement rw/pictureInPictureElement/pointerLockElement/onslotchange/getSelection）。落点：描述符进 css_rules.js 本体（`rec`/`decl`/`acc` 是文件内闭包，外文件够不着——刀66 首次直接扩 CSSOM 本家）；ShadowRoot 五件进 bootstrap.js 类旁 defineProperty 块。

**Chrome 145 三大真相（k66 两轮 68+30 探针）**：①**setter=语法校验制**——合法值原位替换（源拼写保留，symbols 带引号原样读回 `'"A" b'`），非法值**静默忽略**（从不抛错；''、CSS-wide 关键字 initial/inherit/unset/revert、越界语法全拒；`suffix='initial'` 整值拒而 `'a initial'` 双 ident 合法）；②**system setter 在 Chrome 145 是彻底 no-op**——parse 接受（`extends decimal` 进得了 cssText）但 CSSOM 改不动（numeric/fixed/fixed 3/extends decimal/CYCLIC/zzz!! 六值全 no-op），实现为 rw 形状+空 setter 保真；③**cssText 序列化=规范序**与源序/设置序无关（乱序设十描述符后按 system→symbols→additive-symbols→negative→prefix→suffix→**pad→range**→fallback→speak-as 串出——pad<range 是两轮探针双重钉死的反直觉位，第一版表猜反被冒烟当场抓获）。**parse 期同语法过滤**：`negative: (-)(-)` 进不了 cssText 也读不出（Chrome 行为）；name setter=ident 检验拒 none/数字头/引号串（'UPPER'/'-dash' 收）。ShadowRoot：三元素面恒 null（fullscreenElement 保 rw 形状、setter 无抛 no-op）；onslotchange=`__onHandlers` 同款；**getSelection=每根缓存 Selection**（同根同引用、异根异对象、与 document 的互异、初态 type None/rangeCount 0/anchorNode null，addRange 在影内活）。

**grammar 校验器**：引号感知 whitespace tokenizer（`symbols: "A" b`=两符号——第一版当单符号被冒烟抓出）；additive-symbols 每逗段=整数+单符号；negative≤2 符号；pad=整数+单符号；range=auto|逗段边界对（双 infinite 拒）；speak-as=单关键词或 counter-style 名。**访问器钉名+brand 守卫**：csAcc 钉 `get X`/`set X`+setter length 1（共享 acc 不做）+ `D.has(this)` 守卫抛裸 `Illegal invocation`；ShadowRoot 同款（instanceof 守卫）。挂账：css_rules 老面（selectorText/media 等）访问器名仍是 ''、无 foreign 守卫——**家族级缺口**另刀。

数字：missing 82→**80**、same 785→**787**、kind 59→**58**（CSSCounterStyleRule 行退出——ShadowRoot 原本只挂 missing 行不挂 kind 行，账目对齐）、order 28/len 0/statics 11/parent 5/syms 1 持平。枚举 211/204/7 已知不变；实例 **127/127**。ratchet **65 持平**（新引用全调用期，无需垫）。internal_names 404 持平（`__onHandlers` 复用+WeakMap 闭包，零新槽）。shape_fix 重生（order 638/proto 818/static 47/ctor 123 计数持平、内容换血——新面源头即正型故 proto 行不涨）。surface_snapshot 重生成。**刀57 旧断言合法翻新**：`counterOwn` 从 `name,constructor` 更新为 11 面新真值（旧断言钉的是当时折叠态）。
行为验证记档分歧（GUESS 标注处）：prefix/suffix 相邻序未同场探得（按规范序插位，两侧边界已钉）；range 逗段多对（'1 5, 10 15'）按规范收未探；CS_IDENT 宽松版（CSS 转义序列未实现）；CSS-wide 关键字大小写不敏感拒（/i 按规范）。
测试：`knife66_counter_style_and_shadowroot`（新，24 组断言：11 面在+描述符六元组/十一读数含缺省空串/cssText 规范序两场景/合法非法设置矩阵含 system no-op 与 name 拒三态/双 Illegal invocation/ShadowRoot 三 null+setter no-throw/onslotchange 往返+描述符/getSelection 八元组含影内 addRange）。验收：`cargo test -p diting` 失败集与基线**逐名全等**（110 环境败，938 过=937+1）。
**教训**：①**规范序别猜**——pad/range 位序与 CSS 规范条文相反，探针全 cssText 才是账；②**值语法=分词问题**——`"A" b` 是符号表不是单符号，引号感知 tokenizer 是这类描述符校验的底座；③**合刀先核行归属**——kind 账只记双面同在的 kind 差行，ShadowRoot 全程只是 missing 行，「两行都该 -1」的直觉账错了一次；④老测试断言钉中间态时，新刀扩面须同 commit 翻新断言（刀57 先例）。

## M4 刀67：VideoFrame 13 件毕业——双 ctor 验证梯/平面数学/closed 矩阵 + VideoColorSpace 行为接管（missing 方向第十刀）（2026-10-31）

missing 方向第十刀，WebCodecs 值类型大户：**VideoFrame** 从 4 面 stub 类整面重写为 17 面（bootstrap.js 类原位重写——format/timestamp/duration 三旧面语义升级 + codedWidth/codedHeight/codedRect/visibleRect/displayWidth/displayHeight/colorSpace/rotation/flip 十 getter + allocationSize/clone/copyTo/metadata/close 五方法）；顺带 **VideoColorSpace 行为接管**（新文件 videoframe_gaps.js——VCS 是 global_surface mint 的无定义点类，stub_members 只填常量 getter，本刀给它真 ctor+WeakMap store+toJSON）。

**Chrome 145 四大真相（k67-chrome{,2,3,4} 共 128 探针腿）**：①**ctor 验证梯逐字**——source union 报文含 8 类型全名；buffer 路无 init 报 source union 报文、init=null 报专属 `not of type 'VideoFrameBufferInit'`、init={} 报 codedHeight Required member（三档三报文）；成员读序 codedHeight→codedWidth→format；`codedWidth must be nonzero.`/`data is not large enough.`；visibleRect 有自己的消息族（`x/y cannot be negative`、`width/height must be nonzero`、`right 9 exceeds codedWidth 4.`/`bottom …`）；display 成对规则+ulong/ullong range 报文点名成员与 init 字典名。②**visibleRect 收缩**——帧最终塌到可见矩形（coded=visible 尺寸、visible 重锚 (0,0)；vis_x2: x:2 输入→(0,0,2,2)+coded (2,2)），而**数据尺寸按解析态 coded 验**（vis_x3: coded 6x2+visible 2x1，48>32 报 not large enough——同一梯两种尺寸观）；x/y floor 后偶数吸附（1→0、3→2、0.7→0），w/h 截断（2.9→2）。③**closed 矩阵**——format null/ts/duration 存活/几何全 0/rects null/colorSpace=全 null VCS 对象/rotation 0/flip false；allocationSize/clone/metadata 抛 InvalidStateError（metadata 也抛——round4 补钉）、copyTo 异步拒、close 幂等；clone 拷贝全部字段且独立于原件 close。④**copyTo=Promise 返回布局数组**（I420 4x2→`[{0,4},{8,2},{10,2}]`、BGRA→`[{0,16}]`），一切拒绝异步（sync NO-THROW）；canvas 源=真像素（getContext('2d').getImageData RGBA→BGRA 逐字节换位），`alpha:'discard'`→BGRX；allocationSize 平面表 RGBA 族=w*h*4、I420/NV12=w*h*1.5（3x3 I420=17=9+2*4 奇数宽 ceil 钉死）；rotation 45→**90**（平局取高）且 '90' 数字强转；transfer 枚举真值=`iec61966-2-1`（全连字符——点号写法被烟测当场抓获）；getter/method 裸接收者一律 `Illegal invocation`（instanceof brand 守卫，subclass 放行）。

**VCS 接管=FeaturePolicy 夺位律的 ctor 版**：mint prototype 身份保留（sweep 的 toStringTag 品牌、链全在），只换 ctor 函数（length 钉 0、name 保 'VideoColorSpace'），getters 钉 `get X` 名走 WeakMap，toJSON 钉名返回 Chrome 序字典（fullRange→matrix→primaries→transfer，全 null 缺省、fullRange 布尔强转、显式 null 保留 null）。装在 event_hooks 后、lift_own/stub_members 前——fill-if-missing 让位（刀65 律第四用）。

**本刀最耗的坑：函数声明提升遮蔽 mint 面**——IIFE 里 `function VideoColorSpace` 声明提升，首行 `const proto = VideoColorSpace.prototype` 拿到的是**新函数自动生成的裸 prototype**，mint 的 sweep-tagged 原型整个孤儿化：toStringTag 丢、审计 syms 2 差一行。烟测腿全绿（成员全是我自己装的）掩盖了它，shape audit 才暴露；node 三行隔离复现破案。修法=读 mint 面走 `globalThis.VideoColorSpace` 显式寻址。

数字：missing 80→**79**（VideoFrame 毕业行）、same 787→**788**、extra 0/kind 58/len 0/order 28/statics 11/parent 5/syms 1 持平。枚举 211/204/7 已知不变；实例 **127/127**。ratchet **65 持平**（bootstrap 内自足、新文件零跨文件加载期引用）。internal_names 404→**417**（+13 槽：_fmt/_ts/_dur/_cw/_ch/_vw/_vh/_dw/_dh/_cr/_vr/_vcs/_rot/_flip，_bytes/_cs/_closed 复用）。shape_fix 重生（order 638/proto 818→**831**=+13 恰是新面数/static 47/ctor 123）。surface_snapshot 重生成。
行为验证记档分歧（GUESS 标注处）：from-frame 的 options 覆写（timestamp 探过、duration override 未探）；from-closed-frame 报文复用 clone 文案未探；非 canvas 图源（video/img/bitmap）字节全零（canvas 是唯一探过的真像素路）；copyTo 第二参（plane layout options）忽略；VCS ctor 枚举值不校验（'ZZZ' 原样存）；image 路径 visibleRect/display 忽略；timestamp NaN 边缘；奇数宽 crop 的字节级精确（布局对、行拷同构）。
测试：`knife67_videoframe_family`（新，26 组断言：13+4 面全在/ctorLens 双零一/验证梯 16 报文逐字/canvas 九元组/四格式 allocationSize/rects 同引用/VCS 四态+描述符+tag/collapse 七元组/snap 三元组/rot-flip-ts 四元组/alpha/fromFrame/clone 双轮含 I420/metadata 四元组/copyKind 三元组/closed 11 值矩阵+五操作/closedTwice/三 Illegal invocation）。验收：`cargo test -p diting` 失败集与基线**逐名全等**（110 环境败，939 过=938+1）。
**教训**：①**接管文件读 mint 面必须走 globalThis**——IIFE 内函数声明提升会从第一行就遮蔽同名全局，`.prototype` 拿到的是自己的自动原型；烟测全绿不代表接管干净（成员自装自证），audit 的 syms/kind 行才是照妖镜；②**同一验证梯两种尺寸观**（visibleRect 收缩后的帧尺寸 vs 数据校验的解析态尺寸）——别拿终态反推校验，分开记；③枚举串连字符别想当然（iec61966-2-1 非 2.1，真值是三连字符）；④测试腿的帧时序自查——clone 腿 close 了共享帧后，后续 metadata 腿会吃到「closed 后抛」的真值行为（Chrome 正是这样），测试脚本自身得用新帧。

## M4 刀68：IndexedDB 家族 12 面毕业——私有槽 + 原型访问器 + 真 EventTarget 事件链（missing 方向第十一刀，47 件）（2026-10-31）

missing 方向第十一刀，**整族重写**：indexeddb.js 从「纯对象字面量 + setPrototypeOf + 自有属性」重写为真类体系——IDBRequest/IDBOpenDBRequest/IDBDatabase/IDBTransaction 是 EventTarget 子类（原生 dispatch 核），IDBObjectStore/IDBIndex/IDBCursor/IDBCursorWithValue/IDBRecord/IDBFactory/IDBKeyRange 是 illegal-ctor 类；**实例零自有键**（状态全在每接口一张 WeakMap），属性全是原型访问器（`get x`/`set x` 钉名、`on*` 往返只收函数其余落 null）。IDBVersionChangeEvent 行为接管另落 idb_gaps.js（indexeddb.js 先于 Event 加载，类建不出）。12 个 IDB 接口 audit 行全清（missing/extra/order/kind 皆零）。

**Chrome 145 真相（k68-chrome{1,2} 共 124 探针腿，引擎逐腿 0 diff）**：①**事件链**——success=Event（不冒泡）、error=Event（冒泡+可取消）、complete=Event、abort=Event（冒泡）、upgradeneeded/delete 的 success=IDBVersionChangeEvent；error 沿 request→tx→db 冒泡（eventPhase 2/3/3），**未 preventDefault 的 request error 中止事务**（tx.error=该错误，abort 事件在其后）；②**状态机**——pending 读 result **和 error** 都抛 InvalidStateError（source/transaction/readyState 不抛）；升级期 readyState 已是 'done'、result=db、transaction=versionchange tx；升级事务 oncomplete 先于 open 的 success；handler 抛异常 → AbortError 中止升级；显式 abort → request error AbortError(冒泡) 且无 success；③**abort 回滚**（写入快照/undo 栈）、中止后操作抛 TransactionInactiveError "The transaction has finished."（commit/abort/objectStore 抛 InvalidStateError 同文案）；commit 后同样；④**同步 vs 异步错误**——参数/key/keyPath/只读/克隆错同步抛（DataError×4 文案、ReadOnlyError、DataCloneError、TypeError 参数计数），ConstraintError（重复 add、唯一索引冲突）走 request error；⑤**结构**——objectStoreNames/indexNames 是**每次新建**的 DOMStringList 且按 code unit 排序；`tx.objectStore('s')===tx.objectStore('s')`（每事务缓存）、`index.objectStore===store`；store.name/index.name 仅 versionchange 可 set；createObjectStore/createIndex 的 SyntaxError/InvalidAccessError/ConstraintError 全套；⑥**游标**——同一游标对象复用、continue 后 readyState 回 pending、重入抛 InvalidStateError、`advance(0)` TypeError、`continue(key)` 要求越过当前位置、`openKeyCursor` 给 IDBCursor（无 value、update 抛 key cursor）、索引游标 nextunique/prev 序；⑦getAll/getAllKeys 接 options 字典（query/count/direction），**getAllRecords 返回 IDBRecord 数组**（key/primaryKey/value 三访问器）；⑧multiEntry 索引计数按条目（8）、值写入走 structuredClone（改原对象不影响库内、get 返回克隆）。

**实现要点**：key 编码 `_idbEnc`（number/date/string/binary/array 规范串）+ 全序比较（number<date<string<binary<array，数组字典序）；schema 元数据（版本/stores/**含索引**）仍镜像 localStorage，旧三元组格式向后兼容；`__diting_etParents` 隐藏 WeakMap 给 event_hooks.parent 做 request→tx→db 路径；`__diting_etErrTap` 给 event_hooks.report 记录 upgradeneeded 派发窗内的 handler 抛错（原生核吞异常只上报）。破坏性变更（已授权）：db.createObjectStore 在非升级期不再「同参返回现有」而是抛 InvalidStateError；upgradeneeded 事件不再带 `.transaction`（Chrome 无此属性，走 `e.target.transaction`）；deleteDatabase 不再触发 upgradeneeded；游标值是克隆快照。

数字：missing 79→**70** 行、same 788→**797**、missing 件数 243→**196**（-47）、extra 0/kind 58/len 0/order 28/statics 11/parent 5/syms 1 持平。枚举 211/204/7 不变；实例 **127/127**；ratchet crossFileLoadTimeNames 65 持平；internal_names 417→414（旧自有槽 `_closed` 等退役）；快照 9 行变。
行为验证记档分歧（GUESS）：无跨连接 versionchange/blocked 协议（onversionchange/onblocked 访问器在、永不触发）；游标条目表在 open 时快照（游标期间新增记录不入序）；事务自动完成仍是微任务边界而非 Chrome 的宏任务；IDBRecord 对 getAll 的 `count` 为 Uint32 截断未探边界；deleteObjectStore/deleteIndex/store.name setter 的报文按同族推得未逐字探；唯一索引在 createIndex 时对既有数据的回填校验未做；upgrade 中止的 Chrome 报文为「Version change transaction was aborted in upgradeneeded event handler.」（记忆所得，未探 message）。
测试：`knife68_idb_family`（新，持存储锁——schema 镜像写 localStorage，与 localStorage 持久化测试同临界区；32 行断言：illegal ctor/钉名/foreign/range 六元组/cmp/vce 六元组/升级事件字段/属性读数/事务内报文/put 自增键/重复 add 与唯一冲突报文/索引游标三形态/getAllRecords/options 字典/complete 事件/中止后抛错/冒泡序+abort 事件/中止回滚/九条错误梯/closing 报文/deleteDatabase 事件）；旧四条 IDB 测试按 Chrome 真值更新（`e.target.transaction`、同步 DataError、closing 文案、Illegal constructor 带前缀）。
**教训**：①**游标 live 集合别在每次成功后无条件删**——handler 里 `continue()` 会重新入集，随后的无条件 delete 把它删掉，事务提前完成，后续 step 抛 TransactionInactiveError；正解=pending 标志，只在没被重新武装时才脱集；②**测试里数组别当快照推进日志**（`log.push(['bubble', bub])` 后续还在涨，stringify 时才暴露）；③**bare-check**：裸构建抓的 raw 表 order 违例遍地才算裸；④**存储相关测试要与 localStorage 持久化测试同锁**——进程全局镜像被并行测试 flush 成 `{}` 会让「落盘」断言偶发空；⑤engine 的 structuredClone 报文缺函数源码（Chrome：`f() {} could not be cloned.`）——IDB 侧先自己合成，structuredClone 本体报文差异另案。

## M4 刀69：WebGL 两大户清零——statics/成员序/makeXRCompatible/colorSpace rw/扩展对象表（用户裁决「WebGL 573+307 也要搞」）（2026-10-31）

用户 2026-10-31 裁决放行 WebGL 那两个最大失配（WebGL2RenderingContext 573 / WebGLRenderingContext 307）。**拆解后真身只有五类小事**，「几百件」是一个位移放大出来的数字：①**ctor statics 全缺**——Chrome 每个 GL 常量在原型和构造器上各有一份（`WebGLRenderingContext.DEPTH_TEST===2929`，e--；298/559 个），引擎只有原型份；②**成员序**——Chrome 序是 canvas→drawingBuffer*×2→colorSpace×2→常量→方法→drawingBufferFormat→（WebGL1 独有 RGB8/RGBA8）→drawingBufferStorage→constructor→makeXRCompatible，引擎常量居首，且**缺 makeXRCompatible** 令其后位置全部错位，逐位比对于是报出 440/564 个错位；③colorSpace 两个面引擎是 ro、Chrome 是 rw；④create* 五件+createQuery 等在 Chrome 里 length 0、fenceSync length 2，引擎全是 1——**因为所有 creator 共享同一个 `_GL_CREATE` 函数对象**，shape 修复表给 createShader 设的 length 1 漏给了所有人；⑤getContextAttributes 键序/缺 xrCompatible/不认请求字典。

**Chrome 145 真相（软件 WebGL——给 probe_run 的 chrome 腿加了 `DITING_CHROME_WEBGL=1` 开关换掉 `--disable-gpu`，默认抓取不变；k69 两轮 59 腿+扩展表 6 腿，引擎 0 diff 除两处已记档）**：①colorSpace setter **静默忽略**非法值（'bogus'/null 不抛、保持旧值），每 context 独立，foreign receiver 抛 Illegal invocation；②**makeXRCompatible 返回缓存的永不 settle 的 Promise**（9s 仍 pending，同一 context 每次同一个 promise，`xrCompatible` 保持 false；foreign receiver **reject**（不是同步抛）为 Illegal invocation TypeError）；③getContextAttributes 按字母序、带 `xrCompatible`、**回显 getContext 传入的字典**（alpha:false 等原样，powerPreference 非法枚举抛 TypeError）；④资源句柄是真类实例（WebGLBuffer/Texture/Shader/Query/Sampler/TransformFeedback/VertexArrayObject/Sync 全 Illegal-ctor 类）；⑤getExtension 名字**大小写不敏感**、同 context 同名**返回同一对象**、扩展对象零自有键且原型无 constructor、**原型上有 Chrome 的接口 tag**（WebGLDebugRendererInfo/EXTTextureFilterAnisotropic/WebGLLoseContext/OESVertexArrayObject…，扩展接口名不是全局）；⑥常量赋值严格模式 TypeError（原型/ctor/实例三处）；⑦`new WebGLRenderingContext()` = "Failed to construct '…': Illegal constructor"。

**实现**：常量在原型上**先 configurable 装入**，由 shape_fix 的 ORDER 遍排到 Chrome 位置、再由 flag 遍封成 e--；**gen_shape_fix.py 模板里 ORDER 遍移到 flag 遍之前**（旧序下被 flag 遍封死的成员无法再排——这个通用修复**顺带收掉 5 个别的 order 行**）；ctor 上的 statics 按表序一次落定；context 状态（colorSpace/makeXR promise/attrs）全进 WeakMap，键空间=base，`_glBaseOf` 把 Proxy 映回 base（setter 的 this 是 Proxy）；creator 一函数一名一 length；**删掉了「表里没有的名字返回数字 no-op」的 Proxy 兜底**（import 时代遗物，表对 Chrome 已全）——`gl.fenceSync` 在 webgl1 现在是 undefined，特性探测可用；扩展对象来自**生成表**（tools/webplatform/gen/gen_webgl_ext.py ← chrome/webgl-ext-truth-145.json，webgl_ext.js 生成件）。支持哪些扩展仍是 persona 的事（bootstrap `_GL_EXTS1/2`，SwiftShader 抓取机的清单不同不照搬）。

数字：missing 70→**68** 行（两 WebGL 行的 makeXRCompatible）、same 797→**800**（WebGL×2+一个顺带）、order 28→**23**、kind 58→**56**、statics 11→**9**、extra 0/len 0/parent 5/syms 1 持平；**audit「worst by mismatch」榜首的 WebGL 573/307 消失**。枚举 211/204/7；实例 127/127；ratchet crossFileLoadTimeNames 65 持平（callTime 169 持平）；internal_names 414；快照 2 处变。
行为验证记档分歧（GUESS/已知差）：7 个抓取机没有的扩展（KHR_parallel_shader_compile/WEBGL_blend_func_extended/WEBGL_compressed_texture_pvrtc/EXT_render_snorm/EXT_texture_norm16/WEBGL_provoking_vertex/WEBGL_render_shared_exponent）的接口 tag 与常量值来自注册表知识、**未探**（生成器里 GUESS 字典）；扩展方法是 no-op（Chrome 的 createVertexArrayOES 等返回真对象）；`getUniformLocation` 对未链接程序的未知名字引擎返回句柄、Chrome 返回 null（保持引擎的宽容）；常量赋值到构造器的 TypeError 文案里函数源码是 `class …` 而 Chrome 是 `function … { [native code] }`（V8 内部串，JS 层改不了）；foreign receiver 调 getExtension/getParameter 等引擎仍宽容不抛（Chrome 抛 Illegal invocation，本刀只收 makeXRCompatible/colorSpace 两处）；context lost 后的 getExtension 行为未做。
测试：`knife69_webgl_surface`（新，28 组断言：statics 计数/描述符/表尾、rgb8 四值、原型常量描述符、三段成员序、illegal ctor 报文、makeXR 形状+缓存+跨 context 不同、colorSpace 全流程+钉名+foreign、creator lengths、句柄 tag、webgl1 无 fenceSync/无兜底、扩展 tag/缓存/大小写/形状/常量/无参报文、attrs 序与回显与坏枚举、三处严格模式赋值、own keys 与 tag）；旧两条 stealth 测试按 Chrome 145 真值更新（443/792 个成员、未知方法=undefined+TypeError）。
**教训**：①**「某接口失配 573」要先拆再信**——逐位比对里一个缺失成员会把其后全部位置判错，真缺口只有几类；②**共享函数对象是 length 修复的雷**——多个方法指向同一个函数时，对其中一个设 length 会漏给所有人；③**flag 遍与 ORDER 遍的次序有后果**——先封 non-configurable 就排不动了，封口必须最后；④**Chrome 的 Proxy 兜底是反保真**——「未知名字回数字函数」让 `gl.fenceSync` 在 webgl1 变 truthy，特性探测全错；⑤**抓取机能力决定能抓什么**——`--disable-gpu` 下 getContext 回 null，实例级真值全抓不到，开软件 WebGL 才是正解（且把开关做进工具，别一次性）；⑥**真值与 persona 分开**——扩展对象的接口形状是 GPU 无关的（生成表），哪些扩展可用是 persona 选择（抓取机清单不照搬）。

## M4 刀70：SpeechSynthesisUtterance 8 面 + Voice 实例 + Event ctor 梯（missing 方向第十二刀）（2026-10-31）

missing 方向第十二刀：**SpeechSynthesisUtterance** 从「普通类 + 5 个自有数据属性」重写为 EventTarget 子类（webrtc.js 原位，WeakMap 槽、零自有键、原型访问器钉名），补齐 voice + 七个 on*（start/end/error/pause/resume/mark/boundary）；顺带把 **SpeechSynthesisVoice** 做成真实例（getVoices 的 78 个声音原先是普通对象，`utterance.voice=` 无法做品牌校验）、**SpeechSynthesisEvent / ErrorEvent** 的 ctor 原先是占位（`new SpeechSynthesisEvent('x',{utterance})` 抛「2 arguments required, but only 0 present」）补成必填成员梯。speech_gaps.js 接管 voice/event 两面（minted 原型保留，toStringTag 品牌不丢）。

**Chrome 145 真相（k70 两轮 36 腿，引擎逐腿 0 diff 除 persona 差）**：①默认值 `lang === ''`（旧实现是 'en-US'）、`voice === null`、volume/rate/pitch=1；②**text/lang 走 ToString**（`null`→'null'、Symbol 抛「Cannot convert a Symbol value to a string」带属性前缀）；构造器 `new U(undefined).text === ''`、`new U(null).text === 'null'`；③**volume/rate/pitch=ToNumber→非有限抛 TypeError「The provided float value is non-finite.」→夹到 [0,1]/[0.1,10]/[0,2]→float32 舍入**（`rate=0.1` 读回 0.10000000149011612、`'0.3'`→0.30000001192092896、`volume=null`→0）；④voice setter 只收 null/undefined（→null）或 SpeechSynthesisVoice，其余 TypeError「Failed to convert value to 'SpeechSynthesisVoice'.」；⑤on* 属性**保留函数和任意对象**（`onend={}` 读回 `{}`）、其余落 null——与 IDB 的 on* 同源规则的另一面；⑥无 new 调用报「Please use the 'new' operator…」；foreign receiver getter/setter 皆 Illegal invocation；⑦`speak()` 梯：无参「1 argument required」、非 utterance「parameter 1 is not of type 'SpeechSynthesisUtterance'.」；⑧SpeechSynthesisEvent ctor.length=2，`utterance` 必填（缺→「Required member is undefined.」），charIndex ulong 截断（3.7→3）、charLength/elapsedTime/name 缺省 0/0/''。

数字：missing 68→**67** 行、same 800→**801**、order 23/kind 56/statics 9/parent 5/syms 1 持平；枚举 211/204/7；实例 127/127；ratchet 65 持平（exports 块经 `analyze.mjs --gen-exports` 重生）；快照 1 处变。
行为验证记档分歧（GUESS/persona 差）：**speak() 仍无生命周期**（不入队、不发 start/end/error 事件、speaking/pending 恒 false——抓取机无声音时 Chrome 发 `not-allowed` error，桌面 Chrome 有声音时发 start→end，persona 有 78 声音却不发事件是已知缺口）；getVoices 数量/内容是 persona（抓取机 0 个）；SpeechSynthesisEvent/ErrorEvent 的 null/非对象 init 报文、utterance 品牌失败报文、ErrorEvent 的 `error` 必填与枚举报文按 WebIDL 惯例推得**未探**（只探到 utterance 必填一条）；pause/resume/cancel 仍 no-op。
测试：`knife70_speech_utterance_family`（新，22 组断言：形状/链/默认值/ctor 文本四值/无 new 报文/字符串化/Symbol 两报文/float 夹取十一值/非有限三报文/voice 四态/on* 保留规则/dispatch 双路/getter 钉名/foreign/speak 梯/Event ctor 与错误梯/ErrorEvent/getVoices 新数组）。
**教训**：①**`String(sym)` 不抛、模板字符串才抛**——WebIDL ToString 要用 `` `${v}` ``，否则 Symbol 静默通过；②**on* 的「只收函数」不是通用规则**——同一 Chrome 里 IDB 与 Speech 的处理器属性取值规则不同（Speech 保留对象），逐接口探，别移植；③先看家族邻居——SpeechSynthesisEvent 的 ctor 是占位，不探不知道；④webrtc.js 里的类已是真类，改写时 exports 块会陈旧，跑 `--gen-exports`。

## M4 刀71：SVGGradientElement 7 + SVGAElement 6 + interestForElement×4（missing 方向第十三刀，一刀 +5 行）（2026-10-31）

missing 方向第十三刀：**SVGGradientElement**（gradientUnits/gradientTransform/spreadMethod 三视图 + href 改 SVGAnimatedString + 四个 SVG_SPREADMETHOD_* 常量）、**SVGAElement**（target/href 两个 SVGAnimatedString 视图 + download/ping/hreflang/referrerPolicy 四个朴素反射串）、**interestForElement**（HTMLAnchorElement/HTMLAreaElement/HTMLButtonElement/SVGAElement 四个接口各缺这一个——抓真值时才发现 audit 的四行 missing 同源，新文件 interest_for.js 一次补齐）。复用 svg_root.js 既有 SVG 视图基础设施（`__diting_svgAnimEnum/TransformList/String`、`wipeFace`、`ctorLast`），只补块。

**Chrome 145 真相（k71 38 腿，引擎 35 腿首轮即平，余 3 腿=SVGElement 父链问题记档）**：①gradientUnits 默认 2、spreadMethod 默认 1，junk/空串/**大小写不对**（'USERSPACEONUSE'、'REFLECT'）回默认；②**baseVal 写入序列化关键字**（1→userSpaceOnUse、3→repeat）并设**范围梯**：0 抛「The enumeration value provided is 0, which is not settable.」、超最大值抛「…(9) is larger than the largest allowed value (3).」（TypeError，前缀 `Failed to set the 'baseVal' property on 'SVGAnimatedEnumeration'`）——这条规则下沉进共享的 `__diting_svgAnimEnum`（新增可选 `max` 参，旧调用方不变）；③gradientTransform=SVGAnimatedTransformList，junk→0 项；④**href 的 SVGAnimatedString 回落 xlink:href**（普通 href 属性胜出）——改在共享 string 视图里，pattern/use/image 一并受益；⑤常量在原型**和**构造器上各一份（e--）；⑥foreign receiver 与**其他 SVG 元素**（rect）调 getter 都 Illegal invocation；⑦SVGAElement 的 download/ping/hreflang/referrerPolicy 是**朴素反射**：set→String(v)（null→'null'、undefined→'undefined'），referrerPolicy **不做枚举限制**（'bogus' 原样读回）；⑧interestForElement：无属性→null；setter(Element) 记住元素并把属性写成 ''、读回该元素；setattribute 改成别的值后走 **id 查找**（找不到 null）；`=null` 删属性；非 Element → TypeError「Failed to convert value to 'Element'.」（报文里的接口名随接收者）。

数字：missing 67→**62** 行、same 801→**805**、kind 56→**55**、statics 9→**8**；extra 0/len 0/order 23/parent 5/syms 1 持平；枚举 211/204/7；实例 127/127；ratchet 65（新块里 `typeof SVGAElement` 这类裸名会让 crossFileLoadTimeNames 涨到 66——改用 `globalThis.X` 取名压回 65）；快照变。
行为验证记档分歧：**SVGElement 的父链仍是 HTMLElement**（`new SVGGradientElement()` 报「Failed to construct 'HTMLElement'」而非 Chrome 的「…'SVGGradientElement': Illegal constructor」；`SVGLinearGradientElement` 的原型链多一层 HTMLElement）——这正是 audit「parent 5」行（SVGElement/SVGClipPathElement/SVGSymbolElement/CSSStyleDeclaration/CSSStyleRule）的根，是下一刀候选；interestForElement 的 setattribute 与显式关联的取舍（属性值恰为 '' 时视为显式关联、跨树校验未做）；HTML 三个接口只验了往返不验报文。
测试：`knife71_svg_gradient_anchor_family`（新，27 组断言：默认/units 五值/spread 六值/写序列化与范围梯四报文/transform 与 junk/href xlink 回落/常量四值与双份描述符/getter 钉名/foreign 两路/SVGAElement 默认十一值/各反射与空值/referrerPolicy/target 视图/getter 钉名六件/foreign/零自有键/interest 七步/HTML 三接口存在与往返）。
**教训**：①**一族 missing 常常同源**——audit 里分散的四行 interestForElement 是同一个缺口，抓真值时顺手并掉比逐行做省事；②**共享视图的行为改动是杠杆**——xlink:href 回落与枚举范围梯各改一处，惠及一串元素；③**裸全局名会涨 ratchet**——load-time 的 `typeof SomeGlobal` 数一个跨文件名，`globalThis.SomeGlobal` 不数；④Illegal invocation 对**同族但不同接口**的元素同样要抛（rect 调 gradient getter），用 `instanceof` 品牌别用「有没有 attr」。

## M4 刀72：SVG 父链 HTMLElement→Element + SVGGraphicsElement 五件 + CSS 两处父链（audit parent 5→0）（2026-10-31）

audit 「parent 5」行（SVGElement/SVGClipPathElement/SVGSymbolElement/CSSStyleDeclaration/CSSStyleRule）一刀清零，同时补上抓真值时露出的 **SVGGraphicsElement 五件缺口**（transform/nearestViewportElement/farthestViewportElement/requiredExtensions/systemLanguage——引擎里 `rect.transform` 一直是 undefined）与 SVGElement 的五件（className/ownerSVGElement/viewportElement/autofocus/attributeStyleMap）。

**根因是一句错注释**：bootstrap 里 #28 写着「Chrome routes SVG through HTMLElement」并据此让 `SVGElement extends HTMLElement`——Chrome 145 实测**没有这一层**（`svg.hidden/click/innerText/title/lang` 全 undefined、`svg instanceof HTMLElement` false、`SVGSymbolElement` 是 SVGGraphicsElement 而 `SVGClipPathElement` 只是 SVGElement）。SVGElement 自有的 on*/dataset/style/focus 本来就在，被多继承的是 HTMLElement 的 30 个成员（title/lang/translate/dir/hidden/inert/accessKey/draggable/spellcheck/autocapitalize/editContext/contentEditable/enterKeyHint/isContentEditable/inputMode/virtualKeyboardPolicy/offset*/popover/innerText/outerText/writingSuggestions/attachInternals/click/hidePopover/showPopover/togglePopover）。

**Chrome 145 真相（k72 三轮 44 腿；引擎逐腿 0 diff 除记档项）**：①className 是**只读** SVGAnimatedString（赋值静默丢弃，attr 照旧）；②ownerSVGElement=最近 svg 祖先（最外层 svg→null；**直接在 foreignObject 下的 svg→null**）；③**viewportElement/nearestViewportElement=最近 svg/symbol/image 祖先**（marker/pattern/foreignObject 不建 viewport）、farthest=最外层那个；④`new SVGGradientElement()` 等的报文名指**被构造的接口**（`Failed to construct 'SVGGraphicsElement': Illegal constructor`，TypeError）；⑤SVGGraphicsElement 的 transform=SVGAnimatedTransformList（`rotate(45) translate(1 2)`→2 项 type 4/2）、requiredExtensions/systemLanguage=SVGStringList（同对象缓存）；clipPath 无 getBBox/getCTM（只有自有 transform）、symbol 有；⑥**CSS 侧：145 没有 CSS2Properties**——CSSStyleDeclaration 直接在 Object 上，`new CSSStyleDeclaration()` 抛 Illegal constructor，**CSSStyleRule 直接在 CSSRule 上**（旧代码的 CSS2Properties 中间层引的是 155 抓取，目标版本是 145）；⑦**style 属性序列化 `name: value;` 空格连接带尾分号**，attributeStyleMap.set 之前少尾分号。

**实现**：SVGElement `extends Element`+节点 id 门（非数字抛带接口名的 Illegal constructor）；clip/symbol 表项换父；svg_root.js 加 SVGElement/SVGGraphicsElement 两块（`wipeFace` 保留已有 getBBox/getCTM/getScreenCTM）；CSSStyleDeclaration 加一次性 flag 铸造门 `_newCssDecl`（三处内部铸造改走它，css_rules.js 走隐藏全局）；删除 CSS2Properties 中间原型；css_rules.js 父链表 `['CSSStyleRule','CSSRule']`；stylemap.impl.js 序列化修正后重跑 `tools/webplatform/bundle.py`；`_style` 私有槽原靠 HTMLElement 链被隐藏，现登记 hide_priv_extra.json 的 SVGElement 并重生 hide_priv.js（enum 审计抓出的新泄漏——「改父链要跑 enum 审计」）。

数字：**parent 5→0**、missing 62→**60** 行、same 805→**809**、order 23/kind 55/statics 8/extra 0/len 0/syms 1 持平；枚举 211/204/7（过程中一度 203/8，svgEl 泄 `_style`，登记后复原）；实例 127/127；ratchet 65 持平（bootstrap exports 块经 `--gen-exports` 重生）；快照变。
行为验证记档分歧（GUESS/已知差）：**SVGStringList 无索引访问**（`systemLanguage[0]` Chrome 给 'en'、引擎 undefined）；**SVGMaskElement 缺 requiredExtensions/systemLanguage**（Chrome 的 mask 有，clipPath/gradient 没有）；`StylePropertyMap` 之外的解析路径对未知属性的过滤（`a{b:c}` 的 `rule.style.cssText` Chrome 为 ''、引擎保留未知声明）；ownerSVGElement 穿过 HTML 祖先继续上行（Chrome 在 foreignObject 内的细节未全探）。
测试：`knife72_svg_parent_chain`（新，27 组断言：七条原型链/五条 ctor 报文/className 与写入/owner 六值/viewport 八值（含 image 与 marker）/nearest+farthest/autofocus/attributeStyleMap/二十个 HTMLElement 成员全 undefined/十三个仍在成员/instanceof 六值/十接口 graphics 五件/transform 列表/字符串列表/clip 与 symbol 成员/getter 钉名/foreign 两路/零自有键/CSS 链与 ctor 与全局/规则链/style 序列化三值）；旧 `svg_interface_family_and_namespace` 两条 #28 期望（`svg instanceof HTMLElement`、`SVGElement→HTMLElement` 链）按 Chrome 145 改。
**教训**：①**错注释会固化成错架构**——#28 那句「Chrome 就是这样」从未对过抓取，整条 SVG 链错了好几刀才被审计 parent 行揭穿，**注释里写「Chrome 如何如何」必须带探针日期**；②**改父链会连带移走隐式继承来的成员**——先盘点（truth 里谁该有谁不该有），再补自有，再跑 enum/instance 审计看私有槽泄漏；③SVGGraphicsElement 五件缺了很多刀没人发现，因为各具体元素 audit 行都「same」——**基类缺口只在基类行显形**，missing 榜按接口行看别漏了基类；④CSS 目标版本以 145 为准，别信别的版本抓取的注释（`chrome-155 truth` 反例）；⑤stylemap.impl.js 是手写模块，改完必须 `bundle.py` 重生 platform_bundle.js。

## M4 刀73：Selection 六件——direction/baseNode/baseOffset/extentNode/extentOffset/getComposedRanges（missing 方向第十五刀）（2026-10-31）

补 Selection 的六个缺口（range_selection.js 原位）。**Chrome 145 真相（k73 两轮 28 腿，引擎逐腿 0 diff 除 cross-root 一项记档）**：①`direction` 取 `'none'`（无选区、collapse 后、**addRange 加入的范围也是 none**）/`'forward'`/`'backward'`（setBaseAndExtent、extend、selectAllChildren 给 forward/backward）——引擎内部值是 forwards/backwards/none，addRange 与 collapse* 原先记 forwards，改记 none；②baseNode/baseOffset/extentNode/extentOffset 是 anchor/focus 的老别名（无选区时 offset 0、node null）；③`getComposedRanges`：参数是 **options 字典**（`{shadowRoots:[ShadowRoot…]}`）——undefined/null/数组/**直接传 ShadowRoot** 都接受但**不授予任何 shadow 可见性**（Chrome 145 把非字典对象当空 options，位置参数形式无效，多余参数忽略），原始值（5/'x'）抛「The provided value is not of type 'GetComposedRangesOptions'.」，`shadowRoots` 为 null/非可迭代抛「…cannot be converted to a sequence.」、含非 ShadowRoot 抛「Failed to convert value to 'ShadowRoot'.」；返回**每次新建**的 StaticRange 数组、起止已规范化（反向选区也 start≤end）、无选区→[]；端点落在**未列出的 shadow 树**里时被抬到宿主外面——起点=(宿主.parentNode, 宿主下标)、终点=(宿主.parentNode, 宿主下标+1)，列出了对应 shadowRoot 则原样；length 0。

实现：`_inDoc` 改**含 shadow 的包含判断**（沿 parentNode/ShadowRoot.host 上行到文档，shadow 内的节点现在能进选区——原先 `document.contains(shadowNode)` 恒假导致 shadow 里的 collapse/setBaseAndExtent 静默无效）；新访问器经 `_selChk(this)` 品牌检查（foreign receiver Illegal invocation）；getComposedRanges 的 lift 循环沿 shadow 链逐层抬升。

数字：missing 60→**59** 行、same 809→**810**、order 23/kind 55/statics 8/parent 0/extra 0/len 0/syms 1 持平；枚举 211/204/7；实例 127/127；ratchet 65（callTime 170）；快照变。
行为验证记档分歧：**跨 shadow 边界的选区**——`setBaseAndExtent(lightNode, 1, shadowNode, 3)` Chrome 保持起点在 light 树，引擎的 Range 在两个 root 间收拢（k73 round2 `cross` 一腿不平，需要 Range 本身支持跨 root 边界点）；shadow 内选区的 `anchorNode` 是否对外部调用者重定向到宿主（Chrome 返回宿主还是内部节点未单独探）；`modify()` 仍是空函数（需要版面）；getComposedRanges 只返回单个 range（Chrome 与 rangeCount 一致，多 range 不存在）。
测试：`knife73_selection_direction_and_composed`（新，24 组：direction 八态/getter 钉名/foreign/length/空选区/range 规范化与反向/fresh/collapsed/十一种参数梯/retarget/withRoot/positional/零自有键）。
**教训**：①**引擎 probe 端点会把页面脚本跑两遍**（page 里一遍 + /eval 一遍），带副作用的探针（选区、焦点、cookie）第二遍从第一遍留下的状态起跑，要在探针开头显式清状态（本刀 dir_none 的假阳性就是这个）——Chrome 腿只跑一遍所以不受影响；②「shadow 内的节点属于文档」要用 shadow-including 判断，`document.contains` 对 shadow 内容恒假；③`cd ..` 之后跑 probe 会静默用旧结果，起 server/跑探针前先回仓库根。

## M4 刀74：TextMetrics 五件 + 整面重写——WeakMap 槽/只读访问器/度量模型（missing 方向第十六刀）（2026-10-31）

TextMetrics 缺 fontBoundingBoxAscent/Descent 与 hanging/alphabetic/ideographicBaseline 五件；顺带把整面从「`Object.assign(new TextMetrics(), {...})` 自有数据属性」重写为 10 个只读原型访问器（钉名 `get x`）+ WeakMap 槽 + 零自有键（旧实例 `Object.keys` 泄出五个字段、`m.width=99` 能改）。`new TextMetrics()` 抛 Illegal constructor，foreign receiver 抛 Illegal invocation，measureText 经隐藏工厂 `__diting_newTextMetrics` 铸造。

**Chrome 145 真相（k74 27 腿；Mac 上 Arial/Times/monospace/sans-serif 多字号多 baseline/align 的真实数值）与度量模型**：结构腿（shape/ctor/getters/foreign/frozen/args/empty/zero/json 共 9 项）与 Chrome 逐项一致；**数值是模型不是字形几何**（引擎无光栅器），模型按 Chrome 实测关系建：①fontBoundingBoxAscent/Descent=round(size×族比，Arial .905/.212、Times .891/.216、mono .928/.236)；②alphabetic 基线 0、**hangingBaseline=0.8×ascent、ideographicBaseline=−descent**（Arial 16/32px 及 mono 皆验）；③textBaseline 偏移（基线高出 alphabetic 的距离）top=0.77539S、hanging=0.8A、middle=(top+bottom)/2、ideographic=−D、bottom=−(D+0.03711S)，则 fontAscent=A−shift、fontDescent=D+shift、各 baseline=原值−shift；④墨迹框 Left=base−0.08008S、Right=(W−base)−0.03711S（base=textAlign 锚：start/left 0、center W/2、right/end W），ascent .71582S−shift，descent 有降部字符 .21045S 否则 .01172S（+shift）；⑤空串/纯空白无墨迹（Left/Right/Ascent/Descent 皆 0）、**0px 字号全零**；⑥`measureText()` 无参抛「1 argument required」。16px Arial 的 alphabetic/top/hanging/middle/ideographic/bottom 六种基线数值与 Chrome 实测在 1e-3 内吻合。**`width` 仍是引擎既有的 persona 字体模型**（chars×6×scale×factor）原样保留——它与真实字宽差一倍多（55.2 vs 36.46），属字体指纹线的另案，本刀不动以免击穿既有字体探测测试。

数字：missing 59→**58** 行、same 810→**811**、kind 55→**54**、order 23/statics 8/parent 0/extra 0/len 0/syms 1 持平；枚举 211/204/7；实例 127/127；ratchet 65；快照变。
行为验证记档分歧：**width 与真实字宽不符**（见上，字体指纹线）；非 Arial/Times/mono 家族（sans-serif 10px Chrome 是 ascent 11/descent 3/ideographic −1.4，模型给 9/2/−2——系统 sans 字体的度量不是 Arial 比）；ink 的 Left/Right 对不同字符的字形差异（'i' vs 'W'）不分——Chrome 逐字形、模型按固定边距；letterSpacing/wordSpacing/fontKerning 对度量的影响未建模。
测试：`knife74_text_metrics`（新，20 组：结构九项 + 六种 baseline 数值 + 两种字号 + 降部/空串/空白/align 四锚/零字号/无参报文/measureText.length）。
**教训**：①**不可复现的真值（字形几何）就别硬追数值**——抓结构的全部（形状/品牌/只读/零自有键）要精确，数值走「按真实关系建的模型」并在回执里写明哪里是模型；②**在比对里把「引擎故意不同的量」（width）隔离出来**，别让它淹没其余真信号；③**符号方向先在一个 baseline 上验算再铺开**（本刀 ascent=A−shift 我先写成 A+shift，六种 baseline 对账一眼看出）；④**每条命令都以 `cd 仓库根 &&` 开头**——`cd ..` 之后跑的 probe/regen 会静默用旧结果（本轮连踩两次）。

## M4 刀75：missing 尾部一次清零（58 行 → 0）——tail_members.js + 六处整面改造（2026-10-31）

用户令「一直处理完」：把 audit 剩下的 58 个 missing 行（149 件）整批处理，**missing 58→0、same 811→848、syms 1→0**。方法分三路：

**①own→proto 提升（lift_own，复用既有生成器，造合成实例名表喂 `gen_lift_own.py`）**：MediaSource(sourceBuffers/activeSourceBuffers)、TextDecoderStream(5)/TextEncoderStream(2)、RTCError(6，另补 httpRequestStatusCode 赋值)、GPU(wgslLanguageFeatures)、ImageBitmap(width/height)、CryptoKey(4——`makeKey` 的 `defineProperty` 改成普通赋值)、WebSocketStream(url/opened/closed——**`opening`→`opened` 改名**，Chrome 145 名)、SharedWorker(port)、SourceBuffer(5)。同一教训第三次复现：**类构造器里是赋值就能 lift，是 defineProperty/只读访问器就会被挡**（我先用只读访问器做 SharedWorker.port/SourceBuffer 触发 `Cannot set property … which has only a getter`，连带 MSE 两条页面测试崩——改回 lift 即复原）。

**②三面手工整改**：**AudioNode**——节点原来是 `Object.assign` 出来的普通对象（context/channelCount*/numberOfInputs/Outputs 全是自有键，connect/disconnect/addEventListener 也自有）；现在是 `Reflect.construct(EventTarget)` 套 `AudioNode.prototype` 的真 EventTarget，六个属性是原型访问器（读 `__diting_anState` WeakMap 槽；channelCount 校验 [1,32] 抛 NotSupportedError、模式/解释枚举非法静默忽略），connect/disconnect 挂到 `AudioNode.prototype`，**enum 审计的 gain/ctxDest 两个 leaky 对象清掉**；**FileList**——`_makeFileList` 的 length/item/@@iterator/@@toStringTag 全移到 `FileList.prototype`（实例只留下标自有项，构造器 name 钉 'FileList'）；**CSSStyleDeclaration.cssFloat**——proxy 的 set 陷阱原先对「原型成员」一律静默吞写，现在对带 setter 的原型访问器调用 setter；cssnames 表里去掉 cssFloat 的自有识别（泄漏到 own keys）；ImageData `data`——原型访问器带 setter，把构造器赋值变成自有数据属性（Chrome 实例自有 data）。

**③tail_members.js（新，分节容错）**：事件处理器属性（BatteryManager×4/MediaStream×4/OffscreenCanvas×2/Performance/RTCDtlsTransport×2/RTCSctpTransport/ServiceWorkerContainer×3/SharedWorker.onerror/TextTrackCue×2/VisualViewport×3/AudioContext.onsinkchange）、遗留常量（WebSocket×4/EventSource×3/Window×2，原型+构造器双份，原型份先 configurable 以便 ORDER 遍排序再被 flag 遍封口——刀69 的教训）、AudioContext.sinkId/setSinkId、BaseAudioContext.audioWorklet、DocumentType 的 ChildNode 四法、Range.expand、HTMLHtmlElement.version、HTMLIFrameElement 五件、SVGStyleElement.media/title、HTMLCanvasElement.captureStream/transferControlToOffscreen、Path2D.roundRect、File.lastModifiedDate/webkitRelativePath、ImageData.colorSpace/pixelFormat、MediaMetadata.chapterInfo、VideoPlaybackQuality×4、MediaStream.getTrackById、PerformancePaintTiming×3、Performance.interactionCount/toJSON、Geolocation toJSON×2、CSS 规则类十件（FontPaletteValues/ImportRule.supportsText/KeyframesRule.length/ScopeRule.start/end/StyleRule.styleMap/StyleDeclaration.parentRule）、SVGGeometryElement×3/Polygon+Polyline points/animatedPoints、RTCDtlsTransport/SctpTransport/EncodedAudioFrame/StatsReport/IceEvent.candidate、ServiceWorkerContainer.startMessages、WebTransport×3、GPUAdapter×3、WritableStreamDefaultController.signal、ViewTransition×2、ToggleEvent.source、XHR 两个归因方法（非 OPENED 抛 InvalidStateError 带 Chrome 报文）。

数字：missing 58→**0**、same 811→**848**、syms 1→**0**、order 23/statics 7/parent 0/extra 0/len 0 持平、kind 54→**63**（+9：lift 生成的访问器带 setter，Chrome 的只读属性无 setter——刀 lift_own 一贯的已记档折中，ro/rw 行差）；枚举 211/204/7（gain/ctxDest 清、ImageData/style 两度反弹均已修）；实例 127/127；ratchet 65 持平；快照变；cargo test **947 通过 + 110 基线逐名全等**（页面测试的 27 条级联失败是 MSE 两条真失败毒化共享锁所致，修后消失）。
行为验证记档分歧（GUESS）：**tail 的新成员多为「形」而非「行为」**——事件处理器属性不会被引擎真触发（没有对应事件源：电池/TextTrack/VisualViewport/SW 等）；SVG points/animatedPoints 返回空列表对象（无 SVGPointList 解析）、isPointInFill/Stroke 恒 false、pathLength 的默认值取 0；RTCStatsReport 的 entries/keys/values 按 Map 实例委托；CSSFontPaletteValuesRule 三值由 cssText 正则取；GPUAdapter 三值为空桩；ViewTransition.waitUntil/types 桩；`HTMLCanvasElement.captureStream` 返回空 MediaStream；`Path2D.roundRect` 只实现单半径；`Performance.toJSON` 的 timing/navigation 以字段遍历得出；**新访问器的 foreign receiver 只拒绝非对象（对象型 foreign 不抛）**——tail 的统一松弛；AudioNode 现在是真 EventTarget 但各节点类型子原型（GainNode 等）仍未挂上（instanceof GainNode 为 false）。
测试：`knife75_tail_members`（新，29 组：处理器往返/常量描述符/AudioNode 状态与写入校验与 connect 与事件/sinkId/FileList 两形/File/ImageData/lift 五类零自有键与取值/cssFloat 四态/HTML 反射/DocumentType/SVG/XHR 两报文/Geolocation/Performance/杂项/SourceBuffer/CryptoKey）；旧测试 4 处按 Chrome 145 真值同步（AudioContext/BaseAudioContext 原型名单、knife57 的 CSSScopeRule/GPUAdapter 自有名）。
**教训**：①**missing 榜清零靠分类不靠逐个**——一半是 own→proto 的 lift，一半是「形」的批量补，写一份分节容错的 tail 文件最省；②**只读访问器会挡住类构造器里的赋值**——凡既有类内部在赋值的字段，一律走 lift（带 setter），不要手写 getter-only；③**页面测试的大面积级联失败别当回归**——先单测每一条，常是少数真失败毒化共享锁；④**生成器要吃合成输入**——instance-audit 覆盖不到的类，造 `{Class:{names:[…]}}` 的小 JSON 喂 gen_lift_own 即可；⑤清零后 audit 的下一个「脏」是 order/kind/statics，不是 missing。

## M4 刀76：order/statics 清零 + 构造常量全体可配置化（2026-10-31）

刀75 后 audit 剩 order 23 / statics 7 / kind 63。本刀把 **order 23→0、statics 7→0、same 848→889**（936 接口中 889 行完全一致，余 47 行全是 ro/rw 差）：

**根因同一条（刀69 教训的第三次复现）**：创建层把 GL/CSSRule/HTMLMedia/XHR/HTMLTrack/SVG 等接口的**常量以 `{value, enumerable:true}` 即 e-- 非配置**直接落在原型上，且早于其它成员创建——ORDER 遍无法移动非配置成员，于是 20 个接口「常量居首」卡在 Chrome 成员序之外。统一改法：创建层一律 **`configurable:true`**，shape_fix 的 ORDER 遍排序，flag 遍最后封成 e--。涉及：stub_members（模板与生成件同改；该生成器**不再能从当前裸抓取复现 DATA**——许多原纯桩接口后来被真类取代，重跑会丢条目，故生成件按同一模板文本打补丁，DATA 原样保留，已记入档）、css_rules.js（CSSRule 的 ctor+proto 双份）、bootstrap HTMLMediaElement（九常量；另补 **九个 ctor 静态**，且静态序 NETWORK_* 先于 HAVE_*）、xhr.js、gen_iface.py + 生成件 htmlreflect.gen.js（HTMLTrackElement 四常量）。

**shape_fix 生成器两处增强**：①新增 **SORDER 表与静态成员序遍**（构造器自有静态名按 Chrome 序 delete/re-add，length/name/prototype 留首）；②ro/rw 差异不再跳过 e/c flag（flag 与 setter 有无相互独立——StyleSheet/MediaSource 若干 lift 访问器原为不可枚举 `-c`）。

**其余**：Error 系 `.name` 改 configurable（Chrome `-cw`，7 行）；Range 的四个比较常量由 static getter 改为数据静态（e--）；`__diting_install_all`（document.all 的晚装入点，在 shape 遍之后）自行把 HTMLAllCollection `constructor` 放末、`item.length`=0，并把 `all` 插到 `scrollingElement` 之前；该晚装入点同时 **删除 `Iterator.concat`**（本引擎 V8 自带，Chrome 145 无；tail_members 阶段删不掉——它是 realm 初始化后才出现）；静态补齐：HTMLScriptElement.supports、BluetoothUUID 四法（别名规范化 `xxxxxxxx-0000-1000-8000-00805f9b34fb`）、IdentityProvider 三法、MediaSource.canConstructInDedicatedWorker、RestrictionTarget.fromElement；PutForwards 类 setter（CSSImportRule/CSSMediaRule.media→mediaText、CSSKeyframeRule/CSSPageRule/CSSStyleRule.style→cssText、CSSPageRule.selectorText、SVGAElement.relList→value、HTMLTableElement.caption/tHead/tFoot 带 Chrome 报文）；RTCEncodedAudioFrame.data 由方法改 rw 访问器。

数字：shape audit **missing 0 / extra 0 / order 0 / len 0 / statics 0 / parent 0 / syms 0**，same 889，**kind 47**（47 行全是同一类：lift_own 生成的访问器带 setter 而 Chrome 只读无 setter——已记档折中，页面写只读属性会成功而非被忽略）；枚举 211/204/7；实例 127/127；ratchet 65；快照变；cargo test **947 通过 + 110 基线逐名全等**。
**教训**：①**非配置常量是顺序的天敌**——任何创建层都不要直接落 e--，落 configurable 再由最后一遍封口；②**晚于 shape 遍的安装点要自己排序**（`__diting_install_all`）；③**引擎自带的新 V8 特性会比目标 Chrome 多**（Iterator.concat）——审计 statics 的 extra 要看；④**生成器的输入随引擎演化会失真**——stub_members 这类「依赖当时裸抓取」的生成件别重跑，改模板后对生成件打同文补丁并记档。

## M4 刀77：SVG 标签→接口全接线 + 27 个纯桩 SVG 接口变真反射器 + 语音队列 + SVGStringList 下标（2026-10-31）

audit 全零后转向**行为**账。抓 63 个 SVG 标签的 `createElementNS(...).constructor.name`，**34 个标签引擎仍包成裸 SVGElement**（feBlend…feTurbulence 全部滤镜原语、mask、filter、textPath、view、script、metadata、mpath、animateMotion/animateTransform/set，其中 animateMotion/Transform/set 还错包成 SVGAnimateElement）——这些接口类在（mint 桩 + 前几刀的成员面），只是元素工厂不认。

①**svg_tags.js**（新）：bootstrap 暴露 `__diting_svgTagMap`（元素工厂的 `_svgTagInterfaces`），34 个标签各映射到一个 `W(nid){return Reflect.construct(SVGElement,[nid],W)}`，`W.prototype` 指向接口自己的原型——元素穿接口的原型、过 SVGElement 的节点 id 门；63 标签与 Chrome **逐标签全等**。②**svg_filters.js**（**生成件**，`gen_svg_filters.py` ← `chrome/svg-filters-truth-145.json`，抓取探针 svg-filters-probe.js 同入库）：这 27 个接口（mask/filter/mpath/script/textPath/view/fe* 全家/FEFunc 基类/三种光源）的 188 个成员原先是 stub_members 的「getter 答 undefined」桩——一旦标签接对，`mask.maskUnits` 就是 undefined，所以必须同刀换成真视图：枚举(含关键字表与 max 范围梯)/字符串/数字(含 stdDeviationX/Y 之类成对属性)/**整数、布尔视图**(新建，复用 SVGAnimatedInteger/Boolean 原型)/数字列表/PAR/Rect/字符串列表/长度(mask、filter 的 -10%/120% 默认、FE 区域 0%，百分比 `.value` 抛 NotSupportedError)/script.type·async、view.zoomAndPan 的朴素反射；**32 条抓取腿引擎逐条 0 diff**。③**SVGStringList 下标访问**（Proxy 服务 index 键，`list[0]`、`Object.keys`、`1 in list`），**systemLanguage 改按逗号分词**（'en, fr'→['en','fr']）。④**speechSynthesis 真队列**（audio.js）：speak 入队、队首异步 start、按文本长度/rate 估时后 end（60ms/字，夹 [60ms,5s]）、cancel 对运行中与排队者异步各发 `error:'canceled'`、pause/resume 冻结/续播并发事件；speaking/pending/paused 随队列变（end 事件时 speaking 已为 false）。

数字：shape audit 保持 same 889/missing 0/order 0/statics 0/extra 0/kind 47；枚举 211/204/7；实例 127/127；ratchet 65；快照变；cargo test **949 通过 + 110 基线逐名全等**。
记档分歧：filter/mask 的 x/y/width/height 与 FE 区域对**包含块百分比解析**未做（Chrome 同样 `.value` 抛错，行为一致但无 `valueInSpecifiedUnits` 之外的路径）；FE 视图只做反射不做滤镜**渲染**（引擎无滤镜管线）；`SVGFEFuncA/B/G/R` 的成员走基类视图；speech 的 start/end 时序是模型（无真音频）；light source 与 FE 的 `className`/style 等通用成员由 SVGElement 提供。
测试：`knife77_speech_queue_lifecycle`（队列全生命周期 14 步）、`knife77_svg_tags_and_filter_reflectors`（63 标签全表 + mask/filter/feBlend/feColorMatrix/feComposite/feConvolveMatrix/feTurbulence/feGaussianBlur/feFunc/light/textPath/script/view/foreign/strlist 十余组）。
**教训**：①**接对标签会暴露桩的谎言**——纯桩接口 getter 答 undefined，标签映射一改它们就成了可见的错值，必须同刀换真反射；②**「标签→接口」要整表对账**，别只看接口行 same（接口 same 不代表元素真戴着它）；③生成件（stub_members）改模板后无法复现 DATA，补丁同文即可，别重跑；④Proxy 服务索引键最省事的做法是 target 保持原对象，get 陷阱对内部字段强制取 target 作 receiver。

## M4 刀78：构造器保真（全表 `new X()` 对账）+ 15 个 HTML 标签接线（2026-10-31）

按「整表对账」方法对 936 个接口跑 `try { new X() }`，Chrome 与引擎 **183 项不同**：①**114 项 Illegal constructor 报文不同**（引擎要么无 `Failed to construct 'X':` 前缀、要么多一个句点、要么是 `DOMException` 名为 TypeError 而不是真 TypeError，要么错报成别的接口名——FeaturePolicy 报 'PermissionsPolicy'、HTMLFormElement 报 'HTMLElement'）；②20 个接口 Chrome 要求参数个数（`1 argument required, but only 0 present.`）而引擎可无参构造；③约 30 个接口 Chrome 是 Illegal constructor 而引擎能 `new`。

处理：①**源码全局改写**——49 处 `throw new TypeError("Illegal constructor")` 与 4 处 DOMException 形态统一成 `new TypeError("Failed to construct '" + new.target.name + "': Illegal constructor")`（new.target 随子类，HTMLElement 门同理，cssom_sheets 的 StyleSheet 因含 NUL 哨兵需按字节改）；global_surface 生成件表项 FeaturePolicy 改名、生成器加「去尾句点」规整；②**ctor_ladders.js（新）**：20 个需参接口 + 6 个 nid 型节点类（Node/Element/CharacterData/CDATASection/DocumentType/ProcessingInstruction，页面零参形态非法、引擎内部用数字节点 id 铸造）以**薄包装函数**替换全局——校验参数个数后 `Reflect.construct(Orig, args, new.target)`，**沿用原 prototype 对象**（instanceof/constructor/子类化/静态成员全继承，`WebKit*` 别名跟随），内部创建者因必带实参不受影响；③**15 个 HTML 标签接线**（svg_tags.js 的 HTML 半边：data/frameset/picture/listing/xmp 与 applet/bgsound/blink/image/isindex/keygen/menuitem/multicol/nextid/spacer→HTMLUnknownElement；暴露 `__diting_htmlTagMap`，143 标签与 Chrome 全等）。

数字：构造器差异 183→**43**（余项见下）；shape audit 保持 same 889/其余全零/kind 47；枚举 211/204/7；实例 127/127；ratchet 65；快照变；cargo test **950 通过 + 110 基线逐名全等**（过程中两个回归：`WebKitMutationObserver === MutationObserver` 因别名未跟随包装，补别名跟随；RTC 测试的旧报文期望按 Chrome 同步）。
记档分歧（余 43 项）：Chrome 为 Illegal constructor 而引擎仍可 `new` 的约 23 个**内部以 `new X()` 零参铸造**的接口（MessagePort、MimeType(Array)、Plugin(Array)、NodeList、DOMTokenList、CustomElementRegistry、MediaQueryList、PerformanceEntry、Selection、ImageBitmap、SVGPoint、XMLHttpRequestEventTarget、ElementInternals、Window、ServiceWorkerContainer、SourceBuffer、GPU/GPUAdapter、PerformanceObserverEntryList、BaseAudioContext、XMLDocument 等）——无法用实参形态区分页面与内部调用，需逐个改铸造点为「铸造 flag」才能关门；CSSMath* 空参 Chrome 抛 SyntaxError、引擎抛 TypeError；IIRFilterNode/ReadableStreamDefaultReader/RTCEncodedAudioFrame 的需参梯与若干 Rtc/SharedStorage 事件的报文细节；OfflineAudioContext 与 WebSocket、URL、StaticRange 的参数校验报文。
测试：`knife78_constructor_ladders_and_html_tags`（新：12 个 Illegal 报文、18 个需参梯逐名、合法形态仍可构造、六个节点类零参非法、节点功能不坏、子类化、15 标签）。
**教训**：①**整表对账**第三次抓出成片的系统性偏差（标签→接口、构造器报文）——凡「同类行为逐项对账」都应先做全表差异再分类；②**包装全局而非改类**是处理「内部创建与页面创建走同一个构造器」的低成本办法——前提是内部创建者必传实参；③new.target.name 比硬编码接口名更对（子类、别名自动正确）；④含 NUL 哨兵的源文件改字节而非文本。

## M4 刀79：默认值对账（所有无参可构造接口的属性默认值，Chrome 145 vs 引擎）（2026-10-31）

方法：对 Chrome `new X()` 成功的 116 个接口，读其原型上每个 getter 的值（原始值/null/对象构造器名）与引擎逐项比对——**23 个接口 72 个成员不同**。处理：①**12 个纯桩接口的 27 个成员**（Highlight/IdleDetector/CloseWatcher/CaptureController/Origin/ScrollTimeline/ViewTimeline/WebSocketError/WebTransportError/QuotaExceededError/IdentityCredentialError，及 MediaSource 的三个 onsource*）桩 getter 答 undefined，而 Chrome 答原始值/null 默认（priority/size 0、type 'highlight'、axis 'block'、opaque true、source 'stream'……）→ **stub_defaults.js（新，位于 stub_members 之后）**包一层：保留桩的品牌校验，只把 `undefined` 替换为默认；②Animation.rangeStart/rangeEnd 默认 `'normal'`（原为 null，且 setter 不存值）；③MediaStream 初始 `active=false`、id 为 UUID（原 true/''）；④`new Image().complete` 默认 true（有 src 加载时才翻 false）；⑤`XMLHttpRequest.response` 文本类型默认 `''`（原 null）；⑥SourceBufferList 构造器名改 'SourceBufferList'（原 `_SourceBufferList`）。抓取探针与 Chrome 捕获入库（chrome/defaults-probe.js、defaults-truth-145.json）。

数字：默认值差异 23 接口/72 成员 → **10 接口**；shape audit 保持 same 889/其余全零/kind 47；枚举 211/204/7；实例 127/127；ratchet 65；快照变；cargo test **951 通过 + 110 基线逐名全等**。
记档分歧（余 10）：AudioContext.baseLatency（persona/机器相关）、MediaStream.id（随机）、CSSStyleSheet.cssRules/rules 与 DataTransfer.items 的**对象构造器名**（引擎返回 Array/Object 而非 CSSRuleList/DataTransferItemList）、`new Document()`（引擎返回的像主文档而非空文档：URL/activeElement/body 都不同——需把 Document 构造器做成真新文档）、XRRay/XRRigidTransform/ScrollTimeline.source 的对象值默认、SharedStorageClearMethod 缺失。
测试：`knife79_default_values`（16 组默认值断言，覆盖上述 12 个桩接口与其余五处修复）。
**教训**：①**对账的第三维是「默认值」**——结构、构造器报文之后，`new X()` 后逐 getter 对账能一次抓出成片的 undefined 谎言；②桩接口的 getter 设计成「答 undefined」是占位，应随真值表补默认（本刀用一张从 Chrome 捕获派生的表）；③包装桩 getter 而不是重写，才能保住既有品牌校验与 Illegal invocation 行为。

## M4 刀80：方法必需参数梯（76 同步 TypeError + 4 Promise 拒绝）（2026-10-31）

对账第四维：**方法无参调用**。对 115 个无参可构造接口，逐个调用原型上每个方法（无参），与 Chrome 逐项比：**30 接口 121 项不同**，主因是引擎方法缺 WebIDL 必需参数梯（`Failed to execute 'm' on 'I': N argument(s) required, but only 0 present.`——Chrome 在碰接收者前就抛；Promise 返回操作改为**拒绝**而非同步抛）。

**先试了「按 Chrome length>0 全量包装 1219 个方法」，被否决**：全量包装与既有的细致实现冲突——EditContext 的品牌检查先于参数检查（foreign receiver 应得 Illegal invocation 而不是参数错）、platform_errors 测试钉的**错误对象形状**（栈帧数、message 属性标志）、已经手工做对的梯、`history.pushState` 家族的旧行为等 5 个测试同时回归。改为**精确表**：`chrome/ladders-truth-145.json`（Chrome-vs-引擎逐项差异：sync 76 / promise 4，探针 ladders-probe.js 入库）→ `gen_method_ladders.py` → **method_ladders.js（生成件）**：仅对这 80 个方法包一层 method-shorthand 薄包装（无 `prototype`、非构造、native 标记、保留原描述符 flag 与 Chrome length），参数不足时抛/拒绝 Chrome 的 TypeError，否则 `Reflect.apply` 直通。覆盖：FormData/URLSearchParams(append/delete/get/getAll/forEach)、Path2D(arc/arcTo/ellipse/bezierCurveTo/lineTo/addPath…)、XMLHttpRequest(open/setRequestHeader/overrideMimeType/getResponseHeader)、DocumentFragment(getElementById/querySelector(All))、Range(selectNode/comparePoint/insertNode/…)、CSSStyleSheet、FileReader、Highlight、MediaSource、MediaStream、Origin、DOMParser、ReadableStream、TaskController、Text、TextEncoder、RTCPeerConnection、CaptureController、DataTransfer 等；Promise 四个：BarcodeDetector.detect、CSSStyleSheet.replace、CaptureController.forwardWheel、ReadableStream.pipeTo。

数字：无参方法对账残余 arity 差异 121 → **1**（RTCPeerConnection.setRemoteDescription：Chrome 先报参数、引擎先报 closed 状态）；shape audit 保持 same 889/其余全零/kind 47；枚举 211/204/7；实例 127/127；ratchet 65；快照变；cargo test **952 通过 + 110 基线逐名全等**。
记档分歧：覆盖仅限「无参可构造」接口的方法（元素/文档/窗口等需要实例的接口未入表——宽探针在 Chrome 里因个别方法挂起而放弃，改用精确表）；品牌先于参数的顺序对被包装的方法不成立（foreign receiver 先得参数错）；方法的**非参数**行为差异（InvalidStateError 时序、返回值类型如 FormData.entries 返回 Iterator 对象的构造器名、ReadableStream.tee 等）未处理。
测试：`knife80_method_argument_ladders`（FormData/URLSearchParams/Path2D/XHR/DocumentFragment/Range 的逐方法报文含复数规则、两个 Promise 拒绝、包装形状与直通正常调用）。
**教训**：①**「全量机械包装」是反模式**——它覆盖了所有手工做对的细节（品牌序、错误对象形状），应只补**经差异探针证明缺失**的点；②Promise 返回操作的参数错误是**拒绝**不是抛出，表要分开记；③宽探针（调用所有方法）会碰到会导航/挂起的方法，探针要么限定对象集、要么带超时分片，别指望一次扫全；④生成件的输入应是「差异表」而不是「完整真值表」。

## M4 刀81：构造器收尾——XMLDocument 零参封口；两条路线试错记档（2026-10-31）

①XMLDocument 加入 nid 型节点类的零参封口表（`new XMLDocument()` Illegal constructor）。②**试错一（已回退）**：把 `new Document()` 做成「DOMParser 空文档」——DOMParser 的 XML 文档是**普通 Object**（`constructor.name==='Object'`、`instanceof Document` 为 false、带多余 documentElement），反而比现状更差；`new Document()` 的真修法需要先把 DOMParser/`createDocument` 产出的分离文档做成**真 Document/XMLDocument 实例**（独立大工程，挂账）。③**试错二（已回退）**：把 22 个「Chrome 为 Illegal constructor 而引擎仍可 `new`」的接口（BaseAudioContext/CustomElementRegistry/DOMTokenList/ElementInternals/GPU/GPUAdapter/ImageBitmap/MediaQueryList/MessagePort/MimeType(Array)/NodeList/PerformanceEntry/PerformanceObserverEntryList/Plugin(Array)/SVGPoint/Selection/ServiceWorkerContainer/SourceBuffer/Window/XMLHttpRequestEventTarget）整体换成「总是抛 Illegal」的全局包装——**310 个测试同时回归**（引擎内部大量经 `globalThis.X`/裸全局名铸造这些类，Window、NodeList、Selection 尤甚）；回退。结论：这 22 个只能**逐个把铸造点改成「铸造 flag」**，且要先逐类核对内部创建路径，不能靠全局替换。
数字：构造器对账 183→**42**；shape audit 不变；cargo test **952 通过 + 110 基线逐名全等**（回退后复验）。
**教训**：①**「页面可见的全局」与「引擎内部引用」在这个代码库里常是同一个绑定**——全局替换只对「内部用词法绑定或已捕获引用」的类安全（刀78 的 20+6 个恰好如此），否则灾难性回归；②试错后**先回退再验基线**，别带着半截改动往前走；③DOMParser 的分离文档不是真 Document，是 Document/XML 家族的深坑，需单开一刀。

## M4 刀82：零参封口扩 19 类；Document 真实例线判定暂缓（2026-10-31）

ctor_ladders 的 ILLEGAL0 表加 19 类（BaseAudioContext/CustomElementRegistry/DOMTokenList/ElementInternals/GPU/GPUAdapter/MediaQueryList/MessagePort/MimeType/MimeTypeArray/PerformanceEntry/PerformanceObserverEntryList/Plugin/PluginArray/SVGPoint/Selection/ServiceWorkerContainer/SourceBuffer/XMLHttpRequestEventTarget），`new X()` 抛 Illegal constructor。**二分筛出两个不能加**：ImageBitmap（createImageBitmap 内部无参创建）、NodeList（引擎内部无参创建，加了引发整片 page 测试级联失败）——需要先给内部创建者改传 mint 参数才能封。
**Document 真实例线（DOMParser/`new Document()`/createDocument）判定暂缓**：`Document` 的 documentElement/head/body 等 getter 直接打主文档 op（`_dom("document_element")` 无节点参数），做真实例要把一批 op 按文档参数化，改动面大；账本保留。
数字：cargo test 953 通过 + 110 基线逐名全等；shape 889/936 不变；enum 211/204/7；instance 127/127；ratchet 65。
**教训**：多个 `cargo test` 脚本不能并行跑在同一工作树（共享被改的 JS 文件，结果互相污染）。

## M4 刀83：NodeList/ImageBitmap 铸币封口——构造器账清零（2026-10-31）

Chrome 对 NodeList/ImageBitmap **任何参数表**都抛 Illegal constructor（k79-ctor-c），零参封口不够。内部只有两个铸造点（`_nodeList` 工厂、`createImageBitmap` 的 `fromDims`），改传隐藏全局 `__diting_ctorMint`（Symbol）；ctor_ladders 新增 MINT 表：首参≠token 一律 Illegal constructor，token 走 Reflect.construct 透传给忽略参数的真构造器。**936 接口的构造器行为至此全部对齐 Chrome**（ImageBitmap/NodeList 是最后两个页面可 new 的）。
数字：cargo test 954 通过 + 110 基线逐名全等（docmisc 全量跑抖一次、单跑过，已知共享锁家族）；shape 889/936 不变；enum 211/204/8——第 8 个 `ls`=IDB localStorage 双跑抖动（nodeList/htmlColl 的 4-div 差在 k82 前就在 7 个基线里）；instance 127/127；ratchet 65。
**教训**：enum 审计的 `ls`（`__diting_idb__mini*`）随捕获时序闪烁——判新泄漏先 diff 上一份捕获，别只看 leaky 计数。

## M4 刀84：lift 封口——ro/rw 分界的另一半（2026-10-31）

shape audit 的 kind 维（Chrome ro 访问器 vs 引擎 rw 数据/访问器对）收口。**全部 151 个 kind 差 = Chrome `ro:ec` vs 引擎 `rw:ec`**——即引擎把 Chrome 只读成员做成了可写。lift 侧（lift_own.js 的类）：`gen_lift_own.py` v2 把生成件改成 **DATA（Chrome-rw：get+set 对）+ RO（Chrome-ro：仅 getter）** 双表读写共享 WeakMap；ro/rw 拆分挪到「从既有 DATA 合并」之后，堵住 11 个 instance-audit 跳过类（SourceBuffer…OffscreenCanvas）漏封的洞。**类体内赋值恒为 strict——getter-only 原型上的 `this.x = v` 必抛**，所以每个内部写点必须改走隐藏槽：bootstrap.js 头定义 `__diting_liftSlots` WeakMap + `__diting_liftSlot(o)`（**必须放头**：platform_bundle 在快照期先于 lift_own.js 构造封口类如 TransformStream，放 lift_own 里快照直接 ReferenceError 崩）；转写 ~60 写点（xhr.js 三分支 send 全量 60 处收进 `xs` 别名、media_source、bootstrap 的 Attr/File/ImageData/ImageBitmap/TextDecoder(Stream)/WebSocketStream/SharedWorker/CanvasRenderingContext2D/TransformStream、cssom_sheets、crypto_subtle、webrtc、message_port、pointer_touch、tail_members 的 ImageData.data 改 own 数据属性+原型 getter、platform/attributes.impl.js 的 localName/ownerElement 五处 + 重捆绑）。
数字：shape **same 889→912，kind 47→24**（余 24=另一侧类，刀85 账：PerformanceTiming 16、TouchEvent 7、ErrorEvent/MessageEvent/StyleSheet/WebSocket 各 5…）；enum 211/204/7；instance 127/127；ratchet 65（attributes.impl.js 补声明 `legacy:__diting_liftSlot`）；cargo test **954 通过 + 110 基线逐名全等**（test_local_fetch_resolves_on_task_boundary 全量抖、单跑过，共享锁家族）。
测试：`knife84_sealed_ro_accessors`（RO 成员描述符 getter-only、rw 仍成对；sloppy 页面写=静默 no-op 值不变、strict 写抛 TypeError；封口成员经引擎内部状态变化仍活——sync data-URL XHR 的 readyState/status/responseText、createAttributeNS 的 name/localName/prefix）。
**教训**：①类体/构造器里的赋值是 strict 模式——「先造后改」（构造完再 `obj.x=v` 补字段）在封口后是**必炸**模式，同账移交槽；②写点转换的精确范围=audit kind 差 ∩（DATA∪RO）按最新捕获重算，别拿 RO 超集硬转（Image/TextMetrics 已在别处封好，无需再动）；③一个抛点藏一个整族测试：`createImageBitmap` 内部构造抛 TypeError 时，`.then` 链无人接 → 外层 Promise 永不落定 → 测试表现为 **EVAL_TIMEOUT 而非 Promise rejected**——超时不一定是挂起，先查内层同步抛；④`#<W>` 这类无名实例=ctor_ladders 包装函数的内名（`const W = function...`），报文落在包装层不代表抛点在包装层。

## M4 刀85：kind 归零——余 24 类只读面收口（2026-10-31）

shape audit kind 维清账：余 24 接口 / 77 成员全部 `eng=rw:ec vs ch=ro:ec`，按机制分六摊逐一收口，**shape audit 936/936 全同、kind 0**——原型面（名、kind、序、statics）与 Chrome 145 逐位全等。

六摊与手法：
1. **`_evxRo(C, keys)`**（bootstrap.js，`_evxProps` 旁新 getter-only 变体）：9 事件类 33 成员——TouchEvent 7（pointer_touch.js:100）、ErrorEvent 5、MessageEvent 5、AnimationEvent/TransitionEvent/CloseEvent/ProgressEvent 各 3、ToggleEvent 2、PromiseRejectionEvent 2（**同文两处定义**，10276 与 10472 都换，后者胜出但两处一致才不埋雷）。构造走 `_evx.set(this,{...})` 直写槽、不经访问器，天然安全。
2. **`__accRo(C, names)`**（`__acc` 旁新变体，同 `_x` 隐字段、同进 `__accPrivs` 隐藏账）：WebSocket 5（url/readyState/bufferedAmount/extensions/protocol；binaryType+on* 仍 rw）、EventSource 3、FileReader 3、BroadcastChannel 1。类体内状态写全部 `this.x = v` → `__def(this,'_x',v)`（旧 setter 的落点，行为等价）：FileReader 构造/_read/异步完成/abort、EventSource 构造/微任务 open/close、WebSocket 构造 5 写/_fail/_finish（`readyState=3` 同文两处）/pump open/send 缓冲/close、BC 构造 name。
3. **demote_own.js 换单道**：局部 slot WeakMap → 全局 `__diting_liftSlot` 道 + RO 覆盖表（PerformanceTiming=null 全封 16、SpeechSynthesis pending/speaking/paused、Crypto.subtle、ServiceWorkerContainer.controller）——引擎后写（audio.js 语音队列 7 处 `this.pending/speaking/paused` → `__diting_liftSlot(this)`）可达 RO 成员存储。
4. **_interfaceProps 复制去 set 腿**：HTMLDataListElement.options、HTMLSelectElement.options、HTMLTableElement.rows、HTMLTableSectionElement.rows、**HTMLTemplateElement.content（新入表**——content 的 Element 访问器 set 腿只认 meta，template 本就 no-op；入表后 htmlreflect.gen 的 `delegate()` 因 `has()` 跳过，不会装回双腿版）。
5. **NodeList.length**：删 set；工厂 `_nodeList` 写 `nl._len = els.length`（ctor 的 defineProperty 仍造非枚举 `_len`）。
6. **StyleSheet 基类**（cssom_sheets.js，**该文件带 5 个原始 NUL 字节（缓存键字面量）——grep/Read/Edit 全当二进制拒收，只能 python 读写**）：href/ownerNode/parentStyleSheet/title/type 去 set（media/disabled Chrome 本 rw 留双腿）；`_sheetFromCssText` 5 写改直写 `_href` 等字段（hide_priv 静态表已登记这些名）。

**教训**：①bootstrap.js 是 `// @@SCOPE` IIFE + 尾部 `d('_name', fn)` 显式导出账——新助手**必须在 @@EXPORTS 块补一行**，否则后文件加载期 ReferenceError 直接崩快照（`_evxRo is not defined`）；②新跨文件加载期名（pointer_touch 用 `_evxRo`）进 ratchet 记账：`--write-baseline` 有意升（65→66），violation 必须 0；③k84 的「另一侧 24 类」= 机制账而不是漏网——每类先追访问器来源（_evxProps/__acc/demote/_interfaceProps/gen delegate/类体），来源决定封法与写点改道，不追来源直接封必炸类体写。
数字：shape **same 912→936，kind 24→0**（order/statics/len/proto/svals 全 0；symdesc 18 与 k84 逐名相同=历史存账）；enum 212/204/8（8 键与 k84 逐名全等，含已知 ls IDB 抖动）；instance 127/127；ratchet 66/violations 0；cargo test **955 通过 + 110 基线逐名全等，零新增零修复**。
测试：`knife85_readonly_state_faces`（六摊各验：事件字典构造值可读+双写 no-op；状态类构造默认与引擎转移仍活——ws.close()→3、es.close()→2、style.sheet.type/ownerNode；demote 四单例 getter-only 且值可读；反射五+NodeList.length+StyleSheet.href 描述符 [get,set]=[1,0]；strict 写五族全 TypeError）。

## M4 刀86：IndexedDB 跨连接 open 协议（2026-10-31）

Chrome 145 真值（k86-idb 七案 + k86-idb2 事件类）钉死的跨连接协议：升版 `open`（或 `deleteDatabase`）遇已有活连接时——每个活连接先收 `versionchange`（IDBVersionChangeEvent，oldVersion/newVersion）；处理器跑完后仍有人开着 → 请求收 **`blocked`（也是 IDBVersionChangeEvent，带版本对，readyState 仍 'pending'）**并挂起；最后一个 `close()` 放行 → `upgradeneeded`（**此刻 readyState 已 'done'**）→ success。处理器内当场 close → 不发 blocked 直升；`deleteDatabase` 的 success 也是 IDBVersionChangeEvent `{oldVersion: 老, newVersion: null}`；VersionError 先于连接检查（无 vc/blocked）；同版 open 无协议；多连接任一存活即 blocked。

实现（indexeddb.js）：连接记账上 record（`rec.conns` 惰性数组，元素含 `db` 自引用+`closed` 旗）；三助手 `_idbLiveConns/_idbPark/_idbDrain`；`close()` 置 closed+drain；open 的升级体裹成 `startUpgrade` 闭包，`succeed()` 才登记连接（成功一刻起活）；VersionError 路径自带 `rs.done=true`；升级 abort 的 onDone 补 drain（排队者可续）。**同版 open 不进协议**（Chrome 案 F：tv>current 才有 vc/blocked）。
数字：k86-idb 21 条全 SAME、k86-idb2 6 条全 SAME；shape 936/936 全 0；enum 212/204/8 与 k85 逐名全等；instance 127/127；ratchet 66/0；cargo test **110 基线逐名全等零新增**（另有环境摊：root crate `knobs_report_geometry` 布局案 + 45 条 PoisonError 连坐——stash 实证在刀85 提交 ded368a 上同样失败，非本刀所致）。
测试：`knife86_idb_cross_connection_open_protocol`（四段：blocker 不关→vc[1,2]+blocked[instanceof,1,2,'pending']+still-blocked 标记+close 后 upg/ok；vc 内 close→无 blocked 直升；deleteDatabase→vc[1,null]+blocked+del[1,null,instanceof]；同版 open 静默 success）。
**教训**：①probe_run.py 的 engine 腿把探针嵌进所服页面的 `<script>`——导航即先跑一遍、/eval 再跑一遍，**有状态探针（IDB 跨连接）自己和自己赛跑**（两遍执行同库竞态→VersionError 拒绝→/eval 回 null）；单执行捕获走 blank 页 + 裸 /eval（capture_engine_once.py），Chrome 腿本就单执行（读 #o）不受影响；②engine 的 localStorage 跨页跨源共享（IDB 元数据全局可见）——探针开头用 `indexedDB.databases()` 清同名残留，两腿通用且幂等；③enum 探针是**页面敏感**的（htmlColl/nodeList/sheets 数页面节点）——blank 页捕获会假报少键，页面敏感探针必须用原 probe_run 页面；④测试跑前先杀 18199 探针伺服——常驻伺服占共享 store，root crate 的 session/server 测试整族连坐 PoisonError，别误判成回归。

## M4 刀87：构造器参数梯（argc/类型/字典读序/序列/range 全消息位）（2026-10-31）

Chrome 145 真值（k87-ctor-e 57 案 + k87-ctor-f 46 案 + k87-fp 8 案）钉死的九族构造器验证梯，引擎原先整族缺失或消息错位：

- **CSSMath\***（生成件 mkCssMath）：变长四族（Max/Min/Product/Sum）零参 = **SyntaxError「Arguments can't be empty」**；定元族（Invert/Negate=1、Clamp=3）N-present TypeError 插值计数；每参 CSSNumberish 双 coercion，非有限 = TypeError「The provided double value is non-finite.」；成功铸 Object.create(C.prototype) 实例。CSSMathRound/Abs/Sign/Pow/Log/Exp/Hypot 145 不存在（表里本来就没有）。
- **IIRFilterNode**（audio.js 真身，length 2）：argc → ctx instanceof BaseAudioContext → 字典读序 **feedback 先于 feedforward**（两者都缺也只报 feedback）→ 序列转换（WebIDL 首步要求 Object——**原始值含可迭代字符串 'x' 一律「cannot be converted to a sequence」**）→ 元素非有限 → fb[0]===0 / ff 全零两条 InvalidStateError。getFrequencyResponse 落**原型**（length 3, ecw，constructor 居尾——P4 惯例 delete+re-define 挪尾）。
- **BiquadFilterNode**（audio.js 真身，length 1）：argc → ctx 检查 → 成功；工厂 createBiquadFilter/createIIRFilter 改走真构造器。
- **OfflineAudioContext**：0 参「1 argument required, but only 0 present.」；2 参「Overload resolution failed.」；1 参字典读序 **length 先于 sampleRate**（required member 逐名报）；ch [1,32] / frames ≥1 / sampleRate [3000,768000] NotSupportedError 带插值值。
- **ReadableStreamDefaultReader**（bootstrap.js）：argc → 「parameter 1 is not of type 'ReadableStream'.」→ locked 专属文案 → 成功登记 `_rsStream`+`_locked`。
- **RTCEncodedAudioFrame**（webrtc.js）：argc → 只认同类实例（「parameter 1 is not of type 'RTCEncodedAudioFrame'.」），从同类复制 _timestamp/_data。
- **URL**（bootstrap.js）：argc → **base 先单独验**（base 不可解析 = 「Invalid base URL」，首参是绝对 URL 也照样）→ 组合解析失败 = 「Invalid URL」。
- **WebSocket**（bootstrap.js）：0 参 TypeError（非 DOMException）；**http/https 转 ws/wss**（WHATWG 记录换scheme 后的 URL）；scheme 白名单四选文案「must be either 'http', 'https', 'ws', or 'wss'. 'X' is not allowed.」；子协议 token 校验（`^[!#$%&'*+\-.^_`|~0-9A-Za-z]+$`，'{}'→'[object Object]' invalid）先于重复检查。
- **StaticRange**（range_selection.js）：argc → 字典读序 **endContainer 先于 startContainer**（四案一致）→ required/convert 双文案 → Attr(2)+DocumentType(10) 端点 → InvalidNodeTypeError 新文案。

数字：k87-ctor-e 57/57 SAME、k87-fp 8/8 SAME、k87-ctor-f 42/46 SAME（余 4 案单根因 = 引擎全缺 **CSS.px/CSS.em 单位工厂**——`CSS.px(1)` 抛 is not a function，梯子根本没跑到；typed-om 单位族整族缺，另刀挂账）；shape 936/936 全 0（IIR 原型 getFrequencyResponse+constructor 挪尾后归零）；enum 204/211 与 k86 逐行全等；instance 127/127；ratchet 66/0 违约 0；cargo test **110 基线逐名全等零新增**（46 环境摊同刀86）。
测试：`knife87_ctor_validation_ladders`（37 行日志：CSSMath 五案+lens、IIR 六案、Biquad 三案、OAC 四案、RSR/REAF/URL/WS/SR 各三至四案，消息逐字断言）。
**教训**：①生成件数据位错——CSSMATH 表变长族 min 填 0，`arguments.length < 0` 永假→零参不抛；生成器里 min 语义 = 「最少几个参数才算非空」，变长族空参案要 min=1 才进 SyntaxError 支；②WebIDL 序列转换首步「Type(V) is not Object」在 Chrome 145 真跑——可迭代字符串也拒，先对象后迭代器；③shape 真源把接口操作记在**原型**（kinds+order），真构造器把操作塞实例成员会报 missing——照 truth 的 order 落原型并把 constructor 挪尾；④Rust 侧测试文档是 `<html><body></body></html>` 裸页，**没有 doctype**——DocumentType 端点案要用 `document.implementation.createDocumentType` 现造。

## M4 刀88：typed-om 单位族——CSS 命名空间 69 键 + CSSUnitValue 真身（2026-10-31）

Chrome 145 真值（k88-typedom 44 案 + k88-mini 49 案）钉死：CSS 命名空间共 **69 键** = 64 单位工厂 + escape/supports/registerProperty/highlights/paintWorklet；**工厂名小写=规范单位串**（Hz→hz、kHz→khz、Q→q）；工厂参数是纯 double coercion（`CSS.px(CSS.px(3))` 也抛 non-finite——145 的工厂**没有** CSSNumericValue 旁路，只有 CSSMath* 构造器收数值对象）；CSSUnitValue 真身：2 参梯 + 「Invalid unit: X」+ value **rw**/unit **ro**（原型访问器，strict 写 unit 白得 V8 getter-only 消息）+ 实例零 own 键 + instanceof 链穿 CSSNumericValue/CSSStyleValue；序列化骑在 CSSStyleValue.prototype.toString（number 裸值、percent 加 %、其余 value+unit）；registerProperty **先读 inherits 后读 name**（{} 报 inherits），属性读式消息形；CSS.highlights=空 HighlightRegistry（size 0）、CSS.paintWorklet=裸 Worklet（零 own 键）。

实现：bootstrap.js（CSSUnitValue 真身+64 工厂循环，早于 global_surface → SKIP 顶掉 THROW 桩，ET_CHAIN 现有行把我的原型链到 CSSNumericValue 桩上）；web_api_gaps.js（两对象脸 Object.create 桩原型 + toString 接管换同名槽保序 + HighlightRegistry.size getter 装在 stub_members 前）；css_register.js（读序换 inherits 先+消息换 Chrome 145 形——该模块本就拥有 registerProperty，bootstrap 不重复造）。
数字：k88-mini 49/49 SAME、k88-typedom 23/44 SAME（余 21 案=CSSNumericValue 算术 add/sub/mul/div/to/equals/parse + CSSMath 实例成员 value/values/operator + calc 序列化 = **刀89**）；shape 936/936 全 0；enum 204/211 与 k86/k87 逐行全等；instance 127/127；ratchet 66/0；cargo test 110 基线逐名全等零新增（46 环境摊同前）。
测试：`knife88_css_unit_family`（35 行：键数/工厂梯/大小写回程/ctor 梯/rw-ro/own 键/instanceof 三级/五序列化/两对象脸/registerProperty 四案）。
**教训**：①动手前先搜存量——registerProperty 已有整模块 css_register.js（注册表+语法验证+Rust 级联镜像），bootstrap 里再造一份只会被它覆盖（引擎腿抓到的旧消息就是这么漏的）；②真身类想链到「global_surface 桩的原型」上不必等桩——**早装载让 SKIP 顶掉桩 + ET_CHAIN 现有行动态解析 globalThis 自动接链**（Biquad 同法）；③Chrome 的字典读序不按 IDL 声明序（registerProperty 是 inherits 先）——每个家族都得实测，别从规范推。
