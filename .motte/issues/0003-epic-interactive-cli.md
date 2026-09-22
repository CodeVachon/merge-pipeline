---
id: 3
title: "Epic: Interactive CLI"
state: Done
labels: [cli]
blockedBy: [2]
created: 2026-09-17T17:11:21Z
updated: 2026-09-21T20:43:38Z
---

## Description

The user-facing binary, replicating baseline src/index.ts + _CLIOptions.ts behaviour on top of the core library.

Flags (baseline yargs, strict): -c/--cwd <dir>, -w/--workflow <name>, -p/--auto-push (bool), -f/--config <path>, -a/--action <run|dry-run|test>. Baseline spelled the flag `--auto_push`; accept both spellings for one release, prefer `--auto-push`.

Flow, in order: print boxed title "Git Branch / Workflow" in orange; resolve config dir; load workflows, fail if zero enabled (`Expected to find enabled workflows at <dir>. found 0`); ask for working directory if --cwd absent (default: process cwd); refuse if repo is dirty (`Git is in a Dirty State`); select workflow (skip prompt when exactly one enabled; searchable list otherwise); select action (list, labels Title-Cased: "Run", "Dry Run", "Test"); ask "Automatically Push to Origin?" only for action run (default yes). Print `<Action>: <workflow name>` (cyan/orange), then run. On success print "Task Complete"; always print "Work Complete"; non-zero exit on error with the red "Error" block and, for merge conflicts, the list of conflicted paths.

Action semantics to preserve exactly (note the naming oddity): `run` merges and pushes; `dry-run` performs the merges locally but never pushes — it is NOT a no-op; `test` only fetches, maps branches, and prints the steps. Flag the dry-run naming in the README rather than changing it in this pass.

Config dir resolution changes from baseline (baseline looked beside the executable, which does not fit a managed install under ~/.merge-pipeline/versions): order is --config > $MERGE_PIPELINE_CONFIG > <cwd>/.merge-pipeline/ > ~/.config/merge-pipeline/ > <exe dir>/config (compat). Each step is a directory containing workflow JSON files.

## Plan

Children: clap command surface (including subcommands `mcp`, `upgrade`, `uninstall`, `config path` that other epics fill in); inquire-backed Prompter; colors/title/error display; the run/dry-run/test wiring; end-to-end tests with assert_cmd driving a fixture repo non-interactively via flags.

## Notes

### 2026-09-17T17:15:32Z — claude-code (agent)

Baseline oddities preserved on purpose and flagged for the README rather than silently changed: (1) `dry-run` merges locally and still asks per-branch whether to push (auto_push is forced false, so shouldPushBranch prompts); it is not a no-op. (2) `--auto_push` flag spelling; accept both, prefer `--auto-push`. (3) Regexes in pipelines are unanchored case-insensitive searches, so `^Patch-*` means "Patch" + zero or more hyphens. The one deliberate behaviour change is in #15: is_dirty uses `git status --porcelain --untracked-files=no` instead of `git diff --stat`, so staged changes also count as dirty.

### 2026-09-21T20:43:37Z — claude-code (agent)

Epic complete: #18 clap surface, #19 config dir resolution, #20 ui + inquire prompter, #21 interactive flow, plus #32 nudge wiring. Files: src/cli/{mod,config_dir,prompter,interactive}.rs, src/ui.rs, src/main.rs, tests/cli_{surface,config_path,flow}.rs. Whole crate: fmt clean, clippy -D warnings clean, 118 tests green, no network. One added dependency: clap_complete 4.6.11. One added flag beyond the baseline: -y/--yes (see #21). Manual check still worth doing before release: run the binary on a real terminal to see inquire's fuzzy Select for the workflow picker and the colors, which cannot be covered by non-tty tests.
