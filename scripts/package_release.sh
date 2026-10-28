#!/usr/bin/env bash
# Archive asset: diting-{ver}-{target}.tar.gz (+ .zip for winget) + .sha256.
# usage: package_release.sh <target> <binary-path>
set -euo pipefail
. "$(dirname "$0")/release_lib.sh"
target=$1; binpath=$2
dir=diting-$(release_ver)-$target
mkdir "$dir"
cp "$binpath" "$dir/"
cp README.md docs/install.md "$dir/"
# The workflow samples ride along as files — the engine binary carries no
# flows inside the exe (browser stays out of business logic). Repo layout is
# preserved so the flow.md paths stay valid, and md_to_args.py + templates/
# are caller-side tooling for wechat-oa-post either way.
cp -R workflow "$dir/"
cp -R config "$dir/"
tar czf "$dir.tar.gz" "$dir"
# winget requires a zip (no tar.gz installers) — same contents.
if command -v zip >/dev/null 2>&1; then zip -qr "$dir.zip" "$dir"; fi
sha256_of "$dir.tar.gz" > "$dir.tar.gz.sha256"
