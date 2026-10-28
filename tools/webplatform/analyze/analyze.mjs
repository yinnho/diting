#!/usr/bin/env node
// Static dependency analysis of the snapshot scripts (JS-PLATFORM-ARCH M1).
// READ-ONLY: it never edits a source file. It answers, with data:
//   - what each script defines at top level (and what it writes onto globalThis)
//   - which other scripts' bindings it needs at LOAD time (top-level execution)
//     vs only at CALL time (inside function bodies)
//   - load-order violations: a load-time reference to something defined by a
//     LATER script (or later in the same script, for let/const/class = TDZ)
//   - hide-list leak candidates: top-level `_`/diting function/var declarations in
//     scripts that load AFTER interface_sweep.js (its final refresh re-scans the list)
//
// usage: node analyze.mjs [--json out.json] [--md out.md] [--check baseline.json]
//        node analyze.mjs --write-baseline baseline.json
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import * as acorn from 'acorn';
import * as eslintScope from 'eslint-scope';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '..', '..', '..');
const JS_DIR = path.join(ROOT, 'crates/diting/js');

// ---- load order, straight from build.rs (the single source today) ----------
function loadOrder() {
  const rs = fs.readFileSync(path.join(ROOT, 'crates/diting/build.rs'), 'utf8');
  const re = /\("([A-Za-z0-9_.]+\.js)",\s*include_str!\("js\/([A-Za-z0-9_.]+)"\)\)/g;
  const out = [];
  for (let m; (m = re.exec(rs));) out.push(m[2]);
  return out;
}

// Host-provided names that are not defined by any script: JS builtins plus
// what the Rust runtime / PRELUDE injects. Anything else with no provider is
// reported as "external" so a human can classify it.
const NODE_GLOBALS = new Set(Object.getOwnPropertyNames(globalThis));
const HOST_NAMES = new Set(['Deno', 'globalThis', 'window', 'self', 'undefined', 'NaN', 'Infinity', 'arguments']);
const isBuiltin = (n) => NODE_GLOBALS.has(n) || HOST_NAMES.has(n);

// Synchronous callback receivers: a function literal passed to one of these
// runs during the call, so its body is still LOAD time when the call is.
const SYNC_CALLEES = new Set([
  'forEach', 'map', 'filter', 'some', 'every', 'reduce', 'reduceRight', 'find', 'findIndex',
  'flatMap', 'sort', 'from', 'call', 'apply', 'bind',
]);

function parentMap(ast) {
  const parents = new Map();
  (function walk(node, parent) {
    if (!node || typeof node.type !== 'string') return;
    parents.set(node, parent);
    for (const k of Object.keys(node)) {
      if (k === 'loc' || k === 'start' || k === 'end') continue;
      const v = node[k];
      if (Array.isArray(v)) v.forEach((c) => c && typeof c.type === 'string' && walk(c, node));
      else if (v && typeof v.type === 'string') walk(v, node);
    }
  })(ast, null);
  return parents;
}

// Is this function node executed during the enclosing load-time pass?
function runsAtLoad(fn, parents) {
  const p = parents.get(fn);
  if (!p) return false;
  // IIFE: (function(){})() / (() => {})()
  if (p.type === 'CallExpression' && p.callee === fn) return true;
  if (p.type === 'NewExpression' && p.callee === fn) return true;
  // (function(){}).call(...)
  if (p.type === 'MemberExpression' && p.object === fn) {
    const pp = parents.get(p);
    if (pp && pp.type === 'CallExpression' && pp.callee === p) return true;
  }
  // arr.forEach(function(){}) — synchronous callee
  if (p.type === 'CallExpression' && p.arguments.includes(fn)) {
    const c = p.callee;
    const name = c.type === 'MemberExpression' && !c.computed ? c.property.name
      : c.type === 'Identifier' ? c.name : null;
    if (name && SYNC_CALLEES.has(name)) return true;
  }
  return false;
}

// Walk a reference's scope chain up to the global scope: LOAD time iff every
// function boundary on the way is itself run at load.
function referenceIsLoadTime(ref, parents) {
  for (let s = ref.from; s; s = s.upper) {
    if (s.type === 'global' || s.type === 'module') return true;
    if (s.type === 'function') {
      if (!runsAtLoad(s.block, parents)) return false;
    } else if (s.type === 'class-field-initializer') {
      const prop = parents.get(s.block);
      if (!(prop && prop.static)) return false;
    }
    // block / catch / class / with / for / switch / class-static-block: transparent
  }
  return true;
}

