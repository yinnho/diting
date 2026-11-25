//! JS source builders for the eval / callFunctionOn wrapper family — split
//! from the module root (god-file ratchet). Pure string assembly, no runtime
//! state: an expression in, the exact script text `execute_script` will run
//! out.
use super::{EvalInput, JsRuntime};

/// The global a `callFunctionOn` wrapper sets when its declaration came to
/// no function, under a name of its call's number.
pub(super) fn not_function_flag(done_counter: u64) -> String {
    format!("__diting_not_function_{done_counter}")
}

impl JsRuntime {
    /// A Rust string as a JS string literal: serde_json escapes the quotes,
    /// the newlines and U+2028/2029, and nothing else in it can end a
    /// literal.
    fn literal(text: &str) -> String {
        serde_json::to_string(text).unwrap_or_else(|_| "\"\"".to_string())
    }

    /// Object-store slot expression with the id embedded as a JS string
    /// literal, so an objectId — minted or inbound — can never splice itself
    /// into the surrounding script (obscura#843 class).
    pub(super) fn object_slot(oid: &str) -> String {
        format!("globalThis.__diting_objects[{}]", Self::literal(oid))
    }

    /// The engine's trampoline around the indirect `eval` that runs an
    /// evaluated script, with `expr` the script as a literal. `forgiving`
    /// is [`EvalInput::Forgiving`]. It is a
    /// function expression written into the wrapper that calls it, not a
    /// function kept elsewhere: its frame is then a frame of the wrapper's
    /// and goes out with the wrapper's (`page_frames`), and the script it
    /// compiles names the wrapper as what compiled it — never a trampoline
    /// of its own that a page's stack could be recognized by. It also
    /// installs window named access first (`_namedBoot`, #48): a one-flag
    /// no-op after the first successful scan.
    ///
    /// [`EvalInput::Forgiving`] adds: a script that does not parse is run
    /// once more as a function body, so that one written with a top-level
    /// `return` still evaluates. A script that parses as neither is
    /// reported as a script: that is the first SyntaxError, whose message
    /// and lack of engine frames are the parser's own. Only when the
    /// `return` is all that kept it from being a script is the body's error
    /// shown instead.
    fn trampoline(expr: &str, forgiving: bool) -> String {
        if !forgiving {
            return format!(
                "(function(s) {{
                    globalThis._namedBoot && globalThis._namedBoot();
                    return (0, eval)(s);
                }})({expr})"
            );
        }
        format!(
            "(function(s) {{
                globalThis._namedBoot && globalThis._namedBoot();
                try {{ return (0, eval)(s); }}
                catch (x) {{
                    if (x instanceof SyntaxError) {{
                        try {{ return (new Function(s))(); }}
                        catch (y) {{
                            throw y instanceof SyntaxError && !/Illegal return statement/.test(x.message) ? x : y;
                        }}
                    }}
                    throw x;
                }}
            }})({expr})"
        )
    }

    /// The script an eval or a `callFunctionOn` declaration is run as —
    /// what `trampoline` compiles — as a JS literal.
    fn compiled(script: &str, forgiving: bool) -> String {
        Self::trampoline(&Self::literal(script), forgiving)
    }

    /// An `unserializableValue` of a call argument, as V8 reads it
    /// (`InjectedScript::resolveCallArgument`): the text is run as a script
    /// of its own, and the three that are names — `NaN`, `Infinity`,
    /// `-Infinity` — go through `Number("…")`, so the page cannot have
    /// swapped what they mean.
    pub(super) fn call_argument_code(text: &str, slot: &str) -> String {
        let literal = Self::literal(text);
        let value = if matches!(text, "NaN" | "Infinity" | "-Infinity") {
            format!("Number({literal})")
        } else {
            format!("(0, eval)({literal})")
        };
        format!("{slot} = {value};")
    }

    /// Meta-code for Runtime.evaluate. The await variant is the #114 shape:
    /// run the expression, then only `await` when the result is actually a
    /// thenable — everything else (probes, assignments, reads: the majority
    /// of evals) assigns the slot and sets the done sentinel inside this very
    /// execute_script call. An unconditional `await` parks the sentinel in a
    /// microtask, and the runtime executes scripts with no microtask
    /// checkpoint (our Vm matches deno_core's law — #114), so the sentinel
    /// can only turn true on the next event-loop
    /// turn — which a churny page (a revived persistent session re-running
    /// its boot scripts) holds hostage behind multi-second synchronous page
    /// scripts until the eval burns its whole budget into EVAL_TIMEOUT.
    pub(super) fn eval_meta_code(
        expression: &str,
        oid: &str,
        done_counter: u64,
        await_promise: bool,
        input: EvalInput,
    ) -> String {
        let forgiving = input == EvalInput::Forgiving;
        let script = Self::script_text(expression, forgiving);
        let head = format!("__result = {};", Self::compiled(&script, forgiving));
        Self::settle_code("", &head, oid, done_counter, await_promise)
    }

    /// The text an evaluated script runs as. CDP Runtime.evaluate semantics:
    /// the input is a *script*, not an expression — statements are legal and
    /// the completion value of the last statement is the result. Indirect
    /// eval (`(0,eval)(...)`) runs it at global scope and hands back the
    /// completion value, so `var x=1; x*2` returns 2 exactly like Chrome. The
    /// previous `return (...);` expression wrap turned any statement syntax
    /// into `Unexpected token ';'` (unless the script happened to start with
    /// one of six hard-coded statement keywords), which silently broke
    /// clients that evaluate statement scripts — e.g. probing a WAF
    /// challenge page with `try { readygo(); } catch(e) {...}`.
    //
    // [`EvalInput::Forgiving`] adds two conveniences of the HTTP and flow
    // faces, which a typed-in script benefits from and Chrome has neither:
    // the text is trimmed, and a bare `{...}` input is parenthesized so
    // pasted JSON evaluates as an object literal (like DevTools console) —
    // as a script it is a block, or a labeled statement whose value is the
    // last expression's (`{a:1}` is `1`; `{a:1, b:2}` does not parse).
    fn script_text(expression: &str, forgiving: bool) -> String {
        if !forgiving {
            return expression.to_string();
        }
        let trimmed = expression.trim();
        if trimmed.starts_with('{') && trimmed.ends_with('}') {
            format!("({trimmed})")
        } else {
            trimmed.to_string()
        }
    }

    /// Meta-code for Runtime.callFunctionOn, in the #114 sync-first shape of
    /// [`eval_meta_code`](Self::eval_meta_code): the call runs
    /// synchronously, and only a thenable result takes the `await` branch.
    /// The declaration is compiled as Chrome compiles it, `"(" +
    /// declaration + ")"` as a script of its own, and called as a method of
    /// its receiver — so a stack shows `Object.named` / `Array.<anonymous>`
    /// and positions from the declaration's own first character, as it does
    /// in Chrome. Nothing is forgiven of it, as nothing is in Chrome.
    ///
    /// What it compiles to is called only when it is a function, as in
    /// Chrome (`InjectedScript::callFunction`): otherwise the wrapper leaves
    /// a flag of its own — named for this call, so nothing the page keeps
    /// under a name of the engine's can stand in for it — and stores
    /// nothing.
    pub(super) fn call_fn_meta_code(
        setup: &str,
        fn_decl: &str,
        this_expr: &str,
        args_list: &str,
        oid: &str,
        done_counter: u64,
        await_promise: bool,
    ) -> String {
        let head = format!(
            "var __fn = {};\n\
             if (typeof __fn !== 'function') {{ globalThis.{} = true; return; }}\n\
             __result = __fn.call(({}), {});",
            Self::compiled(&format!("({fn_decl})"), false),
            not_function_flag(done_counter),
            this_expr,
            args_list
        );
        Self::settle_code(setup, &head, oid, done_counter, await_promise)
    }

    /// A wrapper that runs `setup` and then `head` — which assigns
    /// `__result`, or throws — and stores what came out in the slot `oid`
    /// names, with its meta in
    /// `globalThis.__diting_await_meta` and `__diting_await_rejected`
    /// saying which it was. All of it synchronously where the result is not
    /// a thenable, so even the failure path needs no event-loop turn (#114).
    fn settle_code(
        setup: &str,
        head: &str,
        oid: &str,
        done_counter: u64,
        await_promise: bool,
    ) -> String {
        let slot = Self::object_slot(oid);
        // The shape reader is written once, as a local of the wrapper: both
        // ways out of it read through it.
        let meta = format!("var __meta = {};\n", Self::meta_fn_js());
        let done = if await_promise {
            format!("globalThis.__diting_done_{done_counter} = true;\n")
        } else {
            String::new()
        };
        let fail = |in_promise| {
            format!(
                "{slot} = e;\n\
                 globalThis.__diting_await_meta = {exc};\n\
                 globalThis.__diting_await_rejected = true;\n\
                 {done}",
                slot = slot,
                exc = Self::exception_meta_extract_js("e", in_promise),
                done = done,
            )
        };
        let kept = format!(
            "{slot} = __result;\n\
             globalThis.__diting_await_meta = {read};\n\
             globalThis.__diting_await_rejected = false;",
            slot = slot,
            read = Self::meta_extract_js("__result"),
        );
        if !await_promise {
            return format!(
                "(function() {{\n\
                    {meta}\
                    {setup}\n\
                    var __result;\n\
                    try {{\n{head}\n{kept}\n}} catch(e) {{\n{fail}\n}}\n\
                }})()",
                meta = meta,
                setup = setup,
                head = head,
                kept = kept,
                fail = fail(false),
            );
        }
        format!(
            "(async function() {{\n\
                {meta}\
                {setup}\n\
                var __result;\n\
                try {{\n{head}\n}} catch(e) {{\n{sync}\nreturn; }}\n\
                if (__result && typeof __result.then === 'function') {{\n\
                    try {{ __result = await __result; }}\n\
                    catch(e) {{\n{promise}\nreturn; }}\n\
                }}\n\
                {kept}\n\
                globalThis.__diting_done_{done_counter} = true;\n\
            }})()",
            meta = meta,
            setup = setup,
            head = head,
            sync = fail(false),
            promise = fail(true),
            kept = kept,
            done_counter = done_counter,
        )
    }

    /// The forgiving text: this is the HTTP and flow faces' wrap.
    pub(super) fn wrap_expression(expression: &str) -> String {
        format!(
            "(function() {{ try {{ return {}; }} catch(e) {{ return null; }} }})()",
            Self::compiled(&Self::script_text(expression, true), true)
        )
    }

    /// What a result is, the way Chrome's RemoteObject says it (Chrome
    /// 158, measured): `type`, and beside it `subtype`, `className`,
    /// `description`, the `value` of a primitive, and the
    /// `unserializableValue` of a number JSON has no spelling for or of a
    /// bigint. `info_from_meta` reads the JSON this builds.
    ///
    /// A symbol is left as something to name by handle, as an object is.
    /// Nothing here is a preview: that would be Chrome's front end's. An
    /// Error's description is its message line; its stack travels beside
    /// as V8 wrote it, and [`page_frames`](super::remote_object::page_frames) sorts the
    /// frames.
    ///
    /// A class name is the `constructor` of the chain read as a data
    /// property — a getter standing there is not called, and nothing above
    /// it counts; a class with no name is known by the one it extends. Where
    /// that is `Object`, the string `Symbol.toStringTag` read the same way
    /// stands in (`Math`, `console`) — but a tag alone makes no `Map`: each
    /// subtype is asked of the object itself. An iterator and a generator
    /// are named by their kind (`MapIterator`, `Array Iterator`); a Map's or
    /// Set's iterator and a generator carry a subtype. An attribute (`Attr`)
    /// is a node, named by the attribute.
    ///
    /// An Error's message line is headed by its `name` where that is a
    /// string other than the bare `"Error"`, and elsewhere by its class —
    /// `Error` where the class has no name; a `message` that is not a
    /// string is not written.
    ///
    /// Nothing of the page's can fail the read: one of its accessors that
    /// throws leaves the plain answer for the type.
    ///
    /// [`meta_extract_js`](Self::meta_extract_js) reads a variable of the
    /// wrapper [`settle_code`](Self::settle_code) builds, through the one
    /// copy of this function it holds.
    pub(super) fn meta_extract_js(var_name: &str) -> String {
        format!("__meta({var_name})")
    }

    fn meta_fn_js() -> String {
        r#"(function(v) {
                var t = typeof v, st = null, cn = '', desc = '', val, unser, stk = '';
                var tag = function() { return Object.prototype.toString.call(v); };
                function data(v, key) {
                    for (var p = v; p; p = Object.getPrototypeOf(p)) {
                        var own = Object.getOwnPropertyDescriptor(p, key);
                        if (own) return (own.get || own.set) ? undefined : own.value;
                    }
                }
                // A brand, not a tag: a `Symbol.toStringTag` says what its
                // owner wrote. The iterator and generator objects are known
                // by their prototypes, the rest by a method only they take.
                function chain(maker, v) {
                    try {
                        return (maker.p || (maker.p = Object.getPrototypeOf(maker()))).isPrototypeOf(v);
                    } catch (_) {
                        return false;
                    }
                }
                function call(method, v) {
                    try { method.call(v); return true; } catch (_) { return false; }
                }
                function slot(Proto, name, v) {
                    return call(Object.getOwnPropertyDescriptor(Proto.prototype, name).get, v);
                }
                function mapIterator() { return new Map().keys(); }
                function setIterator() { return new Set().keys(); }
                function stringIterator() { return ''[Symbol.iterator](); }
                function regexpIterator() { return ''.matchAll(/a/g); }
                function arrayIterator() { return [][Symbol.iterator](); }
                function generator() { return Object.getPrototypeOf(function*(){}).prototype; }
                function asyncGenerator() { return Object.getPrototypeOf(async function*(){}).prototype; }
                function promise() { return Promise.resolve(); }
                function errorLine(v, head) {
                    var n, m;
                    try { n = v.name; } catch (_) { n = undefined; }
                    if (!(typeof n === 'string' && n && n !== 'Error')) n = head;
                    try { m = v.message; } catch (_) { m = undefined; }
                    return (typeof m === 'string' && m) ? n + ': ' + m : n;
                }
                try {
                    if (v === null) { st = 'null'; val = null; }
                    else if (v === undefined) { }
                    else if (Array.isArray(v)) {
                        st = 'array';
                        cn = (v.constructor && v.constructor.name) || 'Array';
                        desc = cn + '(' + v.length + ')';
                    }
                    else if (t === 'object' && typeof v._nid === 'number') {
                        st = 'node';
                        cn = v.constructor ? v.constructor.name : 'Node';
                        if (v.nodeType === 9) cn = 'HTMLDocument';
                        else if (v.nodeType === 1) cn = 'HTML' + (v.tagName || 'Element').charAt(0) + (v.tagName || 'Element').slice(1).toLowerCase() + 'Element';
                        desc = v.tagName ? v.tagName.toLowerCase() : (v.nodeName || 'node');
                        if (v.nodeType === 1 && v.id) desc += '#' + v.id;
                        if (v.nodeType === 1 && typeof v.className === 'string' && v.className.trim()) {
                            desc += '.' + v.className.trim().split(/\s+/).join('.');
                        }
                    }
                    else if (t === 'function') {
                        cn = (v.constructor && v.constructor.name) || 'Function';
                        desc = Function.prototype.toString.call(v);
                    }
                    else if (t === 'object' && v.nodeType === 2) {
                        st = 'node';
                        cn = 'Attr';
                        desc = String(v.name);
                    }
                    else if (t === 'object') {
                        // The class is the constructor's name; a class with
                        // none is known by the one it extends.
                        var ctor = data(v, 'constructor'), own = '';
                        for (; typeof ctor === 'function' && !(own = ctor.name); ctor = Object.getPrototypeOf(ctor)) {}
                        cn = own || 'Object';
                        desc = cn;
                        tag = tag();
                        if (tag === '[object Map Iterator]' && chain(mapIterator, v)) { st = 'iterator'; cn = desc = 'MapIterator'; }
                        else if (tag === '[object Set Iterator]' && chain(setIterator, v)) { st = 'iterator'; cn = desc = 'SetIterator'; }
                        else if (tag === '[object String Iterator]' && chain(stringIterator, v)) { cn = desc = 'StringIterator'; }
                        else if (tag === '[object RegExp String Iterator]' && chain(regexpIterator, v)) { cn = desc = 'RegExpStringIterator'; }
                        else if (tag === '[object Array Iterator]' && chain(arrayIterator, v)) { cn = desc = 'Array Iterator'; }
                        else if (tag === '[object Generator]' && chain(generator, v)) { st = 'generator'; cn = desc = 'Generator'; }
                        else if (tag === '[object AsyncGenerator]' && chain(asyncGenerator, v)) { st = 'generator'; cn = desc = 'AsyncGenerator'; }
                        else if (tag === '[object Date]' && call(Date.prototype.getTime, v)) { st = 'date'; desc = String(v); }
                        else if (tag === '[object RegExp]' && slot(RegExp, 'source', v)) { st = 'regexp'; desc = String(v); }
                        else if (tag === '[object Map]' && call(Map.prototype.has, v)) { st = 'map'; desc = cn + '(' + v.size + ')'; }
                        else if (tag === '[object Set]' && call(Set.prototype.has, v)) { st = 'set'; desc = cn + '(' + v.size + ')'; }
                        else if (tag === '[object WeakMap]' && call(WeakMap.prototype.has, v)) { st = 'weakmap'; }
                        else if (tag === '[object WeakSet]' && call(WeakSet.prototype.has, v)) { st = 'weakset'; }
                        else if (tag === '[object Promise]' && chain(promise, v)) { st = 'promise'; }
                        else if (tag === '[object ArrayBuffer]' && slot(ArrayBuffer, 'byteLength', v)) { st = 'arraybuffer'; desc = cn + '(' + v.byteLength + ')'; }
                        else if (tag === '[object DataView]' && slot(DataView, 'byteLength', v)) { st = 'dataview'; desc = cn + '(' + v.byteLength + ')'; }
                        else if (ArrayBuffer.isView(v)) { st = 'typedarray'; desc = cn + '(' + v.length + ')'; }
                        else if (tag === '[object Arguments]' || cn === 'NodeList' || cn === 'HTMLCollection') {
                            st = 'array';
                            if (tag === '[object Arguments]') cn = 'Arguments';
                            desc = cn + '(' + v.length + ')';
                        }
                        else if (tag === '[object DOMException]' || v instanceof Error) {
                            st = 'error';
                            cn = own || 'Error';
                            desc = errorLine(v, cn);
                            if (tag !== '[object DOMException]' && typeof v.stack === 'string') stk = v.stack;
                        }
                        else if (cn === 'Object') {
                            var named = data(v, Symbol.toStringTag);
                            if (typeof named === 'string' && named) { cn = desc = named; }
                        }
                    }
                    else if (t === 'number') {
                        if (!isFinite(v) || Object.is(v, -0)) { unser = Object.is(v, -0) ? '-0' : String(v); desc = unser; }
                        else { val = v; desc = String(v); }
                    }
                    else if (t === 'bigint') { unser = String(v) + 'n'; desc = unser; }
                    else if (t === 'boolean' || t === 'string') { val = v; }
                    else { desc = String(v); }
                } catch (_) {
                    st = null; cn = ''; desc = ''; val = undefined; unser = undefined; stk = '';
                    if (t === 'function') { cn = 'Function'; desc = 'function () { [native code] }'; }
                    else { cn = 'Object'; desc = 'Object'; }
                }
                return JSON.stringify({type:t,subtype:st,className:cn,description:desc,value:val,unserializableValue:unser,stack:stk});
            })"#
            .to_string()
    }

    /// The value of a held object the way Chrome's `returnByValue` brings
    /// one back (Chrome 158, measured): its own enumerable properties, and
    /// of those again — a prototype chain is left out, a getter is called.
    /// A function inside is `{{}}`; a key whose value is `undefined` is left
    /// out; a number JSON has no spelling for is `null` there and `-0` is
    /// `0`; `toJSON` is not consulted. A bigint or a symbol inside cannot
    /// come back, and neither can a ring of objects.
    ///
    /// The answer is a string with a mark in front: `v` and the JSON; `c`,
    /// a ring; `p`, what cannot come back; `x`, a getter that threw; `n`,
    /// nothing held under that name.
    pub(super) fn by_value_js(var_name: &str) -> String {
        format!(
            r#"(function(v) {{
                var RING = {{}}, PLAIN = {{}}, stack = [];
                function copy(x) {{
                    if (x === null) return null;
                    var t = typeof x;
                    if (t === 'number') {{ if (!isFinite(x)) return null; return Object.is(x, -0) ? 0 : x; }}
                    if (t === 'string' || t === 'boolean') return x;
                    if (t === 'function') return {{}};
                    if (t === 'undefined') return undefined;
                    if (t !== 'object') throw PLAIN;
                    if (stack.indexOf(x) !== -1) throw RING;
                    stack.push(x);
                    var out = Array.isArray(x) ? [] : {{}}, keys = Object.keys(x);
                    for (var i = 0; i < keys.length; i++) {{
                        var k = keys[i], w = copy(x[k]);
                        if (w !== undefined) out[k] = w;
                    }}
                    stack.pop();
                    return out;
                }}
                if (v === undefined) return 'n';
                try {{ return 'v' + JSON.stringify(copy(v)); }}
                catch (e) {{ return e === RING ? 'c' : e === PLAIN ? 'p' : 'x'; }}
            }})({var_name})"#,
            var_name = var_name,
        )
    }

    /// What was thrown, as JSON: the message line of the value (`Error:
    /// boom`), the stack as V8 wrote it, whether it is an `Error` (as Chrome
    /// counts one for a message — a `DOMException` is not, though it is one
    /// by `instanceof`) and whether it is what a promise was rejected with
    /// rather than what a script threw, and
    /// [`meta_extract_js`](Self::meta_extract_js) of it — a thrown value is
    /// shown as a result is, class name and all. Meant for the wrapper of
    /// [`settle_code`](Self::settle_code), where `__meta` is in scope.
    ///
    /// The message line of an Error is `Error.prototype.toString`'s, as
    /// Chrome words a rejected promise: headed by `name` where that is a
    /// non-empty string and elsewhere by `Error`; a `message` that is not a
    /// string is not written. A `DOMException` is headed by its class where
    /// it still says `Error`. (The result shown beside a rejected Error is
    /// headed by the class instead — see `meta_fn_js`.) Anything else thrown
    /// is named by what it says of itself.
    pub(super) fn exception_meta_extract_js(var_name: &str, in_promise: bool) -> String {
        format!(
            r#"(function(e) {{
                var name = '', msg = '', desc = 'Uncaught exception', stk = '', isErr = false, thrown = null;
                try {{
                    if (e !== null && e !== undefined) {{
                        if (typeof e === 'object' || typeof e === 'function') {{
                            name = e.name || (e.constructor && e.constructor.name) || '';
                            if (typeof e.message === 'string') msg = e.message;
                            if (typeof e.stack === 'string') stk = e.stack;
                            isErr = e instanceof Error && Object.prototype.toString.call(e) !== '[object DOMException]';
                        }} else {{ msg = String(e); }}
                    }}
                    if (e instanceof Error) {{
                        name = (typeof e.name === 'string' && e.name) ? e.name : '';
                        if (!isErr && name === 'Error') name = '';
                        name = name || (e.constructor && e.constructor.name) || (isErr ? 'Error' : '');
                        msg = (typeof e.message === 'string' && e.message) ? e.message : '';
                    }}
                    desc = msg ? (name ? name + ': ' + msg : msg) : (name || msg || 'Uncaught exception');
                    thrown = JSON.parse(__meta(e));
                }} catch (_) {{}}
                return JSON.stringify({{description:desc, stack:stk, error:isErr, inPromise:{in_promise}, meta: thrown}});
            }})({var_name})"#,
            in_promise = in_promise,
            var_name = var_name,
        )
    }
}
