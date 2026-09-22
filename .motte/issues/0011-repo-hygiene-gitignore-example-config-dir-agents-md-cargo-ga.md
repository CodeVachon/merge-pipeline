---
id: 11
title: "Repo hygiene: .gitignore, example config dir, AGENTS.md cargo gate"
state: Done
parent: 1
assignee: claude-code
labels: [scaffold]
blockedBy: [9]
created: 2026-09-17T17:12:21Z
updated: 2026-09-22T15:16:22Z
---

## Description

Baseline .gitignore hid config/*.json, so the example workflows never shipped. Here `examples/config/` holds patch.json, minor.json, canary.json, default.json (copied from baseline, with `$schema`). .gitignore: target/, *.gz, dist/. AGENTS.md: keep the motte block; add a section requiring cargo fmt, cargo clippy -D warnings, cargo test before finishing, and forbidding network access in tests.

## Notes

### 2026-09-17T17:48:15Z — claude-code (agent)

Added .gitignore (/target/, *.gz, /dist/, .DS_Store), examples/config/{patch,minor,canary,default}.json copied from the baseline with "$schema": "./../../schema/config.json" prepended, schema/config.json copied verbatim from the baseline (draft-07), LICENSE (MIT), and a "Build and verification" section at the top of AGENTS.md above the untouched motte block. Validated every example against the schema's required/allowed keys and minItems 2 with a throwaway script; #38 adds the durable CI check.

Deviation: the baseline LICENSE named a different copyright holder; the new LICENSE is under Christopher Vachon (CodeVachon), confirmed by the user on 2026-09-22.

Note for #13: default.json has "disabled": true, so with the examples dir as the config source only three workflows are enabled — same as the baseline.

### 2026-09-22T15:16:22Z — claude-code (agent)

User decision 2026-09-22: everything lives under the CodeVachon GitHub user and the previous project's copyright holder is not carried over. LICENSE now reads "Copyright (c) 2026 Christopher Vachon (CodeVachon)". Repository, homepage, installer REPO constants, schema $id and release API paths were already CodeVachon/merge-pipeline; no other change was needed.
