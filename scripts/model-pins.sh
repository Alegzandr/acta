#!/bin/sh
# Print the model files Acta pins (ADR 0014, 4), one per line:
#   <const> <file> <size> <sha256> <owner/repo>
# read from crates/acta-core/src/voice/models.rs, the one place they are written.
# Used by scripts/vad.sh and the weekly drift check (scripts/ci/check-model-pins.sh).
# Fails when a pin cannot be read in full, so a reformatted file is not taken for
# "nothing pinned".
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
models=$root/crates/acta-core/src/voice/models.rs
[ -f "$models" ] || { echo "error: $models not found" >&2; exit 1; }

awk '
    # Every ModelFile constant, whatever its form: each must be read below.
    /: ModelFile = / { total++ }
    # String constants a pin may name as its repository (WHISPER_REPO).
    /^(pub )?const [A-Z_]+: &str = "[^"]*";$/ {
        name = $0; sub(/^(pub )?const /, "", name); sub(/:.*/, "", name)
        value = $0; sub(/^[^"]*"/, "", value); sub(/";$/, "", value)
        strs[name] = value
        next
    }
    /^pub const [A-Z_]+: ModelFile = ModelFile \{$/ {
        pin = $3; sub(/:$/, "", pin)
        file = ""; size = ""; sha = ""; repo = ""
        next
    }
    pin != "" && /^ *file: "[^"]*",$/ { file = $0; sub(/^[^"]*"/, "", file); sub(/",$/, "", file) }
    pin != "" && /^ *size: [0-9_]+,$/ { size = $2; sub(/,$/, "", size); gsub(/_/, "", size) }
    pin != "" && /^ *sha256: "[0-9a-f]+",$/ { sha = $0; sub(/^[^"]*"/, "", sha); sub(/",$/, "", sha) }
    pin != "" && /^ *repo: / {
        repo = $2; sub(/,$/, "", repo)
        if (repo ~ /^"/) { gsub(/"/, "", repo) } else { repo = strs[repo] }
    }
    pin != "" && /^\};$/ {
        if (file == "" || size == "" || length(sha) != 64 || repo !~ /^[^\/ ]+\/[^\/ ]+$/) {
            printf "error: could not read pin %s in models.rs\n", pin > "/dev/stderr"
            bad = 1
        } else {
            print pin, file, size, sha, repo
            count++
        }
        pin = ""
    }
    END {
        if (bad) exit 1
        if (count == 0) { print "error: no ModelFile pin found in models.rs" > "/dev/stderr"; exit 1 }
        if (count != total) {
            printf "error: models.rs has %d ModelFile constants, %d could be read\n", total, count > "/dev/stderr"
            exit 1
        }
    }
' "$models"
