#!/usr/bin/env bash
# Archive asset: diting-{ver}-{target}.tar.gz + .sha256 (Windows also .zip).
# usage: package_release.sh <target> <binary-path>
set -euo pipefail
. "$(dirname "$0")/release_lib.sh"
target=$1; binpath=$2
dir=diting-$(release_ver)-$target
mkdir "$dir"
cp "$binpath" "$dir/"
cp README.md docs/install.md "$dir/"
# The readable copy of the manual; the engine writes the compiled-in one to
# its workflow directory at boot.
cp -R workflow "$dir/"
cp -R config "$dir/"
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
