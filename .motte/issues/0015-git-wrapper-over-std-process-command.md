---
id: 15
title: Git wrapper over std::process::Command
state: Done
parent: 2
assignee: claude-code
labels: [core]
blockedBy: [9]
created: 2026-09-17T17:13:05Z
updated: 2026-09-21T20:22:41Z
---

## Description

Port utl/GitApi.ts. `Git::new(cwd).verbose(bool)`; `call(args: &[&str]) -> Result<String>` (stdout trimmed; on non-zero exit return GitError{args, status, stderr}); `branch_list` (strip `* `, drop blanks); `current_branch`; `checkout(branch)`; `checkout_new(branch)` erroring if exists; `fetch`; `pull`; `merge(src)` = merge <src> --no-verify; `push()` = push --no-verify, `push_upstream(remote)` = push -u <remote> <current> --no-verify; `has_upstream()` via rev-parse --abbrev-ref --symbolic-full-name @{u}, mapping stderr containing "no upstream configured" to Ok(false); `conflicted_paths()` parsing `ls-files --unmerged` lines `^\S+\s+\S+\s+\S+\t(.*)$`, deduplicated in first-seen order, spaces preserved; `is_dirty()`. Verbose logging emits `$ git <args>` through a `Log` sink (not println directly) with whitespace/quote-containing args JSON-quoted.

Deliberate behaviour change: baseline is_dirty used `git diff --stat`, which misses staged changes. Use `git status --porcelain --untracked-files=no` non-empty instead. Record the change in the README migration notes.

## Plan

Unit-test parsing with a `CommandRunner` trait so `call` can be faked (mirrors vi.spyOn(git, 'call')): port the three GitApi.test.ts cases. Add a `tests/fixtures.rs` helper that builds a real temp repo with a local bare `origin`, branches main/staging-patch/staging-release/Release-0.1.0/Patch-v0.1.1/Patch-v0.1.2/Release-0.2.0 (the bootstrap.sh set), and integration-test has_upstream, push, merge, conflicted_paths against it.

## Notes

### 2026-09-21T20:22:40Z — claude-code (agent)

src/git.rs done. Command execution behind `trait CommandRunner { run(cwd, args, envs) -> CommandOutput }` with SystemRunner; Git::with_runner lets tests fake stdout exactly like vi.spyOn(git,'call'). Git::env(k,v) adds per-call env so tests isolate from global config (GIT_CONFIG_GLOBAL=/dev/null, GIT_CONFIG_NOSYSTEM, GIT_CONFIG_COUNT/KEY/VALUE for commit.gpgsign=false, core.hooksPath=<empty dir>, init.defaultBranch=main). Verbose via `Git::verbose(impl CommandLog)` (closures work) emitting `git <args>` with whitespace/quote args JSON-quoted. API: call, fetch, branch_list, current_branch, checkout, checkout_new (BranchExists error, baseline message), is_dirty, pull, merge (--no-verify), has_upstream (maps "no upstream configured" stderr to Ok(false)), branch_has_upstream (checks out first, as baseline), push (--no-verify), push_upstream(remote), conflicted_paths + pub parse_unmerged. Deliberate change kept: is_dirty = `git status --porcelain --untracked-files=no` non-empty, so staged changes count and untracked still don't (integration test asserts all three). Ported all 3 GitApi.test.ts cases as unit tests with the fake runner; tests/common/mod.rs Fixture builds a temp work repo + bare origin with the bootstrap.sh branch set (6 pushed with upstream, 3 local-only); tests/git_repo.rs covers branch list, has_upstream both ways, is_dirty, merge+push landing on origin, conflict -> conflicted_paths, checkout_new.
