# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.3.0] - 2026-09-24

### Added

- Once a day, an interactive run starts by asking whether to install a newer release if one
  exists. Yes (the default, also answered by `--yes`) upgrades in place and continues the same
  run on the new version; no continues on the current version. Off with
  `MERGE_PIPELINE_NO_UPDATE_CHECK=1` or under `CI`; never shown for a binary that is not a
  managed installation.
- `merge-pipeline versions [--check] [--json]` lists the versions installed under
  `~/.merge-pipeline/versions/`, marking the active one and the one running.
- `merge-pipeline use <version>` switches the active version, downloading and verifying it
  first when it is not on disk.
- `merge-pipeline versions remove <version>...` and `merge-pipeline versions prune [--keep N]`
  delete old versions; the active version and the one running are never removed.
- A full documentation site at <https://codevachon.github.io/merge-pipeline/>, covering
  everything above plus the MCP tool reference, built with mdBook from `docs/`.

### Changed

- The dimmed "an update is available" line that appeared after a run is gone; the update offer
  above replaces it, so you are told once and can act on it.
- `upgrade` no longer says to open a new shell. It never was needed: PATH points at the stable
  `current` link, so every version change applies to the next command. `upgrade` and `use` now
  end with `now using vX.Y.Z; takes effect on your next merge-pipeline command`.
- Development now goes through pull requests; release notes are generated from PR titles
  and labels (`.github/release.yml`).

## [0.2.0] - 2026-09-23

### Added

- `merge-pipeline config doctor [-f DIR] [-c REPO] [--json]` validates every workflow file:
  JSON, `name`, `pipeline` length, each pattern as a regular expression, duplicate names,
  unknown keys, mixed `order`, and, given a repository, which local branches each pattern
  matches. Exit 1 only when a run would fail.
- `merge-pipeline config init [--scope project|user] [--force]` writes the default workflow
  files (the four in `examples/config/`, embedded in the binary) without overwriting yours.
- `merge-pipeline install` now also creates the workflow directory for the chosen scope;
  `--no-config` opts out. Running it twice changes nothing.
- MCP tools `doctor_config` and `init_config`, returning the same JSON as the two commands.
- Branch sync before every action: `git fetch --prune origin`, then for branches matching the
  workflow's patterns, a prompt to delete local branches whose upstream is `[gone]` (default
  yes) and automatic local tracking branches for new remote branches. `--no-sync` restores the
  plain fetch. `plan_workflow` and `run_workflow` take a `sync` argument (`{stale: keep|delete,
  fetch_new}` or `false`; the default never deletes) and report `sync` in their results.

### Changed

- The pre-run fetch is now `git fetch --prune origin` (was `git fetch`).

## [0.1.0] - 2026-09-22

First release of the Rust rewrite of `@codevachon/cli-merge-pipeline`. Functionally equivalent
to the Bun/TypeScript tool for the interactive workflow run; everything else below is new.
See [MIGRATING.md](MIGRATING.md) for the upgrade path.

### Added

- Single self-contained binary, `merge-pipeline`, with `git` as the only runtime prerequisite.
- Interactive workflow run with the baseline's flags (`-c/--cwd`, `-w/--workflow`,
  `-p/--auto-push`, `-f/--config`, `-a/--action run|dry-run|test`) plus `-y/--yes` to answer
  every confirmation with its default, which makes scripted runs possible.
- Workflow directory search order: `--config`, `$MERGE_PIPELINE_CONFIG`,
  `<cwd>/.merge-pipeline/`, the user config directory, then `<binary dir>/config`;
  `merge-pipeline config path` reports which rule won.
- Automatic resolution of `package.json` `"version"` merge conflicts, grouped by version pair
  across a monorepo, with the higher version offered first (ported from the baseline).
- `merge-pipeline mcp`: Model Context Protocol server over stdio for agents, and
  `merge-pipeline install` to wire it into `.mcp.json`.
- `merge-pipeline upgrade [target] [--check] [--keep N] [--force] [--json]` and
  `merge-pipeline uninstall [--yes] [--json]`, mirroring motte's managed-install layout under
  `~/.merge-pipeline` with checksum-verified release assets.
- A once-a-day, best-effort update notice at the end of interactive runs
  (opt out with `MERGE_PIPELINE_NO_UPDATE_CHECK=1`).
- `install.sh` (POSIX) and `install.ps1` installers.
- `merge-pipeline completion <shell>` for bash, zsh, fish, elvish and powershell.
- Published JSON schema for workflow files at
  `https://raw.githubusercontent.com/CodeVachon/merge-pipeline/main/schema/config.json`,
  with the four example workflows under `examples/config/`.
- Hermetic test suite: unit tests beside the code and integration tests that build their own
  git repositories with a local bare `origin`; no network access.
- GitHub Actions CI (fmt, clippy, tests, schema validation) and a tag-gated release workflow
  building darwin-arm64, darwin-x64, linux-x64, linux-arm64 and windows-x64.

### Changed

- The dirty-tree guard uses `git status --porcelain --untracked-files=no` instead of
  `git diff --stat`, so staged-but-uncommitted changes are also refused.
- `--auto_push` is spelled `--auto-push`; the old spelling is accepted as a hidden alias.
- A workflow whose `pipeline` has fewer than two entries is reported as a load error instead
  of being accepted.
- Workflow files are no longer expected beside the executable; that location is the last
  fallback rather than the first choice.

### Removed

- `bootstrap.sh`, the `data/` directory, and all Node/Bun tooling.

[Unreleased]: https://github.com/CodeVachon/merge-pipeline/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/CodeVachon/merge-pipeline/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/CodeVachon/merge-pipeline/releases/tag/v0.2.0
[0.1.0]: https://github.com/CodeVachon/merge-pipeline/releases/tag/v0.1.0
