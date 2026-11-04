#!/usr/bin/env bash
#
# check-inline-scripts.sh
#
# Fails when a `run:` block in a workflow or composite action holds more than
# INLINE_MAX lines of code (default 15). Longer logic belongs in a script under
# .github/scripts/, where it can be run locally, shellchecked and tested.
#
#   - Blank lines and comment lines do not count.
#   - A block may opt out with a line `# inline-ok: <reason>` inside it.
#
# Usage: check-inline-scripts.sh [file ...]
#        With no arguments, checks .github/workflows/*.yml and
#        .github/actions/*/action.yml.
# Needs: yq (mikefarah v4) and jq, both preinstalled on GitHub-hosted runners.

set -uo pipefail

MAX="${INLINE_MAX:-15}"

# A different `yq` (the Python wrapper around jq) rejects the flags below; without
# this guard a broken parse would look like a clean pass.
if ! yq --version 2>&1 | grep -q 'mikefarah'; then
    echo "::error::check-inline-scripts.sh needs mikefarah yq v4 on PATH." >&2
    exit 2
fi

if [ "$#" -gt 0 ]; then
    files=("$@")
else
    shopt -s nullglob
    files=(.github/workflows/*.yml .github/workflows/*.yaml .github/actions/*/action.yml .github/actions/*/action.yaml)
fi

failed=0
for file in "${files[@]}"; do
    # One JSON string per run block. Read into a variable so a yq failure stops the
    # check instead of passing it.
    if ! blocks="$(yq -o=json -I=0 '.. | select(tag == "!!map") | select(has("run")) | .run | select(tag == "!!str")' "$file")"; then
        echo "::error file=${file}::yq could not parse the file." >&2
        exit 2
    fi
    while IFS= read -r block; do
        [ -z "$block" ] && continue
        text="$(jq -r '.' <<<"$block")"
        if grep -Eq '^[[:space:]]*#[[:space:]]*inline-ok:' <<<"$text"; then
            continue
        fi
        count="$(grep -Evc '^[[:space:]]*(#.*)?$' <<<"$text" || true)"
        if [ "$count" -gt "$MAX" ]; then
            first="$(grep -Ev '^[[:space:]]*(#.*)?$' <<<"$text" | head -n 1 | cut -c1-60)"
            echo "::error file=${file}::A run block has ${count} lines of code (limit ${MAX}), starting \"${first}\". Move it to a script in .github/scripts/, or add '# inline-ok: <reason>' inside the block."
            failed=1
        fi
    done <<<"$blocks"
done

if [ "$failed" -eq 0 ]; then
    echo "No run block exceeds ${MAX} lines of code."
fi
exit "$failed"
