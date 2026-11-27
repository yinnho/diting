#!/usr/bin/env python3
"""Validate the zstd-block font-metrics artifact end to end.

This is deliberately stronger than a checksum-only check: every frame is
inflated, every indexed entry is sliced from its inflated block, blob hashes
must match, and the canonical semantic digest is recomputed over
(table, key, exact raw JSON bytes).
"""
from __future__ import annotations

import hashlib
import json
import struct
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
JS = ROOT / 'crates/diting_js/js'


def sha(b: bytes) -> str:
    return hashlib.sha256(b).hexdigest()


def inflate(frame: bytes) -> bytes:
    return subprocess.run(['zstd', '-dc', '-q', '-c'], input=frame,
                          stdout=subprocess.PIPE, check=True).stdout


def semantic_digest(items) -> str:
    h = hashlib.sha256()
    h.update(b'font-metrics-semantics-v2\0')
    by_table = {}
    for tbl, key, raw in items:
        by_table.setdefault(tbl, []).append((key, raw))
    for tbl in sorted(by_table):
        tb = tbl.encode('utf8')
        h.update(struct.pack('>Q', len(tb)))
        h.update(tb)
        for key, raw in sorted(by_table[tbl]):
            kb = key.encode('utf8')
            h.update(struct.pack('>Q', len(kb)))
            h.update(kb)
            h.update(struct.pack('>Q', len(raw)))
            h.update(raw)
    return h.hexdigest()


def main() -> int:
    index_path = JS / 'font_metrics_index.json'
    data_path = JS / 'font_metrics_data.zst'
    ix = json.loads(index_path.read_text())
    blob = data_path.read_bytes()
    errors = []
    if ix.get('format') != 'font-metrics-index-v2':
        errors.append('format is not font-metrics-index-v2')
    if ix.get('codec') != 'zstd-frame-concat-v1':
        errors.append('codec is not zstd-frame-concat-v1')
    if ix.get('data_size') != len(blob):
        errors.append(f"data_size {ix.get('data_size')} != blob {len(blob)}")
    if ix.get('data_sha256') != sha(blob):
        errors.append('data_sha256 mismatch')

    blocks = []
    pos = 0
    for i, bm in enumerate(ix.get('blocks', [])):
        if bm['off'] != pos:
            errors.append(f'block {i}: non-contiguous offset')
        frame = blob[pos:pos + bm['clen']]
        try:
            raw = inflate(frame)
        except subprocess.CalledProcessError as e:
            errors.append(f'block {i}: zstd inflate failed: {e}')
            break
        if len(raw) != bm['ulen']:
            errors.append(f'block {i}: ulen mismatch')
        if sha(raw) != bm['sha256']:
            errors.append(f'block {i}: sha256 mismatch')
        blocks.append(raw)
        pos += bm['clen']
    if pos != len(blob):
        errors.append(f'blocks end at {pos}, blob is {len(blob)}')

    items = []
    if len(blocks) == len(ix.get('blocks', [])):
        for tbl, keys in ix.get('tables', {}).items():
            for key, ent in keys.items():
                try:
                    raw = blocks[ent['b']][ent['o']:ent['o'] + ent['n']]
                except IndexError:
                    errors.append(f'{tbl}/{key}: missing block')
                    continue
                if len(raw) != ent['n'] or not raw:
                    errors.append(f'{tbl}/{key}: bad entry slice')
                    continue
                try:
                    json.loads(raw)
                except Exception as e:
                    errors.append(f'{tbl}/{key}: entry is not valid JSON: {e}')
                    continue
                items.append((tbl, key, raw))
        sem = semantic_digest(items)
        if sem != ix.get('semantic_sha256'):
            errors.append(f'semantic digest mismatch: {sem}')

    if errors:
        print('font artifact invalid:', file=sys.stderr)
        for e in errors[:40]:
            print('  - ' + e, file=sys.stderr)
        if len(errors) > 40:
            print(f'  ... {len(errors) - 40} more', file=sys.stderr)
        return 1
    entries = sum(len(v) for v in ix['tables'].values())
    print(f'font artifact ok: {len(blocks)} blocks, {entries} entries, semantic {ix["semantic_sha256"][:16]}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
