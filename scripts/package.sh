#!/bin/sh
# Stage the release binaries and build the Velopack packages for macOS or Linux.
# The release workflow calls this script, so CI and local packaging cannot drift.
# The Windows counterpart is scripts/package.ps1.
#
# Usage: scripts/package.sh [--channel osx-arm64|osx-x64|linux-x64] [--target <triple>]
#                           [--bin-dir <dir>] [--version <x.y.z>] [--out <dir>]
#                           [--fetch-previous]
#
#   --channel         default: detected from this machine
#   --target          Rust target triple the binaries were built for; binaries are then
#                     read from target/<triple>/release instead of target/release
#   --bin-dir         directory holding Acta and acta-cli (overrides --target)
#   --version         default: the workspace version from `cargo metadata`
#   --out             default: releases/<channel>
#   --fetch-previous  download the latest published release of the channel first, so
#                     that vpk also emits a delta package (set VPK_TOKEN to avoid the
#                     anonymous GitHub API rate limit)
#
# Requires: the binaries already built (`cargo build --release --locked`), the licence
# notices collected in dist/licenses (`scripts/licenses.sh`), .NET SDK >= 8
# and vpk 1.2.161 (`dotnet tool install -g vpk --version 1.2.161`). macOS packages can
# only be built on macOS (codesign, ditto). Linux needs mksquashfs (squashfs-tools).
set -eu

PACK_ID=Acta
BUNDLE_ID=io.github.alegzandr.acta
REPO_URL=https://github.com/Alegzandr/acta

die() {
    echo "error: $*" >&2
    exit 1
}

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"

channel=
target=
bin_dir=
version=
out=
fetch_previous=0
while [ $# -gt 0 ]; do
    case "$1" in
        --channel) channel=${2:?--channel needs a value}; shift 2 ;;
        --target) target=${2:?--target needs a value}; shift 2 ;;
        --bin-dir) bin_dir=${2:?--bin-dir needs a value}; shift 2 ;;
        --version) version=${2:?--version needs a value}; shift 2 ;;
        --out) out=${2:?--out needs a value}; shift 2 ;;
        --fetch-previous) fetch_previous=1; shift ;;
        *) die "unknown argument: $1" ;;
    esac
done

os=$(uname -s)
if [ -z "$channel" ]; then
    case "$os-$(uname -m)" in
        Darwin-arm64) channel=osx-arm64 ;;
        Darwin-x86_64) channel=osx-x64 ;;
        Linux-x86_64) channel=linux-x64 ;;
        *) die "no channel for $os $(uname -m); pass --channel" ;;
    esac
fi
case "$channel" in
    osx-arm64) want_os=Darwin; want_arch=arm64 ;;
    osx-x64) want_os=Darwin; want_arch=x86_64 ;;
    linux-x64) want_os=Linux; want_arch=x86_64 ;;
    *) die "unsupported channel: $channel" ;;
esac
[ "$os" = "$want_os" ] || die "channel $channel must be packaged on $want_os, this is $os"

if [ -z "$version" ]; then
    version=$(cargo metadata --no-deps --format-version 1 |
        sed -n 's/.*"name":"acta-app","version":"\([^"]*\)".*/\1/p')
    [ -n "$version" ] || die "could not read the acta-app version from cargo metadata"
fi
if [ -z "$bin_dir" ]; then
    if [ -n "$target" ]; then bin_dir=target/$target/release; else bin_dir=target/release; fi
fi
[ -n "$out" ] || out=releases/$channel

for bin in Acta acta-cli; do
    [ -f "$bin_dir/$bin" ] || die "$bin_dir/$bin not found; run cargo build --release --locked first"
done
# Every package carries the third-party notices; a package without them must not exist.
licenses=dist/licenses
[ -s "$licenses/THIRD-PARTY-RUST.txt" ] || die "$licenses is missing or incomplete; run scripts/licenses.sh first"
# Dictation's Silero VAD ships in every package, checked against its pin (ADR 0014, 4).
sh scripts/vad.sh || die "the Silero VAD is missing or not the pinned file; run scripts/vad.sh --fetch, then rebuild"
vad=$(sh scripts/model-pins.sh | awk '$1 == "SILERO_VAD" { print "assets/voice/" $2 }')
# Acta's own licence goes next to them, as Acta-LICENSE.txt.
[ -s LICENSE ] || die "LICENSE is missing"
command -v vpk >/dev/null 2>&1 || die "vpk not found; run: dotnet tool install -g vpk --version 1.2.161"
# A vpk that cannot start (no .NET 10 runtime where its launcher looks: set DOTNET_ROOT)
# must stop here, not be mistaken below for "no previous release".
vpk --help >/dev/null || die "vpk is installed but does not start; see its message above"

