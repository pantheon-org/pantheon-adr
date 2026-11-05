#!/usr/bin/env bash
#
# release-automerge.sh
#
# Squash-merges the open release-please PR under the pantheon-ai-bot App token, so
# the workflows on the merge to main fire (a merge by GITHUB_TOKEN would not).
# Ported from pantheon-org/tekhne. Run by .github/workflows/release-automerge.yml.
# Env: GH_TOKEN (App token), GH_REPO (owner/name)

set -euo pipefail

# The release-please head branch starts with release-please--branches--main and may
# carry a component suffix, so match by prefix instead of passing --head.
pr=$(gh pr list --repo "$GH_REPO" --state open \
  --json number,title,mergeStateStatus,isDraft,headRefName \
  --jq '[.[] | select(.headRefName | startswith("release-please--branches--main"))][0] // empty')

if [ -z "$pr" ]; then
  echo "No open release PR found." | tee -a "$GITHUB_STEP_SUMMARY"
  exit 0
fi

number=$(jq -r '.number' <<<"$pr")
state=$(jq -r '.mergeStateStatus' <<<"$pr")
draft=$(jq -r '.isDraft' <<<"$pr")
echo "Found release PR #$number (mergeStateStatus=$state, draft=$draft)"

if [ "$draft" = "true" ]; then
  echo "PR #$number is a draft; skipping." | tee -a "$GITHUB_STEP_SUMMARY"
  exit 0
fi

# pr-auto-rebase.yml skips release-please branches, so BEHIND is expected here:
# refresh the branch and merge on the next run instead of merging a stale one.
case "$state" in
  BEHIND)
    gh pr update-branch "$number" --repo "$GH_REPO" --rebase ||
      echo "::warning::Could not rebase PR #$number; it needs a manual rebase."
    echo "- PR #$number: was BEHIND, rebased; merge deferred to the next run." >> "$GITHUB_STEP_SUMMARY"
    exit 0
    ;;
  DIRTY|BLOCKED)
    echo "::warning::PR #$number is not mergeable ($state); skipping."
    echo "- PR #$number: not mergeable ($state); skipped." >> "$GITHUB_STEP_SUMMARY"
    exit 0
    ;;
esac

gh pr merge "$number" --repo "$GH_REPO" --squash --delete-branch
echo "- PR #$number: merged." >> "$GITHUB_STEP_SUMMARY"
