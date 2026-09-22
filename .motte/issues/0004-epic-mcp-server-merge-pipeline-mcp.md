---
id: 4
title: "Epic: MCP server (`merge-pipeline mcp`)"
state: Done
labels: [mcp]
blockedBy: [2]
created: 2026-09-17T17:11:33Z
updated: 2026-09-21T20:53:43Z
---

## Description

Expose the tool to agents over the Model Context Protocol, stdio transport, the same way motte does (`motte mcp`, wired via .mcp.json). Implementation is a minimal hand-rolled JSON-RPC 2.0 loop (newline-delimited JSON over stdin/stdout, serde_json), handling: initialize (report protocolVersion echo, serverInfo, capabilities.tools), notifications/initialized, ping, tools/list, tools/call. Unknown methods return -32601. All logging goes to stderr; stdout is protocol only.

Tools (all take `cwd`, default process cwd, and optional `config` dir):
- list_workflows → every discovered workflow: name, description, order, disabled, pipeline, source path, plus discovery root used.
- inspect_repo → clean/dirty, current branch, local branches, whether each has an upstream. Runs `git fetch` only when `fetch: true`.
- plan_workflow {workflow, selections?} → resolved steps. Where a regex matches several branches and no selection was supplied, return the candidates under `ambiguous` instead of failing, so the agent can call again with `selections: {"^Release-*": "Release-0.2.0"}`.
- run_workflow {workflow, selections, auto_push, version_strategy: higher|ours|theirs|ask→error, confirm: true} → executes via the non-interactive Prompter. Refuses (isError) without `confirm: true`, when the repo is dirty, or when any question would need a human. Returns per-step outcome and, on conflict, the conflicted paths so the agent can report them.

Non-interactive Prompter: answers come from the tool arguments; any unanswered question aborts with a structured error naming the question and its options. This is the mechanism that keeps one core for both surfaces.

## Plan

Children: JSON-RPC stdio loop + protocol types; tool registry with JSON schemas; the four tools; non-interactive Prompter; integration test that spawns the binary, sends initialize → tools/list → tools/call over pipes and asserts responses; `install` subcommand that writes/merges a `.mcp.json` entry (mirroring `motte install`), which is optional and last.

## Notes

### 2026-09-17T17:15:29Z — claude-code (agent)

Decision point worth a human glance before #22 starts: the plan hand-rolls the MCP stdio server (JSON-RPC 2.0 over serde_json, ~300 lines, no async runtime). The alternative is the official `rmcp` crate, which brings tokio and makes the whole binary async. Hand-rolled keeps the binary small and the codebase synchronous, which is the stated goal; rmcp is the fallback if we later want resources, prompts, progress notifications, or Streamable HTTP. Claude Code, Codex and Cursor all speak plain stdio JSON-RPC, and motte's server only uses tools/list + tools/call, so the minimal surface is sufficient today.

### 2026-09-21T20:53:41Z — claude-code (agent)

Epic complete. src/mcp/{mod,jsonrpc,server,install}.rs + src/mcp/tools/{mod,list_workflows,inspect_repo,plan_workflow,run_workflow}.rs (~1,650 lines incl. unit tests) and tests/mcp.rs (~600 lines, 10 e2e tests). Hand-rolled stdio JSON-RPC as planned; no new dependencies. Manual check for the docs epic: `.mcp.json` entry is {"merge-pipeline": {"command": "merge-pipeline", "args": ["mcp"]}} and `merge-pipeline install` writes it. Tool list for the README: list_workflows, inspect_repo(fetch), plan_workflow(workflow, selections, fetch), run_workflow(workflow, confirm, selections, auto_push, version_strategy higher|lower|fail, abort_on_conflict, answers).
