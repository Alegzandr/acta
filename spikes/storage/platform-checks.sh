#!/usr/bin/env bash
# Answers the open questions of ADR 0003 on a real macOS or Linux machine (GitHub runner).
# Run by .github/workflows/platform-checks.yml; prints a summary table at the end and
# appends it to $GITHUB_STEP_SUMMARY when set. Throwaway, like the rest of the spike.
#
# Every step runs under a time limit: a keychain password prompt on a runner shows up
# as a hang, which is recorded as TIMEOUT (with a screenshot on macOS) instead of
# blocking the job. After a timeout the prompt is dismissed and every later result is
# marked "(after timeout)", because it was not obtained from a clean state. Only a failed
# build makes the script fail; everything else is a result.
set -u

cd "$(dirname "$0")" || exit 1
work=$(mktemp -d)
logs=$work/logs
shots=${SHOTS_DIR:-$work/shots}
mkdir -p "$logs" "$shots"
summary=$work/summary.tsv
: >"$summary"
step_no=0
timeouts=0

# record <step> <result> <detail>
record() {
    local result=$2
    if [ "$timeouts" -gt 0 ]; then result="$result (after timeout)"; fi
    printf '%s\t%s\t%s\n' "$1" "$result" "$3" >>"$summary"
}

# A keychain prompt is drawn by SecurityAgent, not by the step that caused it: killing
# the step leaves the dialog on screen, where it would hold up the following steps too.
# Prints what it found, for the detail column.
dismiss_prompt() {
    if [ "$(uname -s)" != Darwin ]; then
        echo "nothing to dismiss on this system"
    elif ! pgrep -x SecurityAgent >/dev/null 2>&1; then
        echo "no SecurityAgent process, so no keychain prompt was up"
    else
        pkill -x SecurityAgent 2>/dev/null || sudo -n pkill -x SecurityAgent 2>/dev/null
        sleep 2
        if pgrep -x SecurityAgent >/dev/null 2>&1; then
            echo "SecurityAgent could not be stopped, its prompt may still be up"
        else
            echo "SecurityAgent prompt dismissed"
        fi
    fi
}

# run_step <limit seconds> <step name> <command...>
# Result: OK, FAIL (exit code and the last output line, which carries the platform
# error and its OSStatus) or TIMEOUT.
run_step() {
    local limit=$1 name=$2
    shift 2
    step_no=$((step_no + 1))
    local log=$logs/$step_no.log
    echo
    echo "=== [$step_no] $name"
    echo "    \$ $*"
    "$@" >"$log" 2>&1 &
    local pid=$! waited=0
    while kill -0 "$pid" 2>/dev/null && [ "$waited" -lt "$limit" ]; do
        sleep 1
        waited=$((waited + 1))
    done
    local result detail timed_out=0
    if kill -0 "$pid" 2>/dev/null; then
        if command -v screencapture >/dev/null 2>&1; then
            screencapture -x "$shots/step-$step_no.png" 2>/dev/null || true
        fi
        kill -9 "$pid" 2>/dev/null
        wait "$pid" 2>/dev/null
        result=TIMEOUT
        timed_out=1
        detail="no answer after ${limit}s; $(dismiss_prompt)"
    else
        wait "$pid"
        local rc=$?
        if [ "$rc" -eq 0 ]; then
            result=OK
            detail=$(tail -n 1 "$log" | cut -c 1-200)
        else
            result=FAIL
            detail="exit $rc: $(grep -v '^[[:space:]]*$' "$log" | tail -n 1 | cut -c 1-300)"
        fi
    fi
    sed 's/^/    | /' "$log" | tail -n 60
    echo "    => $result  $detail"
    record "$name" "$result" "$detail"
    # Counted after recording: the mark is for the steps that follow, not for this one.
    timeouts=$((timeouts + timed_out))
}

