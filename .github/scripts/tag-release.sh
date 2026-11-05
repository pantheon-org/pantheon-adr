#!/usr/bin/env bash
#
# tag-release.sh
#
# Pushes the tag vX.Y.Z for the crate version on the current commit when that tag
# does not exist yet. release-please only opens the release PR (it must not create
# the GitHub Release, because cargo-dist creates it in release.yml and fails if one
# already exists), so merging the release PR is what makes the version new here.
# The tag is created under the pantheon-ai-bot App token so the push fires release.yml.
# Run by .github/workflows/release-please.yml.
# Env: GH_TOKEN (App token), GH_REPO (owner/name), GITHUB_SHA (commit to tag)

set -euo pipefail

version=$(sed -n 's/^version = "\(.*\)"$/\1/p' crates/adr/Cargo.toml | head -n 1)
if [ -z "$version" ]; then
  echo "::error::Could not read the crate version from crates/adr/Cargo.toml."
  exit 1
fi
tag="v$version"

if gh api "repos/$GH_REPO/git/ref/tags/$tag" >/dev/null 2>&1; then
  echo "Tag $tag already exists. Nothing to do."
  exit 0
fi

gh api "repos/$GH_REPO/git/refs" -f ref="refs/tags/$tag" -f sha="$GITHUB_SHA" >/dev/null
echo "Tagged $GITHUB_SHA as $tag. release.yml will build the release."
echo "Tagged $tag at $GITHUB_SHA." >> "$GITHUB_STEP_SUMMARY"
