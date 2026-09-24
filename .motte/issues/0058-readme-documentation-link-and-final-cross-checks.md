---
id: 58
title: README documentation link and final cross-checks
state: Done
parent: 55
assignee: claude-code
labels: [docs]
blockedBy: [56, 57]
created: 2026-09-23T18:33:37Z
updated: 2026-09-23T18:44:30Z
---

## Description

Add a short "Documentation" line near the top of README.md pointing at https://codevachon.github.io/merge-pipeline/, without pulling detailed content back into the README (the split is: README stays a fast-start reference, the book is the full manual). Re-read every book page once more against the live `--help` output of the actual built binary from this branch (including `versions`, `use`, `upgrade`, `config doctor`, `config init`) and fix any drift before opening the PR.

## Plan

Diff each page's command examples against `target/release/merge-pipeline <cmd> --help` output one more time as a final pass.

## Notes

### 2026-09-23T18:44:29Z — claude-code (agent)

Added a "Full documentation" line to README.md right after the MIGRATING.md pointer, linking https://codevachon.github.io/merge-pipeline/. Kept it to one line as directed — the book is the manual, README stays the fast-start reference.

Final cross-check performed exactly as the plan asked: rebuilt `target/release/merge-pipeline` fresh on this branch and re-captured all 14 `--help` outputs (root, mcp, upgrade, uninstall, install, config, config path, config doctor, config init, versions, versions remove, versions prune, use, completion) into a scratch directory, then diffed each against the corresponding docs/src/help/*.txt file committed in #56. All 14 matched byte for byte — no drift between when #56 captured them and now. Also re-probed the live MCP `tools/list` response over stdio and confirmed all six tool names and descriptions match what mcp-server.md documents.

Full rebuild after the README change: `mdbook build docs` zero warnings, check-links.py 14 pages / 0 broken links, `cargo fmt --check` / `cargo clippy -D warnings` / `cargo test` (218 tests) all green.
