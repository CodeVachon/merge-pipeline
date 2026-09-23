---
id: 45
title: Git primitives for branch sync
state: Done
parent: 44
assignee: claude-code
labels: [core, cli]
created: 2026-09-22T19:23:16Z
updated: 2026-09-22T19:25:53Z
---

## Description

Add to src/git.rs, each over the existing CommandRunner so they are unit-testable with ScriptedRunner: `fetch_prune()` = `fetch --prune origin`; `remote_branches()` from `branch -r` (strip leading whitespace, drop `origin/HEAD -> …`, keep `origin/<name>` and return the `<name>` part plus the full ref); `local_branch_tracking()` from `for-each-ref --format='%(refname:short)%09%(upstream:short)%09%(upstream:track)' refs/heads/` → Vec<{name, upstream: Option<String>, gone: bool}> where gone = track contains `[gone]`; `create_tracking_branch(name, remote_ref)` = `branch --track <name> <remote_ref>`; `delete_branch(name, force)` = `branch -D|-d <name>`; `remote_default_branch()` = `symbolic-ref refs/remotes/origin/HEAD` → strip `refs/remotes/origin/`, Ok(None) when unset.

## Plan

Unit tests with ScriptedRunner for the parsers (branch -r output with HEAD pointer and whitespace; for-each-ref rows with no upstream, with upstream in sync, with [ahead 1], with [gone]). Integration tests on tests/common Fixture: delete a branch in the bare origin then fetch_prune → tracking shows gone; create a branch in origin via a temp clone or `git -C <bare> branch new origin-side` then fetch → remote_branches lists it; create_tracking_branch makes a local branch with upstream set.

## Notes

### 2026-09-22T19:25:52Z — claude-code (agent)

Added to src/git.rs: fetch_prune (`fetch --prune origin`), remote_branches (`branch -r` → RemoteBranch{name, remote_ref}, skips `origin/HEAD ->` and other remotes), local_branch_tracking (`for-each-ref --format=%(refname:short)%09%(upstream:short)%09%(upstream:track) refs/heads/` → BranchTracking{name, upstream, gone}), create_tracking_branch (`branch --track`), delete_branch (`-D`/`-d`), remote_default_branch (`symbolic-ref refs/remotes/origin/HEAD`, Ok(None) when unset). Parsers are pub fns tested with ScriptedRunner (4 unit tests). Fixture gained origin_delete_branch / origin_add_branch (both operate on the bare repo directly, no clone needed) and set_origin_head; 4 integration tests in tests/git_repo.rs cover gone detection after prune, new-branch discovery + tracking creation without checkout, -d refusing unmerged work vs -D, and origin/HEAD resolution. Decision: `gone` comes only from git's `[gone]` marker, so a branch that never had an upstream can never be classed stale. Gate: fmt/clippy clean, 175 tests green.
