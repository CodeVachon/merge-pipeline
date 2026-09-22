---
id: 19
title: Config directory resolution
state: Done
parent: 3
assignee: claude-code
labels: [cli]
blockedBy: [9]
created: 2026-09-17T17:13:18Z
updated: 2026-09-21T20:39:02Z
---

## Description

Order: --config > $MERGE_PIPELINE_CONFIG > <cwd>/.merge-pipeline > ~/.config/merge-pipeline (dirs::config_dir()) > <exe dir>/config (backward compat with the Bun layout). First existing directory wins; if none exists the error names every location tried. `config path` subcommand prints the winner and the rule.

## Plan

Unit tests with temp dirs and env overrides; document in README (docs epic).

## Notes

### 2026-09-21T20:39:01Z — claude-code (agent)

Done. Lives in src/cli/config_dir.rs (cli.rs became src/cli/mod.rs). `resolve(&Sources)` is pure and fully injectable (flag, env value, cwd, user config dir, exe dir); `resolve_from_process` feeds it the real ones (dirs::config_dir(), current_exe().parent()). Order: --config > $MERGE_PIPELINE_CONFIG > <cwd>/.merge-pipeline > <config_dir>/merge-pipeline > <exe dir>/config. Decision: an explicit flag or non-empty env var that is not a directory is its own error (NotADirectory) rather than falling through, so a typo cannot silently pick up a different config set; an empty env var is ignored. NotFound lists every location tried with its rule. `config path` prints the path and `chosen by: <rule>`; the same resolver is public for the MCP tools (merge_pipeline::cli::config_dir). Tests: 3 unit + 4 assert_cmd (flag, env, repo-local, missing flag dir exits 1). Concurrency note: during this issue the self-update agent's src/selfupdate/ was mid-restructure, so I waited ~5 min for it to compile again before running the gate; their tests/selfupdate.rs currently has 2 red tests which are theirs, everything in my scope is green.
