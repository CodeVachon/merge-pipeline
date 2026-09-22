---
id: 17
title: "Workflow runner: run, dry-run, test"
state: Done
parent: 2
assignee: claude-code
labels: [core]
blockedBy: [12, 14, 15, 16]
created: 2026-09-17T17:13:05Z
updated: 2026-09-21T20:28:16Z
---

## Description

Port actions/runWorkflow.ts, testWorkflow.ts, shared.ts into `runner.rs` with no direct console output — it emits `Event`s (PipelineResolved, StepsPlanned, StepStarted, Checkout, Pulled, Merged, ConflictsResolved, Pushed, StepSkippedByUser…) through a sink so the CLI renders them and the MCP tool collects them.

prepare: git.fetch(); branches = git.branch_list(); mapped = map_pipeline(...); steps = make_steps(mapped). `test` stops here. `run`/`dry-run` for each step: confirm `step:<src>>>tgt` ("Merge <src> into <tgt>?", default yes; a no aborts with `User chose not to proceed…`); checkout src, pull if has_upstream; checkout tgt, pull if has_upstream; merge; on merge failure → conflicted_paths → resolver; if remaining is non-empty raise MergeError{step, files} else continue; then if tgt has_upstream and (auto_push || confirm `push:<tgt>` default yes) → push. dry-run forces auto_push=false but STILL asks per-branch push confirmation in the baseline? No: baseline passes auto_push=false, so shouldPushBranch prompts. Preserve that (dry-run prompts to push and pushes if the user says yes — document it; the CLI epic may add a `--no-push` later).

## Plan

Integration tests on the fixture repo with ScriptedPrompter: happy path 3-step merge with pushes lands commits in the bare origin; user declines a step → error, repo left on the checked-out branch; conflict in package.json auto-resolved and committed; conflict elsewhere → MergeError listing the path and the merge left in progress (so a human can finish it, as in baseline).

## Notes

### 2026-09-21T20:28:15Z — claude-code (agent)

src/runner.rs done. Action{Run,DryRun,Test} with FromStr (accepts dry-run/dry_run/dryrun, case-insensitive) and label() via action_to_string; error string matches the baseline's "Unexpected Action ..." message. Settings{cwd, action, auto_push}. Event enum (Pipeline, Fetched, BranchesMapped, StepsPlanned, StepStarted, CheckedOut, Pulled, Merged, ConflictsDetected, ConflictsResolved, Pushed, PushSkipped{NoUpstream|Declined}) through `trait EventSink` (closures work; CollectingSink for tests). prepare() = fetch → branch_list → map_pipeline → make_steps, which is all `test` does. run_step: confirm step:<src>>><tgt> (decline → RunError::UserDeclined with baseline message) → checkout+pull source if upstream → same for target → merge; on failure conflicted_paths → package.json resolver; unresolved → MergeError{step, files, cause} with "Merge Error: a into b. N conflicted file(s)" → push if target has upstream and (auto_push || push:<branch> confirm). dry-run forces auto_push=false but still prompts per push, as the baseline did (documented oddity). Deviation: baseline has_upstream check for the target happened again inside shouldPushBranch (an extra checkout+rev-parse); here the upstream flag from update_branch is reused — same result, fewer git calls. tests/runner.rs (7, real repo): auto-push happy path lands on origin; test touches nothing; decline aborts; dry-run asks push and declines; local-only target no pull/no push; package.json conflict auto-resolved+committed+pushed with prompt keys asserted; README conflict → MergeError listing the file with merge left in progress on the target branch.
