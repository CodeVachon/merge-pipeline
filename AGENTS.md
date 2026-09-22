# Agent instructions

## Build and verification

This is a Rust crate (`Cargo.toml` at the repo root, toolchain pinned by `rust-toolchain.toml`).
Before finishing any issue, all three of these must pass:

```
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Rules for tests:

- Tests must not touch the network. Anything that needs a git remote builds its own fixture
  repository under a temporary directory (`tempfile`), with a local bare repository standing in
  for `origin`. `tests/common/mod.rs` provides `Fixture::new()`, which creates exactly that: a
  work repo, a bare `origin`, the baseline `bootstrap.sh` branch set, and an environment that
  isolates git from the user's global config, hooks and commit signing.
- Anything that talks HTTP (release lookup, download) takes an injectable base URL and is tested
  against a `std::net::TcpListener` fake in `tests/selfupdate.rs`.
- Integration tests drive the built binary with `assert_cmd`; unit tests live beside the code.
  Strip ANSI (`merge_pipeline::ui::strip_ansi`) before asserting on terminal output.
- Every interactive question goes through the `Prompter` trait (`src/prompt.rs`). In tests use
  `ScriptedPrompter`; the CLI uses `InteractivePrompter` (inquire) and `--yes` wraps it in
  `AssumeDefaults`.

`cargo run -- --help` is the development entry point. `cargo build --release` produces the
single self-contained binary at `target/release/merge-pipeline`. `install.sh` and
`src/selfupdate/layout.rs` implement the same on-disk layout; a test reads the script to keep
them in agreement, so change both together.

User-facing behaviour is documented in `README.md`; deviations from the original Bun tool go in
`MIGRATING.md` and `CHANGELOG.md`. Keep those in step with code changes.

<!-- motte:start -->

## Tracking work with motte

This project's work lives in `.motte/issues/` as Markdown files, committed alongside the code. Track
work there rather than in an ad-hoc TODO list, a commit message, or a pull request description.

Start by asking what to do:

```
motte next --why
```

That is not `motte list`, and not quite `motte ready` either. **Ready** means unsettled with every
blocker settled; `next` orders that set by what a piece of work would unblock, how close it is to a leaf,
and how long it has waited — and it leaves out anything somebody else holds. `motte ready --blocked` shows
what is waiting and on what.

The loop for an issue you pick up:

1. Claim it — `motte claim <ref>`. If that fails, somebody else is on it: ask `motte next` again.
2. Read it — `motte show <ref>`
3. Refine its **Plan** if the plan on the file is not what you are actually going to do
4. Add notes as you go, especially for decisions and dead ends — `motte note <ref> "..."`
5. Move it to Done when the verification for that issue passes, or `motte release <ref>` if you stop

A `<ref>` is an issue number or a fragment of the title, so `motte show parser` works as well as
`motte show 12`.

If you discover a prerequisite mid-task, record it with `motte block <ref> <blocker>` rather than
describing it in prose. Prose is not queryable, and `motte ready` is what the next agent reads.

Every read command accepts `--json`. The MCP server exposes the same operations, and notes written
through it are attributed to the agent rather than to the repository's git user — which is the point:
one shared record of who decided what.

<!-- motte:end -->
