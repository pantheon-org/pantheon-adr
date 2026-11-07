#!/usr/bin/env bash
# check-commit-types.sh
#
# Fails when the type list copied into the PR title gate's regex has drifted
# from .github/commit-types.txt, the canonical list, or when that regex no
# longer accepts or rejects the titles in the table below.
#
# The gate runs on pull_request_target and so must not read repository files:
# the list is copied into its regex instead, and this is what keeps the copy
# honest. Two lists that must agree is how a version string drifted before.
#
# Usage: .github/scripts/check-commit-types.sh [types-file] [workflow-file]
# Exit:  0 in step; 1 drift or a wrong verdict; 2 unreadable or unparseable input.
set -euo pipefail

types_file="${1:-.github/commit-types.txt}"
workflow_file="${2:-.github/workflows/conventional-commits.yml}"

die() {
  echo "check-commit-types: $1" >&2
  exit 2
}

[[ -r "$types_file" ]] || die "cannot read $types_file"
[[ -r "$workflow_file" ]] || die "cannot read $workflow_file"

pattern="$(sed -n "s/^ *pattern='\(.*\)'$/\1/p" "$workflow_file" | head -n 1)"
[[ -n "$pattern" ]] || die "no pattern='...' assignment in $workflow_file. The regex shape changed; update this script with it."

copied="$(printf '%s\n' "$pattern" | sed -n 's/^\^(\([a-z|]*\)).*/\1/p' | tr '|' '\n' | sort)"
[[ -n "$copied" ]] || die "no leading type group in the regex from $workflow_file. The regex shape changed; update this script with it."

canonical="$(grep -v -e '^[[:space:]]*#' -e '^[[:space:]]*$' "$types_file" | sed 's/^[[:space:]]*//; s/[[:space:]]*$//' | sort)"
[[ -n "$canonical" ]] || die "no types in $types_file"

status=0

missing="$(comm -23 <(printf '%s\n' "$canonical") <(printf '%s\n' "$copied"))"
extra="$(comm -13 <(printf '%s\n' "$canonical") <(printf '%s\n' "$copied"))"
if [[ -n "$missing" || -n "$extra" ]]; then
  echo "check-commit-types: the PR title gate has drifted from $types_file" >&2
  [[ -z "$missing" ]] || echo "  rejected by the gate but canonical: $(tr "\n" " " <<<"$missing")" >&2
  [[ -z "$extra" ]] || echo "  accepted by the gate but not canonical: $(tr "\n" " " <<<"$extra")" >&2
  echo "  Update whichever is wrong. Do not silence this." >&2
  status=1
fi

# Titles the gate must accept, including release-please's and dependabot's own.
accept=(
  'feat: add a flag'
  'fix(scorer): count an empty section once'
  'feat!: drop the old flag'
  'refactor(cli)!: rename the subcommand'
  'chore(main): release 0.4.0'
  'chore(main): release 0.1.0-rc.4'
  'ci(deps): bump actions/checkout from 6 to 7'
  'docs: explain the title rule'
)
# Titles the gate must reject.
reject=(
  'Add a flag'
  'feat: Add a flag'
  'feat:add a flag'
  'feature: add a flag'
  'wip: add a flag'
  'fix(): add a flag'
  'fix: '
  'fix: trailing space '
)

for title in "${accept[@]}"; do
  if [[ ! "$title" =~ $pattern ]]; then
    echo "check-commit-types: the gate rejects a title it must accept: '$title'" >&2
    status=1
  fi
done
for title in "${reject[@]}"; do
  if [[ "$title" =~ $pattern ]]; then
    echo "check-commit-types: the gate accepts a title it must reject: '$title'" >&2
    status=1
  fi
done

if [[ "$status" -eq 0 ]]; then
  echo "check-commit-types OK: $(tr "\n" " " <<<"$canonical")"
fi
exit "$status"
