# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.1.0] - unreleased

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

[Unreleased]: https://github.com/CodeVachon/merge-pipeline/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/CodeVachon/merge-pipeline/releases/tag/v0.1.0