// `globalThis.X = …`, `window.X = …`, `self.X = …`, Object.defineProperty(globalThis,'X',…)
// executed at load time ⇒ a global-property provider.
const GLOBAL_OBJS = new Set(['globalThis', 'window', 'self']);
function collectGlobalWrites(ast, parents) {
  const writes = []; // {name, line, load}
  const loadCtx = (node) => {
    for (let n = parents.get(node); n; n = parents.get(n)) {
      if (n.type === 'FunctionDeclaration' || n.type === 'FunctionExpression' || n.type === 'ArrowFunctionExpression') {
        if (!runsAtLoad(n, parents)) return false;
      }
    }
    return true;
  };
  (function walk(node) {
    if (!node || typeof node.type !== 'string') return;
    if (node.type === 'AssignmentExpression' && node.left.type === 'MemberExpression'
        && node.left.object.type === 'Identifier' && GLOBAL_OBJS.has(node.left.object.name)) {
      const k = node.left.computed ? (node.left.property.type === 'Literal' ? String(node.left.property.value) : null) : node.left.property.name;
      if (k) writes.push({ name: k, line: node.loc.start.line, load: loadCtx(node) });
    }
    if (node.type === 'CallExpression' && node.callee.type === 'MemberExpression'
        && node.callee.object.type === 'Identifier' && node.callee.object.name === 'Object'
        && node.callee.property.name === 'defineProperty'
        && node.arguments[0] && node.arguments[0].type === 'Identifier' && GLOBAL_OBJS.has(node.arguments[0].name)
        && node.arguments[1] && node.arguments[1].type === 'Literal') {
      writes.push({ name: String(node.arguments[1].value), line: node.loc.start.line, load: loadCtx(node) });
    }
    for (const k of Object.keys(node)) {
      if (k === 'loc') continue;
      const v = node[k];
      if (Array.isArray(v)) v.forEach(walk); else if (v && typeof v.type === 'string') walk(v);
    }
  })(ast);
  return writes;
}

// Cross-file prototype mutation: `X.prototype.m = …`, `X.prototype = …`,
// Object.defineProperty(X.prototype, …), Object.setPrototypeOf(X.prototype, …).
// The free-variable graph cannot see this coupling (the shared object is the
// channel), so it is measured separately.
function collectProtoMutations(ast) {
  const out = []; // {target, line}
  const protoTarget = (node) => {
    // node is `X.prototype` => X ; also `X.prototype.y` base
    if (node && node.type === 'MemberExpression' && !node.computed && node.property.name === 'prototype'
        && node.object.type === 'Identifier') return node.object.name;
    return null;
  };
  (function walk(node) {
    if (!node || typeof node.type !== 'string') return;
    if (node.type === 'AssignmentExpression' && node.left.type === 'MemberExpression') {
      const t = protoTarget(node.left) || protoTarget(node.left.object);
      if (t) out.push({ target: t, line: node.loc.start.line });
    }
    if (node.type === 'CallExpression' && node.callee.type === 'MemberExpression'
        && node.callee.object.type === 'Identifier' && node.callee.object.name === 'Object'
        && ['defineProperty', 'defineProperties', 'setPrototypeOf'].includes(node.callee.property.name)) {
      const t = protoTarget(node.arguments[0]);
      if (t) out.push({ target: t, line: node.loc.start.line });
    }
    for (const k of Object.keys(node)) {
      if (k === 'loc') continue;
      const v = node[k];
      if (Array.isArray(v)) v.forEach(walk); else if (v && typeof v.type === 'string') walk(v);
    }
  })(ast);
  return out;
}

