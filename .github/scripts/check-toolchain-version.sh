#!/usr/bin/env bash
# Fails unless every place that names the Rust toolchain agrees with
# rust-toolchain.toml, the single source of truth:
#   - mise.toml                       rust = { version = "X.Y.Z", ... }
#   - .github/workflows/rust-ci.yml   rustup toolchain install / default X.Y.Z
#   - .github/workflows/cargo-deny.yml  the same two lines
#   - Cargo.toml                      rust-version = "X.Y" (the major.minor of X.Y.Z)
# Run it from anywhere inside the repository: bash .github/scripts/check-toolchain-version.sh
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

channel=$(sed -nE 's/^channel *= *"([^"]+)".*/\1/p' rust-toolchain.toml | head -n 1)
if [[ -z "$channel" ]]; then
  echo "error: no channel found in rust-toolchain.toml" >&2
  exit 1
fi

failures=0
expect() { # label, found, wanted
  if [[ "$2" != "$3" ]]; then
    echo "mismatch: $1 has '$2', rust-toolchain.toml has '$3'" >&2
    failures=$((failures + 1))
  fi
}

mise=$(sed -nE 's/^rust *=.*version *= *"([^"]+)".*/\1/p' mise.toml | head -n 1)
expect "mise.toml" "$mise" "$channel"

for workflow in .github/workflows/rust-ci.yml .github/workflows/cargo-deny.yml; do
  versions=$(sed -nE 's/^ *rustup (toolchain install|default) ([^ ]+).*/\2/p' "$workflow")
  if [[ -z "$versions" ]]; then
    echo "mismatch: $workflow names no toolchain version" >&2
    failures=$((failures + 1))
    continue
  fi
  while IFS= read -r version; do
    expect "$workflow" "$version" "$channel"
  done <<<"$versions"
done

msrv=$(sed -nE 's/^rust-version *= *"([^"]+)".*/\1/p' Cargo.toml | head -n 1)
expect "Cargo.toml rust-version" "$msrv" "${channel%.*}"

if ((failures > 0)); then
  echo "error: $failures toolchain version mismatch(es); change them together" >&2
  exit 1
fi
echo "ok: toolchain $channel is used everywhere (rust-version ${channel%.*})"
