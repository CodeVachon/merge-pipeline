---
id: 56
title: "mdBook scaffold: book.toml, SUMMARY.md, and content pages"
state: Done
parent: 55
assignee: claude-code
labels: [docs]
created: 2026-09-23T18:33:37Z
updated: 2026-09-23T18:40:54Z
---

## Description

Create docs/book.toml (title "merge-pipeline", src="src", a git-repository-url pointing at github.com/CodeVachon/merge-pipeline, the fold/search/edit-url-template defaults mdBook ships with) and docs/src/SUMMARY.md wiring these pages: introduction.md, install.md, workflow-files.md, usage.md, branch-sync.md, config-diagnostics.md, mcp-server.md, keeping-up-to-date.md, migrating.md, changelog.md. Every page is written from the actual source of truth — README.md, MIGRATING.md, CHANGELOG.md, schema/config.json, examples/config/*.json, and `cargo run -- --help` / `cargo run -- <subcommand> --help` output captured from the real binary (build it first) — not re-derived from memory or from the motte issue text alone, since several features (versions/use/the daily update offer) only exist on the open PR branches and must be described exactly as implemented there. The changelog page is a single line: `{{#include ../../CHANGELOG.md}}`. The MCP page includes a table of all six tools (list_workflows, inspect_repo, plan_workflow, run_workflow, doctor_config, init_config) with their arguments, taken from src/mcp/tools/*.rs and a live `tools/list` probe over stdio, not guessed.

## Plan

Work on a branch stacked on the tip of the open PR chain (feat/update-prompt), since that is the only branch where versions/use/the daily update offer exist. `mdbook build docs` must succeed with zero warnings; `mdbook serve docs` and spot-check a handful of pages by reading the rendered HTML for obviously wrong content (e.g. grep the built HTML for command names and flag spellings against a real `--help` run).

## Notes

### 2026-09-23T18:40:53Z — claude-code (agent)

Built docs/ as an mdBook book: book.toml, SUMMARY.md, 9 content pages (introduction, install, workflow-files, usage, branch-sync, config-diagnostics, mcp-server, keeping-up-to-date, MIGRATING, changelog) plus docs/src/help/*.txt holding real `--help` output captured from a `cargo build --release` binary and included verbatim via {{#include}}.

Grounding: every flag, subcommand, and default value quoted in the book comes from `target/release/merge-pipeline <cmd> --help` or a real `initialize` -> `tools/list` JSON-RPC probe over stdio into `merge-pipeline mcp` (all six tools' exact descriptions and schemas captured, including the `sync` argument's oneOf shape and the `run_workflow` `version_strategy` enum, which is `higher|lower|fail` — not `higher|ours|theirs` as an earlier plan draft had guessed). config-diagnostics.md's exit-code claim (0 with warnings, 1 with errors) verified directly against src/cli/mod.rs::run_config_doctor.

changelog.md and MIGRATING.md are single-line {{#include}}s of the real CHANGELOG.md/MIGRATING.md so they cannot drift.

Real bug found and fixed while building: CHANGELOG.md's own `[MIGRATING.md](MIGRATING.md)` link, embedded verbatim on the changelog page, is dead in the book — mdBook rewrites relative `.md` links to `.html`, giving `MIGRATING.html`, but my book page was originally named `migrating.md` (lowercase), producing `migrating.html`. On this Mac's case-insensitive filesystem that mismatch is invisible; on GitHub Pages' Linux runner it would 404. Fixed by naming the book page `docs/src/MIGRATING.md` (same case as the real file), so mdBook's own rewrite lands on it exactly — no redirect hack needed. Verified with a script that extracts every internal href from the built HTML and checks it against the actual output files: zero bad links after the fix, versus one before.

`mdbook build docs` is clean (zero warnings) from a from-scratch build. Made a small, justified edit to MIGRATING.md itself (backticked a bare `<branch>` placeholder that mdBook's stricter HTML-tag parser flagged as an unclosed tag) — harmless and arguably a correctness fix in every context that renders it.

Removed docs/src/help/mcp_tools.txt (the raw JSON-RPC probe capture) before committing since the MCP page renders that content as hand-written tables rather than including the raw file, so the raw capture was dead weight.

Full `cargo test` (218 tests) and fmt/clippy stayed green throughout — no Rust files touched except MIGRATING.md.
