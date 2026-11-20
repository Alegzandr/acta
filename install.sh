#!/bin/sh
# Acta installer for macOS and Linux: silent, per-user, never uses sudo.
#
#   curl -fsSL https://github.com/Alegzandr/acta/releases/latest/download/install.sh | sh
#
# A specific version:   ... | sh -s -- 0.1.0      (or ACTA_VERSION=0.1.0)
# Do not start Acta:    ACTA_NO_LAUNCH=1
#
# It downloads the package for this machine from the GitHub release and checks its
# SHA-256 against the release's SHA256SUMS. macOS: Acta.app goes to ~/Applications.
# Linux: the AppImage goes to ~/.local/share/acta. Acta then keeps itself up to date.
set -eu

REPO=https://github.com/Alegzandr/acta

die() {
    echo "error: $*" >&2
    exit 1
}

fetch() {
    # fetch <url> <file>
    curl --fail --silent --show-error --location --proto '=https' --tlsv1.2 \
        --retry 3 --output "$2" "$1"
}

sha256_of() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d ' ' -f 1
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$1" | cut -d ' ' -f 1
    else
        die "sha256sum or shasum is required"
    fi
}

tmp=
app=
# macOS reinstall: where the installed app waits while the new one is moved in.
previous=
cleanup() {
    # Whatever stopped the reinstall (a failed move, Ctrl-C), Acta must still be there.
    if [ -n "$previous" ] && [ -e "$previous" ] && [ ! -e "$app" ]; then
        if mv "$previous" "$app"; then
            echo "The previous Acta was put back in $app." >&2
        else
            echo "error: could not put the previous Acta back; it is in $previous" >&2
            return 0
        fi
    fi
    if [ -n "$tmp" ]; then rm -rf "$tmp"; fi
}

# Process ids of this user's GUI started from the installed bundle; empty when there is
# none. The command line and MCP server never run from the bundle (ADR 0004).
# The executable path is compared as a plain string, not as a pattern: a home directory
# may contain spaces or characters that mean something in a regular expression.
acta_pids() {
    ps -x -o pid= -o comm= | ACTA_EXE="$app/Contents/MacOS/Acta" awk '{
        pid = $1
        sub(/^[[:space:]]*[0-9]+[[:space:]]+/, "")
        if ($0 == ENVIRON["ACTA_EXE"]) print pid
    }'
}

# Ask a running Acta to terminate and wait for it; fails if it is still there.
quit_acta() {
    pids=$(acta_pids)
    [ -n "$pids" ] || return 0
    echo "Acta is running: closing it to replace it..."
    # shellcheck disable=SC2086 # one argument per process id
    kill $pids 2>/dev/null || true
    waited=0
    while [ -n "$(acta_pids)" ]; do
        [ "$waited" -lt 10 ] || return 1
        sleep 1
        waited=$((waited + 1))
    done
    was_running=1
}

