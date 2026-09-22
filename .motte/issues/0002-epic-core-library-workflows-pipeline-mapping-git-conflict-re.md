---
id: 2
title: "Epic: Core library — workflows, pipeline mapping, git, conflict resolution"
state: Done
labels: [core]
blockedBy: [1]
created: 2026-09-17T17:11:10Z
updated: 2026-09-21T20:28:23Z
---

## Description

Port the baseline's business logic into `src/lib.rs` modules with no terminal I/O, so the CLI and the MCP server share one implementation. Every behaviour below is observed in the baseline source and its tests; parity is the acceptance bar unless an issue says otherwise.

Config model (schema/config.json + utl/Workflows.ts):
- fields: disabled (bool, default false), name (string, required), description (string, default ""), order (optional integer; lower first), pipeline (array of ≥2 strings; each is a regex).
- Discovery: if `<dir>/config` is a directory use it as root, else `<dir>`; walk recursively; every `*.json` file is a workflow; unreadable/unparseable files are reported and skipped, not fatal.
- Sort: workflows with a finite `order` first ascending, then the rest in filename (localeCompare) order. Test: Workflows.test.ts.
- Enabled = disabled == false. Name lookup is exact match among enabled; error lists valid names quoted and comma-separated.

Pipeline → branches (utl/mapPipelineValueToBranch.ts): for each pattern, case-insensitive regex search over local branch names, excluding branches already mapped earlier in the pipeline. 0 matches → error `No matching branch found for "<pattern>"`. 1 → take it. >1 → ask (via Prompter). Steps are consecutive pairs (a→b, b→c). Test: makeWorkflowSteps.test.ts.

Git (utl/GitApi.ts): branch list from `git branch` stripping the `*` marker; current branch via rev-parse --abbrev-ref HEAD; checkout; pull; fetch; merge `<src> --no-verify`; push `--no-verify` (with `-u <remote> <branch>` variant); upstream detection via `rev-parse --abbrev-ref --symbolic-full-name @{u}` treating "no upstream configured" as false; conflicted paths from `ls-files --unmerged` parsing `<mode> <sha> <stage>\t<path>`, deduplicated, spaces preserved (GitApi.test.ts). Verbose mode logs each command as `$ git ...` with args containing whitespace or quotes JSON-quoted.

Conflict resolver (actions/resolvePackageJsonVersionConflicts.ts): see child issue for the full contract; it is the most-tested piece in the baseline and must be ported test-first.

## Plan

Children cover: config loading, pipeline mapping + steps, git wrapper, Prompter trait, package.json version conflict resolver, and the workflow runner (run/dry-run/test) that composes them. Port baseline tests alongside each module; add hermetic git fixture helpers (tempfile + a local bare repo as `origin`) so nothing touches the network — the baseline's bootstrap.sh cloned a real GitHub repo for manual testing, which the Rust tests must not need.

## Notes

### 2026-09-21T20:28:22Z — claude-code (agent)

Epic complete. Baseline vitest → Rust mapping: makeWorkflowSteps.test.ts → pipeline::tests::make_steps_returns_an_array_of_objects; Workflows.test.ts → config::tests::sorts_workflows_by_order_and_keeps_file_order_as_the_fallback; GitApi.test.ts (3) → git::tests::conflicted_paths_{returns_an_empty_list…, returns_unique_paths…, preserves_paths_containing_spaces}; resolvePackageJsonVersionConflicts.test.ts (2) → conflicts::tests::{resolves_grouped_monorepo_version_conflicts_and_commits_the_merge, does_not_stage_or_commit_when_a_package_json_still_has_remaining_conflicts}; plural/capitalize/convertActionToString/arrayToStringList tests → text::tests::*. Totals: 42 unit + 6 git integration + 7 runner integration + 2 CLI smoke = 57, fmt/clippy clean. No network in tests; tests/common/mod.rs Fixture builds the bootstrap.sh branch set against a local bare origin with global git config isolated. Public surface for the CLI/MCP epics: config::{load_workflows, LoadResult, WorkflowConfig, ConfigError}, pipeline::{map_pipeline, map_pipeline_report, make_steps, Step}, git::Git (+ verbose(), env()), conflicts::try_resolve_package_json_versions, prompt::{Prompter, ScriptedPrompter, Answer, Question, Choice}, runner::{run_workflow, prepare, Action, Settings, Event, EventSink, RunError, MergeError, RunReport}, text::*.
