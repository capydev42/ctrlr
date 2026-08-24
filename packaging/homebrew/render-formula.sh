#!/usr/bin/env bash
# Render the Homebrew formula for a released version.
#
#   render-formula.sh <tag-or-version> <checksums.txt> [template] > ctrlr.rb
#
# The checksums file is the one every release ships: "<sha256>  <asset>" lines,
# produced by the "Generate checksums" step in .github/workflows/release.yml.
# A missing asset is a hard error — a formula with a stale sha256 installs a
# binary from the previous release without saying so.
set -euo pipefail

version="${1#v}"
checksums="$2"
template="${3:-$(dirname "$0")/ctrlr.rb.tmpl}"

sha_for() {
    local asset="$1" sha
    sha="$(awk -v a="$asset" '$2 == a { print $1 }' "$checksums")"
    if [[ -z "$sha" ]]; then
        echo "error: no checksum for $asset in $checksums" >&2
        exit 1
    fi
    printf '%s' "$sha"
}

# Resolved into variables first, not inline in the sed arguments: an `exit 1`
# inside a command substitution only leaves the subshell, so a missing asset
# would otherwise render a formula with an empty sha256 and exit 0.
sha_macos_arm="$(sha_for ctrlr-aarch64-apple-darwin.tar.gz)"
sha_macos_intel="$(sha_for ctrlr-x86_64-apple-darwin.tar.gz)"
sha_linux_arm="$(sha_for ctrlr-aarch64-unknown-linux-gnu.tar.gz)"
sha_linux_intel="$(sha_for ctrlr-x86_64-unknown-linux-gnu.tar.gz)"

sed \
    -e "s|@VERSION@|${version}|g" \
    -e "s|@SHA_MACOS_ARM@|${sha_macos_arm}|g" \
    -e "s|@SHA_MACOS_INTEL@|${sha_macos_intel}|g" \
    -e "s|@SHA_LINUX_ARM@|${sha_linux_arm}|g" \
    -e "s|@SHA_LINUX_INTEL@|${sha_linux_intel}|g" \
    "$template"
