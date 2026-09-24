---
id: 52
title: "CLI: `versions`, `use`, `versions remove`, `versions prune`; fix the restart message"
state: Done
parent: 50
assignee: claude-code
labels: [self-update, cli]
blockedBy: [51]
created: 2026-09-23T15:16:17Z
updated: 2026-09-23T15:29:01Z
---

## Description

clap: top-level `versions` with default action list and subcommands `remove <version>...` and `prune [--keep N]`; top-level `use <version>`; all with `--json`. `use` on a missing version downloads+verifies via fetch_verified_binary then switches. Output lines: list shows `* v0.2.0  (current, running)` style markers; use/upgrade end with `now using vX.Y.Z; takes effect on your next merge-pipeline command`. Remove the `Open a new shell` line from upgrade. Not-managed → existing curl|sh hint, exit 1.

## Plan

assert_cmd tests against a fake managed layout (copy the built test binary into versions/vA/bin as done in tests/selfupdate.rs) with MERGE_PIPELINE_INSTALL_DIR: versions lists two with markers; use switches and `current` target changes; use of an uninstalled version downloads from the fake server; remove refuses current; prune --keep 1 keeps current+running; --json shapes; --help snapshot updated.

## Notes

### 2026-09-23T15:29:01Z — claude-code (agent)

Commit e8dc25e on feat/version-management. clap: `versions [--check] [--json]` with subcommands `remove <VERSION>... [--json]` and `prune [--keep N] [--json]`; top-level `use <VERSION> [--json]`. Adapters in selfupdate/mod.rs; main.rs dispatch. upgrade's closing "Open a new shell" line replaced by `now using vX (was vY); takes effect on your next merge-pipeline command`. tests/versions.rs (7 e2e, unix-only) copies the test binary into versions/v<crate version>/bin so locate_install sees a managed layout, and serves a fake release for `use` of an uninstalled version (also asserts a checksum mismatch writes nothing). Whole crate: 208 tests, fmt + clippy -D warnings clean.

Manual run against a fake layout (debug build):
$ merge-pipeline versions
* v0.2.0  current, running
  v0.1.0
$ merge-pipeline use 0.1.0
now using v0.1.0 (was v0.2.0); takes effect on your next merge-pipeline command
$ merge-pipeline versions remove 0.1.0
! kept v0.1.0: it is the active version; `merge-pipeline use <other>` first   (exit 1)
$ merge-pipeline use 0.2.0 && merge-pipeline versions remove 0.1.0
✓ removed v0.1.0
$ merge-pipeline versions prune --keep 1
✓ removed v0.0.2
✓ removed v0.0.1
--json shapes: list {installRoot,current,running,versions[{version,current,running}],latest}; use {from,to,downloaded,installRoot}; remove/prune {removed,kept[{version,reason}],missing,installRoot}.
