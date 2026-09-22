---
id: 25
title: run_workflow tool over the non-interactive Prompter
state: Done
parent: 4
assignee: claude-code
labels: [mcp]
blockedBy: [17, 23]
created: 2026-09-17T17:13:33Z
updated: 2026-09-21T20:53:39Z
---

## Description

Args: workflow, selections, auto_push (bool, default false), version_strategy ("higher"|"ours"|"theirs"|"fail", default "fail"), confirm (must be true), config, cwd. Builds a ScriptedPrompter that answers step:* confirms yes, push:* per auto_push, branch:* from selections, conflict:auto_resolve per strategy, conflict:version:* by strategy. Any UnansweredQuestion → isError with {question, choices}. Refuses on dirty repo. Returns {steps:[{source,target,outcome}], events} and on MergeError {conflicted_paths}.

## Notes

### 2026-09-21T20:53:26Z — claude-code (agent)

run_workflow {workflow, confirm(required true), selections, auto_push=false, version_strategy, abort_on_conflict=true, answers, cwd, config}. Builds a ScriptedPrompter: step:* → yes, push:* → auto_push, branch:<pattern> from selections, conflict:auto_resolve → strategy != fail, conflict:version:* → Index(0) for higher / Index(1) for lower; `answers` (key → bool|string|integer) override everything so any exact question key can be pre-answered. Deviations from the epic text, deliberate: (1) version_strategy is higher|lower|fail, not higher|ours|theirs — the resolver's question key `conflict:version:<lo>|<hi>` is symmetric and carries no ours/theirs, so those cannot be honoured without changing the core; an agent wanting a specific side passes the exact version via `answers`. (2) Added abort_on_conflict (default true): the baseline leaves a conflicted merge in progress for the human; an unattended agent should not leave the repo mid-merge, so the tool runs `git merge --abort` after capturing conflicted_paths and reports merge_aborted/merge_in_progress. (3) Result includes starting_branch/current_branch because the run leaves HEAD on the last target, as the CLI does. Unanswered questions → isError with data.question{key,message,choices} plus the events collected so far. Dirty tree refused with the baseline's exact message.
