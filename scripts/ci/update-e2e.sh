#!/bin/sh
# End-to-end test of install and self-update on macOS and Linux, run by
# .github/workflows/update-e2e.yml on a disposable runner. It installs Acta for the
# current user from the published GitHub releases and starts it several times; the
# instances it observes are killed, the updated one is left running (the cleanup
# phase stops it) and nothing is uninstalled. Never run it on a machine in use: it
# opens windows and leaves Acta installed.
#
# Usage: scripts/ci/update-e2e.sh install <version>   install.sh of that release
#        scripts/ci/update-e2e.sh first-start <version>
#        scripts/ci/update-e2e.sh verify <version>    installed package, CLI, PATH link
#        scripts/ci/update-e2e.sh update <from> <to>  normal launch until it runs as <to>
#        scripts/ci/update-e2e.sh collect <dir>       copy the logs for an artifact
#        scripts/ci/update-e2e.sh cleanup             stop every Acta process left
#
# Versions are given without the leading v. Paths are the ones of ADR 0004.
set -eu

REPO=https://github.com/Alegzandr/acta
# Bound on the whole update phase, every launch included.
UPDATE_TIMEOUT=${UPDATE_TIMEOUT:-180}

os=$(uname -s)
case "$os" in
    Darwin)
        app=$HOME/Applications/Acta.app
        log=$HOME/Library/Logs/Acta/acta.log
        cli_root="$HOME/Library/Application Support/ActaCli"
        velopack_log=$HOME/Library/Logs/velopack_Acta.log
        ;;
    Linux)
        data_home=${XDG_DATA_HOME:-$HOME/.local/share}
        # install.sh puts the AppImage there whatever XDG_DATA_HOME says.
        app=$HOME/.local/share/acta/Acta.AppImage
        log=${XDG_STATE_HOME:-$HOME/.local/state}/acta/acta.log
        cli_root=$data_home/acta-cli
        velopack_log=${TMPDIR:-/tmp}/velopack_Acta.log
        ;;
    *)
        echo "error: update-e2e.sh is for macOS and Linux" >&2
        exit 1
        ;;
esac
cli=$cli_root/bin/acta
link=$HOME/.local/bin/acta

fail() {
    echo "::error::$*"
    if [ -f "$log" ]; then
        echo "--- $log ---"
        cat "$log"
    fi
    exit 1
}

expect() {
    # expect <actual> <expected> <what>
    if [ "$1" != "$2" ]; then
        fail "$3: expected '$2', got '$1'"
    fi
    echo "ok: $3 is '$2'"
}

now() {
    date +%s
}

line_count() {
    if [ -f "$log" ]; then wc -l <"$log" | tr -d ' '; else echo 0; fi
}

# log_query <first line> <version> <text>: "<line number> <pid>" of every line of the
# app log, from <first line> on, written by Acta <version> and whose message contains
# <text>. A line is "<ms> [<version> pid <pid>] <message>" (crates/acta-app/src/log.rs).
log_query() {
    [ -f "$log" ] || return 0
    LQ_VERSION=$2 LQ_TEXT=$3 awk -v first="$1" '
        NR >= first {
            version = substr($2, 2)
            pid = $4
            sub(/\]$/, "", pid)
            message = $0
            sub(/^[^]]*\] /, "", message)
            if (version == ENVIRON["LQ_VERSION"] && index(message, ENVIRON["LQ_TEXT"]) > 0)
                print NR, pid
        }' "$log"
}

# Same, first match only, line number or pid.
first_line() { log_query "$@" | head -n 1 | cut -d ' ' -f 1; }
last_pid() { log_query "$@" | tail -n 1 | cut -d ' ' -f 2; }

# Whether <pid> is the installed GUI.
is_installed_app() {
    kill -0 "$1" 2>/dev/null || return 1
    case "$os" in
        Darwin)
            # -ww: the full command line, never cut to a terminal width.
            cmdline=$(ps -ww -p "$1" -o command=) || return 1
            case "$cmdline" in
                "$app/Contents/MacOS/Acta" | "$app/Contents/MacOS/Acta "*) return 0 ;;
                *) return 1 ;;
            esac
            ;;
        Linux)
            # The AppImage runtime runs the app from its mount and sets APPIMAGE.
            tr '\0' '\n' <"/proc/$1/environ" 2>/dev/null | grep -qxF "APPIMAGE=$app"
            ;;
    esac
}

