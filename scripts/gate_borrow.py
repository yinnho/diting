#!/usr/bin/env python3
"""RefCell re-borrow gate — the two shapes that bit in the field.

The borrow checker does not look inside a RefCell. A guard taken in an
`if let` / `match` scrutinee or a `for` head, plus a conflicting borrow of the
same cell in the block, compiles and panics at run time. Under an op poison
wall the panic is swallowed and the page is left half-mutated: that was
0e1c0304 (`ensure_layout_run` took the style cache in an `if let` scrutinee
and re-borrowed it in the block — six hits in the field, 2026-11-20/21 — and
`ModuleMap::wake`, same shape, latent).

Runs in CI (ci.yml `layers` job) and locally (`python3 scripts/gate_borrow.py`).
Reads source as text — a tripwire, not a proof:

  A  `if let` / `while let` / `match`: the scrutinee takes a guard of X and the
     block — and before edition 2024 also the `else` arm — borrows X in
     conflict (a `RefMut` plus any borrow, or a `Ref` plus a `borrow_mut`).
  B  `for`: the head takes a guard of X and the body borrows X in conflict.

"Guard taken" = `.borrow()` / `.borrow_mut()` / `try_borrow*().unwrap()` /
`.try_borrow*().expect(..)`, outside any nested `{ }` block and outside a
closure body — both drop the guard first (checked against rustc, editions
2021 and 2024: `--self-test` carries the table). Shared + shared is fine and
is not reported. Not seen: a guard bound by a `let` and re-borrowed before its
`drop` (tried: five hits on the tree, all false), and a re-borrow that goes
through a call (`wake()` re-entering) — no text scan can follow that one.

Scope: production code of the house crates — every `crates/<name>/src` whose
crate is not in the vendored table (crates/VENDORED.md). `tests.rs`, files
reached only through a `#[cfg(test)] mod` declaration and inline
`#[cfg(test)]` items are skipped, same rule as the god-file ratchet.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

import audit_layers as audit  # sibling script: what counts as test-only code

ROOT = Path(__file__).resolve().parent.parent

SITE = re.compile(
    r"\.(?P<kind>borrow|borrow_mut|try_borrow|try_borrow_mut)\(\)"
    r"(?P<escaped>\s*\.\s*(?:unwrap\(\)|expect\([^)]*\)|ok\(\)\s*\.\s*unwrap\(\)))?"
)
# Words an expression cannot continue past (scanning left from a `.borrow()`).
STOP_WORDS = (" if ", " let ", " match ", " while ", " else ", " in ", " return ", "=>", "&&", "||")


# ---------------------------------------------------------------- text helpers

def strip_comments(src: str) -> str:
    """Blank out comments and string bodies; keep every offset and newline."""
    out = list(src)
    i = 0
    n = len(src)
    while i < n:
        if src.startswith("//", i):
            j = src.find("\n", i)
            j = n if j < 0 else j
            for k in range(i, j):
                out[k] = " "
            i = j
        elif src.startswith("/*", i):
            depth = 1
            j = i + 2
            while j < n and depth:
                if src.startswith("/*", j):
                    depth += 1
                    j += 2
                elif src.startswith("*/", j):
                    depth -= 1
                    j += 2
                else:
                    j += 1
            for k in range(i, j):
                if out[k] != "\n":
                    out[k] = " "
            i = j
        elif src[i] == '"':
            # plain "..." and raw r"..." / r#"..."# / br#"..."#
            opener = re.search(r'b?r#*"$', src[max(0, i - 10):i + 1])
            hashes = ""
            if opener:
                hashes = opener.group(0)[:-1].lstrip("b").lstrip("r")
            closer = '"' + hashes
            j = src.find(closer, i + 1)
            j = n if j < 0 else j + len(closer)
            for k in range(i + 1, j - len(closer)):
                if out[k] != "\n":
                    out[k] = " "
            i = j
        elif src[i] == "'":
            m = re.match(r"'(\\.[^']*|[^'\\])'", src[i:])
            if m:  # a char literal; a lifetime does not match
                for k in range(i + 1, i + len(m.group(0)) - 1):
                    out[k] = " "
                i += len(m.group(0))
            else:
                i += 1
        else:
            i += 1
    return "".join(out)


def match_brace(s: str, open_idx: int) -> int:
    """Index of the bracket closing the one at `open_idx`, or -1."""
    stack = []
    i = open_idx
    while i < len(s):
        c = s[i]
        if c in "{([":
            stack.append(c)
        elif c in ")}]":
            stack.pop()
            if not stack:
                return i
        i += 1
    return -1


def line_of(s: str, idx: int) -> int:
    return s.count("\n", 0, idx) + 1


def block_after(s: str, i: int) -> int:
    """`{` opening the block of the `if let` / `match` / `for` whose head starts at `i`.

    Skips block expressions inside the head (`= { .. }`, a closure's body);
    stops at a statement boundary, so a keyword seen out of context is dropped.
    """
    depth = 0
    while i < len(s):
        c = s[i]
        if c in "([{":
            if c == "{" and depth == 0:
                prev = s[:i].rstrip()
                if prev.endswith("=>") or prev.endswith("="):
                    i = match_brace(s, i) + 1
                    continue
                return i
            depth += 1
        elif c in ")]}":
            if depth == 0:
                return -1
            depth -= 1
        elif c == ";":
            return -1
        i += 1
    return -1


def else_tail(s: str, close: int) -> tuple[int, int] | None:
    """[start, end) of the `else` part following the block that closes at `close`."""
    j = close + 1
    while j < len(s) and s[j].isspace():
        j += 1
    if not s.startswith("else", j) or (s[j + 4:j + 5].isalnum() or s[j + 4:j + 5] == "_"):
        return None
    k = j + 4
    depth = 0
    while k < len(s):
        c = s[k]
        if c in "([":
            depth += 1
        elif c in ")]":
            depth -= 1
        elif c == "{" and depth == 0:
            break
        elif c in ";}" and depth == 0:
            return None
        k += 1
    end = match_brace(s, k) if k < len(s) and s[k] == "{" else -1
    return (j, end + 1) if end >= 0 else None


# ---------------------------------------------------------------- guard sites

def in_braces(s: str, idx: int) -> bool:
    return s.count("{", 0, idx) != s.count("}", 0, idx)


def in_closure_body(s: str, idx: int) -> bool:
    """`idx` sits right of a `|` inside its own bracket group and argument:
    a `|a| body` closure (a bitwise `a | b` reads the same — it only hides)."""
    depth = 0
    i = idx - 1
    while i >= 0:
        c = s[i]
        if c in ")]}":
            depth += 1
        elif c in "([{":
            if depth == 0:
                return False
            depth -= 1
        elif depth == 0 and c in ",;":
            return False
        elif depth == 0 and c == "|":
            return True
        i -= 1
    return False


def receiver_of(m: re.Match, s: str) -> str:
    """Text of the expression the borrow is called on (best effort)."""
    end = m.start()
    start = 0
    i = end - 1
    depth = 0
    while i >= 0:
        c = s[i]
        if c in ")}]":
            depth += 1
        elif c in "([{":
            if depth == 0:
                break
            depth -= 1
        elif depth == 0:
            if c in ";{},!" or (c == "=" and s[i - 1:i] not in "><!="):
                break
            word = next((w for w in STOP_WORDS if s.startswith(w, i)), None)
            if word:
                start = i + len(word)  # the keyword is not part of the receiver
                break
        i -= 1
    else:
        start = 0
    if i >= 0 and start == 0:
        start = i + 1
    text = "".join(s[start:end].split()).lstrip("*&")
    while text.startswith("(") and match_brace(text, 0) == len(text) - 1:
        text = text[1:-1].lstrip("*&")
    return text


def sites(s: str, nested: bool) -> list[tuple[str, str]]:
    """Guards taken in `s`: [(kind, receiver)], kind in {"shared", "mut"}.

    `nested` (a scrutinee / head): a guard taken inside a `{ }` block or a
    closure body of `s` is dropped before the block runs — not counted.
    """
    out = []
    for m in SITE.finditer(s):
        # a bare try_borrow() Result is moved into a closure or bound as Ok(g):
        # dropped first, so only its unwrapped forms count as a guard
        if m.group("kind").startswith("try_") and not m.group("escaped"):
            continue
        if nested and (in_braces(s, m.start()) or in_closure_body(s, m.start())):
            continue
        recv = receiver_of(m, s)
        if recv:
            out.append(("mut" if m.group("kind").endswith("_mut") else "shared", recv))
    return out


def conflicting(head: str, body: str) -> list[str]:
    """Receivers whose guard in `head` is borrowed in conflict inside `body`.

    Everything in `body` runs while the head's guard is held — nested blocks
    included — so nothing in it is filtered.
    """
    guards: dict[str, str] = {}
    for kind, recv in sites(head, nested=True):
        guards[recv] = "mut" if kind == "mut" else guards.get(recv, "shared")
    return sorted(
        r for kind, r in sites(body, nested=False)
        if r in guards and (guards[r] == "mut" or kind == "mut")
    )


# ---------------------------------------------------------------- the two shapes

def shape_a(s: str, edition: int) -> list[tuple[int, str, str]]:
    """`if let` / `while let` / `match`: guard in the scrutinee, re-borrow in the block."""
    hits = []
    for m in re.finditer(r"\b(?:if let|while let|match)\b", s):
        block = block_after(s, m.end())
        close = match_brace(s, block) if block >= 0 else -1
        if close < 0:
            continue
        body = s[block + 1:close]
        if edition < 2024:  # the scrutinee's temporaries also outlive the else arm
            tail = else_tail(s, close)
            while tail:
                body += s[tail[0]:tail[1]]
                tail = else_tail(s, tail[1] - 1)
        for recv in conflicting(s[m.end():block], body):
            hits.append((line_of(s, m.start()), recv, m.group(0)))
    return hits


def shape_b(s: str) -> list[tuple[int, str, str]]:
    """`for`: guard in the head, re-borrow in the body."""
    hits = []
    for m in re.finditer(r"\bfor\b", s):
        if s[m.end():m.end() + 1] == "<":  # for<'a>
            continue
        block = block_after(s, m.end())
        close = match_brace(s, block) if block >= 0 else -1
        if close < 0:
            continue
        head = s[m.end():block]
        if " in " not in head or len(head) > 400:
            continue
        for recv in conflicting(head.split(" in ", 1)[1], s[block + 1:close]):
            hits.append((line_of(s, m.start()), recv, "for"))
    return hits


def scan_text(raw: str, edition: int = 2021) -> list[tuple[int, str, str]]:
    """[(line, receiver, construct)] for every re-borrow in the source."""
    s = strip_comments(raw)
    return sorted(set(shape_a(s, edition) + shape_b(s)))


# ---------------------------------------------------------------- scope

def house_crates() -> list[Path]:
    vendored = set(
        re.findall(r"^\| `([a-z_0-9]+)`", (ROOT / "crates" / "VENDORED.md").read_text(), re.M)
    )
    return [
        d for d in sorted((ROOT / "crates").iterdir())
        if (d / "src").is_dir() and d.name not in vendored
    ]


def edition_of(crate: Path) -> int:
    manifest = (crate / "Cargo.toml").read_text()
    m = re.search(r'^edition\s*=\s*"(\d{4})"', manifest, re.M)
    if m:
        return int(m.group(1))
    if re.search(r"^edition\.workspace\s*=\s*true", manifest, re.M):
        m = re.search(r'^edition\s*=\s*"(\d{4})"', (ROOT / "Cargo.toml").read_text(), re.M)
        if m:
            return int(m.group(1))
    return 2015


def production_text(lines: list[str]) -> str:
    """Source with `#[cfg(test)]`-gated items blanked; offsets are kept."""
    out = list(lines)
    i = 0
    while i < len(lines):
        if audit.is_attr_line(lines[i]):
            j = i
            while j < len(lines) and audit.is_attr_line(lines[j]):
                j += 1
            if any(audit.is_test_only_cfg_attr(x) for x in lines[i:j]):
                end = audit._skip_rust_item(lines, i)
                for k in range(i, min(end, len(lines))):
                    out[k] = ""
                i = end
                continue
        i += 1
    return "\n".join(out)


def main() -> int:
    bases = house_crates()
    test_only = audit.cfg_test_mod_files(bases)
    hits = []
    for crate in bases:
        edition = edition_of(crate)
        for f in audit.rs_files(crate / "src"):
            if f.name == "tests.rs" or f in test_only:
                continue
            for line, recv, what in scan_text(production_text(f.read_text().split("\n")), edition):
                hits.append(
                    f"{audit.rel(f)}:{line}: `{what}` takes a RefCell guard of `{recv}` and its "
                    "block borrows it again — compiles, panics at run time (RefCell already "
                    "borrowed). Take in its own statement first: `let taken = cell.borrow_mut()"
                    ".take();` then the `if let`."
                )
    for h in hits:
        print(f"     {h}")
    print(("FAIL" if hits else "ok  ") + f" borrow gate: {len(hits)} hit(s)")
    return 1 if hits else 0


# ---------------------------------------------------------------- self-test

def _self_test() -> None:
    """`python3 scripts/gate_borrow.py --self-test`."""

    def flagged(src: str, edition: int = 2021) -> list[str]:
        return [recv for _, recv, _ in scan_text(src, edition)]

    # The field bug, as it stood before 0e1c0304.
    layout_run = """fn ensure_layout_run(gs: &JsState, dom: &DomTree, epoch: u64) {
    if gs.mock_cascade_done.get() && dom.len() > MOCK_CASCADE_FREEZE {
        if let Some((_, styles)) = gs.style_cache.borrow_mut().take() {
            *gs.style_cache.borrow_mut() = Some((epoch, styles));
        }
        return;
    }
}"""
    got = scan_text(layout_run)
    assert got == [(3, "gs.style_cache", "if let")], got

    must_flag = {
        # (source, receiver)
        "else arm (edition 2021)": (
            "fn f() { if let Some(v) = cell.borrow().clone() { let _ = v; } else { *cell.borrow_mut() = None; } }",
            "cell",
        ),
        "else-if condition": (
            "fn f() { if let Some(v) = cell.borrow().clone() { let _ = v; } else if cell.borrow_mut().take().is_some() {} }",
            "cell",
        ),
        "match scrutinee": (
            "fn f() { match sheet_cache.borrow().get(&id) { Some(r) => { sheet_cache.borrow_mut().insert(2, r.clone()); } None => {} } }",
            "sheet_cache",
        ),
        "while let": (
            "fn f() { while let Some(v) = cell.borrow().clone() { let _ = v; *cell.borrow_mut() = None; } }",
            "cell",
        ),
        "shared guard, mut re-borrow": (
            "fn f() { if let Some(v) = cell.borrow().as_ref() { let _ = v; cell.borrow_mut().take(); } }",
            "cell",
        ),
        "try_borrow_mut().unwrap()": (
            "fn f() { if let Some(v) = cell.try_borrow_mut().unwrap().take() { let _ = v; let _ = cell.borrow().is_some(); } }",
            "cell",
        ),
        "closure after the guard does not release it": (
            "fn f() { if let Some(v) = cell.borrow().map(|x| x * 10).filter(|x| *x > 5) { let _ = v; *cell.borrow_mut() = None; } }",
            "cell",
        ),
        "for head, mut body": (
            "fn f() { for x in list.borrow().iter() { if *x == 1 { list.borrow_mut().push(9); } } }",
            "list",
        ),
        "for head mut, shared body": (
            "fn f() { for x in list.borrow_mut().iter() { if *x == 1 { let _ = list.borrow().len(); } } }",
            "list",
        ),
        "macro body": (
            "fn f() { if let Some(v) = cell.borrow_mut().take() { let _ = v; assert_eq!(*cell.borrow_mut(), Some(1)); } }",
            "cell",
        ),
        "nested block in the body": (
            "fn f() { if let Some(v) = cell.borrow_mut().take() { let _ = v; if v { *cell.borrow_mut() = None; } } }",
            "cell",
        ),
        "closure in the body, called inside": (
            "fn f() { if let Some(v) = cell.borrow_mut().take() { let _ = v; let f = || cell.borrow_mut().take(); f(); } }",
            "cell",
        ),
        "receiver split across lines": (
            "fn f(&self) {\n"
            "    if let Some(x) = self\n"
            "        .map\n"
            "        .borrow()\n"
            "        .get(&k)\n"
            "    {\n"
            "        self.map.borrow_mut().insert(k, x);\n"
            "    }\n"
            "}",
            "self.map",
        ),
    }
    for name, (src, recv) in must_flag.items():
        got = flagged(src)
        assert got == [recv], (name, got)

    # rustfmt splits the chain; the receiver must still read the same on both sides.
    wrapped = """fn f(&self) {
    if let Some((epoch, styles)) = self
        .style_cache
        .borrow_mut()
        .take()
    {
        *self.style_cache.borrow_mut() = Some((epoch, styles));
    }
}"""
    assert flagged(wrapped) == ["self.style_cache"], "wrapped chain"

    must_pass = {
        "the fix, 0e1c0304": "fn f() { let taken = gs.style_cache.borrow_mut().take(); if let Some((_, styles)) = taken { *gs.style_cache.borrow_mut() = Some((epoch, styles)); } }",
        "ModuleMap::wake, the fix": "fn f(&self) { let waker = self.waker.borrow_mut().take(); if let Some(waker) = waker { waker.wake(); } }",
        "shared + shared": "fn f() { if let Some(v) = cell.borrow().as_ref() { let _ = v; let _ = cell.borrow().is_some(); } }",
        "closure consumes the guard": "fn f() { if cell.try_borrow().map(|g| g.is_some()).unwrap_or(false) { *cell.borrow_mut() = None; } }",
        "block in the scrutinee": "fn f() { if let Some(v) = { let g = cell.borrow(); *g } { let _ = v; *cell.borrow_mut() = None; } }",
        "bare try_borrow moved into a closure": "fn f() { if let Some(v) = cell.try_borrow_mut().ok().map(|mut g| g.take()) { let _ = v; let _ = cell.borrow().is_some(); } }",
        "other cell": "fn f() { if let Some(x) = map.borrow().get(&k) { other.borrow_mut().push(x); } }",
        "string and comment": 'fn f() { if let Some(v) = cell.borrow_mut().take() { let _ = v; let s = "cell.borrow_mut()"; /* cell.borrow() */ } }',
        "plain if condition": "fn f() { if cell.borrow().is_some() { cell.borrow_mut().take(); } }",
        "match guard": "fn f() { match true { _ if cell.borrow().is_some() => { cell.borrow_mut().take(); } _ => {} } }",
        "let else": "fn f() { let Some(v) = cell.borrow().clone() else { *cell.borrow_mut() = Some(3); return; }; let _ = v; }",
        "match arm block in the scrutinee": "fn f() { match m { Some(k) => match map.borrow().get(&k) { Some(v) => { other.borrow_mut().push(v); } None => {} } None => {} } }",
        "for, shared body": "fn f() { for x in list.borrow().iter() { if *x == 1 { let _ = list.borrow().len(); } } }",
        "for, other cell": "fn f() { for x in list.borrow().iter() { other.borrow_mut().push(*x); } }",
        "impl ... for": "fn f() { impl Iterator for X { fn f(&self) { let _ = cell.borrow(); let _ = cell.borrow_mut(); } } }",
    }
    for name, src in must_pass.items():
        got = flagged(src)
        assert got == [], (name, got)

    # edition 2024 drops the scrutinee's temporaries before the else arm
    src = "fn f() { if let Some(v) = cell.borrow().clone() { let _ = v; } else { *cell.borrow_mut() = None; } }"
    assert flagged(src, 2024) == [], "edition 2024 else arm"
    assert flagged(src, 2021) == ["cell"], "edition 2021 else arm"

    # inline #[cfg(test)] items are not production code
    test_gated = """fn f() {
    if let Some(v) = live.borrow_mut().take() {
        live.borrow_mut().take();
    }
    #[cfg(test)]
    mod tests {
        fn t() {
            if let Some(v) = dead.borrow_mut().take() {
                dead.borrow_mut().take();
            }
        }
    }
}"""
    lines = production_text(test_gated.split("\n"))
    assert "mod tests" not in lines and "fn f()" in lines, "cfg(test) masking"
    assert flagged(lines) == ["live"], flagged(lines)

    assert shape_b("fn f() { for<'a> Fn(&'a T); }") == [], "for<'a>"


if __name__ == "__main__":
    if "--self-test" in sys.argv:
        _self_test()
        print("gate_borrow self-test: ok")
        sys.exit(0)
    sys.exit(main())
