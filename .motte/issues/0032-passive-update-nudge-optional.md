---
id: 32
title: Passive update nudge (optional)
state: Done
parent: 5
assignee: claude-code
labels: [self-update]
blockedBy: [28, 30]
created: 2026-09-17T17:13:55Z
updated: 2026-09-21T20:43:34Z
---

## Description

On interactive runs only (not `mcp`, not --json, stdout is a tty), if update-check.json is older than 24h, do a background-free best-effort check with a 1.5s timeout and print one dim stderr line `merge-pipeline vX is available — merge-pipeline upgrade` at the end. Opt out with MERGE_PIPELINE_NO_UPDATE_CHECK=1. Never fail the run because of it.

## Notes

### 2026-09-21T20:39:33Z — claude-code (agent)

Implemented the mechanism but not the hook: selfupdate::nudge::maybe_line() -> Option<String> returns the one-line "merge-pipeline vX is available (you have vY) — merge-pipeline upgrade" when running from a managed install and a newer release is known. Uses update-check.json when younger than 24h; otherwise one lookup with a 1.5s timeout, cached. Silent for dev builds, when MERGE_PIPELINE_NO_UPDATE_CHECK or CI is set, or on any failure. Not yet called from main.rs, which the CLI owner (#21) holds: call it after "Work Complete" on interactive runs only (not `mcp`, not --json, stdout is a tty) and print to stderr, dimmed. Leaving this issue Todo for that one-line wiring.

### 2026-09-21T20:43:32Z — claude-code (agent)

Wired in main.rs (CLI epic picked this up as a handoff from the self-update agent, which wrote selfupdate::nudge::maybe_line). Called only for the root interactive command, after "Work Complete", and only when stdout is a terminal; `mcp`, `upgrade` and the other subcommands never reach it. The call is wrapped in std::panic::catch_unwind and its Option is simply ignored when None, so nothing inside the check can change the run's exit code. The line is printed dimmed to stderr. Gating (24h cache in update-check.json, 1.5s lookup timeout, MERGE_PIPELINE_NO_UPDATE_CHECK / CI opt-out, managed-install only) lives in nudge.rs and is the self-update agent's; the CLI tests set MERGE_PIPELINE_NO_UPDATE_CHECK=1 and never run on a tty anyway. Not exercised by an automated test: it needs a managed install and a tty; verify manually after the first release with `merge-pipeline -a test` from an older installed version.
