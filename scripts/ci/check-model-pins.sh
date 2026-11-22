#!/bin/sh
# Weekly drift check (ADR 0014, 4, "Integrity"): every model file Acta pins must still
# be on Hugging Face with the pinned size and SHA-256, so a re-uploaded model is caught
# before users hit "The file did not match". Reads the tree API of each repository
# (file metadata only, nothing is downloaded). Writes a table to the job summary when
# GITHUB_STEP_SUMMARY is set. Needs curl and jq.
set -eu

root=$(cd "$(dirname "$0")/../.." && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

sh "$root/scripts/model-pins.sh" >"$work/pins"
summary=${GITHUB_STEP_SUMMARY:-/dev/null}
{
    echo "### Model pins against Hugging Face"
    echo
    echo "| Pin | File | Repository | Result |"
    echo "|-----|------|------------|--------|"
} >>"$summary"

# One listing per repository, following the API's pagination (Link: rel="next").
awk '{ print $5 }' "$work/pins" | sort -u >"$work/repos"
while read -r repo; do
    out=$work/$(echo "$repo" | tr '/' '_').json
    echo '[]' >"$out"
    url=https://huggingface.co/api/models/$repo/tree/main
    pages=0
    while [ -n "$url" ]; do
        pages=$((pages + 1))
        [ "$pages" -le 20 ] || { echo "error: more than 20 pages listing $repo" >&2; exit 1; }
        curl -fsS --proto '=https' --tlsv1.2 --retry 3 -D "$work/headers" -o "$work/page.json" "$url" ||
            { echo "error: could not read $url" >&2; exit 1; }
        jq -e 'type == "array"' "$work/page.json" >/dev/null ||
            { echo "error: $url did not return a list of files" >&2; exit 1; }
        jq -s 'add' "$out" "$work/page.json" >"$work/merged.json"
        mv "$work/merged.json" "$out"
        url=$(tr -d '\r' <"$work/headers" | sed -n 's/^[Ll]ink:.*<\([^>]*\)>; *rel="next".*/\1/p' | head -n 1)
    done
done <"$work/repos"

drift=0
while read -r pin file size sha repo; do
    out=$work/$(echo "$repo" | tr '/' '_').json
    found=$(jq -r --arg f "$file" '.[] | select(.type == "file" and .path == $f)
        | "\(.size) \(.lfs.oid // "none") \(.lfs.size // "none")"' "$out")
    if [ -z "$found" ]; then
        result="missing from the repository"
    else
        # shellcheck disable=SC2086 # three words by construction
        set -- $found
        if [ "$1" = "$size" ] && [ "$2" = "$sha" ] && [ "$3" = "$size" ]; then
            result=ok
        else
            result="changed: size $1, SHA-256 $2 (pinned: $size, $sha)"
        fi
    fi
    echo "$pin $repo/$file: $result"
    echo "| $pin | \`$file\` | $repo | $result |" >>"$summary"
    if [ "$result" != ok ]; then
        echo "::error::$pin ($repo/$file) $result; update models.rs and ADR 0014 after checking the new file"
        drift=1
    fi
done <"$work/pins"

exit "$drift"
