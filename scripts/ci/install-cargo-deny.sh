#!/bin/sh
# Install the pinned cargo-deny release binary into ~/.cargo/bin on a Linux x86_64
# runner, after checking its SHA-256. A prebuilt binary takes seconds; compiling it with
# `cargo install` takes minutes on every run. Bump the version and the checksum
# together (the checksum is the `.sha256` file published next to the archive).
# Dependabot does not bump this pin; pick a release that is a few weeks old.
set -eu

VERSION=0.20.2
SHA256=9f12ed4c49936e09b48bf862b595cde2fe64fcbd9d74dfacac6131ca824c8d5f

if [ "$(uname -s)-$(uname -m)" != Linux-x86_64 ]; then
    echo "error: this script installs the Linux x86_64 binary only; elsewhere run:" >&2
    echo "  cargo install cargo-deny --locked --version $VERSION" >&2
    exit 1
fi

name=cargo-deny-$VERSION-x86_64-unknown-linux-musl
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

curl --fail --silent --show-error --location --proto '=https' --tlsv1.2 --retry 3 \
    --output "$tmp/$name.tar.gz" \
    "https://github.com/EmbarkStudios/cargo-deny/releases/download/$VERSION/$name.tar.gz"
echo "$SHA256  $tmp/$name.tar.gz" | sha256sum --check --quiet -
tar -xzf "$tmp/$name.tar.gz" -C "$tmp"
mkdir -p "$HOME/.cargo/bin"
install -m 755 "$tmp/$name/cargo-deny" "$HOME/.cargo/bin/cargo-deny"
cargo deny --version
