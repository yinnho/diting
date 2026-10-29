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
  python3 $WP/gen/gen_iface.py $WP/spec/storage.json crates/diting/js/platform/gen/storage.gen.js gen/storage
  python3 $WP/gen/gen_iface.py $WP/spec/htmlelement.json crates/diting/js/platform/gen/htmlelement.gen.js gen/htmlelement $WP/chrome/htmlelement-truth-145.json patch-common
  python3 $WP/gen/gen_iface.py $WP/spec/htmlmedia.json crates/diting/js/platform/gen/htmlmedia.gen.js gen/htmlmedia $WP/chrome/htmlmedia-truth-145.json patch-media
  python3 $WP/gen/gen_iface.py $WP/spec/htmlmedia.json crates/diting/js/platform/gen/htmlreflect.gen.js gen/htmlreflect $WP/chrome/htmlreflect-truth-145.json patch-reflect
  python3 $WP/bundle.py
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
# 3. IDL-generated module skeletons, then the module bundle built from them
python3 $WP/gen/gen_iface.py $WP/spec/storage.json "$tmp/storage.gen.js" gen/storage >/dev/null
diff -q "$tmp/storage.gen.js" crates/diting/js/platform/gen/storage.gen.js >/dev/null \
  || { echo "FAIL: crates/diting/js/platform/gen/storage.gen.js is stale vs gen_iface.py" >&2; fail=1; }
# 3b. the patch-strategy modules (HTMLElement common layer, media/image family,
# HTML reflect family) are measured-truth products too: regenerate and diff
gen_patch() { # <spec> <name> <truth> <strategy>
  python3 $WP/gen/gen_iface.py "$WP/spec/$1" "$tmp/$2.gen.js" "gen/$2" "$WP/chrome/$3" "$4" >/dev/null
  diff -q "$tmp/$2.gen.js" "crates/diting/js/platform/gen/$2.gen.js" >/dev/null \
    || { echo "FAIL: crates/diting/js/platform/gen/$2.gen.js is stale vs gen_iface.py ($4)" >&2; fail=1; }
}
gen_patch htmlelement.json htmlelement htmlelement-truth-145.json patch-common
gen_patch htmlmedia.json htmlmedia htmlmedia-truth-145.json patch-media
gen_patch htmlmedia.json htmlreflect htmlreflect-truth-145.json patch-reflect
python3 $WP/bundle.py "$tmp/platform_bundle.js" >/dev/null
diff -q "$tmp/platform_bundle.js" crates/diting/js/platform_bundle.js >/dev/null \
  || { echo "FAIL: crates/diting/js/platform_bundle.js is stale vs bundle.py" >&2; fail=1; }
# 4. every Chrome full version we can claim really shipped; all layers follow the one target
python3 tools/apply_chrome_target.py --check >/dev/null && python3 tools/check_coherence.py >/dev/null || { python3 tools/check_coherence.py >&2; fail=1; }
python3 $WP/check_versions.py >/dev/null || { python3 $WP/check_versions.py >&2; fail=1; }
[ "$fail" = 0 ] && echo "webplatform generated files fresh (7 artifacts, $(ls $WP/chrome/*.json | wc -l | tr -d ' ') truth files)"
exit $fail
