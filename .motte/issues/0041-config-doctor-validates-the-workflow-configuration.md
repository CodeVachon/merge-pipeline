---
id: 41
title: "`config doctor` validates the workflow configuration"
state: Done
parent: 40
assignee: claude-code
labels: [cli]
created: 2026-09-22T19:13:21Z
updated: 2026-09-22T19:19:07Z
---

## Description

`merge-pipeline config doctor [-f <dir>] [-c <repo>] [--json]`. Resolve the config dir with the existing rules and report which rule chose it (or the full list of locations tried, as an error). Load workflows with config::load_workflows and report, per file: path, name, enabled/disabled, order, pipeline. Checks: (error) file unreadable or not JSON; (error) missing/empty name; (error) pipeline missing or fewer than 2 entries; (error) a pipeline entry is not a valid regex when compiled as `(?i)<pattern>`; (error) duplicate name among enabled workflows (the CLI selects by exact name); (error) zero enabled workflows; (warning) unknown top-level keys other than `$schema` (they are silently ignored today); (warning) `order` present on some workflows but not others; (info) `$schema` missing. When --cwd is given and is a git repo, additionally report for each enabled workflow how many local branches each pattern matches right now (0 is a warning, since the run would fail with `No matching branch found`; >1 is info: will prompt). Human output: ✓/!/✗ lines with the file path, then a summary line; exit 0 when no errors (warnings allowed), 1 otherwise. --json emits {root, rule, workflows:[…], problems:[{level,file,message}], ok}.

## Plan

Implement in src/cli/doctor.rs reusing config.rs (LoadResult already carries errors) and pipeline.rs (map_pipeline_report for the branch counts). Add a `Doctor` variant to the config subcommand enum in src/cli/mod.rs. Tests: unit tests with temp dirs for each check; assert_cmd tests for exit codes and --json against a good dir, a dir with a bad regex, a dir with only disabled workflows, and against the tests/common Fixture with --cwd for the branch-count report. Document in README under a new `config doctor` row and in CHANGELOG Unreleased.

## Notes

### 2026-09-22T19:19:06Z — claude-code (agent)

Implemented in src/cli/doctor.rs (pure `diagnose(dir, rule, branches) -> Report` + `render`), wired as `config doctor [-f] [-c] [--json]` in src/cli/mod.rs; main.rs's run_subcommand now returns an exit code so doctor can exit 1 without being an "error". Checks: loader errors (bad JSON, <2 pipeline entries), missing/empty/non-string name, invalid regex compiled as `(?i)pat` with the index, duplicate enabled names, zero enabled, unknown keys (warning), mixed `order` (warning), missing `$schema` (info); with `--cwd <git repo>` each pattern is matched against local branches via map_pipeline_report: 0 matches = warning, >1 = info. Problems are sorted by file then severity. Tests: 7 unit (incl. the shipped examples are clean) + 6 assert_cmd in tests/cli_doctor.rs (incl. Fixture-backed branch report). 157 tests green, fmt/clippy clean.

Real output, release binary:

$ merge-pipeline config doctor -f examples/config
Workflow directory: examples/config (chosen by: --config flag)

✓ canary.json  Canary Staging Pipeline order 3
    ^Patch-* > ^Release-* > ^Release-* > ^staging-canary$

✓ default.json  Default Pipeline (disabled) order 100
    staging > main
…
4 workflows, 3 enabled · 0 errors, 0 warnings
✓ configuration is usable
(exit 0)

$ merge-pipeline config doctor -f /tmp/broken   # {"name":"Bad","pipeline":["(oops","main"],"colour":1}
✗ bad.json  Bad
    (oops > main
    ✗ pipeline[0] "(oops" is not a valid regular expression: error: unclosed group
    ! unknown key "colour" (ignored)
    · no "$schema" key; add one so editors can validate the file

1 workflow, 1 enabled · 1 error, 1 warning
✗ configuration has errors
(exit 1)

Decision: the branch check only runs when --cwd names a git repo (has .git); the process cwd is not assumed to be one, so `config doctor -f dir` from anywhere stays a pure file check.
