---
id: 21
title: Wire the interactive flow end to end
state: Done
parent: 3
assignee: claude-code
labels: [cli]
blockedBy: [17, 18, 19, 20]
created: 2026-09-17T17:13:18Z
updated: 2026-09-21T20:43:27Z
---

## Description

main.rs: title; parse; resolve config; load workflows (print collected LoadErrors as warnings, fail if zero enabled); cwd prompt if absent; is_dirty guard; workflow select (skip if one); action select; auto-push confirm only for run; print `<Action>: <name>`; run via runner with the inquire Prompter and stdout event renderer; "Task Complete" on success; "Work Complete" in all cases; exit code 1 on error.

## Plan

assert_cmd tests run fully non-interactively by passing every flag against the fixture repo: `test` prints the three expected steps; `run -p` merges and pushes; dirty repo exits 1 with the exact message; unknown workflow lists the valid names.

## Notes

### 2026-09-21T20:43:26Z — claude-code (agent)

Done. src/cli/interactive.rs reproduces index.ts setup()/main() in the same order: title → config dir (resolver from #19, cwd hint = --cwd or process cwd) → load workflows (LoadErrors printed as `warning:` on stderr, zero enabled is fatal with the baseline message) → cwd prompt if --cwd absent → is_dirty guard ("Git is in a Dirty State") → workflow (flag, else auto-pick when exactly one enabled, else select) → action select → auto-push confirm only for run → `<Action>: <name>` → runner with verbose `$ git` echo and StdoutRenderer → "Task Complete" / always "Work Complete" → exit 1 on error via ui::print_error.

Deviation (addition): a `-y/--yes` flag. clap cannot tell "flag absent" from "false" for booleans, and inquire errors on a non-tty, so the baseline's per-step "Merge X into Y?" and per-branch push confirmations made a fully scripted run impossible. --yes wraps the prompter in AssumeDefaults, which answers every confirm with its default and any select/text that has a default with that default; questions without a default still prompt. Cosmetic: the workflow question reads "Which Workflow would you like to run?" (baseline omitted "you").

main.rs: root command returns an exit code; subcommands return anyhow::Result and print `Error: ...`; selfupdate's own finish() handles its non-zero exits. Nudge wired per #32.

Tests (tests/cli_flow.rs, 10, all against the hermetic Fixture, additions since the baseline had no CLI tests): test action prints title/pipeline/steps and moves nothing; run -p -y fast-forwards every step and origin heads match; dry-run -y merges locally and pushes only because the confirmation default is yes (documents the baseline semantics); dirty tree exits 1 with the message; staged-only change is dirty too; unknown workflow lists names quoted; zero enabled workflows; broken JSON warns but runs; README conflict stops with the path listed and MERGE_HEAD left in place; package.json version conflict auto-resolves to 1.10.0 and commits and pushes.
