---
id: 44
title: "Epic: branch sync before a workflow runs (prune stale, fetch new)"
state: Done
labels: [core, cli]
blockedBy: [40]
created: 2026-09-22T19:22:47Z
updated: 2026-09-22T19:35:45Z
---

## Description

Requested 2026-09-22. Recurring pain: a Patch branch is merged and deleted on origin, so the next `merge-pipeline` run either picks a dead local Patch branch or cannot see the new one. Before the selected workflow starts, sync local branches against origin for every pattern in the pipeline: remove local branches whose upstream is gone (after prompting), and create local tracking branches for new remote branches that match.

Behaviour (runs for run, dry-run and test, right after the existing fetch and before pattern→branch mapping):
1. `git fetch --prune origin` instead of plain `git fetch`, so deleted remote branches disappear from refs/remotes. (Interpretation of "pull against origin main": fetch from origin to learn its state; local `main` is not pulled here because each step already pulls its own branches. Easy to add later if wanted.)
2. Stale detection uses git's own signal: `git for-each-ref --format='%(refname:short)%09%(upstream:short)%09%(upstream:track)' refs/heads/`, where `[gone]` means the configured upstream no longer exists. Only branches matching a pipeline pattern AND marked gone are candidates. Local branches that never had an upstream are never proposed for deletion (they were never pushed, so they are not ours to clean up).
3. For each stale candidate, prompt `sync:delete:<branch>` — "Local branch <b> no longer exists on origin. Delete it?" default yes — and on yes run `git branch -D <b>` (force, because the merge happened on origin and local HEAD may not contain it). If the stale branch is the checked-out branch, first checkout the remote default branch (from `git symbolic-ref refs/remotes/origin/HEAD`, fallback `main`, else skip with a warning). `--yes` answers yes.
4. New remote branches: `git branch -r` entries `origin/<name>` (excluding `origin/HEAD`) matching a pattern with no local `<name>` get `git branch --track <name> origin/<name>` automatically, each reported as a line. No prompt: fetching is non-destructive and the user asked for it to just happen.
5. `--no-sync` on the root command skips the whole step (baseline behaviour). Events: SyncStarted, StaleBranch{branch, deleted: bool}, BranchCreated{branch}, SyncSkipped.
6. MCP: plan_workflow and run_workflow gain `sync: {stale: "keep"|"delete" (default keep), fetch_new: bool (default true)}` and report `sync: {stale: [...], deleted: [...], created: [...]}`; non-interactive never deletes unless told to.

## Plan

Children: git primitives (#45), sync module + runner integration (#46), CLI flag/rendering/e2e (#47), MCP args + docs (#48). Blocked by epic #40 only to keep one agent in the runner/CLI/MCP files at a time, not for a code reason.

## Notes

### 2026-09-22T19:35:44Z — claude-code (agent)

Epic complete 2026-09-22 (#45–#48 Done). Interpretation recorded: "pull against origin main" was implemented as `git fetch --prune origin` before mapping; local `main` itself is not pulled (each step still pulls its own branches). Deviations from the plan text: none of substance; SyncEvent carries a Warning variant for the "checked-out stale branch with no default branch to switch to" case, and MCP `sync` also accepts a bare boolean. Not committed — 195 tests green, ready for the user to review and commit.
