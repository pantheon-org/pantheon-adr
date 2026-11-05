#!/usr/bin/env bash
#
# cargo-deny.sh
#
# Runs `cargo deny check` (advisories, licenses, bans, sources) against the
# policy in deny.toml, installing the pinned cargo-deny release first if needed.
# Duplicate crate versions only warn; every other finding fails the run.
#
# Usage: cargo-deny.sh [extra cargo-deny check arguments]
# Env:   CARGO_DENY_VERSION  override the pinned version

set -euo pipefail

VERSION="${CARGO_DENY_VERSION:-0.20.2}"

if ! command -v cargo-deny >/dev/null 2>&1 || ! cargo-deny --version | grep -q " ${VERSION}\$"; then
  cargo install cargo-deny --version "${VERSION}" --locked
fi

cargo deny --locked check "$@"
