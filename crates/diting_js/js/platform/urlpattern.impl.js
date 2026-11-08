//! module: urlpattern
//! layer: L2
//! exposes: URLPattern
//! requires-load: -
//! requires-call: legacy:URL

// URL Pattern (M4 knife 12): the WHATWG URL Pattern standard - tokenizer,
// pattern-string parser, per-component canonicalization, regexp generation,
// pattern-string generation, constructor-string parser and matching - written
// from the standard's algorithms and checked against real Chrome 145 on a
// 900-key corpus (chrome/urlpattern-truth-145.json, urlpattern-probe.js):
// constructions of every shape, exact liburlpattern-style error wording, and a
// 741-case pattern x input match matrix. The old bootstrap class was an inert
// stub (test() always false, exec() always null).

const COMPONENTS = ['protocol', 'username', 'password', 'hostname', 'port', 'pathname', 'search', 'hash'];
const SPECIAL = { http: '80', https: '443', ws: '80', wss: '443', ftp: '21', file: '' };
const slots = new WeakMap();

// ---- helpers
function isObj(v) { return (typeof v === 'object' && v !== null) || typeof v === 'function'; }
function escapeRegexp(s) { return s.replace(/[.+*?^${}()[\]|/\\]/g, '\\$&'); }
function escapePattern(s) { return s.replace(/[+*?:{}()\\]/g, '\\$&'); }
function isNumericName(n) { return n.length > 0 && n.charCodeAt(0) >= 48 && n.charCodeAt(0) <= 57; }
const ID_START = /[\p{ID_Start}$_]/u;
const ID_CONT = /[\p{ID_Continue}$‌‍]/u;
function validNameCp(cp, first) { return (first ? ID_START : ID_CONT).test(cp); }

// ---- tokenizer (strict: errors throw; lenient: invalid chars become tokens)
class PatternError extends Error {}
function perr(msg) { return new PatternError(msg); }
function tokenize(input, strict) {
  const cps = Array.from(input);
  // byte-ish index of each code point in the ORIGINAL string (UTF-16 units)
  const pos = []; let u = 0;
  for (const c of cps) { pos.push(u); u += c.length; }
  pos.push(u);
  const tokens = [];
  const add = (type, value, i) => tokens.push({ type, value, index: pos[i] });
  let i = 0;
  const bad = (msg, at, lenientIdx) => {
    if (strict) throw perr(msg);
    add('invalid-char', cps[lenientIdx] === undefined ? '' : cps[lenientIdx], lenientIdx);
  };
  while (i < cps.length) {
    const c = cps[i];
    if (c === '*') { add('asterisk', c, i); i++; continue; }
    if (c === '+' || c === '?') { add('modifier', c, i); i++; continue; }
    if (c === '\\') {
      if (i === cps.length - 1) { bad('Trailing escape character at index ' + pos[i] + '.', i, i); i++; continue; }
      add('escaped-char', cps[i + 1], i); i += 2; continue;
    }
    if (c === '{') { add('open', c, i); i++; continue; }
    if (c === '}') { add('close', c, i); i++; continue; }
    if (c === ':') {
      let j = i + 1;
      while (j < cps.length && validNameCp(cps[j], j === i + 1)) j++;
      if (j <= i + 1) { bad('Missing parameter name at index ' + pos[i] + '.', i, i); i++; continue; }
      add('name', cps.slice(i + 1, j).join(''), i); i = j; continue;
    }
    if (c === '(') {
      let depth = 1, j = i + 1, err = null;
      const start = j;
      while (j < cps.length) {
        const d = cps[j];
        if (d.codePointAt(0) > 127) { err = 'Invalid non-ASCII character 0x' + d.codePointAt(0).toString(16) + ' at index ' + pos[j] + '.'; break; }
        if (j === start && d === '?') { err = "Regex cannot start with '?' at index " + pos[j]; break; }
        if (d === '\\') {
          if (j === cps.length - 1) { err = 'Trailing escape character at index ' + pos[j] + '.'; break; }
          const e = cps[j + 1];
          if (e.codePointAt(0) > 127) { err = 'Invalid non-ASCII character 0x' + e.codePointAt(0).toString(16) + ' at index ' + pos[j + 1] + '.'; break; }
          j += 2; continue;
        }
        if (d === ')') { depth--; if (depth === 0) { j++; break; } }
        else if (d === '(') {
          depth++;
          if (j === cps.length - 1 || cps[j + 1] !== '?') { err = 'Unnamed capturing groups are not allowed at index ' + pos[j + 1] + '.'; break; }
        }
        j++;
      }
      if (err) { bad(err, i, i); i++; continue; }
      if (depth !== 0) { bad('Unbalanced regex at index ' + pos[i] + '.', i, i); i++; continue; }
      const len = j - start - 1;
      if (len === 0) { bad('Missing regex at index ' + pos[i] + '.', i, i); i++; continue; }
      add('regexp', cps.slice(start, j - 1).join(''), i); i = j; continue;
    }
    add('char', c, i); i++;
  }
  tokens.push({ type: 'end', value: '', index: u });
  return tokens;
}
const TOKEN_NAME = { open: "'{'", close: "'}'", end: 'end of pattern', char: 'character', 'escaped-char': 'escaped character',
  name: 'parameter name', regexp: 'regex', asterisk: "'*'", modifier: 'modifier', 'invalid-char': 'invalid character' };

