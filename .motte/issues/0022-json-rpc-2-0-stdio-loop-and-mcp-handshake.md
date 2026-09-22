---
id: 22
title: JSON-RPC 2.0 stdio loop and MCP handshake
state: Done
parent: 4
assignee: claude-code
labels: [mcp]
blockedBy: [9, 18]
created: 2026-09-17T17:13:33Z
updated: 2026-09-21T20:53:35Z
---

## Description

`merge-pipeline mcp` reads newline-delimited JSON-RPC from stdin, writes responses to stdout, logs to stderr. Handle initialize (respond with the client's protocolVersion if we support it, else our latest; serverInfo {name: "merge-pipeline", version}; capabilities {tools: {}}), notifications/initialized (no reply), ping ({}), tools/list, tools/call; -32601 for unknown methods, -32700 for parse errors, -32602 for invalid params. Notifications (no id) never get a reply. Batch requests may be rejected. Tool results use `{content:[{type:"text",text:<json>}], isError}`.

## Plan

Types with serde; a `Server` that takes `Box<dyn Read>`/`Box<dyn Write>` so tests can drive it with in-memory pipes; tests for handshake, unknown method, malformed line.

## Notes

### 2026-09-21T20:53:11Z — claude-code (agent)

Implemented in src/mcp/jsonrpc.rs + src/mcp/server.rs. Hand-rolled JSON-RPC 2.0 over any BufRead/Write (Server::run) with Server::handle_line for in-memory tests; serve_stdio() locks stdin/stdout. Notification vs request is decided by presence of the `id` key (IdField::Absent vs Present(null)), so `"id": null` is still answered and a notification for a request method is still silent. Batches return -32600 (not supported). Protocol negotiation: echo the client's protocolVersion when in {2025-06-18, 2025-03-26, 2024-11-05}, else offer 2025-06-18. initialize also returns an `instructions` string steering agents to plan_workflow before run_workflow. Unknown tool → -32602 (per spec), tool failures → result with isError. 9 unit tests: handshake, ping, notification silence, unknown method, parse error, batch, bad tools/call params, in-memory pipe run, tool error shape.