# Start the installed app the way a user does: Finder/Dock (`open`), or the AppImage
# as the desktop entry does. Arguments are passed to the app.
launch() {
    case "$os" in
        Darwin)
            if [ "$#" -gt 0 ]; then open "$app" --args "$@"; else open "$app"; fi
            ;;
        Linux)
            (cd / && nohup "$app" "$@" >/dev/null 2>&1 &)
            ;;
    esac
}

stop_app() {
    pid=$1
    kill "$pid" 2>/dev/null || return 0
    waited=0
    while kill -0 "$pid" 2>/dev/null; do
        if [ "$waited" -ge 15 ]; then
            kill -9 "$pid" 2>/dev/null || true
            break
        fi
        sleep 1
        waited=$((waited + 1))
    done
    # Let an AppImage runtime unmount and a macOS app unregister.
    sleep 2
}

manifest_version() {
    sed -n 's:.*<version>\([^<]*\)</version>.*:\1:p' "$1" | head -n 1
}

summary() {
    echo "$*"
    if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then echo "$*" >>"$GITHUB_STEP_SUMMARY"; fi
}

cmd_install() {
    version=$1
    url=$REPO/releases/download/v$version/install.sh
    echo "Installing Acta $version with $url"
    # What a user runs is `curl -fsSL <url> | sh`; the script is fetched first only so
    # that a failed download fails here instead of feeding sh an empty script.
    script=$(curl --fail --silent --show-error --location --proto '=https' --tlsv1.2 "$url") ||
        fail "could not download $url"
    # ACTA_NO_LAUNCH: the test starts the app itself, to observe the first start.
    printf '%s\n' "$script" | ACTA_NO_LAUNCH=1 sh -s -- "$version" ||
        fail "install.sh of $version failed"
    [ -e "$app" ] || fail "install.sh finished but $app is missing"
}

# macOS and Linux export the CLI and create the PATH link at the first start, not at
# install (ADR 0004). This start uses --specimen, which skips the update check, so the
# installed version can be observed before it updates itself.
cmd_first_start() {
    version=$1
    first=$(($(line_count) + 1))
    launch --specimen
    deadline=$(($(now) + 60))
    while :; do
        pid=$(last_pid "$first" "$version" "export (start): ")
        if [ -n "$pid" ] && [ -L "$link" ]; then
            if [ "$os" != Linux ] || [ -n "$(first_line "$first" "$version" "desktop entry: ")" ]; then
                break
            fi
        fi
        [ "$(now)" -lt "$deadline" ] || fail "the first start of $version did not export the CLI within 60 s"
        sleep 1
    done
    is_installed_app "$pid" || fail "pid $pid that logged the export is not the installed app"
    if [ "$os" = Darwin ]; then
        # install.sh finds a running Acta with `ps -o comm=`, which must print the full path.
        echo "ps -o comm= of the app: $(ps -p "$pid" -o comm=)"
    fi
    echo "First start of $version (pid $pid) exported the CLI; closing it"
    stop_app "$pid"
}

