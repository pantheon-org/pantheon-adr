#!/usr/bin/env bash
# Create or update every label listed in .github/labels.txt (name|color|description).
# Needs GH_TOKEN with issues: write and GH_REPO (or a git checkout). Never deletes labels.
set -euo pipefail

file="${1:-.github/labels.txt}"

while IFS='|' read -r name color description; do
  case "$name" in ''|'#'*) continue ;; esac
  gh label create "$name" --color "$color" --description "$description" --force
done < "$file"
