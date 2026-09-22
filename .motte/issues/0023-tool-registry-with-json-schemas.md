---
id: 23
title: Tool registry with JSON schemas
state: Done
parent: 4
assignee: claude-code
labels: [mcp]
blockedBy: [22]
created: 2026-09-17T17:13:33Z
updated: 2026-09-21T20:53:36Z
---

## Description

`trait Tool { name, description, input_schema: Value, call(args: Value) -> ToolResult }`. Registry lists tools for tools/list and dispatches tools/call. Schemas hand-written as serde_json::json! (no schemars, keeps deps small). Shared args: cwd (string, default process cwd), config (string, optional).

## Notes

### 2026-09-21T20:53:14Z — claude-code (agent)

src/mcp/tools/mod.rs: `Tool` trait (name/description/input_schema/call), `Registry` (list, get, call; default order list_workflows, inspect_repo, plan_workflow, run_workflow), `ToolError` rendered as {content:[text JSON], structuredContent, isError:true}; successes carry the same JSON in both `content[0].text` and `structuredContent` (2025-06-18 field, harmless to older clients). Schemas are json! literals via object_schema(extra, required) which splices the shared `cwd`/`config` properties; additionalProperties:false. CommonArgs::parse validates cwd is a directory and reuses cli::config_dir::resolve_from_process so the MCP surface honours the same search order as the CLI (including <cwd>/.merge-pipeline). Helpers optional_string/required_string/bool_or/string_map give typed argument errors as tool errors, not protocol errors.