// ---- main analysis ---------------------------------------------------------
const order = loadOrder();
const files = {};
for (const f of order) {
  const src = fs.readFileSync(path.join(JS_DIR, f), 'utf8');
  const ast = acorn.parse(src, { ecmaVersion: 'latest', sourceType: 'script', locations: true, ranges: true, allowHashBang: true });
  const parents = parentMap(ast);
  const sm = eslintScope.analyze(ast, { ecmaVersion: 2024, sourceType: 'script', ignoreEval: true, childVisitorKeys: null });
  const g = sm.globalScope;
  const decl = []; // top-level declarations
  for (const v of g.variables) {
    if (!v.defs.length) continue;
    const d = v.defs[0];
    const kind = d.type === 'FunctionName' ? 'function' : d.type === 'ClassName' ? 'class'
      : d.parent && d.parent.kind ? d.parent.kind : 'var';
    decl.push({ name: v.name, kind, line: d.name.loc.start.line });
  }
  const implicit = (g.implicit?.variables || []).map((v) => ({ name: v.name, line: v.identifiers[0].loc.start.line }));
  const writes = collectGlobalWrites(ast, parents);
  const refs = []; // free references
  for (const r of g.through) {
    refs.push({ name: r.identifier.name, line: r.identifier.loc.start.line, load: referenceIsLoadTime(r, parents), write: !!r.isWrite() });
  }
  // same-file TDZ: load-time ref to let/const/class declared LATER in this file
  const tdz = [];
  for (const v of g.variables) {
    const d = v.defs[0];
    if (!d) continue;
    const kind = d.type === 'ClassName' ? 'class' : d.parent && d.parent.kind;
    if (kind !== 'let' && kind !== 'const' && kind !== 'class') continue;
    for (const r of v.references) {
      if (r.init) continue;
      if (referenceIsLoadTime(r, parents) && r.identifier.start < d.name.start) {
        tdz.push({ name: v.name, useLine: r.identifier.loc.start.line, declLine: d.name.loc.start.line });
      }
    }
  }
  files[f] = { lines: src.split('\n').length, decl, implicit, writes, refs, tdz, protoMut: collectProtoMutations(ast) };
}

// provider index: name -> [{file, idx, how, load}]
const providers = new Map();
const add = (name, file, idx, how, extra = {}) => {
  if (!providers.has(name)) providers.set(name, []);
  providers.get(name).push({ file, idx, how, ...extra });
};
order.forEach((f, idx) => {
  const F = files[f];
  F.decl.forEach((d) => add(d.name, f, idx, d.kind, { line: d.line }));
  F.implicit.forEach((d) => add(d.name, f, idx, 'implicit-global', { line: d.line }));
  F.writes.filter((w) => w.load).forEach((w) => add(w.name, f, idx, 'global-prop', { line: w.line }));
});
// Names assigned onto globalThis only inside functions (call time): late providers
const lateWriters = new Map();
order.forEach((f, idx) => files[f].writes.filter((w) => !w.load).forEach((w) => {
  if (!lateWriters.has(w.name)) lateWriters.set(w.name, new Set());
  lateWriters.get(w.name).add(f);
}));

const crossProto = []; // {file, target, provider, count}
for (const f of order) {
  const idx0 = order.indexOf(f);
  const by = new Map();
  for (const m of files[f].protoMut) {
    const provs = (providers.get(m.target) || []).filter((p) => p.file !== f && p.idx < idx0);
    if (!provs.length) continue;
    const key = m.target + '|' + provs[provs.length - 1].file;
    by.set(key, (by.get(key) || 0) + 1);
  }
  for (const [k, c] of by) { const [t, pf] = k.split('|'); crossProto.push({ file: f, target: t, provider: pf, count: c }); }
}
const report = { crossProto, order, files: {}, totals: {}, violations: [], external: {}, hubs: {}, leaks: [] };
const fileIdx = Object.fromEntries(order.map((f, i) => [f, i]));
const hubCount = new Map();
let loadEdges = 0, callEdges = 0;

