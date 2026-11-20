#!/bin/sh
# Collect the third-party licence notices that every Acta package must carry, into
# dist/licenses. scripts/package.sh and scripts/package.ps1 copy that directory into
# the package and refuse to run without it.
#
# Usage: scripts/licenses.sh [--out <dir>]        (on Windows: from Git Bash)
#
# What goes in:
#   THIRD-PARTY-RUST.txt   every crate of the dependency tree, all targets, with its
#                          licence text, written by cargo-about from Cargo.lock.
#                          Fails if a crate has a licence about.toml does not accept.
#   assets/...             every licence file found under assets/, at its own path.
#                          Found by name, not listed here: fonts and icons come and go.
#                          Fails if a font directory has none.
#   *                      the files of scripts/licenses/third-party: notices for things
#                          that are neither a crate of their own nor an asset directory
#                          (the Lucide icons, which come inside a crate).
#
# Requires cargo-about 0.9.3: `cargo install cargo-about --locked --version 0.9.3`, or
# scripts/ci/install-cargo-about.sh on Linux x86_64. Downloads the crates of every
# target that are not in the local cargo registry yet (`cargo fetch --locked`).
set -eu

CARGO_ABOUT_VERSION=0.9.3

die() {
    echo "error: $*" >&2
    exit 1
}

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"

out=dist/licenses
while [ $# -gt 0 ]; do
    case "$1" in
        --out) out=${2:?--out needs a value}; shift 2 ;;
        *) die "unknown argument: $1" ;;
    esac
done

found=$(cargo about --version 2>/dev/null | awk '{ print $2 }') || found=
[ -n "$found" ] || die "cargo-about not found; run: cargo install cargo-about --locked --version $CARGO_ABOUT_VERSION"
[ "$found" = "$CARGO_ABOUT_VERSION" ] ||
    die "cargo-about $found is installed, $CARGO_ABOUT_VERSION is the pinned version"

rm -rf "$out"
mkdir -p "$out"

echo "Rust dependency tree..."
# A build only fetches the crates of its own target; the notices cover all of them.
cargo fetch --locked
# --frozen: Cargo.lock as committed, and licence texts only from the crates themselves,
# so the result does not depend on a network service.
cargo about generate --frozen scripts/licenses/rust.hbs --output-file "$out/THIRD-PARTY-RUST.txt"
[ -s "$out/THIRD-PARTY-RUST.txt" ] || die "cargo-about wrote an empty file"

echo "Assets..."
is_license() {
    case "$(basename "$1" | tr '[:lower:]' '[:upper:]')" in
        LICENSE* | LICENCE* | OFL* | COPYING* | NOTICE*) return 0 ;;
        *) return 1 ;;
    esac
}
if [ -d assets ]; then
    find assets -type f | LC_ALL=C sort | while IFS= read -r file; do
        if is_license "$file"; then
            mkdir -p "$out/$(dirname "$file")"
            cp "$file" "$out/$file"
            echo "  $file"
        fi
    done
    for dir in assets/fonts/*/; do
        [ -d "$dir" ] || continue
        dir=${dir%/}
        if [ ! -d "$out/$dir" ]; then
            die "$dir has no licence file (LICENSE*, OFL*, COPYING* or NOTICE*); a font cannot ship without it"
        fi
    done
fi

echo "Other notices..."
for file in scripts/licenses/third-party/*; do
    [ -f "$file" ] || continue
    cp "$file" "$out/"
    echo "  $file"
done

echo "Notices in $out:"
find "$out" -type f | LC_ALL=C sort | sed 's/^/  /'
