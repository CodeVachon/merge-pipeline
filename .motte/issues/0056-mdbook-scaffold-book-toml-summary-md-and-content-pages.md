---
id: 56
title: "mdBook scaffold: book.toml, SUMMARY.md, and content pages"
state: In Progress
parent: 55
assignee: claude-code
labels: [docs]
created: 2026-09-23T18:33:37Z
updated: 2026-09-23T18:35:10Z
---

## Description

Create docs/book.toml (title "merge-pipeline", src="src", a git-repository-url pointing at github.com/CodeVachon/merge-pipeline, the fold/search/edit-url-template defaults mdBook ships with) and docs/src/SUMMARY.md wiring these pages: introduction.md, install.md, workflow-files.md, usage.md, branch-sync.md, config-diagnostics.md, mcp-server.md, keeping-up-to-date.md, migrating.md, changelog.md. Every page is written from the actual source of truth — README.md, MIGRATING.md, CHANGELOG.md, schema/config.json, examples/config/*.json, and `cargo run -- --help` / `cargo run -- <subcommand> --help` output captured from the real binary (build it first) — not re-derived from memory or from the motte issue text alone, since several features (versions/use/the daily update offer) only exist on the open PR branches and must be described exactly as implemented there. The changelog page is a single line: `{{#include ../../CHANGELOG.md}}`. The MCP page includes a table of all six tools (list_workflows, inspect_repo, plan_workflow, run_workflow, doctor_config, init_config) with their arguments, taken from src/mcp/tools/*.rs and a live `tools/list` probe over stdio, not guessed.

## Plan

Work on a branch stacked on the tip of the open PR chain (feat/update-prompt), since that is the only branch where versions/use/the daily update offer exist. `mdbook build docs` must succeed with zero warnings; `mdbook serve docs` and spot-check a handful of pages by reading the rendered HTML for obviously wrong content (e.g. grep the built HTML for command names and flag spellings against a real `--help` run).
