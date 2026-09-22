---
id: 13
title: Workflow config loading and ordering
state: Done
parent: 2
assignee: claude-code
labels: [core]
blockedBy: [9]
created: 2026-09-17T17:13:05Z
updated: 2026-09-21T20:22:33Z
---

## Description

Port utl/Workflows.ts + readJSONFile.ts. `WorkflowConfig { disabled: bool=false, name: String, description: String="", order: Option<i64>, pipeline: Vec<String>, source: PathBuf }`. `load_workflows(dir) -> LoadResult { workflows: Vec<WorkflowConfig>, errors: Vec<LoadError> }` — unreadable files are collected, not fatal. Discovery root: `<dir>/config` if it is a directory, else `<dir>`; recursive; `*.json` only. Sort: finite `order` ascending first, then remaining by path string order. `enabled()`, `names()`, `find(name)` with the quoted-list error message. Reject pipelines with fewer than 2 entries at load time with a LoadError (schema says minItems 2; baseline silently accepted).

## Plan

Port Workflows.test.ts (ordered-one/two before unordered a-first/z-last) and add: nested directories, bad JSON collected in errors, disabled filtered from names(), <2 pipeline entries rejected.

## Notes

### 2026-09-21T20:22:32Z — claude-code (agent)

src/config.rs done. WorkflowConfig{disabled,name,description,order:Option<f64>,pipeline,source}; load_workflows(dir) -> LoadResult{root, workflows, errors} never fails as a whole. Root = <dir>/config if directory else <dir>; recursive walk; *.json (case-insensitive ext). Unknown keys ignored (so `$schema` is fine) — baseline spread-merged JSON, schema says additionalProperties false; lenient wins for editor-friendliness. Sort: files sorted by lowercase path then bytes (approximates localeCompare), then stable sort by order (finite first ascending). Deviation per plan: pipeline < 2 entries is a LoadError. Defaults match baseline (name "not named"). LoadResult::{enabled, names, find, require_enabled}; error strings match baseline exactly ("Could not find Workflow named ..." / "Expected to find enabled workflows at <root>. found 0"). Ported Workflows.test.ts (ordered/unordered sort) plus nested dirs, bad JSON + short pipeline collected, disabled filtered, defaults, require_enabled message.