print_summary() {
    local note=
    if [ "$timeouts" -gt 0 ]; then
        note="$timeouts step(s) timed out. A result marked \"(after timeout)\" was obtained after a hung step was killed and its prompt dismissed, not from a clean state: read it as an indication only."
    fi
    echo
    echo "================ SUMMARY ($(uname -s) $(uname -m)) ================"
    awk -F '\t' '{ printf "%-58s %-23s %s\n", $1, $2, $3 }' "$summary"
    if [ -n "$note" ]; then
        echo
        echo "$note"
    fi
    if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
        {
            echo "### ADR 0003 platform checks: $(uname -s) $(uname -m)"
            echo
            echo "| Step | Result | Error code / detail |"
            echo "|---|---|---|"
            awk -F '\t' '{ gsub(/\|/, "\\|", $3); printf "| %s | %s | %s |\n", $1, $2, $3 }' "$summary"
            if [ -n "$note" ]; then
                echo
                echo "$note"
            fi
        } >>"$GITHUB_STEP_SUMMARY"
    fi
}

build_failed() {
    record "$1" FAIL "build failed, see the log"
    print_summary
    exit 1
}

bin=target/release/acta-storage-spike

linux_checks() {
    echo "perl: $(command -v perl || echo MISSING) $(perl -e 'print $^V' 2>/dev/null)"
    echo "make: $(command -v make || echo MISSING) $(make --version 2>/dev/null | head -n 1)"
    if command -v perl >/dev/null 2>&1 && command -v make >/dev/null 2>&1; then
        record "perl and make present (vendored OpenSSL build)" OK "perl $(perl -e 'print $^V') / $(make --version | head -n 1)"
    else
        record "perl and make present (vendored OpenSSL build)" FAIL "perl: $(command -v perl || echo missing), make: $(command -v make || echo missing)"
    fi

    echo "=== build with vendored OpenSSL"
    local start=$SECONDS
    cargo build --release --locked || build_failed "build: SQLCipher + vendored OpenSSL"
    record "build: SQLCipher + vendored OpenSSL" OK "$((SECONDS - start))s"

    echo "DBUS_SESSION_BUS_ADDRESS=${DBUS_SESSION_BUS_ADDRESS:-<unset>}"
    run_step 180 "spike run, no D-Bus session at all" \
        env -u DBUS_SESSION_BUS_ADDRESS "$bin" run
    run_step 180 "spike run, D-Bus session but no Secret Service" \
        dbus-run-session -- "$bin" run
    # gnome-keyring creates and unlocks the login keyring from the password on stdin.
    # shellcheck disable=SC2016 # $0 is expanded by the inner sh, on purpose
    run_step 300 "spike run, gnome-keyring unlocked non-interactively" \
        dbus-run-session -- sh -c 'printf "%s" "ci-keyring-password" | gnome-keyring-daemon --daemonize --unlock --components=secrets >/dev/null && exec "$0" run' "$bin"
    # shellcheck disable=SC2016
    run_step 60 "key-get on a missing item, gnome-keyring unlocked" \
        dbus-run-session -- sh -c 'printf "%s" "ci-keyring-password" | gnome-keyring-daemon --daemonize --unlock --components=secrets >/dev/null && exec "$0" key-get acta-ci-missing' "$bin"
}

# sign <file> <identifier>: ad-hoc signature, as the release build does.
sign() {
    codesign --force --sign - --identifier "$2" "$1"
    codesign --display --verbose=4 "$1" 2>&1 | grep -E '^(Identifier|CDHash|Signature)=' | sed 's/^/    /'
    codesign --display --requirements - "$1" 2>&1 | grep -i 'designated' | sed 's/^/    /'
}

# build_variant <id> <destination> <identifier>
build_variant() {
    ACTA_SPIKE_BUILD_ID=$1 cargo build --release --locked --no-default-features --features commoncrypto ||
        build_failed "build variant $1"
    mkdir -p "$(dirname "$2")"
    rm -f "$2"
    cp "$bin" "$2"
    echo "build $1 -> $2"
    sign "$2" "$3"
}

svc_own=acta-ci-keychain-own
svc_open=acta-ci-keychain-open
account=db-key-v1

