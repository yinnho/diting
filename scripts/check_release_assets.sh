#!/usr/bin/env bash
# Gate: every diting-* file in the cwd must have an isomorphic name.
#   diting-{version}-{target}[-screenshot][-debug][.tar.gz|.zip][.sha256]
# Fails the job BEFORE upload, so a drifted name never reaches a release.
set -euo pipefail
re='^diting-(main-[0-9a-f]+|[0-9]+\.[0-9]+\.[0-9]+(-[A-Za-z0-9.]+)?)-(aarch64-apple-darwin|x86_64-apple-darwin|x86_64-unknown-linux-gnu|x86_64-pc-windows-msvc|aarch64-unknown-linux-musl)(-screenshot)?(-debug)?(\.tar\.gz|\.zip)?(\.sha256)?$'
n=0; bad=0
for f in diting-*; do
  [ -f "$f" ] || continue
  n=$((n+1))
  if ! [[ $f =~ $re ]]; then echo "BAD ASSET NAME: $f" >&2; bad=1; fi
done
[ "$n" -gt 0 ] || { echo "no diting-* assets found" >&2; exit 1; }
[ "$bad" = 0 ] && echo "asset names OK ($n files)"
exit "$bad"
