#!/usr/bin/env bash
# Freshness gate (JS-PLATFORM-ARCH §3.5/M0): the checked-in generated JS must be
# exactly what the checked-in generators produce from the checked-in Chrome
# truth, and the truth files must not have been edited silently.
#   tools/webplatform/check_fresh.sh           # verify
#   tools/webplatform/check_fresh.sh --update  # regenerate artifacts + manifest
set -euo pipefail
cd "$(dirname "$0")/../.."
WP=tools/webplatform
sha() { if command -v shasum >/dev/null 2>&1; then shasum -a 256 "$@"; else sha256sum "$@"; fi; }

if [ "${1:-}" = "--update" ]; then
  python3 $WP/gen/gen_globals_surface.py
  python3 $WP/gen/gen_canvas_face.py
  (cd $WP/chrome && sha *.json > MANIFEST.sha256)
  echo "updated"; exit 0
fi

fail=0
# 1. truth files unchanged vs manifest (re-capturing is a deliberate --update)
(cd $WP/chrome && sha -c MANIFEST.sha256 >/dev/null) || { echo "FAIL: chrome truth differs from MANIFEST.sha256 (run --update if the recapture is deliberate)" >&2; fail=1; }
# 2. regenerate into a temp dir and diff against the checked-in artifacts
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
python3 $WP/gen/gen_globals_surface.py "$tmp" >/dev/null
python3 $WP/gen/gen_canvas_face.py "$tmp" >/dev/null
for f in global_surface.js canvas_face.js; do
  if ! diff -q "$tmp/$f" "crates/diting/js/$f" >/dev/null; then
    echo "FAIL: crates/diting/js/$f is stale vs its generator" >&2; fail=1
  fi
done
[ "$fail" = 0 ] && echo "webplatform generated files fresh (2 artifacts, $(ls $WP/chrome/*.json | wc -l | tr -d ' ') truth files)"
exit $fail
