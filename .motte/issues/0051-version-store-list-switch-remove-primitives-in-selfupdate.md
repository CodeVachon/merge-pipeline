---
id: 51
title: "Version store: list, switch, remove primitives in selfupdate"
state: In Progress
parent: 50
assignee: claude-code
labels: [self-update, cli]
created: 2026-09-23T15:16:17Z
updated: 2026-09-23T15:22:33Z
---

## Description

In src/selfupdate/: `VersionStore` (or free functions) over a located install: `list()` → Vec<{version, is_current, is_running}> newest first (current from reading the `current` link/junction target; running from locate_install().version); `switch(version)` repoints `current` using the same code path install_binary uses (extract that repoint into a shared fn `point_current(root, version_dir)` handling unix symlink and the Windows junction/copy fallback); `remove(versions)` deleting directories with refusal reasons for current/running; `prune(keep)` wrapping prune_versions. Pure over a root path so tests build a fake layout in a temp dir.

## Plan

Unit tests on a temp fake install (versions/v0.1.0, v0.2.0, current → v0.2.0): list marks current; switch repoints and list reflects it; remove refuses current and running with the exact reasons and removes others; prune keeps N newest plus current/running.
