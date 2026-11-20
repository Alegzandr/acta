#!/bin/sh
# System packages needed to build Acta on Ubuntu (22.04 and later), and optionally
# to package it and to run the GUI headless. Used by CI; also the reference list
# for building from source. This is the only script here that uses sudo.
# Usage: scripts/ci/linux-deps.sh [--package] [--headless]
set -eu

# Build: GPUI links fontconfig, wayland, xkbcommon and xcb; Vulkan is loaded at run time.
packages="libfontconfig-dev libwayland-dev libxkbcommon-x11-dev libx11-xcb-dev libvulkan1"
for arg in "$@"; do
    case "$arg" in
        # vpk builds the AppImage with mksquashfs.
        --package) packages="$packages squashfs-tools" ;;
        # Xvfb and Mesa's software Vulkan driver (lavapipe) for smoke runs without a GPU.
        --headless) packages="$packages xvfb mesa-vulkan-drivers" ;;
        *) echo "error: unknown option $arg" >&2; exit 1 ;;
    esac
done

sudo apt-get update -q
# shellcheck disable=SC2086 # word splitting of the package list is intended
sudo apt-get install -y -q --no-install-recommends $packages