main() {
    command -v curl >/dev/null 2>&1 || die "curl is required"
    [ -n "${HOME:-}" ] || die "HOME is not set"
    [ "$(id -u)" != 0 ] || die "do not run this installer as root; Acta installs per user"

    os=$(uname -s)
    arch=$(uname -m)
    case "$os" in
        Darwin)
            # A shell running under Rosetta reports x86_64 on Apple silicon.
            if [ "$(sysctl -n hw.optional.arm64 2>/dev/null || echo 0)" = 1 ]; then arch=arm64; fi
            case "$arch" in
                arm64) asset=Acta-osx-arm64-Portable.zip ;;
                x86_64) asset=Acta-osx-x64-Portable.zip ;;
                *) die "unsupported Mac architecture: $arch" ;;
            esac
            ;;
        Linux)
            case "$arch" in
                x86_64 | amd64) asset=Acta-linux-x64.AppImage ;;
                *) die "no Acta build for Linux on $arch yet (x86_64 only)" ;;
            esac
            ;;
        *) die "unsupported system: $os (on Windows use install.ps1)" ;;
    esac

    version=${1:-${ACTA_VERSION:-}}
    version=${version#v}
    if [ -z "$version" ]; then
        # Resolve "latest" once through the redirect of /releases/latest (no API call,
        # so no rate limit), then download everything from that one release.
        latest=$(curl --fail --silent --show-error --location --head --output /dev/null \
            --write-out '%{url_effective}' "$REPO/releases/latest") ||
            die "could not reach $REPO/releases/latest"
        case "$latest" in
            */releases/tag/v*) version=${latest##*/releases/tag/v} ;;
            *) die "no published Acta release found at $REPO/releases" ;;
        esac
    fi
    base=$REPO/releases/download/v$version

    case "$os" in
        Darwin) dest_dir=$HOME/Applications ;;
        *) dest_dir=$HOME/.local/share/acta ;;
    esac
    mkdir -p "$dest_dir"
    # Work next to the destination so the final move is a rename on the same volume.
    tmp=$(mktemp -d "$dest_dir/.acta-install.XXXXXX") || die "cannot write to $dest_dir"
    trap cleanup EXIT
    trap 'exit 1' INT TERM HUP

    echo "Downloading Acta $version..."
    fetch "$base/SHA256SUMS" "$tmp/SHA256SUMS" || die "download of $base/SHA256SUMS failed"
    fetch "$base/$asset" "$tmp/$asset" || die "download of $base/$asset failed"

    expected=$(awk -v name="$asset" '{ f = $2; sub(/^\*/, "", f) } f == name { print $1; exit }' "$tmp/SHA256SUMS")
    [ -n "$expected" ] || die "SHA256SUMS of Acta $version has no entry for $asset"
    actual=$(sha256_of "$tmp/$asset")
    [ "$actual" = "$expected" ] ||
        die "checksum mismatch for $asset (expected $expected, got $actual); nothing was installed"

    echo "Installing..."
    case "$os" in
        Darwin)
            app=$dest_dir/Acta.app
            ditto -x -k "$tmp/$asset" "$tmp/unpacked" || die "could not unpack $asset"
            [ -d "$tmp/unpacked/Acta.app" ] || die "$asset does not contain Acta.app"
            # curl sets no quarantine attribute; clear it anyway in case this shell does.
            xattr -dr com.apple.quarantine "$tmp/unpacked/Acta.app" 2>/dev/null || true
            was_running=0
            if [ -e "$app" ]; then
                quit_acta ||
                    die "Acta is still running; quit it and run this installer again (nothing was changed)"
                # Set before the move: an interruption right after it must find the
                # previous app and put it back.
                previous=$tmp/previous.app
                mv "$app" "$previous" ||
                    die "could not move the installed $app aside; nothing was changed"
            fi
            # On failure the EXIT trap puts the previous app back.
            mv "$tmp/unpacked/Acta.app" "$app" || die "could not move Acta.app into $dest_dir"
            previous=
            echo "Acta $version is installed in $app."
            if [ "${ACTA_NO_LAUNCH:-}" = 1 ]; then
                if [ "$was_running" = 1 ]; then
                    echo "Acta was closed for the update and ACTA_NO_LAUNCH is set: start it again yourself."
                fi
            elif ! open "$app"; then
                # No graphical session (ssh), for instance: the install itself is done.
                echo "warning: Acta is installed but could not be opened from here; open $app yourself" >&2
            fi
            ;;
        *)
            app=$dest_dir/Acta.AppImage
            chmod 755 "$tmp/$asset"
            mv -f "$tmp/$asset" "$app"
            echo "Acta $version is installed in $app."
            if [ "${ACTA_NO_LAUNCH:-}" = 1 ]; then
                :
            elif [ -n "${DISPLAY:-}" ] || [ -n "${WAYLAND_DISPLAY:-}" ]; then
                nohup "$app" >/dev/null 2>&1 &
            else
                echo "No graphical session detected; start it later with: $app"
            fi
            ;;
    esac

    # This script never touches PATH or a shell profile, so it has no "open a new
    # terminal" to announce. Acta creates the link shortly after it first starts, so it
    # may not be there yet: only the directory can be checked.
    case ":${PATH:-}:" in
        *":$HOME/.local/bin:"*) ;;
        *) echo "Acta links the acta command into $HOME/.local/bin; add that directory to your PATH to use it." ;;
    esac
}

main "$@"
