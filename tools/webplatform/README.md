# tools/webplatform — truth data and generators for the JS platform layer

Everything the page-visible Web platform is *derived from* lives here, in the
repo, reproducible. (Before 2026-10-29 the generators and the Chrome captures
sat in the gitignored `tmp-agent/` scratch dir, so nobody else could
regenerate `global_surface.js` / `canvas_face.js`.) Design context:
`docs/JS-PLATFORM-ARCH.md` in the private repo.

## Layout

| Path | What |
|---|---|
| `chrome/` | Chrome truth captures (CDP, real Chrome) + `MANIFEST.sha256` |
| `gen/gen_globals_surface.py` | → `crates/diting/js/global_surface.js` |
| `gen/gen_canvas_face.py` | → `crates/diting/js/canvas_face.js` |
| `gen/gen_gl_tables.py` | HISTORICAL one-shot (spliced GL tables into bootstrap.js once); not run by the gate |
| `capture/chrome154_face_capture.py` | re-shoots GL / blank-canvas / audio / UA-data truth from a portable Chrome on CDP 9223 |
| `extract_spec.py` | pulls pinned-webref IDL slices into `spec/*.json` |
| `gen/gen_iface.py` | IDL slice -> `crates/diting/js/platform/gen/*.gen.js` (module skeleton + descriptor expectations) |
| `bundle.py` | `platform/core.js` + modules -> `crates/diting/js/platform_bundle.js` (topological order, layer rules) |
| `analyze/` | static dependency analysis + module-contract check + ratchet baseline |
| `chrome/*-probe.js` + `capture/chrome_probe_capture.py` | behavior probes run in real Chrome -> `chrome-*-truth.json`; the SAME probe runs in the engine: `storage-probe.js` (test `storage_matches_chrome_truth`), `error-probe.js` (error shape: stack header, shim frames, descriptors; test `platform_errors_match_chrome_truth`) |
| `check_fresh.sh` | the gate: truth unchanged vs manifest, generated files equal generator output |

## Commands

```
tools/webplatform/check_fresh.sh           # verify (CI job "webplatform generated files fresh")
tools/webplatform/check_fresh.sh --update  # regenerate artifacts + manifest after a deliberate recapture
```

## Truth files

Chrome version/platform per file, as far as the repo's own comments record it.
"Capture script" = what produced it; **none found** means the file survives
but its one-off capture script was never saved — recapturing needs a new
script (open follow-up, see below).

| File | Consumed by | Recorded provenance | Capture script |
|---|---|---|---|
| `fp-chrome-shapes.json` | gen_globals_surface | Chrome 155, window-name shapes (typeof/length/ctor message/descriptor flags) | none found |
| `fp-chrome-objflags.json` | gen_globals_surface | Chrome 155, object/bool/on* accessor detail | none found |
| `fp-chrome-winkeys-example.json` | gen_globals_surface | Chrome 155, window own-key list on an https page (key ORDER matters) | none found |
| `fp-winkeys-diff.json` | gen_globals_surface | **frozen historical input**: names the engine lacked when first generated (701); not recomputable from today's engine | derived (fp-diff era) |
| `fp-chrome-winkeys.json`, `fp-chrome.json` | comments / diff tooling | Chrome 155 macOS, 2026-10-14 | `xhs-fp-chrome-capture.py` era, not migrated |
| `xhs-chrome-blank-consts.json` | gen_canvas_face | Chrome 155 / macOS arm64, blank-canvas toDataURL, 10 sizes x png/jpeg/webp | capture/chrome154_face_capture.py |
| `chrome-gl-const-tables.json`, `chrome-gl-accessor-values.json` | bootstrap.js GL tables (artifact) | Chrome 155.0.8059.41 | capture/chrome154_face_capture.py |
| `chrome-audio-truth.json` | audio.js | Chrome 155 audio surface | capture/chrome154_face_capture.py |
| `chrome-windesc-truth-1015.json`, `conn-uad-truth-1015.json` | tests / navigator face | Chrome 155, 2026-10-15 | capture/chrome154_face_capture.py (uad part) |
| `chrome-window-on-set.json` | bootstrap.js on* list | written by gen_globals_surface.py | generator output |

**Verified vs claimed**: only `fp-chrome.json` and `chrome-gl-accessor-values.json`
carry an in-file version stamp (155.0.8059.41). For every other file "Chrome
155" is what the code comments say; the data cannot confirm it. A recapture
should write a stamp (version, platform, date, script) next to the file.

## Rules

- Never hand-edit a generated file or a truth file; the gate fails on either.
- A deliberate recapture = replace the truth file(s), run `--update`, commit the
  truth, the regenerated artifact and the new manifest together, and expect the
  surface snapshot fixture (`runtime/surface_snapshot.txt`) to show only the
  intended delta.
- `tmp-agent/` stays gitignored scratch. Anything a committed file derives from
  belongs here, not there.

## Open follow-ups (JS-PLATFORM-ARCH M0)

- Write a saved capture script for shapes / objflags / winkeys (today only the
  captured JSON survives).
- Add the webref `idlparsed` subset (pinned commit) under `spec/` once the IDL
  generator (M2) needs it; `scripts/surface_webidl_diff.py` already parses it.