// ---- canonicalizers (encoding callbacks). Failure -> PatternError text that
// the component wrapper prefixes with "Invalid <name> pattern '<p>'. "
function dummy() { return new URL('https://dummy.test'); }
const canon = {
  protocol(v) {
    if (v === '') return v;
    if (!/^[a-zA-Z][a-zA-Z0-9+.\-]*$/.test(v)) throw perr("Invalid protocol '" + v + "'.");
    return v.toLowerCase();
  },
  username(v) { if (v === '') return v; const u = dummy(); u.username = v; return u.username; },
  password(v) { if (v === '') return v; const u = dummy(); u.password = v; return u.password; },
  hostname(v) {
    if (v === '') return v;
    // the host parser stops at / ? # \ (state override): keep only that prefix
    const cut = v.split(/[/?#\\]/)[0];
    const u = dummy(); u.hostname = cut;
    if (cut === '' || (u.hostname === 'dummy.test' && cut !== 'dummy.test')) throw perr("Invalid hostname '" + v + "'.");
    return u.hostname;
  },
  ipv6(v) {
    if (v === '') return v;
    if (!/^[0-9a-fA-F:[\]]*$/.test(v)) throw perr("Invalid IPv6 hostname '" + v + "'.");
    return v.toLowerCase();
  },
  port(v) {
    if (v === '') return v;
    const m = /^[0-9]+/.exec(v);
    if (!m || parseInt(m[0], 10) > 65535) throw perr("Invalid port '" + v + "'.");
    return String(parseInt(m[0], 10));
  },
  pathname(v) {
    if (v === '') return v;
    const lead = v[0] === '/';
    const u = dummy(); u.pathname = lead ? v : '/-' + v;
    const r = u.pathname;
    return lead ? r : r.slice(2);
  },
  opaquePathname(v) {
    if (v === '') return v;
    // C0-control percent-encode set only
    return v.replace(/[\u0000-\u001f\u007f-￿]/gu, (c) => encodeURIComponent(c));
  },
  search(v) { if (v === '') return v; const u = dummy(); u.search = v; return u.search.slice(1); },
  hash(v) { if (v === '') return v; const u = dummy(); u.hash = v; return u.hash.slice(1); },
};

// ---- pattern-string parser
const FULL_WILDCARD = '.*';
function segmentWildcard(opts) { return opts.delimiter === '' ? '[^]+?' : '[^' + escapeRegexp(opts.delimiter) + ']+?'; }
const DEFAULT_OPTS = { delimiter: '', prefix: '', ignoreCase: false };
const HOST_OPTS = { delimiter: '.', prefix: '', ignoreCase: false };
const PATH_OPTS = { delimiter: '/', prefix: '/', ignoreCase: false };

function parsePattern(input, opts, encode) {
  const tokens = tokenize(input, true);
  const parts = [];
  let pending = '', idx = 0, nextName = 0;
  const segRe = segmentWildcard(opts);
  const tryConsume = (type) => (tokens[idx].type === type ? tokens[idx++] : null);
  // a modifier is an other-modifier OR an asterisk; an asterisk is a wildcard
  // only when no name token precedes it (':a*' is a name plus a modifier)
  const tryModifier = () => tryConsume('modifier') || tryConsume('asterisk');
  const tryReOrWild = (nameTok) => tryConsume('regexp') || (nameTok ? null : tryConsume('asterisk'));
  const consumeText = () => {
    let r = '';
    for (;;) {
      const t = tryConsume('char') || tryConsume('escaped-char');
      if (!t) break;
      r += t.value;
    }
    return r;
  };
  const must = (type) => {
    const t = tryConsume(type);
    if (t) return t;
    const cur = tokens[idx];
    throw perr('Unexpected ' + TOKEN_NAME[cur.type] + " '" + cur.value + "' at index " + cur.index + ', expected ' + TOKEN_NAME[type] + '.');
  };
  const flush = () => {
    if (pending === '') return;
    parts.push({ type: 'fixed', value: encode(pending), modifier: 'none', name: '', prefix: '', suffix: '' });
    pending = '';
  };
  const isDup = (n) => parts.some((p) => p.name === n);
  const addPart = (prefix, nameTok, reTok, suffix, modTok) => {
    let modifier = 'none';
    if (modTok) modifier = modTok.value === '?' ? 'optional' : modTok.value === '*' ? 'zero-or-more' : 'one-or-more';
    if (!nameTok && !reTok && modifier === 'none') { pending += prefix; return; }
    flush();
    if (!nameTok && !reTok) {
      if (prefix === '') return;
      parts.push({ type: 'fixed', value: encode(prefix), modifier, name: '', prefix: '', suffix: '' });
      return;
    }
    let re = !reTok ? segRe : reTok.type === 'asterisk' ? FULL_WILDCARD : reTok.value;
    let type = 'regexp';
    if (re === segRe) { type = 'segment'; re = ''; } else if (re === FULL_WILDCARD) { type = 'full'; re = ''; }
    let name = '';
    if (nameTok) name = nameTok.value; else if (reTok) name = String(nextName++);
    if (isDup(name)) {
      const at = nameTok ? nameTok.index + 1 + nameTok.value.length : tokens[idx - 1].index;
      throw perr("Duplicate group name '" + name + "' at index " + at + '.');
    }
    parts.push({ type, value: re, modifier, name, prefix: encode(prefix), suffix: encode(suffix) });
  };
  while (idx < tokens.length) {
    const charTok = tryConsume('char');
    const nameTok = tryConsume('name');
    const reTok = tryReOrWild(nameTok);
    if (nameTok || reTok) {
      let prefix = charTok ? charTok.value : '';
      if (prefix !== '' && prefix !== opts.prefix) { pending += prefix; prefix = ''; }
      flush();
      addPart(prefix, nameTok, reTok, '', tryModifier());
      continue;
    }
    const fixed = charTok || tryConsume('escaped-char');
    if (fixed) { pending += fixed.value; continue; }
    if (tryConsume('open')) {
      const prefix = consumeText();
      const n2 = tryConsume('name');
      const r2 = tryReOrWild(n2);
      const suffix = consumeText();
      must('close');
      addPart(prefix, n2, r2, suffix, tryModifier());
      continue;
    }
    flush();
    must('end');
    break;
  }
  return parts;
}
const modStr = (m) => (m === 'optional' ? '?' : m === 'zero-or-more' ? '*' : m === 'one-or-more' ? '+' : '');
function genRegexp(parts, opts) {
  let r = '^';
  const names = [];
  for (const p of parts) {
    if (p.type === 'fixed') {
      r += p.modifier === 'none' ? escapeRegexp(p.value) : '(?:' + escapeRegexp(p.value) + ')' + modStr(p.modifier);
      continue;
    }
    names.push(p.name);
    const rv = p.type === 'segment' ? segmentWildcard(opts) : p.type === 'full' ? FULL_WILDCARD : p.value;
    if (p.prefix === '' && p.suffix === '') {
      r += p.modifier === 'none' || p.modifier === 'optional' ? '(' + rv + ')' + modStr(p.modifier) : '((?:' + rv + ')' + modStr(p.modifier) + ')';
      continue;
    }
    if (p.modifier === 'none' || p.modifier === 'optional') {
      r += '(?:' + escapeRegexp(p.prefix) + '(' + rv + ')' + escapeRegexp(p.suffix) + ')' + modStr(p.modifier);
      continue;
    }
    r += '(?:' + escapeRegexp(p.prefix) + '((?:' + rv + ')(?:' + escapeRegexp(p.suffix) + escapeRegexp(p.prefix) + '(?:' + rv + '))*)' + escapeRegexp(p.suffix) + ')';
    if (p.modifier === 'zero-or-more') r += '?';
  }
  return { source: r + '$', names };
}
function genPatternString(parts, opts) {
  let r = '';
  for (let i = 0; i < parts.length; i++) {
    const p = parts[i], prev = i > 0 ? parts[i - 1] : null, next = i < parts.length - 1 ? parts[i + 1] : null;
    if (p.type === 'fixed') {
      r += p.modifier === 'none' ? escapePattern(p.value) : '{' + escapePattern(p.value) + '}' + modStr(p.modifier);
      continue;
    }
    const custom = !isNumericName(p.name);
    let group = p.suffix !== '' || (p.prefix !== '' && p.prefix !== opts.prefix);
    if (!group && custom && p.type === 'segment' && p.modifier === 'none' && next && next.prefix === '' && next.suffix === '') {
      if (next.type === 'fixed') { const last = Array.from(next.value).pop(); if (validNameCp(last, false)) group = true; }
      else group = !isNumericName(next.name);
    }
    if (!group && p.prefix === '' && prev && prev.type === 'fixed') {
      const last = Array.from(prev.value).pop();
      if (last === opts.prefix) group = true;
    }
    if (group) r += '{';
    r += escapePattern(p.prefix);
    if (custom) r += ':' + p.name;
    if (p.type === 'regexp') r += '(' + p.value + ')';
    else if (p.type === 'segment' && !custom) r += '(' + segmentWildcard(opts) + ')';
    else if (p.type === 'full') {
      if (!custom && (!prev || prev.type === 'fixed' || prev.modifier !== 'none' || group || p.prefix !== '')) r += '*';
      else r += '(' + FULL_WILDCARD + ')';
    }
    if (p.type === 'segment' && custom && p.suffix !== '' && validNameCp(Array.from(p.suffix)[0], false)) r += '\\';
    r += escapePattern(p.suffix);
    if (group) r += '}';
    if (p.modifier !== 'none') r += modStr(p.modifier);
  }
  return r;
}
function compile(name, input, opts, encode, ignoreCase) {
  try {
    const parts = parsePattern(input, opts, encode);
    const gen = genRegexp(parts, opts);
    let re;
    try { re = new RegExp(gen.source, ignoreCase ? 'ui' : 'u'); } catch (e) {
      const bad = parts.find((p) => p.type === 'regexp');
      throw perr("Custom regular expression group '" + (bad ? bad.value : gen.source) + "' is invalid.");
    }
    return { pattern: genPatternString(parts, opts), re, names: gen.names, hasRe: parts.some((p) => p.type === 'regexp') };
  } catch (e) {
    if (e instanceof PatternError) throw perr("Invalid " + name + " pattern '" + input + "'. " + e.message);
    throw e;
  }
}

// ---- URLPatternInit processing
const isSpecial = (p) => Object.prototype.hasOwnProperty.call(SPECIAL, p);
function stripColon(v) { return v.endsWith(':') ? v.slice(0, -1) : v; }
function isAbsPath(v, type) {
  if (v === '') return false;
  if (v[0] === '/') return true;
  if (type === 'url') return false;
  if (v.length < 2) return false;
  return (v[0] === '\\' && v[1] === '/') || (v[0] === '{' && v[1] === '/');
}
function processInit(init, type) {
  const r = {};
  let base = null;
  if (init.baseURL !== undefined) {
    try { base = new URL(init.baseURL); } catch (e) { throw perr("Invalid baseURL '" + init.baseURL + "'."); }
    const esc = (s) => (type === 'pattern' ? escapePattern(s) : s);
    const has = (...ks) => ks.some((k) => init[k] !== undefined);
    if (init.protocol === undefined) r.protocol = esc(stripColon(base.protocol));
    if (type !== 'pattern' && !has('protocol', 'hostname', 'port', 'username')) r.username = esc(base.username);
    if (type !== 'pattern' && !has('protocol', 'hostname', 'port', 'username', 'password')) r.password = esc(base.password);
    if (!has('protocol', 'hostname')) r.hostname = esc(base.hostname);
    if (!has('protocol', 'hostname', 'port')) r.port = esc(base.port);
    if (!has('protocol', 'hostname', 'port', 'pathname')) r.pathname = esc(base.pathname);
    if (!has('protocol', 'hostname', 'port', 'pathname', 'search')) r.search = esc(base.search.slice(1));
    if (!has('protocol', 'hostname', 'port', 'pathname', 'search', 'hash')) r.hash = esc(base.hash.slice(1));
  }
  const pat = type === 'pattern';
  if (init.protocol !== undefined) r.protocol = pat ? stripColon(init.protocol) : canon.protocol(stripColon(init.protocol));
  if (init.username !== undefined) r.username = pat ? init.username : canon.username(init.username);
  if (init.password !== undefined) r.password = pat ? init.password : canon.password(init.password);
  if (init.hostname !== undefined) r.hostname = pat ? init.hostname : canon.hostname(init.hostname);
  if (init.port !== undefined) {
    if (pat) r.port = init.port;
    else {
      let p = canon.port(init.port);
      const proto = r.protocol !== undefined ? r.protocol : '';
      if (isSpecial(proto) && SPECIAL[proto] === p) p = '';
      r.port = p;
    }
  }
  if (init.pathname !== undefined) {
    let pn = init.pathname;
    // a relative pathname joins the base's DIRECTORY (opaque base paths have none)
    if (base !== null && base.pathname.startsWith('/') && !isAbsPath(pn, type)) {
      let bp = base.pathname;
      const si = bp.lastIndexOf('/');
      if (si >= 0) bp = bp.slice(0, si + 1);
      pn = (pat ? escapePattern(bp) : bp) + pn;
    }
    if (pat) r.pathname = pn;
    else if (pn === '') r.pathname = pn;
    else {
      const proto = r.protocol !== undefined ? r.protocol : '';
      r.pathname = proto === '' || isSpecial(proto) ? canon.pathname(pn) : canon.opaquePathname(pn);
    }
  }
  if (init.search !== undefined) { const s = init.search.startsWith('?') ? init.search.slice(1) : init.search; r.search = pat ? s : canon.search(s); }
  if (init.hash !== undefined) { const h = init.hash.startsWith('#') ? init.hash.slice(1) : init.hash; r.hash = pat ? h : canon.hash(h); }
  return r;
}

// ---- constructor-string parser
function parseConstructorString(input) {
  const tokens = tokenize(input, false);
  const result = {};
  const P_ = { tokens, input, state: 'init', start: 0, idx: 0, inc: 1, depth: 0, v6: 0, special: false };
  const safe = (i) => (i < tokens.length ? tokens[i] : tokens[tokens.length - 1]);
  const nonSpecial = (i, v) => { const t = safe(i); return t.value === v && (t.type === 'char' || t.type === 'escaped-char' || t.type === 'invalid-char'); };
  const comp = () => input.slice(safe(P_.start).index, tokens[P_.idx].index);
  const rewind = () => { P_.idx = P_.start; P_.inc = 0; };
  const change = (ns, skip) => {
    if (P_.state !== 'init' && P_.state !== 'done') result[P_.state] = comp();
    if (P_.state !== 'init' && ns !== 'done') {
      if (['protocol', 'authority', 'username', 'password'].includes(P_.state) && ['port', 'pathname', 'search', 'hash'].includes(ns) && result.hostname === undefined) result.hostname = '';
      if (['protocol', 'authority', 'username', 'password', 'hostname', 'port'].includes(P_.state) && ['search', 'hash'].includes(ns) && result.pathname === undefined) result.pathname = P_.special ? '/' : '';
      if (['protocol', 'authority', 'username', 'password', 'hostname', 'port', 'pathname'].includes(P_.state) && ns === 'hash' && result.search === undefined) result.search = '';
    }
    P_.state = ns; P_.idx += skip; P_.start = P_.idx; P_.inc = 0;
  };
  const rewindSet = (s) => { rewind(); P_.state = s; };
  const isSearch = () => {
    if (nonSpecial(P_.idx, '?')) return true;
    if (tokens[P_.idx].value !== '?') return false;
    if (P_.idx - 1 < 0) return true;
    const prev = safe(P_.idx - 1);
    return !['name', 'regexp', 'close', 'asterisk'].includes(prev.type);
  };
  const isHash = () => nonSpecial(P_.idx, '#');
  const isPathStart = () => nonSpecial(P_.idx, '/');
  while (P_.idx < tokens.length) {
    P_.inc = 1;
    const tok = tokens[P_.idx];
    if (tok.type === 'end') {
      if (P_.state === 'init') {
        rewind();
        if (isHash()) change('hash', 1); else if (isSearch()) change('search', 1); else change('pathname', 0);
        P_.idx += P_.inc; continue;
      }
      if (P_.state === 'authority') { rewindSet('hostname'); P_.idx += P_.inc; continue; }
      change('done', 0); break;
    }
    if (tok.type === 'open') { P_.depth++; P_.idx += P_.inc; continue; }
    if (P_.depth > 0) {
      if (tok.type === 'close') P_.depth--; else { P_.idx += P_.inc; continue; }
    }
    switch (P_.state) {
      case 'init': if (nonSpecial(P_.idx, ':')) rewindSet('protocol'); break;
      case 'protocol':
        if (nonSpecial(P_.idx, ':')) {
          try {
            const c = compile('protocol', comp(), DEFAULT_OPTS, canon.protocol, false);
            P_.special = Object.keys(SPECIAL).some((s) => c.re.test(s));
          } catch (e) { P_.special = false; }
          let ns = 'pathname', skip = 1;
          if (nonSpecial(P_.idx + 1, '/') && nonSpecial(P_.idx + 2, '/')) { ns = 'authority'; skip = 3; }
          else if (P_.special) { ns = 'authority'; skip = 1; }
          change(ns, skip);
        }
        break;
      case 'authority':
        if (nonSpecial(P_.idx, '@')) rewindSet('username');
        else if (isPathStart() || isSearch() || isHash()) rewindSet('hostname');
        break;
      case 'username':
        if (nonSpecial(P_.idx, ':')) change('password', 1);
        else if (nonSpecial(P_.idx, '@')) change('hostname', 1);
        break;
      case 'password': if (nonSpecial(P_.idx, '@')) change('hostname', 1); break;
      case 'hostname':
        if (nonSpecial(P_.idx, '[')) P_.v6++;
        else if (nonSpecial(P_.idx, ']')) P_.v6--;
        else if (nonSpecial(P_.idx, ':') && P_.v6 === 0) change('port', 1);
        else if (isPathStart()) change('pathname', 0);
        else if (isSearch()) change('search', 1);
        else if (isHash()) change('hash', 1);
        break;
      case 'port':
        if (isPathStart()) change('pathname', 0); else if (isSearch()) change('search', 1); else if (isHash()) change('hash', 1);
        break;
      case 'pathname': if (isSearch()) change('search', 1); else if (isHash()) change('hash', 1); break;
      case 'search': if (isHash()) change('hash', 1); break;
      default: break;
    }
    P_.idx += P_.inc;
  }
  if (result.hostname !== undefined && result.port === undefined) result.port = '';
  return result;
}

// ---- construction
const INIT_KEYS = ['baseURL', 'hash', 'hostname', 'password', 'pathname', 'port', 'protocol', 'search', 'username'];
function readInit(o, pre, fn) {
  const init = {};
  for (const k of INIT_KEYS) {
    const v = o[k];
    if (v !== undefined) {
      init[k] = P.conv.USVString(v, pre + ": Failed to read the '" + k + "' property from 'URLPatternInit'", fn);
    }
  }
  return init;
}
function URLPattern() {
  const PRE = "Failed to construct 'URLPattern'";
  if (new.target === undefined) P.fail(URLPattern, "Please use the 'new' operator, this DOM object constructor cannot be called as a function.", PRE);
  // null converts to the empty dictionary (URLPatternInput), not to the string 'null'
  const input = arguments[0] === null ? undefined : arguments[0], second = arguments[1], third = arguments[2];
  let init, baseURL = null, options = {};
  // three arguments select the (input, baseURL, options) overload: the second is
  // ALWAYS a USVString there ('undefined' included)
  const secondIsString = arguments.length >= 3 || (second !== undefined && second !== null && !isObj(second));
  if (isObj(input)) {
    init = readInit(input, PRE, URLPattern);
    if (secondIsString) {
      const b = P.conv.USVString(second, PRE, URLPattern);
      P.fail(URLPattern, "Invalid second argument baseURL '" + b + "' provided with a URLPatternInit input. Use the URLPatternInit.baseURL property instead.", PRE);
    }
    if (second !== undefined && second !== null) options = second;
  } else {
    const s = input === undefined ? undefined : P.conv.USVString(input, PRE, URLPattern);
    if (secondIsString) {
      baseURL = P.conv.USVString(second, PRE, URLPattern);
      if (third !== undefined && third !== null) options = third;
    } else if (second !== undefined && second !== null) options = second;
    if (s === undefined) init = {};
    else {
      try { init = parseConstructorString(s); } catch (e) { P.fail(URLPattern, e.message, PRE); }
      if (baseURL === null && init.protocol === undefined) {
        P.fail(URLPattern, "Relative constructor string '" + s + "' must have a base URL passed as the second argument.", PRE);
      }
      if (baseURL !== null) init.baseURL = baseURL;
    }
  }
  if (options !== undefined && options !== null && !isObj(options)) {
    P.fail(URLPattern, "The provided value is not of type 'URLPatternOptions'.", PRE);
  }
  const ignoreCase = !!(isObj(options) && options.ignoreCase);
  let pi;
  try { pi = processInit(init, 'pattern'); } catch (e) { if (e instanceof PatternError) P.fail(URLPattern, e.message, PRE); throw e; }
  for (const c of COMPONENTS) if (pi[c] === undefined) pi[c] = '*';
  const lp = pi.protocol.toLowerCase();
  if (isSpecial(lp) && pi.port === SPECIAL[lp]) pi.port = '';
  const comps = {};
  try {
    comps.protocol = compile('protocol', pi.protocol, DEFAULT_OPTS, canon.protocol, ignoreCase);
    comps.username = compile('username', pi.username, DEFAULT_OPTS, canon.username, ignoreCase);
    comps.password = compile('password', pi.password, DEFAULT_OPTS, canon.password, ignoreCase);
    const v6 = pi.hostname.length >= 2 && (pi.hostname[0] === '[' || (pi.hostname[0] === '\\' && pi.hostname[1] === '[') || (pi.hostname[0] === '{' && pi.hostname[1] === '['));
    comps.hostname = v6 ? compile('hostname', pi.hostname, DEFAULT_OPTS, canon.ipv6, ignoreCase)
      : compile('hostname', pi.hostname, HOST_OPTS, canon.hostname, ignoreCase);
    comps.port = compile('port', pi.port, DEFAULT_OPTS, canon.port, ignoreCase);
    const special = Object.keys(SPECIAL).some((s) => comps.protocol.re.test(s));
    comps.pathname = special ? compile('pathname', pi.pathname, PATH_OPTS, canon.pathname, ignoreCase)
      : compile('pathname', pi.pathname, DEFAULT_OPTS, canon.opaquePathname, ignoreCase);
    comps.search = compile('search', pi.search, DEFAULT_OPTS, canon.search, ignoreCase);
    comps.hash = compile('hash', pi.hash, DEFAULT_OPTS, canon.hash, ignoreCase);
  } catch (e) { if (e instanceof PatternError) P.fail(URLPattern, e.message, PRE); throw e; }
  const self = Reflect.construct(Object, [], new.target);
  slots.set(self, comps);
  return self;
}
const UP = P.iface(URLPattern, 'URLPattern', 0);
function slot(o, fn) { const s = o !== null && typeof o === 'object' ? slots.get(o) : undefined; if (!s) P.illegal(fn); return s; }
for (const c of COMPONENTS) P.getter(UP, c, function __f() { return slot(this, __f)[c].pattern; });
P.getter(UP, 'hasRegExpGroups', function __f() { const s = slot(this, __f); return COMPONENTS.some((c) => s[c].hasRe); });

// ---- matching
function runMatch(self, name, args) {
  const comps = slots.get(self);
  const PRE = "Failed to execute '" + name + "' on 'URLPattern'";
  const input = args[0] === null ? undefined : args[0], baseArg = args[1];
  let inputs, vals;
  if (isObj(input)) {
    const init = readInit(input, PRE, runMatch);
    if (baseArg !== undefined) {
      const b = P.conv.USVString(baseArg, PRE, runMatch);
      P.fail(runMatch, "Invalid second argument baseURL '" + b + "' provided with a URLPatternInit input. Use the URLPatternInit.baseURL property instead.", PRE);
    }
    inputs = [Object.assign({}, init)];
    try {
      const pi = processInit(init, 'url');
      vals = {};
      for (const c of COMPONENTS) vals[c] = pi[c] === undefined ? '' : pi[c];
    } catch (e) { if (e instanceof PatternError) return null; throw e; }
  } else {
    const s = input === undefined ? undefined : P.conv.USVString(input, PRE, runMatch);
    const b = baseArg === undefined ? undefined : P.conv.USVString(baseArg, PRE, runMatch);
    if (s === undefined) { inputs = [{}]; vals = {}; for (const c of COMPONENTS) vals[c] = ''; }
    else {
      inputs = b === undefined ? [s] : [s, b];
      let u;
      try { u = b === undefined ? new URL(s) : new URL(s, b); } catch (e) { return null; }
      vals = { protocol: stripColon(u.protocol), username: u.username, password: u.password, hostname: u.hostname, port: u.port,
        pathname: u.pathname, search: u.search.slice(1), hash: u.hash.slice(1) };
    }
  }
  const out = {};
  for (const c of COMPONENTS) {
    const m = comps[c].re.exec(vals[c]);
    if (m === null) return null;
    const groups = {};
    comps[c].names.forEach((n, i) => { groups[n] = m[i + 1]; });
    out[c] = { groups, input: vals[c] };
  }
  // dictionary members in WebIDL (lexicographic) order
  return { hash: out.hash, hostname: out.hostname, inputs, password: out.password, pathname: out.pathname, port: out.port,
    protocol: out.protocol, search: out.search, username: out.username };
}
P.method(UP, 'exec', 0, function __f() { slot(this, __f); return runMatch(this, 'exec', arguments); });
P.method(UP, 'test', 0, function __f() { slot(this, __f); return runMatch(this, 'test', arguments) !== null; });
P.seal(URLPattern);

P.expose('URLPattern', URLPattern);
