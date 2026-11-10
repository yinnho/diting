#!/usr/bin/env bash
# Gate: cargo fmt --check over every workspace member EXCEPT the vendored
# crates listed in crates/VENDORED.md (#64). The vendor trees keep upstream
# byte-shape on purpose — reformating them would wreck the per-file diff3
# sync campaigns (taffy family). The exempt list is read from the table
# itself, so a crate vendored tomorrow is exempt the moment its row lands,
# and a new house crate is gated the moment it joins the workspace.
set -euo pipefail
cd "$(dirname "$0")/.."

VENDORED="$(grep -oE '^\| `diting[a-z_0-9]*`' crates/VENDORED.md | tr -d '|` ' | tr '\n' ' ')"

FAILED=0
while read -r member; do
  case " $VENDORED " in *" $member "*) continue ;; esac
  # check output (a full diff) goes to stdout, not stderr — capture it and
  # only re-show it when the gate fails and GATE_FMT_VERBOSE is set.
  if ! out="$(cargo fmt --check -p "$member" 2>/dev/null)"; then
    echo "FAIL fmt: $member"
    if [ -n "${GATE_FMT_VERBOSE:-}" ]; then printf '%s\n' "$out"; fi
    FAILED=1
  fi
done < <(cargo metadata --no-deps --format-version 1 \
  | python3 -c 'import json,sys
for p in json.load(sys.stdin)["packages"]:
    print(p["name"])' | sort)

if [ "$FAILED" -ne 0 ]; then
  echo "fmt gate: dirty crates above. Fix with: cargo fmt -p <crate>"
  exit 1
fi
echo "fmt gate: clean (${VENDORED} exempt as vendored)"
