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
# window_order.js follows the persona major: its truth is winorder-truth-<major>.json, so a bump that forgot to
# recapture the window own-name order fails here (missing file) instead of silently keeping the old order (#28:
# the 145 order hid every 146..157 interface as engine-internal - FontFaceSet at 11, requestResize non-enumerable).
MAJ=$(sed -n 's/^major *= *\([0-9]*\).*/\1/p' crates/diting_env/chrome-target.toml)
WINORDER=$WP/chrome/winorder-truth-$MAJ.json

if [ "${1:-}" = "--update" ]; then
  python3 $WP/gen/gen_globals_surface.py
  python3 $WP/gen/gen_canvas_face.py
  python3 $WP/gen/gen_window_order.py "$WINORDER"
  python3 $WP/gen/gen_iface.py $WP/spec/storage.json crates/diting_js/js/platform/gen/storage.gen.js gen/storage
  python3 $WP/gen/gen_iface.py $WP/spec/geometry.json crates/diting_js/js/platform/gen/geometry.gen.js gen/geometry $WP/chrome/geometry-truth-145.json
  python3 $WP/gen/gen_iface.py $WP/spec/css-typed-om.json crates/diting_js/js/platform/gen/cssunit.gen.js gen/cssunit $WP/chrome/cssunit-truth-157.json
  python3 $WP/gen/gen_iface.py $WP/spec/css-animations.json crates/diting_js/js/platform/gen/animationevent.gen.js gen/animationevent
  python3 $WP/gen/gen_iface.py $WP/spec/webvtt.json crates/diting_js/js/platform/gen/vttcue.gen.js gen/vttcue $WP/chrome/vttcue-truth-157.json
  python3 $WP/gen/gen_iface.py $WP/spec/htmlelement.json crates/diting_js/js/platform/gen/htmlelement.gen.js gen/htmlelement $WP/chrome/htmlelement-truth-145.json patch-common
  python3 $WP/gen/gen_iface.py $WP/spec/htmlmedia.json crates/diting_js/js/platform/gen/htmlmedia.gen.js gen/htmlmedia $WP/chrome/htmlmedia-truth-145.json patch-media
  python3 $WP/gen/gen_iface.py $WP/spec/htmlmedia.json crates/diting_js/js/platform/gen/htmlreflect.gen.js gen/htmlreflect $WP/chrome/htmlreflect-truth-145.json patch-reflect
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
python3 $WP/gen/gen_window_order.py "$WINORDER" "$tmp/window_order.js" >/dev/null \
  || { echo "FAIL: $WINORDER missing or unreadable - recapture Object.getOwnPropertyNames(window) on Chrome $MAJ" >&2; fail=1; }
for f in global_surface.js canvas_face.js window_order.js; do
  if ! diff -q "$tmp/$f" "crates/diting_js/js/$f" >/dev/null; then
    echo "FAIL: crates/diting_js/js/$f is stale vs its generator" >&2; fail=1
  fi
done
# 3. IDL-generated module skeletons, then the module bundle built from them
python3 $WP/gen/gen_iface.py $WP/spec/storage.json "$tmp/storage.gen.js" gen/storage >/dev/null
diff -q "$tmp/storage.gen.js" crates/diting_js/js/platform/gen/storage.gen.js >/dev/null \
  || { echo "FAIL: crates/diting_js/js/platform/gen/storage.gen.js is stale vs gen_iface.py" >&2; fail=1; }
# 3a. the value-strategy modules (geometry, cssunit) carry their own truth
# files; cssunit is the issue #67 gen-family pilot (ctor conversion preamble).
gen_value() { # <spec> <name> <truth>
  python3 $WP/gen/gen_iface.py "$WP/spec/$1" "$tmp/$2.gen.js" "gen/$2" "$WP/chrome/$3" >/dev/null
  diff -q "$tmp/$2.gen.js" "crates/diting_js/js/platform/gen/$2.gen.js" >/dev/null \
    || { echo "FAIL: crates/diting_js/js/platform/gen/$2.gen.js is stale vs gen_iface.py" >&2; fail=1; }
}
gen_value geometry.json geometry geometry-truth-145.json
gen_value css-typed-om.json cssunit cssunit-truth-157.json
# vttcue: the #67 marginal-cost family, generated with no generator change.
gen_value webvtt.json vttcue vttcue-truth-157.json
python3 $WP/gen/gen_iface.py $WP/spec/css-animations.json "$tmp/animationevent.gen.js" gen/animationevent >/dev/null
diff -q "$tmp/animationevent.gen.js" crates/diting_js/js/platform/gen/animationevent.gen.js >/dev/null \
  || { echo "FAIL: crates/diting_js/js/platform/gen/animationevent.gen.js is stale vs gen_iface.py" >&2; fail=1; }
# 3b. the patch-strategy modules (HTMLElement common layer, media/image family,
# HTML reflect family) are measured-truth products too: regenerate and diff
gen_patch() { # <spec> <name> <truth> <strategy>
  python3 $WP/gen/gen_iface.py "$WP/spec/$1" "$tmp/$2.gen.js" "gen/$2" "$WP/chrome/$3" "$4" >/dev/null
  diff -q "$tmp/$2.gen.js" "crates/diting_js/js/platform/gen/$2.gen.js" >/dev/null \
    || { echo "FAIL: crates/diting_js/js/platform/gen/$2.gen.js is stale vs gen_iface.py ($4)" >&2; fail=1; }
}
gen_patch htmlelement.json htmlelement htmlelement-truth-145.json patch-common
gen_patch htmlmedia.json htmlmedia htmlmedia-truth-145.json patch-media
gen_patch htmlmedia.json htmlreflect htmlreflect-truth-145.json patch-reflect
python3 $WP/bundle.py "$tmp/platform_bundle.js" >/dev/null
diff -q "$tmp/platform_bundle.js" crates/diting_js/js/platform_bundle.js >/dev/null \
  || { echo "FAIL: crates/diting_js/js/platform_bundle.js is stale vs bundle.py" >&2; fail=1; }
# 4. every Chrome full version we can claim really shipped; all layers follow the one target
python3 tools/apply_chrome_target.py --check >/dev/null && python3 tools/check_coherence.py >/dev/null || { python3 tools/check_coherence.py >&2; fail=1; }
python3 $WP/check_versions.py >/dev/null || { python3 $WP/check_versions.py >&2; fail=1; }
[ "$fail" = 0 ] && echo "webplatform generated files fresh (8 artifacts, $(ls $WP/chrome/*.json | wc -l | tr -d ' ') truth files)"
exit $fail