for (const f of order) {
  const F = files[f];
  const idx = fileIdx[f];
  const loadReq = {}, callReq = {}; // provider file -> Set(names)
  const external = new Set();
  for (const r of F.refs) {
    if (isBuiltin(r.name)) continue;
    const provs = providers.get(r.name) || [];
    const earlier = provs.filter((p) => p.idx < idx || (p.idx === idx));
    const cross = provs.filter((p) => p.idx !== idx);
    if (!provs.length) {
      if (!lateWriters.has(r.name)) external.add(r.name);
      continue;
    }
    const src = earlier.filter((p) => p.idx < idx).pop() || null; // nearest earlier provider
    if (src) {
      const bucket = r.load ? loadReq : callReq;
      (bucket[src.file] ||= new Set()).add(r.name);
      hubCount.set(r.name, (hubCount.get(r.name) || 0) + 1);
    } else if (cross.length && !earlier.length) {
      // defined only by a LATER script
      if (r.load && !r.write) {
        report.violations.push({ kind: 'forward-load-ref', file: f, line: r.line, name: r.name, provider: cross[0].file });
      }
    }
  }
  for (const t of F.tdz) report.violations.push({ kind: 'same-file-tdz', file: f, line: t.useLine, name: t.name, declLine: t.declLine });
  const flat = (o) => Object.fromEntries(Object.entries(o).map(([k, s]) => [k, [...s].sort()]));
  const lr = flat(loadReq), cr = flat(callReq);
  loadEdges += Object.values(lr).reduce((a, b) => a + b.length, 0);
  callEdges += Object.values(cr).reduce((a, b) => a + b.length, 0);
  // hide-list leak candidates: declarations that mint a window property AFTER bootstrap
  // interface_sweep.js ends with a "final refresh" that re-scans `_`/diting names into
  // __diting_hide_list, so only scripts that load AFTER it can leak.
  const leaks = idx <= (fileIdx['interface_sweep.js'] ?? Infinity) ? [] : F.decl.filter((d) => (d.kind === 'function' || d.kind === 'var')
    && (d.name.startsWith('_') || /diting/i.test(d.name)));
  leaks.forEach((l) => report.leaks.push({ file: f, name: l.name, line: l.line, kind: l.kind }));
  report.files[f] = {
    lines: F.lines,
    defines: { total: F.decl.length, byKind: F.decl.reduce((a, d) => (a[d.kind] = (a[d.kind] || 0) + 1, a), {}),
               globalPropWrites: F.writes.filter((w) => w.load).length, lateGlobalWrites: F.writes.filter((w) => !w.load).length,
               implicitGlobals: F.implicit.length },
    loadRequires: lr, callRequires: cr,
    external: [...external].sort(),
  };
  external.forEach((n) => { (report.external[n] ||= []).push(f); });
}

report.hubs = Object.fromEntries([...hubCount.entries()].sort((a, b) => b[1] - a[1]).slice(0, 25));
report.totals = {
  files: order.length,
  lines: order.reduce((a, f) => a + files[f].lines, 0),
  bootstrapLines: files['bootstrap.js']?.lines,
  topLevelBindings: order.reduce((a, f) => a + files[f].decl.length, 0),
  crossFileLoadTimeNames: loadEdges,
  crossFileCallTimeNames: callEdges,
  filesWithNoCrossFileLoadReq: order.filter((f) => !Object.keys(report.files[f].loadRequires).length).length,
  forwardLoadRefs: report.violations.filter((v) => v.kind === 'forward-load-ref').length,
  sameFileTdz: report.violations.filter((v) => v.kind === 'same-file-tdz').length,
  hideListLeakCandidates: report.leaks.length,
  externalNames: Object.keys(report.external).length,
  crossFileProtoMutations: crossProto.reduce((a, c) => a + c.count, 0),
  crossFileProtoMutationTargets: crossProto.length,
};