cmd_verify() {
    version=$1
    case "$os" in
        Darwin)
            [ -d "$app" ] || fail "$app is missing"
            manifest=$app/Contents/MacOS/sq.version
            [ -f "$manifest" ] || manifest=$app/Contents/Resources/sq.version
            [ -f "$manifest" ] || fail "no sq.version in $app"
            expect "$(manifest_version "$manifest")" "$version" "version in the bundle's sq.version"
            expect "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$app/Contents/Info.plist")" \
                "${version%%-*}" "CFBundleShortVersionString"
            expect "$("$app/Contents/MacOS/acta-cli" --version)" "acta $version" "packaged acta-cli"
            codesign --verify --deep "$app" || fail "the installed bundle fails codesign --verify"
            echo "ok: the installed bundle has a valid (ad-hoc) signature"
            ;;
        Linux)
            [ -x "$app" ] || fail "$app is missing or not executable"
            extract=$(mktemp -d)
            # Read the package without FUSE.
            (cd "$extract" && "$app" --appimage-extract >/dev/null) || fail "could not extract $app"
            expect "$(manifest_version "$extract/squashfs-root/usr/bin/sq.version")" "$version" \
                "version in the AppImage's sq.version"
            expect "$("$extract/squashfs-root/usr/bin/acta-cli" --version)" "acta $version" "packaged acta-cli"
            rm -rf "$extract"
            desktop=$data_home/applications/acta.desktop
            [ -f "$desktop" ] || fail "$desktop is missing"
            grep -qF "$app" "$desktop" || fail "$desktop does not start $app"
            [ -f "$data_home/icons/hicolor/512x512/apps/acta.png" ] || fail "the desktop icon is missing"
            echo "ok: desktop entry and icon in place"
            ;;
    esac

    # The CLI exported outside the package, and the link that puts it on PATH.
    [ -x "$cli" ] || fail "the exported CLI $cli is missing"
    expect "$("$cli" --version)" "acta $version" "exported CLI"
    expect "$(cat "$cli_root/bin/version.txt")" "$version" "export marker"
    [ -L "$link" ] || fail "$link is not a symlink"
    expect "$(readlink "$link")" "$cli" "target of $link"
    # A shell with ~/.local/bin on PATH, as `acta setup` and most distributions set up.
    user_path=$HOME/.local/bin:$PATH
    found=$(env PATH="$user_path" sh -c 'command -v acta') ||
        fail "acta is not found with ~/.local/bin on PATH"
    expect "$found" "$link" "acta found on PATH"
    expect "$(env PATH="$user_path" acta --version)" "acta $version" "acta --version from PATH"
}

# api_quota: "<remaining> <limit> <reset epoch>" of the unauthenticated GitHub REST
# API for this runner's IP, which the app's update check uses (60 requests an hour,
# shared by every job that had the IP). Reading it does not count against it.
api_quota() {
    curl --silent --show-error --proto '=https' --tlsv1.2 https://api.github.com/rate_limit |
        jq -r '.resources.core | "\(.remaining) \(.limit) \(.reset)"'
}

# report_quota <when>: print the quota; returns 1 when too little is left for one
# update (the check and the download each list the releases, the new version checks
# once more).
report_quota() {
    quota=$(api_quota) || quota=
    # shellcheck disable=SC2086 # split "<remaining> <limit> <reset>" into $2 $3 $4
    set -- "$1" $quota
    case "$#:${2:-}${3:-}${4:-}" in
        4: | 4:*[!0-9]* | [!4]*)
            echo "GitHub API quota $1: could not be read"
            return 0
            ;;
    esac
    echo "GitHub API quota of this runner's IP $1: $2 of $3 left, reset at $(date -u -r "$4" +%H:%M:%S 2>/dev/null || date -u -d "@$4" +%H:%M:%S) UTC"
    [ "$2" -ge 3 ]
}

