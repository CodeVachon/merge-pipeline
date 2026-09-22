---
id: 16
title: package.json version-conflict resolver
state: Done
parent: 2
assignee: claude-code
labels: [core]
blockedBy: [12, 15]
created: 2026-09-17T17:13:05Z
updated: 2026-09-21T20:26:04Z
---

## Description

Port actions/resolvePackageJsonVersionConflicts.ts test-first; it is the baseline's most-tested logic. Contract: input conflicted paths; keep those matching `(^|/)package.json$`; read each; find blocks `<<<<<<< …\n<ws>"version": "A"<,?>\n=======\n<ws>"version": "B"<,?>\n>>>>>>> …` (CRLF tolerant); if none → attempted=false. Ask `conflict:auto_resolve` confirm ("Auto-resolve N package.json version conflict(s) across M file(s)?", default yes); if no → attempted=false. Group conflicts by the unordered version pair; per group with 2 distinct versions ask `conflict:version:<lo>|<hi>` select with the HIGHER version first labelled "<v> (higher)" and as default; message names the single path or "N package.json conflicts". Apply chosen line (ours or theirs, whitespace preserved) from the end of the file backwards. Write files that changed. `git add` only files with no remaining `^(<<<<<<<|=======|>>>>>>>)` markers. Re-query conflicted paths; if none remain run `git commit --no-edit --no-verify`. Return {attempted, remaining}.

Version ordering: numeric core segments (missing = 0), build metadata after `+` ignored, a version without prerelease outranks one with, prerelease identifiers compared numerically when both numeric, numeric < alphanumeric, else lexical, shorter prerelease list is lower. Use the `semver` crate where it agrees; keep a small custom comparator if `semver` rejects lenient inputs like `1.2` (baseline tolerated them).

## Plan

Port both resolvePackageJsonVersionConflicts.test.ts cases exactly (monorepo grouping → one version prompt, choices [1.10.0, 1.9.0], three git calls add/add/commit; unresolved second hunk → no git calls, version replaced but markers remain). Add: CRLF file; `ours` chosen; prerelease ordering table; non-package.json conflicts pass through untouched.

## Notes

### 2026-09-21T20:25:57Z — claude-code (agent)

src/conflicts.rs done, test-first. try_resolve_package_json_versions(cwd, conflicted_paths, &mut Git, &mut dyn Prompter) -> ResolveOutcome{attempted, remaining, rewritten, committed}. Regexes identical to the baseline (regex crate, (?m), CRLF tolerant). Version compare is a custom comparator with the baseline's rules (lenient "1.2" == "1.2.0"; build metadata ignored; no-pre > pre; numeric < alnum identifiers; shorter pre list is lower) — semver crate rejects "1.2" so it is not used here. Group key = conflict:version:<lo>|<hi>; choices highest-first with "(higher)" label and default Some(0); message "Which version should be used for N package.json conflicts?" or "...for <path>?" for one. Both baseline vitest cases ported exactly: grouped monorepo case asserts 2 prompts, the message, choice order [1.10.0, 1.9.0], default 0, and git call sequence add, add, ls-files (the re-check the baseline mocked separately), commit; unresolved case asserts no add/commit, version replaced, markers remain. Extra: non-package.json pass-through, decline leaves files alone, CRLF + choosing ours, single-path message, version ordering table, path detection. ScriptedRunner (cfg(test), crate-visible in git::fakes) replaces vi.spyOn.
