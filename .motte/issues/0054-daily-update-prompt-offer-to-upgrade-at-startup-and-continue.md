---
id: 54
title: "Daily update prompt: offer to upgrade at startup and continue on the new version"
state: In Progress
assignee: claude-code
labels: [self-update, cli]
created: 2026-09-23T15:52:15Z
updated: 2026-09-23T15:52:48Z
---

## Description

Requested 2026-09-23. At most once per day, when merge-pipeline starts an interactive run, check for a newer release and ask the user whether to update now. Replaces the passive after-run nudge (#32), which only printed a line.

Behaviour:
- Applies to the root interactive command only (not mcp, upgrade, use, versions, install, config, completion, and not with --json or when stdout is not a tty). Skipped when MERGE_PIPELINE_NO_UPDATE_CHECK is set, when the binary is not a managed install (cargo target/, so upgrading is impossible), or when MERGE_PIPELINE_REEXEC=1 (see below).
- Timing: right after the title banner, before config is loaded, so the prompt is the first thing the user sees and the run that follows is on the new version.
- Once per day: reuse update-check.json (lastAttemptAt). Any attempt, including a declined prompt or a failed lookup, counts, so the user is asked at most once per 24h. Lookup timeout stays 1.5s; network failures are silent.
- Prompt via the Prompter (key `update:upgrade`): "merge-pipeline vX is available (you have vY). Update now?" default yes. `--yes` answers yes.
- On yes: run the same code path as `upgrade` (download, verify, install, repoint current, prune), print its normal lines, then re-execute the new binary with the identical argv and MERGE_PIPELINE_REEXEC=1 so the current run continues on the new version without asking again: `exec` on unix (std::os::unix::process::CommandExt), spawn-and-wait with the child's exit code on Windows. If the upgrade fails, print the error as a warning and continue the run on the current version.
- On no: print `skipping; run merge-pipeline upgrade any time` dimmed and continue.
- `versions --check`, `upgrade --check` and the nudge module share the release lookup and the cache record; remove the after-run nudge line so the user is not told twice.
- Tests: unit tests for the once-per-day gate using an injected clock and a temp update-check.json; end-to-end with the fake managed layout + fake release server: declined → run proceeds and cache updated; accepted with --yes → new version installed, current repointed, and the child process ran (assert on an env-visible marker or on `--version` output after re-exec); second run within 24h → no prompt; MERGE_PIPELINE_NO_UPDATE_CHECK → no prompt; unmanaged → no prompt. README "Keeping it up to date" and CHANGELOG Unreleased.

## Plan

Implement in src/selfupdate/nudge.rs (rename to update_prompt.rs or keep, note the choice), wire in src/cli/interactive.rs before config loading with the InteractivePrompter/AssumeDefaults already in scope. Work on branch feat/update-prompt stacked on feat/version-management (PR #2 still open); open a `feature` PR with base feat/version-management.
