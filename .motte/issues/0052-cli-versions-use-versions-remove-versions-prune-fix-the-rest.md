---
id: 52
title: "CLI: `versions`, `use`, `versions remove`, `versions prune`; fix the restart message"
state: In Progress
parent: 50
assignee: claude-code
labels: [self-update, cli]
blockedBy: [51]
created: 2026-09-23T15:16:17Z
updated: 2026-09-23T15:26:11Z
---

## Description

clap: top-level `versions` with default action list and subcommands `remove <version>...` and `prune [--keep N]`; top-level `use <version>`; all with `--json`. `use` on a missing version downloads+verifies via fetch_verified_binary then switches. Output lines: list shows `* v0.2.0  (current, running)` style markers; use/upgrade end with `now using vX.Y.Z; takes effect on your next merge-pipeline command`. Remove the `Open a new shell` line from upgrade. Not-managed → existing curl|sh hint, exit 1.

## Plan

assert_cmd tests against a fake managed layout (copy the built test binary into versions/vA/bin as done in tests/selfupdate.rs) with MERGE_PIPELINE_INSTALL_DIR: versions lists two with markers; use switches and `current` target changes; use of an uninstalled version downloads from the fake server; remove refuses current; prune --keep 1 keeps current+running; --json shapes; --help snapshot updated.
