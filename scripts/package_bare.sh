#!/usr/bin/env bash
# Bare static asset (agpkg convention) + symbol-bearing -debug twin + sha256s.
# usage: package_bare.sh <target> <variant-suffix-or-empty> <unstripped-binary>
# Prints the shipped binary's name as `bin=...` (append to $GITHUB_ENV).
set -euo pipefail
. "$(dirname "$0")/release_lib.sh"
target=$1; suffix=$2; src=$3
bin=diting-$(release_ver)-$target$suffix
cp "$src" "$bin"
cp "$bin" "$bin-debug"
strip "$bin"
sha256_of "$bin" > "$bin.sha256"
sha256_of "$bin-debug" > "$bin-debug.sha256"
echo "bin=$bin"
