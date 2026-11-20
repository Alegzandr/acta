#!/bin/sh
# Install the toolchain pinned in rust-toolchain.toml with rustup (preinstalled on
# GitHub runners), plus rustfmt, clippy and any extra targets given as arguments.
# Usage: scripts/ci/setup-rust.sh [target-triple...]
set -eu
cd "$(dirname "$0")/../.."

channel=$(sed -n 's/^[[:space:]]*channel[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' rust-toolchain.toml)
if [ -z "$channel" ]; then
    echo "error: no channel found in rust-toolchain.toml" >&2
    exit 1
fi

rustup toolchain install "$channel" --profile minimal --no-self-update \
    --component rustfmt --component clippy
for target in "$@"; do
    rustup target add --toolchain "$channel" "$target"
done
rustc --version
cargo --version
