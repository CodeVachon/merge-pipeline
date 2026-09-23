---
id: 46
title: Sync module and runner integration
state: Done
parent: 44
assignee: claude-code
labels: [core, cli]
blockedBy: [45]
created: 2026-09-22T19:23:16Z
updated: 2026-09-22T19:30:31Z
---

## Description

New src/sync.rs: `plan_sync(patterns, tracking, remote) -> SyncPlan { stale: Vec<String>, new_remote: Vec<(name, remote_ref)> }` as a pure function (a local branch is stale iff it matches any pattern case-insensitively AND gone; a remote branch is new iff it matches any pattern AND no local branch has that name), and `apply_sync(git, plan, prompter, sink, opts) -> SyncReport { stale, deleted, kept, created }` that asks `sync:delete:<branch>` per stale branch (message "Local branch <b> no longer exists on origin. Delete it?", default yes), checks out the remote default branch first if the stale branch is current (warning + skip if none), deletes with force, and creates tracking branches for every new remote without prompting. Runner: replace `git.fetch()` in prepare with fetch_prune + sync unless `Settings.sync == false`; run it for all three actions before mapping. Events: SyncStarted, StaleBranch{branch, deleted}, BranchCreated{branch}, SyncSkipped{reason}. RunReport gains the SyncReport.

## Plan

Unit tests for plan_sync (matching is case-insensitive; never-pushed local branch not stale; branch matching no pattern ignored even if gone; staging-patch present on origin never stale). Integration on Fixture: origin deletes Patch-v0.1.1 and adds Patch-v0.1.3; run with ScriptedPrompter answering yes → Patch-v0.1.1 gone locally, Patch-v0.1.3 exists with upstream origin/Patch-v0.1.3, subsequent mapping of ^Patch-* offers only v0.1.2/v0.1.3; answering no → kept and reported; current branch stale → HEAD moved to main first; sync disabled → nothing changes and SyncSkipped emitted.

## Notes

### 2026-09-22T19:30:29Z — claude-code (agent)

New src/sync.rs: StalePolicy {Ask (CLI default), Keep (MCP default), Delete}, SyncOptions {stale, fetch_new}, pure plan_sync(patterns, tracking, remote) → SyncPlan {stale, new_remote} (stale = matches a pattern AND gone AND had an upstream; new = remote match with no local of that name), apply_sync(git, plan, options, prompter, emit) → SyncReport {stale, deleted, kept, created}. Prompt key `sync:delete:<branch>`, message "Local branch <b> no longer exists on origin. Delete it?", default yes. Before deleting the checked-out branch it moves to origin/HEAD's branch, else a local main/master, else warns and keeps. Runner: Settings.sync: Option<SyncOptions> (None = baseline plain `git fetch`, emits Sync(Skipped{disabled})); with Some it runs `fetch --prune origin` then sync before mapping, for all three actions. prepare() now takes settings and returns a Prepared struct (clippy type_complexity); RunReport.sync added; Event::Sync(SyncEvent) with Started/StaleBranch/BranchCreated/Warning/Skipped; RunError::Sync. pipeline.rs gained pub pattern_matches(). MCP run_workflow event_json handles the new events; its Settings uses sync: None until #48. Tests: 5 unit in sync.rs, 7 integration in tests/runner.rs (delete on yes + new branch tracked + mapping offers only survivors; kept on no; checked-out stale branch moves to origin/HEAD; fallback to local main when origin/HEAD unset; sync disabled changes nothing; Keep policy never prompts; Delete policy with fetch_new=false). Gate: fmt/clippy clean, 187 tests green.
