#!/usr/bin/env bash
# Archive asset: diting-{ver}-{target}.tar.gz + .sha256 (Windows also .zip).
# usage: package_release.sh <target> <binary-path>
set -euo pipefail
. "$(dirname "$0")/release_lib.sh"
target=$1; binpath=$2
dir=diting-$(release_ver)-$target
mkdir "$dir"
cp "$binpath" "$dir/"
# Only what the software reads: the binary and its runtime config data.
# No docs — the workflow manual is compiled in and written to the workflow
# directory at boot; guides live on GitHub (README / SKILL.md).
mkdir "$dir/config"
cp config/*.json "$dir/config/"
tar czf "$dir.tar.gz" "$dir"
sha256_of "$dir.tar.gz" > "$dir.tar.gz.sha256"
# Windows hands get a zip too — same contents.
case "$target" in
  *-windows-*)
    if command -v zip >/dev/null 2>&1; then
      zip -qr "$dir.zip" "$dir"
    else
      7z a -bso0 "$dir.zip" "$dir"
    fi
    sha256_of "$dir.zip" > "$dir.zip.sha256"
    ;;
esac
