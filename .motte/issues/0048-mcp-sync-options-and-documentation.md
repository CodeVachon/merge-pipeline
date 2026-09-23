---
id: 48
title: MCP sync options and documentation
state: Done
parent: 44
assignee: claude-code
labels: [core, cli]
blockedBy: [46]
created: 2026-09-22T19:23:16Z
updated: 2026-09-22T19:35:41Z
---

## Description

plan_workflow and run_workflow accept `sync: {stale: "keep"|"delete" (default keep), fetch_new: bool (default true)}` (and `sync: false` to skip entirely); results include `sync: {stale, deleted, kept, created}`. Non-interactive default never deletes. README: new "Branch sync" subsection under Usage explaining the four rules (fetch --prune, [gone] detection, prompt per delete, auto-create new), the `--no-sync` flag, and the MCP `sync` argument; MIGRATING: note this is new behaviour versus the baseline's plain fetch; CHANGELOG Unreleased.

## Plan

tests/mcp.rs: plan_workflow with default sync on a fixture with a stale branch reports it under sync.stale and does not delete; with stale:"delete" deletes; sync:false leaves the stale branch and reports nothing.

## Notes

### 2026-09-22T19:35:40Z — claude-code (agent)

MCP: shared `sync` argument in tools/mod.rs — sync_schema() (boolean or {stale: keep|delete, fetch_new: bool}), sync_options() (absent/true/{} → Keep+fetch_new, false → None, "ask" rejected since nobody can answer), sync_report_json() ({stale, deleted, kept, created} or null when skipped). plan_workflow: with fetch=true runs fetch --prune + runner::sync_branches (now pub) before map_pipeline_report, plain fetch when sync=false, nothing when fetch=false; payload gains `sync`. run_workflow: Settings.sync from the argument; report_json gains `sync`; descriptions updated. Tests: unit test for every argument form; 3 e2e in tests/mcp.rs (default sync keeps stale + creates new and both appear as candidates; sync:false skips and sync:{delete,fetch_new:false} deletes only; run_workflow reports sync events/kept then deletes with stale:delete). Docs: README "Branch sync" subsection under Usage with the four rules and sample output, --no-sync row, MCP table + `sync` paragraph, sync:delete key; MIGRATING §4 bullet; CHANGELOG Added + Changed (fetch is now --prune). Gate: fmt/clippy clean, 195 tests; live tools/list confirms both schemas advertise `sync`.
