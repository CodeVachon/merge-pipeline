---
id: 37
title: "README: install, workflows, flags, actions, MCP, upgrade"
state: Done
parent: 7
assignee: claude-code
labels: [docs]
blockedBy: [21, 26, 30]
created: 2026-09-17T17:14:13Z
updated: 2026-09-21T20:55:09Z
---

## Description

Sections: one-line install; what a workflow file is with the four baseline examples; config directory search order; every flag; the three actions with the honest dry-run note; `.mcp.json` snippet {"merge-pipeline": {"command": "merge-pipeline", "args": ["mcp"]}} and the tool list; upgrade/uninstall; development (cargo test builds its own git fixtures, no bootstrap.sh).

## Notes

### 2026-09-21T20:55:08Z — claude-code (agent)

README.md written from the shipped binary, not the plan: every flag/subcommand was taken from `--help` output, the MCP tool table from a live `tools/list` against target/release/merge-pipeline (protocol 2025-06-18, four tools: list_workflows, inspect_repo, plan_workflow, run_workflow), and the install wiring from src/mcp/install.rs (project scope → ./.mcp.json, user scope → ~/.claude.json mcpServers). Documented run_workflow's real argument set, which differs from the epic text: version_strategy is higher|lower|fail (no "ours"/"theirs"), and there are `abort_on_conflict` (default true, runs `git merge --abort`) and `answers` arguments. Also documented the macOS user config path (~/Library/Application Support/merge-pipeline). Added `readme = "README.md"` to Cargo.toml. Not verifiable locally: the install one-liner and the schema `$id` URL only resolve after the repo is pushed to GitHub and a release exists.
