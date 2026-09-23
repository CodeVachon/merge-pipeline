# merge-pipeline

Merge git branches, one into the next, according to a configured workflow. Point it at a
repository, pick a workflow such as "Patch Release to Staging", and it fetches, checks out each
branch, pulls, merges, resolves `package.json` version conflicts for you, and pushes.

A single self-contained binary with `git` as its only prerequisite. It also serves the same
operations to agents over the [Model Context Protocol](mcp-server.md) and
[updates itself in place](keeping-up-to-date.md).

This is the Rust rewrite of `@codevachon/cli-merge-pipeline`. Coming from that tool? Read
[Migrating from cli-merge-pipeline](MIGRATING.md).

## Quick start

```sh
curl -fsSL https://raw.githubusercontent.com/CodeVachon/merge-pipeline/main/install.sh | sh
cd ~/src/app
merge-pipeline install        # wires .mcp.json and writes example workflow files
merge-pipeline                # interactive: pick a workflow, pick an action, go
```

## What's in this book

| Page | What it covers |
| --- | --- |
| [Install](install.md) | The installer, supported platforms, environment variables |
| [Workflow Files](workflow-files.md) | The JSON format, the config schema, where files are found |
| [Usage](usage.md) | Every flag, the interactive flow, the three actions |
| [Branch Sync](branch-sync.md) | Why a stale local branch no longer breaks a run |
| [Config Diagnostics](config-diagnostics.md) | `config doctor` and `config init` |
| [MCP Server](mcp-server.md) | The six tools an agent can call, and how to wire them up |
| [Keeping Up to Date](keeping-up-to-date.md) | `upgrade`, `versions`, `use`, `uninstall`, the daily update offer |
| [Migrating from cli-merge-pipeline](MIGRATING.md) | What changed coming from the old Bun tool |
| [Changelog](changelog.md) | Every release |

Source: [github.com/CodeVachon/merge-pipeline](https://github.com/CodeVachon/merge-pipeline).
