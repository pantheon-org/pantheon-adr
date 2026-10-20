# pantheon-adr

Architecture Decision Record (ADR) command line tool, with a bundled `adr-creator` skill that agents
can install.

`pantheon-adr` creates, lists, checks and indexes ADRs (default directory `docs/adr`, or the `ADR_DIR`
environment variable), provides agent hooks and shell completion, and `pantheon-adr skill install`
copies the bundled skill into detected agent directories.

## Install

Releases ship a shell installer and tarballs for macOS and Linux (arm64 and x86_64) once this
repository has its own release. Until then the latest is
[tekhne `tool/pantheon-adr-v0.2.4`](https://github.com/pantheon-org/tekhne/releases/tag/tool%2Fpantheon-adr-v0.2.4).

## Development

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

The shared helpers come from
[agentic-harness-dev-kit](https://github.com/pantheon-org/agentic-harness-dev-kit)
(`ahdk-core`, `ahdk-install`), pinned to a git tag until they are published to crates.io. The
skill in `adr-creator/` is embedded into the binary at build time and audited in CI with
[pantheon-skill-auditor](https://github.com/pantheon-org/pantheon-skill-auditor).

The tool keeps its state under `$XDG_DATA_HOME/tekhne/adr` (or `~/.local/share/tekhne/adr`), the path
it used before it moved here, so existing data stays where it is.

## History

Extracted from [pantheon-org/tekhne](https://github.com/pantheon-org/tekhne) with `git filter-repo`
(`crates/adr` and `skills/documentation/adr-creator`). Commit messages keep their original `#NNN`
references, which point at tekhne pull requests.
