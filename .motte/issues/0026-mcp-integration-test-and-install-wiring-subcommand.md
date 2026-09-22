---
id: 26
title: MCP integration test and `install` wiring subcommand
state: Done
parent: 4
assignee: claude-code
labels: [mcp]
blockedBy: [24, 25]
created: 2026-09-17T17:13:33Z
updated: 2026-09-21T20:53:40Z
---

## Description

Spawn the release-profile or debug binary with `mcp`, pipe initialize → initialized → tools/list → tools/call(list_workflows) → tools/call(plan_workflow) against the fixture repo, assert shapes. Then `merge-pipeline install [--scope project|user]` writes/merges a `merge-pipeline` entry {command: "merge-pipeline", args: ["mcp"]} into .mcp.json (project) — mirror motte install's record-keeping only if cheap; otherwise a plain merge with a printed diff is enough.

## Notes

### 2026-09-21T20:53:33Z — claude-code (agent)

tests/mcp.rs: 10 end-to-end tests spawning the debug binary with `mcp` over piped stdio against the tests/common Fixture — handshake + ping + tools/list + list_workflows (incl. broken.json in errors) + -32601; inspect_repo upstream flags and dirty detection; plan_workflow ambiguous → selections → complete, unknown workflow error; run_workflow refuses without confirm and on a dirty tree; run with auto_push lands the merge in the bare origin; without auto_push merges locally only; ambiguous pattern returns the open question and touches nothing, then succeeds with selections; conflicting README → conflicted_paths + merge_aborted + clean tree; package.json version conflict: fail strategy aborts, higher strategy resolves to 1.2.0 and commits. `install` (src/mcp/install.rs): project scope merges {command:"merge-pipeline",args:["mcp"]} into <cwd>/.mcp.json, user scope into ~/.claude.json `mcpServers` (Claude Code's user-level file), preserving other keys; outcomes Created/Added/Updated/Unchanged printed. serde_json's default map does not preserve key order, so a rewritten ~/.claude.json has its top-level keys sorted; functionally harmless, noted rather than adding the preserve_order feature. No motte-style install record is kept (uninstall does not unwire agents) — cheap to add later if wanted. Whole crate: 144 tests green, fmt + clippy -D warnings clean.
