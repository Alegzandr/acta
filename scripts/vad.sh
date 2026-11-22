#!/bin/sh
# The Silero VAD shipped in every package (ADR 0014, 4): assets/voice/<file>, checked
# against the size and SHA-256 pinned in crates/acta-core/src/voice/models.rs.
#
# Usage: scripts/vad.sh [--fetch]
#   (no option)  check the file that is there; fail when it is missing or differs
#   --fetch      download it from Hugging Face first when it is missing (HTTPS only)
#
# A file that does not match is deleted, never shipped. crates/acta-app/build.rs checks
# the same file again when the app is built, and the packaging scripts check it before
# copying it next to the executable.
set -eu

die() {
    echo "error: $*" >&2
    exit 1
}

fetch=0
case "${1:-}" in
    --fetch) fetch=1 ;;
    '') ;;
    *) die "unknown option $1" ;;
esac

root=$(cd "$(dirname "$0")/.." && pwd)
pin=$(sh "$root/scripts/model-pins.sh" | awk '$1 == "SILERO_VAD"')
[ -n "$pin" ] || die "SILERO_VAD is not pinned in models.rs"
# shellcheck disable=SC2086 # five words by construction
set -- $pin
file=$2 size=$3 sha=$4 repo=$5
dest=$root/assets/voice/$file

sha256_of() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d ' ' -f 1
    else
        shasum -a 256 "$1" | cut -d ' ' -f 1
    fi
}

if [ ! -f "$dest" ]; then
    [ "$fetch" = 1 ] || die "$dest is missing; run scripts/vad.sh --fetch"
    mkdir -p "$(dirname "$dest")"
    url=https://huggingface.co/$repo/resolve/main/$file
    echo "Downloading $url"
    # The resolver redirects to Hugging Face's CDN: redirects stay on HTTPS.
    curl -fsSL --proto '=https' --proto-redir '=https' --tlsv1.2 --retry 3 \
        -o "$dest.part" "$url" || { rm -f "$dest.part"; die "could not download $file"; }
    mv "$dest.part" "$dest"
fi

actual_size=$(wc -c <"$dest" | tr -d ' ')
actual_sha=$(sha256_of "$dest")
if [ "$actual_size" != "$size" ] || [ "$actual_sha" != "$sha" ]; then
    rm -f "$dest"
    die "$file is $actual_size bytes with SHA-256 $actual_sha; models.rs pins $size bytes, $sha (deleted)"
fi
echo "ok: $file, $size bytes, SHA-256 $sha"
