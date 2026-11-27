#!/usr/bin/env python3
"""Tracked-asset boundary gate.

GitHub's language bar made the visible symptom obvious, but the enforced rule
is about repository custody, not presentation: any tracked file large enough
to matter must be explicitly classified and owned in tools/asset-ledger.json.
Files may shrink or leave the ledger; they may not grow silently.
"""
from __future__ import annotations

import hashlib
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LEDGER_PATH = ROOT / "tools/asset-ledger.json"


def tracked_files() -> list[str]:
    raw = subprocess.check_output(["git", "ls-files", "-z"], cwd=ROOT)
    return [p.decode() for p in raw.split(b"\0") if p]


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def main() -> int:
    ledger = json.loads(LEDGER_PATH.read_text())
    hard_limit = int(ledger["hard_limit_bytes"])
    threshold = int(ledger["register_threshold_bytes"])
    entries = {e["path"]: e for e in ledger["entries"]}
    tracked = set(tracked_files())
    errors: list[str] = []

    for path in sorted(tracked):
        p = ROOT / path
        if not p.is_file():
            continue
        size = p.stat().st_size
        ent = entries.get(path)
        if size >= hard_limit:
            if not ent or not ent.get("grandfathered_hard_cap"):
                errors.append(f"{path}: {size} bytes crosses the {hard_limit}-byte hard cap")
                continue
        if size >= threshold and ent is None:
            errors.append(f"{path}: {size} bytes is tracked without an asset-ledger entry")
            continue
        if ent is None:
            continue
        if size > int(ent["max_bytes"]):
            errors.append(f"{path}: grew from ledger max {ent['max_bytes']} to {size} bytes")
        want_hash = ent.get("sha256")
        if want_hash and size == int(ent["max_bytes"]):
            got = sha256(p)
            if got != want_hash:
                errors.append(f"{path}: sha256 changed without ledger update ({got})")

    stale = sorted(set(entries) - tracked)
    for path in stale:
        errors.append(f"{path}: ledger entry is stale; remove it when the file leaves the repo")

    if errors:
        print("asset gate failed:", file=sys.stderr)
        for e in errors:
            print(f"  - {e}", file=sys.stderr)
        return 1
    print(f"asset gate ok: {len(entries)} registered assets, threshold {threshold} bytes")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