echo "Packaging $PACK_ID $version for $channel from $bin_dir into $out"

stage=dist/stage/$channel
rm -rf "$stage"
mkdir -p "$stage" "$out"

if [ "$fetch_previous" = 1 ]; then
    # Fails when the channel has no published release yet: then there is simply no delta.
    vpk download github --repoUrl "$REPO_URL" --channel "$channel" --outputDir "$out" ||
        echo "warning: no previous $channel release downloaded; no delta package will be built" >&2
fi
# vpk refuses to pack a version that is already in the output directory (a local re-run,
# or a dry run of the version that is currently published).
for stale in "$out/$PACK_ID-$version-$channel-full.nupkg" "$out/$PACK_ID-$version-$channel-delta.nupkg"; do
    if [ -f "$stale" ]; then
        echo "warning: removing existing $stale; no delta against the same version" >&2
        rm -f "$stale"
    fi
done

if [ "$want_os" = Darwin ]; then
    for bin in Acta acta-cli; do
        archs=$(lipo -archs "$bin_dir/$bin")
        [ "$archs" = "$want_arch" ] || die "$bin_dir/$bin is built for '$archs', $channel needs $want_arch"
    done

    app=$stage/$PACK_ID.app
    mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
    cp "$bin_dir/Acta" "$bin_dir/acta-cli" "$app/Contents/MacOS/"
    chmod 755 "$app/Contents/MacOS/Acta" "$app/Contents/MacOS/acta-cli"
    cp -R "$licenses" "$app/Contents/Resources/licenses"
    cp LICENSE "$app/Contents/Resources/licenses/Acta-LICENSE.txt"
    # Data goes in Resources, never in Contents/MacOS (code only); the app looks there.
    cp "$vad" "$app/Contents/Resources/"
    # The permission prompts in English and French (ADR 0014, 8).
    for lproj in assets/macos/*.lproj; do
        plutil -lint "$lproj/InfoPlist.strings" >/dev/null || die "$lproj/InfoPlist.strings is invalid"
        mkdir -p "$app/Contents/Resources/$(basename "$lproj")"
        cp "$lproj/InfoPlist.strings" "$app/Contents/Resources/$(basename "$lproj")/"
    done

    icns=$app/Contents/Resources/Acta.icns
    if [ -f assets/icons/acta.icns ]; then
        cp assets/icons/acta.icns "$icns"
    else
        [ -f assets/icons/acta.png ] || die "assets/icons/acta.icns and assets/icons/acta.png are both missing"
        iconset=dist/stage/Acta.iconset
        rm -rf "$iconset"
        mkdir -p "$iconset"
        for size in 16 32 128 256; do
            sips -z "$size" "$size" assets/icons/acta.png --out "$iconset/icon_${size}x${size}.png" >/dev/null
            sips -z $((size * 2)) $((size * 2)) assets/icons/acta.png --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
        done
        sips -z 512 512 assets/icons/acta.png --out "$iconset/icon_512x512.png" >/dev/null
        iconutil -c icns -o "$icns" "$iconset"
        rm -rf "$iconset"
    fi

    # The oldest macOS the binary itself declares it can run on.
    min_macos=$(vtool -show-build "$bin_dir/Acta" 2>/dev/null | awk '$1 == "minos" { print $2; exit }')
    [ -n "$min_macos" ] || min_macos=11.0
    # CFBundleShortVersionString takes three integers; drop any pre-release suffix.
    short_version=${version%%-*}
    short_version=${short_version%%+*}

    cat >"$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleDisplayName</key>
    <string>Acta</string>
    <key>CFBundleExecutable</key>
    <string>Acta</string>
    <key>CFBundleIconFile</key>
    <string>Acta.icns</string>
    <key>CFBundleIdentifier</key>
    <string>$BUNDLE_ID</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>Acta</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>$short_version</string>
    <key>CFBundleVersion</key>
    <string>$version</string>
    <key>LSApplicationCategoryType</key>
    <string>public.app-category.productivity</string>
    <key>LSMinimumSystemVersion</key>
    <string>$min_macos</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSPrincipalClass</key>
    <string>NSApplication</string>
    <key>NSSupportsAutomaticGraphicsSwitching</key>
    <true/>
</dict>
</plist>
EOF
    # The usage descriptions: macOS ends the process that captures audio or asks for
    # speech recognition without them. The base text is the English of en.lproj.
    for key in NSMicrophoneUsageDescription NSSpeechRecognitionUsageDescription; do
        text=$(plutil -extract "$key" raw -o - assets/macos/en.lproj/InfoPlist.strings) ||
            die "assets/macos/en.lproj/InfoPlist.strings has no $key"
        plutil -insert "$key" -string "$text" "$app/Contents/Info.plist"
    done
    plutil -insert CFBundleLocalizations -xml '<array><string>en</string><string>fr</string></array>' "$app/Contents/Info.plist"
    plutil -lint "$app/Contents/Info.plist" >/dev/null || die "generated Info.plist is invalid"

    # Ad-hoc signature (identity "-"): free, and mandatory on Apple silicon. vpk adds its
    # updater to the bundle, which invalidates this signature, and signs again with the
    # same identity; signing here first keeps the staged bundle runnable on its own.
    codesign --force --deep --sign - "$app"

    vpk pack --packId "$PACK_ID" --packTitle Acta --packVersion "$version" \
        --packDir "$app" --mainExe Acta --icon "$icns" \
        --runtime "$channel" --channel "$channel" --outputDir "$out" \
        --noInst --signAppIdentity -

    # The package is useless if the signature did not survive: check what users get.
    portable=$out/$PACK_ID-$channel-Portable.zip
    [ -f "$portable" ] || die "vpk did not produce $portable"
    check=dist/stage/verify-$channel
    rm -rf "$check"
    mkdir -p "$check"
    ditto -x -k "$portable" "$check"
    [ -d "$check/$PACK_ID.app" ] || die "$portable does not contain $PACK_ID.app"
    codesign --verify --deep --strict --verbose=2 "$check/$PACK_ID.app" ||
        die "the packaged $PACK_ID.app has no valid signature"
    codesign --display --verbose=2 "$check/$PACK_ID.app" 2>&1 | grep -E '^(Identifier|Signature|CodeDirectory|TeamIdentifier)' || true
    # What dictation needs in the bundle users get (ADR 0014, 4 and 8): both usage
    # descriptions (macOS ends the process without them), their French and English
    # translations, and the Silero VAD where the app looks for it.
    shipped=$check/$PACK_ID.app/Contents
    for key in NSMicrophoneUsageDescription NSSpeechRecognitionUsageDescription; do
        plutil -extract "$key" raw -o - "$shipped/Info.plist" >/dev/null ||
            die "the packaged Info.plist has no $key"
        for lang in en fr; do
            plutil -extract "$key" raw -o - "$shipped/Resources/$lang.lproj/InfoPlist.strings" >/dev/null ||
                die "the packaged $lang.lproj/InfoPlist.strings has no $key"
        done
    done
    [ -f "$shipped/Resources/$(basename "$vad")" ] || die "the packaged bundle has no $(basename "$vad")"
    rm -rf "$check"
else
    command -v mksquashfs >/dev/null 2>&1 || die "mksquashfs not found; install squashfs-tools"
    [ -f assets/icons/acta.png ] || die "assets/icons/acta.png is missing"
    cp "$bin_dir/Acta" "$bin_dir/acta-cli" "$stage/"
    chmod 755 "$stage/Acta" "$stage/acta-cli"
    cp -R "$licenses" "$stage/licenses"
    cp LICENSE "$stage/licenses/Acta-LICENSE.txt"
    cp "$vad" "$stage/"

    vpk pack --packId "$PACK_ID" --packTitle Acta --packVersion "$version" \
        --packDir "$stage" --mainExe Acta --icon assets/icons/acta.png \
        --runtime "$channel" --channel "$channel" --outputDir "$out" \
        --categories "Office;ProjectManagement"

    [ -f "$out/$PACK_ID-$channel.AppImage" ] || die "vpk did not produce $out/$PACK_ID-$channel.AppImage"
fi

[ -f "$out/$PACK_ID-$version-$channel-full.nupkg" ] || die "vpk did not produce the full package"
[ -f "$out/assets.$channel.json" ] || die "vpk did not write assets.$channel.json"

if [ "$fetch_previous" = 1 ]; then
    # Keep only what this build produced: the fetched packages are already published.
    for pkg in "$out"/*.nupkg; do
        [ -f "$pkg" ] || continue
        case "$(basename "$pkg")" in
            "$PACK_ID-$version-$channel-"*) ;;
            *) rm -f "$pkg" ;;
        esac
    done
fi

echo "Packages in $out:"
ls -l "$out"
