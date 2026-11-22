#!/bin/sh
# Provides the Vulkan SDK pieces whisper.cpp's Vulkan backend is built with on Linux
# (ADR 0014, amendment "Vulkan in Acta.exe"): the headers and glslc, from LunarG's SDK
# at a pinned version, checked against LunarG's published SHA-256. Free; nothing of
# it ships. Ubuntu 22.04 has no glslc package, and distribution headers differ by
# release, so every Linux build uses this one. The loader's link library comes from
# `libvulkan-dev` (linux-deps.sh); Acta does not load it unless dictation uses the
# graphics card.
#
# Only include/ and bin/glslc are kept in the folder, which a workflow may
# cache (keyed on the version). With GITHUB_ENV set, later steps get VULKAN_SDK.
# Usage: scripts/ci/vulkan-sdk.sh <folder>
set -eu

version=1.4.350.0
sha256=b65f068ab36263559da49d7cacd7e7b9df23824ca8b68ccc522a2b06f5725df2

dir=${1:?usage: vulkan-sdk.sh <folder>}
if [ ! -x "$dir/bin/glslc" ]; then
    file="vulkansdk-linux-x86_64-$version.tar.xz"
    tmp=$(mktemp -d)
    echo "Downloading $file"
    curl -fsSL --retry 3 -o "$tmp/$file" "https://sdk.lunarg.com/sdk/download/$version/linux/$file"
    echo "$sha256  $tmp/$file" | sha256sum -c -
    mkdir -p "$dir"
    tar -xJf "$tmp/$file" -C "$dir" --strip-components=2 --wildcards \
        "$version/x86_64/include/*" "$version/x86_64/bin/glslc"
    rm -rf "$tmp"
fi

for need in include/vulkan/vulkan.hpp bin/glslc; do
    [ -e "$dir/$need" ] || { echo "::error::no $need in $dir"; exit 1; }
done
"$dir/bin/glslc" --version | head -n 1
echo "VULKAN_SDK=$dir"
if [ -n "${GITHUB_ENV:-}" ]; then
    echo "VULKAN_SDK=$dir" >> "$GITHUB_ENV"
fi