// ---- outputs ---------------------------------------------------------------
const arg = (k) => { const i = process.argv.indexOf(k); return i > 0 ? process.argv[i + 1] : null; };
function markdown() {
  const T = report.totals;
  const L = [];
  L.push('# JS 平台层依赖基线（生成件 · `tools/webplatform/analyze`）', '');
  L.push('> 静态分析：acorn + eslint-scope。**近似**：加载期 = 顶层执行路径（含 IIFE 与同步回调 forEach/map 等）；函数体内引用记为调用期。动态写入（`globalThis[expr] =`）、`eval`、PRELUDE 注入名分析不到，归入 external。', '');
  L.push('## 总览', '', '| 指标 | 值 |', '|---|---|');
  L.push(`| 快照脚本数 | ${T.files} |`, `| 总行数 | ${T.lines} |`, `| bootstrap.js 行数 | ${T.bootstrapLines} |`);
  L.push(`| 顶层绑定数 | ${T.topLevelBindings} |`, `| 跨文件·加载期依赖（文件×名字） | ${T.crossFileLoadTimeNames} |`);
  L.push(`| 跨文件·调用期依赖（文件×名字） | ${T.crossFileCallTimeNames} |`, `| 无任何跨文件加载期依赖的文件 | ${T.filesWithNoCrossFileLoadReq} / ${T.files} |`);
  L.push(`| **加载序违规**（加载期引用较晚文件的绑定） | ${T.forwardLoadRefs} |`, `| **同文件 TDZ** | ${T.sameFileTdz} |`);
  L.push(`| hide list 泄漏候选（bootstrap 之后的 \`_\`/diting 函数声明或 var） | ${T.hideListLeakCandidates} |`, `| 无提供者的外部名 | ${T.externalNames} |`, `| **跨文件原型改写**（点数 / 文件×目标对） | ${T.crossFileProtoMutations} / ${T.crossFileProtoMutationTargets} |`, '');
  L.push('## 枢纽绑定（被最多引用点依赖）', '', '| 名字 | 引用点数 |', '|---|---|');
  for (const [n, c] of Object.entries(report.hubs)) L.push(`| \`${n}\` | ${c} |`);
  L.push('', '## 每文件', '', '| 文件 | 行 | 顶层绑定 | 全局属性写 | 加载期依赖（来源文件:名字数） | 调用期依赖文件数 |', '|---|---|---|---|---|---|');
  for (const f of order) {
    const r = report.files[f];
    const lr = Object.entries(r.loadRequires).map(([k, v]) => `${k}:${v.length}`).join(' ') || '—';
    L.push(`| ${f} | ${r.lines} | ${r.defines.total} | ${r.defines.globalPropWrites} | ${lr} | ${Object.keys(r.callRequires).length} |`);
  }
  if (report.violations.length) {
    L.push('', '## 违规明细', '', '| 类型 | 文件:行 | 名字 | 说明 |', '|---|---|---|---|');
    for (const v of report.violations) {
      L.push(`| ${v.kind} | ${v.file}:${v.line} | \`${v.name}\` | ${v.kind === 'forward-load-ref' ? '提供者在较晚文件 ' + v.provider : '声明在第 ' + v.declLine + ' 行'} |`);
    }
  }
  if (report.leaks.length) {
    L.push('', '## hide list 泄漏候选', '', '| 文件:行 | 名字 | 种类 |', '|---|---|---|');
    for (const l of report.leaks) L.push(`| ${l.file}:${l.line} | \`${l.name}\` | ${l.kind} |`);
  }
  if (report.crossProto.length) {
    L.push('', '## 跨文件原型改写（自由变量图看不见的隐式耦合）', '', '| 文件 | 目标（定义于） | 改写点数 |', '|---|---|---|');
    for (const c of [...report.crossProto].sort((a, b) => b.count - a.count)) L.push(`| ${c.file} | \`${c.target}.prototype\`（${c.provider}） | ${c.count} |`);
  }
  const ext = Object.entries(report.external).sort((a, b) => b[1].length - a[1].length);
  L.push('', `## 无提供者的外部名（${ext.length}）`, '', '主机/PRELUDE 注入或动态定义，需要人工归类：', '');
  L.push(ext.map(([n, fs_]) => `\`${n}\`(${fs_.length})`).join('、'), '');
  return L.join('\n');
}

if (arg('--json')) fs.writeFileSync(arg('--json'), JSON.stringify(report, null, 1));
if (arg('--md')) fs.writeFileSync(arg('--md'), markdown());
if (arg('--write-baseline')) {
  fs.writeFileSync(arg('--write-baseline'), JSON.stringify({
    note: 'ratchet: these totals may only go DOWN (violations/leaks must stay at 0 or below). Regenerate with --write-baseline after a deliberate improvement.',
    forwardLoadRefs: report.totals.forwardLoadRefs, sameFileTdz: report.totals.sameFileTdz,
    hideListLeakCandidates: report.totals.hideListLeakCandidates,
    crossFileLoadTimeNames: report.totals.crossFileLoadTimeNames,
    crossFileProtoMutations: report.totals.crossFileProtoMutations,
  }, null, 1) + '\n');
}
if (arg('--check')) {
  const base = JSON.parse(fs.readFileSync(arg('--check'), 'utf8'));
  const bad = [];
  for (const k of ['forwardLoadRefs', 'sameFileTdz', 'hideListLeakCandidates', 'crossFileLoadTimeNames', 'crossFileProtoMutations']) {
    if (report.totals[k] > base[k]) bad.push(`${k}: ${report.totals[k]} > baseline ${base[k]}`);
  }
  if (bad.length) { console.error('FAIL dependency ratchet:\n  ' + bad.join('\n  ')); report.violations.slice(0, 20).forEach((v) => console.error('  ', JSON.stringify(v))); process.exit(1); }
  console.log('dependency ratchet OK', JSON.stringify(report.totals));
} else if (!arg('--json') && !arg('--md') && !arg('--write-baseline')) {
  console.log(JSON.stringify(report.totals, null, 1));
}
