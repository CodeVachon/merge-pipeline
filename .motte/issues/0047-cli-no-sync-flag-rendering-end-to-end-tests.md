---
id: 47
title: "CLI: --no-sync flag, rendering, end-to-end tests"
state: Done
parent: 44
assignee: claude-code
labels: [core, cli]
blockedBy: [46]
created: 2026-09-22T19:23:16Z
updated: 2026-09-22T19:32:38Z
---

## Description

Root flag `--no-sync` ("Skip pruning stale local branches and fetching new ones before the workflow"). Renderer lines: `Syncing branches with origin…`, `  ✗ deleted Patch-v0.1.1 (gone from origin)`, `  · kept Patch-v0.1.1`, `  ✓ created Patch-v0.1.3 tracking origin/Patch-v0.1.3`, and the SyncSkipped reason. `--yes` makes deletes automatic (AssumeDefaults already does this via the confirm default). Update `--help` snapshot test.

## Plan

assert_cmd tests on the Fixture with a stale + new branch: `-a test --yes` output shows the delete and create lines and the step list uses the new branch; `--no-sync -a test --yes` shows neither and still lists the stale branch as a candidate (proving the flag works).

## Notes

### 2026-09-22T19:32:37Z — claude-code (agent)

Root flag `--no-sync` (RunArgs.no_sync → Settings.sync = None). ui.rs format_sync_event renders: "Syncing branches with origin...", "  ✗ deleted <b> (gone from origin)", "  · kept <b> (gone from origin)", "  ✓ created <b> tracking origin/<b>", "  ! <warning>", "Branch sync skipped (<reason>)"; StdoutRenderer forwards Event::Sync. `--yes` answers the delete confirm with its default (yes) via AssumeDefaults. Tests: clap parse test, ui unit test for each line, help snapshot needles for -y/--yes and --no-sync, and two assert_cmd e2e tests on the Fixture with origin drifted (Patch-v0.1.1 deleted, Patch-v0.1.3 added): `-a test -y` prints fetch --prune, the delete and create lines and plans "Merge Patch-v0.1.3 into staging-patch"; `--no-sync -a test -y` prints plain fetch, "Branch sync skipped (disabled)", keeps the dead branch and plans against it. Gate: fmt/clippy clean, 191 tests.

Real output of target/release/merge-pipeline -c <repo> -a test --yes on a throwaway repo where origin deleted Patch-v0.1.1 and added Patch-v0.1.2:

Pipeline: ^Patch-* > ^staging-patch$
$ git fetch --prune origin
$ git for-each-ref --format=%(refname:short)%09%(upstream:short)%09%(upstream:track) refs/heads/
$ git branch -r
Syncing branches with origin...
$ git rev-parse --abbrev-ref HEAD
$ git branch -D Patch-v0.1.1
  ✗ deleted Patch-v0.1.1 (gone from origin)
$ git branch --track Patch-v0.1.2 origin/Patch-v0.1.2
  ✓ created Patch-v0.1.2 tracking origin/Patch-v0.1.2
$ git branch

Step 1: Merge Patch-v0.1.2 into staging-patch

Task Complete
Work Complete

Afterwards `git branch -vv` shows Patch-v0.1.2 tracking origin/Patch-v0.1.2 and no Patch-v0.1.1.
