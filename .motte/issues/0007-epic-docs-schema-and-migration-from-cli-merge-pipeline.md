---
id: 7
title: "Epic: Docs, schema, and migration from cli-merge-pipeline"
state: Done
labels: [docs]
blockedBy: [3, 4, 5]
created: 2026-09-17T17:12:03Z
updated: 2026-09-21T20:55:13Z
---

## Description

Make the new tool adoptable by someone who used the Bun version, and make the config discoverable.

- README: install one-liner (curl | sh), what a workflow file is (with the four example configs from the baseline: patch, minor, canary, default), the config directory search order, all flags, the three actions with an honest note that `dry-run` still merges locally, MCP setup snippet for .mcp.json, upgrade/uninstall.
- Publish schema/config.json (draft-07, copied from baseline, `$id` pointing at the GitHub Pages URL or raw main URL) so editors validate workflow files; example files reference it via `$schema`.
- Migration notes: copy `config/*.json` from the old tool into ~/.config/merge-pipeline/ (or repo .merge-pipeline/); flag rename auto_push → auto-push; identical regex semantics (case-insensitive, `Patch-*` means "Patch" plus zero or more hyphens, as before).
- CHANGELOG.md starting at 0.1.0.
- Update AGENTS.md: cargo fmt/clippy/test gate, `cargo run -- -c <fixture>` dev loop replacing bootstrap.sh, and a note that tests build their own git fixtures.

## Notes

### 2026-09-21T20:55:11Z — claude-code (agent)

Epic complete: README.md, MIGRATING.md, CHANGELOG.md written; schema published with CI validation (#38); AGENTS.md build section expanded with the fixture helper, HTTP fake, Prompter conventions and the install.sh/layout.rs coupling. CHANGELOG 0.1.0 is marked unreleased pending #36.
