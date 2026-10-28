# Sourced by the release scripts: the ONE place asset names are derived.
# Shape: diting-{version}-{target}[-screenshot][-debug][.tar.gz|.zip][.sha256]
# {version} = tag without its v (v0.8.3 -> 0.8.3); non-tag builds are
# main-<short-sha>. Never a v prefix (2026-10-28 naming drift).
release_ver() {
  case "${GITHUB_REF:-}" in
    refs/tags/*) echo "${GITHUB_REF_NAME#v}" ;;
    *) echo "main-$(git rev-parse --short HEAD)" ;;
  esac
}
sha256_of() {
  if command -v shasum >/dev/null 2>&1; then shasum -a 256 "$1"; else sha256sum "$1"; fi
}
