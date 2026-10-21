---
id: 59
title: "Optional conflict command: offer to run a configured tool when a merge conflicts"
state: Done
assignee: Christopher Vachon
labels: [feature]
created: 2026-10-21T15:35:23Z
updated: 2026-10-21T15:43:45Z
---

## Description

Requested 2026-10-21. A workflow file may set `conflictCommand` (a shell command, e.g. a code generator). When a merge stops on conflicts that the package.json resolver did not clear, ask (key `conflict:tool`) whether to run it. Yes: run it in the repo, stage every previously-conflicted file that no longer has conflict markers, and if git then reports no conflicts commit with `--no-edit --no-verify` and carry on (push as usual). Conflicts remaining, or the command failing, is a MergeError that leaves the merge in progress. No: same MergeError as today.

## Plan

1. config.rs: parse optional conflictCommand. 2. conflicts.rs: try_run_conflict_command (prompt, run via sh -c / cmd /C, stage clean files, commit). 3. runner.rs: call it from merge_step after the package.json resolver; new Event::ConflictCommandRan; UI + MCP rendering. 4. doctor known field + schema/config.json. 5. Tests (conflicts unit + tests/runner.rs integration). 6. README, docs book, CHANGELOG.

## Notes

### 2026-10-21T15:37:51Z — Christopher Vachon (user)

Implemented as workflow field conflictCommand (string). Runs after the package.json resolver, only on conflicts it left behind. Decisions: prompt key conflict:tool (default yes, so --yes runs it); runs via sh -c / cmd /C in the repo root with inherited stdio; afterwards stages each previously-conflicted path with no markers (or deleted) via git add -A, commits --no-edit --no-verify only when git reports zero conflicts; a non-zero exit is ConflictError::CommandFailed and leaves the merge in progress. MCP callers answer conflict:tool via answers. Verified: fmt, clippy -D warnings, cargo test, mdbook build + check-links all green.

### 2026-10-21T15:43:45Z — Christopher Vachon (user)

Follow-up 2026-10-21: added conflictNoVerify (default true) and conflictFiles (regexes, offered when any conflicted path matches; validated at load). Intended use is resolve-merge-conflicts (stages itself, never commits, exits 1 when files remain conflicted -> stops the run, merge in progress), documented in usage.md/README.
