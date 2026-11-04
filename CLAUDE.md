# pantheon-adr

Rust command line tool (`adr`) that creates, lists, checks and indexes Architecture Decision Records, plus the
bundled `adr-creator` agent skill. See `README.md` for what it does and how it is installed.

## Layout

- `crates/adr/`: the only workspace member. `src/lib.rs` documents each module; `tests/integration.rs` holds the
  integration tests; `docs/design/` holds the design notes.
- `skills/adr-creator/`: the skill (`SKILL.md`, `references/`, `scripts/`, `evals/`). `build.rs` embeds it in the
  binary, so a change here changes the binary.
- `.github/workflows/`: CI. `rust-ci.yml` is the source of truth for what must pass.
- `.context/`: gitignored working state. Never commit it.

## Commands

Tools are pinned in `mise.toml` (`mise install` sets them up and installs the git hooks).

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --workspace --locked
cargo test --workspace --locked
mise run lint        # markdown, via rumdl
mise run check       # every hk check, the same steps as the git hooks
```

Run `cargo` with `--locked`: CI does. When checking test results, require `0 failed` on every suite; do not add up
the "passed" counts.

## Conventions

- Branch off `main`, never commit to it. Conventional commits with a scope, for example `fix(cli): reject an empty
  title`; the squash-merge subject ends in the PR number.
- PR bodies use the three headings in `.github/pull_request_template.md`: What, Why, Notes.
- Git hooks run through `hk` (`hk.pkl`): rustfmt, rumdl, yamllint and shellcheck on commit; clippy and tests on push.
  Do not skip them.
- Markdown is linted by rumdl with `.rumdl.toml`. Shell scripts must pass shellcheck.
- The Rust toolchain version is set in `rust-toolchain.toml`, `mise.toml` and `rust-ci.yml`. Change all three together.
- The `pahdk-*` dependencies are pinned to a git tag in the root `Cargo.toml`. Bump the tag on purpose.
- State is stored under `$XDG_DATA_HOME/tekhne/adr` (or `~/.local/share/tekhne/adr`). Keep that path.
- Behaviour is a pure function of on-disk content plus an injectable "today", so tests need no clock.
