#!/bin/sh
# Install the pinned cargo-about release binary into ~/.cargo/bin on a Linux x86_64
# runner, after checking its SHA-256. A prebuilt binary takes seconds; compiling it with
# `cargo install` takes minutes on every run. The version must equal
# CARGO_ABOUT_VERSION in scripts/licenses.sh; bump both and the checksum together (the
# checksum is the `.sha256` file published next to the archive).
set -eu

VERSION=0.9.3
SHA256=3ad1e94ea1e63d69d1961f4fb15ccbbe9575d906111f96d2cb4dce6df48f71b4

if [ "$(uname -s)-$(uname -m)" != Linux-x86_64 ]; then
    echo "error: this script installs the Linux x86_64 binary only; elsewhere run:" >&2
    echo "  cargo install cargo-about --locked --version $VERSION" >&2
    exit 1
fi

name=cargo-about-$VERSION-x86_64-unknown-linux-musl
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

curl --fail --silent --show-error --location --proto '=https' --tlsv1.2 --retry 3 \
    --output "$tmp/$name.tar.gz" \
    "https://github.com/EmbarkStudios/cargo-about/releases/download/$VERSION/$name.tar.gz"
echo "$SHA256  $tmp/$name.tar.gz" | sha256sum --check --quiet -
tar -xzf "$tmp/$name.tar.gz" -C "$tmp"
mkdir -p "$HOME/.cargo/bin"
install -m 755 "$tmp/$name/cargo-about" "$HOME/.cargo/bin/cargo-about"
cargo about --version