# Runs again at exit, whatever happened in between.
cleanup_items() {
    security delete-generic-password -s "$svc_own" -a "$account" >/dev/null 2>&1
    security delete-generic-password -s "$svc_open" -a "$account" >/dev/null 2>&1
    security delete-generic-password -s acta-spike -a "$account" >/dev/null 2>&1
    return 0
}

# The test value has to reach `security` as an argument: the tool offers no other way
# in. It must not reach the log, and run_step prints the command it is given, so it is
# given this function instead of the arguments. `exec`, so that the process run_step
# kills on a timeout is `security` itself and not a shell waiting for it.
open_item_value=
add_open_item() {
    exec security add-generic-password -A -s "$svc_open" -a "$account" -w "$open_item_value"
}

macos_checks() {
    local app=$work/app/Acta cli=$work/app/acta-cli
    trap cleanup_items EXIT
    cleanup_items

    echo "=== keychain state on this machine"
    sw_vers
    security list-keychains
    security default-keychain
    security show-keychain-info 2>&1 || true

    echo "=== build with CommonCrypto (no OpenSSL)"
    local start=$SECONDS
    cargo build --release --locked --no-default-features --features commoncrypto ||
        build_failed "build: SQLCipher + CommonCrypto"
    record "build: SQLCipher + CommonCrypto" OK "$((SECONDS - start))s"
    if otool -L "$bin" | grep -Ei 'libssl|libcrypto'; then
        record "no OpenSSL linked" FAIL "otool -L lists libssl/libcrypto"
    else
        record "no OpenSSL linked" OK "otool -L shows no libssl/libcrypto"
    fi
    run_step 300 "spike run (keychain item created and read by one build)" "$bin" run

    echo
    echo "=== keychain test: item created by ad-hoc build A"
    build_variant A "$app" io.github.alegzandr.acta
    run_step 30 "A creates the item" "$app" key-set "$svc_own"
    run_step 30 "A reads its own item" "$app" key-get "$svc_own"

    # Build B replaces A at the same path, as an update does; the CLI is a third
    # executable with its own name and identifier.
    build_variant B "$app" io.github.alegzandr.acta
    run_step 30 "B (update of A, same path) reads A's item" "$app" key-get "$svc_own"
    build_variant C "$cli" acta-cli
    run_step 30 "second executable (acta-cli) reads A's item" "$cli" key-get "$svc_own"
    run_step 30 "B overwrites A's item" "$app" key-set "$svc_own"
    security delete-generic-password -s "$svc_own" -a "$account" >/dev/null 2>&1
    if security find-generic-password -s "$svc_own" -a "$account" >/dev/null 2>&1; then
        record "cleanup: item of the first test deleted" FAIL "still present"
    else
        record "cleanup: item of the first test deleted" OK ""
    fi

    echo
    echo "=== keychain test: item created by 'security add-generic-password -A'"
    # A throwaway value; the real key would have to transit through this tool the same way.
    open_item_value=$(openssl rand -hex 32)
    run_step 30 "security add-generic-password -A creates the item" add_open_item
    open_item_value=
    run_step 30 "B reads the -A item" "$app" key-get "$svc_open"
    run_step 30 "second executable (acta-cli) reads the -A item" "$cli" key-get "$svc_open"
    build_variant D "$app" io.github.alegzandr.acta
    run_step 30 "D (another update, same path) reads the -A item" "$app" key-get "$svc_open"
    run_step 30 "D overwrites the -A item" "$app" key-set "$svc_open"
    run_step 30 "acta-cli reads the item after D overwrote it" "$cli" key-get "$svc_open"
    security delete-generic-password -s "$svc_open" -a "$account" >/dev/null 2>&1
    if security find-generic-password -s "$svc_open" -a "$account" >/dev/null 2>&1; then
        record "cleanup: item of the second test deleted" FAIL "still present"
    else
        record "cleanup: item of the second test deleted" OK ""
    fi
}

case "$(uname -s)" in
    Linux) linux_checks ;;
    Darwin) macos_checks ;;
    *) echo "error: platform-checks.sh runs on macOS and Linux only" >&2; exit 1 ;;
esac
print_summary