cmd_update() {
    from=$1
    to=$2
    # Up to 0.1.3 the app checks through the REST API without a token. With the IP's
    # quota spent that check fails as a network error, which says nothing about Acta:
    # stop here and say so. Later versions read releases/latest/download (ADR 0004).
    uses_api=0
    if [ "$(printf '%s
%s
' "$from" 0.1.3 | sort -V | head -n 1)" = "$from" ]; then
        uses_api=1
        if ! report_quota "before the first launch"; then
            fail "the unauthenticated GitHub API quota of this runner's IP is spent, so Acta $from cannot check for updates; rerun the job later"
        fi
    fi
    started=$(now)
    deadline=$((started + UPDATE_TIMEOUT))
    launches=0
    relaunch=1
    first=1
    while :; do
        if [ "$relaunch" = 1 ]; then
            launches=$((launches + 1))
            first=$(($(line_count) + 1))
            echo "Launch $launches of Acta $from ($(($(now) - started)) s)"
            launch
            relaunch=0
        fi
        restarted=$(last_pid 1 "$to" "update: started by the updater")
        if [ -n "$restarted" ] && is_installed_app "$restarted"; then
            break
        fi
        # This launch opened its window before the update was applied: the download
        # was slower than the launch budget or the check failed. A user sees the update
        # at the next start, so close the app and start it again, at most twice.
        if [ "$launches" -lt 3 ] && {
            [ -n "$(first_line "$first" "$from" "not applying in this launch")" ] ||
                [ -n "$(first_line "$first" "$from" "retrying at next launch")" ]
        }; then
            pid=$(last_pid "$first" "$from" "update: ")
            echo "Launch $launches left the update to the next start; closing pid $pid"
            stop_app "$pid"
            relaunch=1
            continue
        fi
        if [ "$(now)" -ge "$deadline" ]; then
            if [ "$uses_api" = 1 ]; then report_quota "at the end" || true; fi
            fail "Acta did not restart as $to within $UPDATE_TIMEOUT s ($launches launch(es))"
        fi
        sleep 2
    done
    elapsed=$(($(now) - started))

    # The restarted app exports the new CLI in the background.
    deadline=$(($(now) + 60))
    until [ -n "$(first_line 1 "$to" "export (start): ")" ]; do
        [ "$(now)" -lt "$deadline" ] || fail "Acta $to did not export its CLI within 60 s of the restart"
        sleep 1
    done

    # The update sequence, in this order, in the app log.
    available=$(first_line 1 "$from" "update: $to available, downloading")
    downloaded=$(first_line 1 "$from" "update: $to downloaded")
    restart=$(first_line 1 "$to" "update: started by the updater")
    [ -n "$available" ] || fail "the log has no '$from: update: $to available'"
    [ -n "$downloaded" ] || fail "the log has no '$from: update: $to downloaded'"
    if ! { [ "$available" -lt "$downloaded" ] && [ "$downloaded" -lt "$restart" ]; }; then
        fail "the update lines are out of order (available $available, downloaded $downloaded, restart $restart)"
    fi
    echo "ok: log shows $from finding $to (line $available), downloading it (line $downloaded), $to restarted by the updater (line $restart)"

    # Still up a little later: the new version did not crash after the restart.
    sleep 10
    is_installed_app "$restarted" || fail "Acta $to (pid $restarted) did not stay up"
    echo "ok: Acta $to runs as pid $restarted from $app"
    summary "$(uname -s) $(uname -m): $from updated itself to $to in $elapsed s, $launches launch(es)"
    echo "--- $log ---"
    cat "$log"
}

cmd_collect() {
    dir=$1
    mkdir -p "$dir"
    cp "$log" "$dir/acta.log" 2>/dev/null || echo "no app log at $log" >"$dir/acta.log.missing"
    cp "$velopack_log" "$dir/" 2>/dev/null || echo "no Velopack log at $velopack_log" >"$dir/velopack.log.missing"
    {
        echo "# $app"
        ls -la "$app" 2>&1
        echo "# $cli_root"
        ls -laR "$cli_root" 2>&1
        echo "# $HOME/.local/bin"
        ls -la "$HOME/.local/bin" 2>&1
        echo "# processes"
        # shellcheck disable=SC2009 # the full command lines are the point, on both systems
        ps -x -o pid= -o command= 2>&1 | grep -i acta || true
    } >"$dir/state.txt"
}

cmd_cleanup() {
    # Every process started from the installed app (macOS: the bundle; Linux: an
    # AppImage whose APPIMAGE is the installed one). Leaves anything else alone.
    for pid in $(ps -x -o pid=); do
        if is_installed_app "$pid"; then
            echo "Stopping $pid"
            kill -9 "$pid" 2>/dev/null || true
        fi
    done
}

[ "$#" -ge 1 ] || fail "usage: $0 install|first-start|verify|update|collect|cleanup ..."
phase=$1
shift
case "$phase" in
    install) cmd_install "$1" ;;
    first-start) cmd_first_start "$1" ;;
    verify) cmd_verify "$1" ;;
    update) cmd_update "$1" "$2" ;;
    collect) cmd_collect "$1" ;;
    cleanup) cmd_cleanup ;;
    *) fail "unknown phase $phase" ;;
esac
