---
id: 39
title: Migration notes and CHANGELOG
state: Done
parent: 7
assignee: claude-code
labels: [docs]
blockedBy: [21]
created: 2026-09-17T17:14:13Z
updated: 2026-09-21T20:47:31Z
---

## Description

MIGRATING.md: copy old config/*.json to ~/.config/merge-pipeline/ or <repo>/.merge-pipeline/; --auto_push → --auto-push (old spelling still accepted); is_dirty now also catches staged changes; regex semantics unchanged (case-insensitive search; `^Patch-*` matches "Patch" plus any number of hyphens — consider `^Patch-.*` in your configs); dry-run behaviour unchanged. CHANGELOG.md with 0.1.0 entry.

## Notes

### 2026-09-21T20:47:29Z — claude-code (agent)

Wrote MIGRATING.md and CHANGELOG.md, grounded in the shipped code rather than the plan. Two facts worth knowing that differ from the epic text: (1) on macOS `dirs::config_dir()` is `~/Library/Application Support`, not `~/.config`, so the user-level workflow directory is `~/Library/Application Support/merge-pipeline/` on a Mac (verified by running `config path` on this machine); MIGRATING.md lists both. (2) Pipelines with fewer than two entries are now a load error (config.rs), which the baseline silently accepted; documented as a behaviour change. CHANGELOG 0.1.0 is marked "unreleased" because no tag exists yet (#36).
