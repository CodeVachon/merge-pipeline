---
id: 51
title: "Version store: list, switch, remove primitives in selfupdate"
state: Done
parent: 50
assignee: claude-code
labels: [self-update, cli]
created: 2026-09-23T15:16:17Z
updated: 2026-09-23T15:25:37Z
---

## Description

In src/selfupdate/: `VersionStore` (or free functions) over a located install: `list()` → Vec<{version, is_current, is_running}> newest first (current from reading the `current` link/junction target; running from locate_install().version); `switch(version)` repoints `current` using the same code path install_binary uses (extract that repoint into a shared fn `point_current(root, version_dir)` handling unix symlink and the Windows junction/copy fallback); `remove(versions)` deleting directories with refusal reasons for current/running; `prune(keep)` wrapping prune_versions. Pure over a root path so tests build a fake layout in a temp dir.

## Plan

Unit tests on a temp fake install (versions/v0.1.0, v0.2.0, current → v0.2.0): list marks current; switch repoints and list reflects it; remove refuses current and running with the exact reasons and removes others; prune keeps N newest plus current/running.

## Notes

### 2026-09-23T15:25:37Z — claude-code (agent)

Implemented in src/selfupdate/versions.rs. `download::point_current_at` (unix symlink, Windows symlink_dir with copy fallback) is now pub and is the single repoint used by install_binary and `switch`. `current_version(root)` reads the `current` link's target; on a Windows install that fell back to copying bin/ it returns None (documented). `prune` here protects both `current` and the running version, unlike download::prune_versions which upgrade uses right after installing the newest version and only protects the running one; kept versions are reported with a reason. Command runners (`run_list_with` with an optional --check API lookup reusing the update-check cache, `run_use_with` which downloads+verifies through the existing fetch path when the version is not on disk, `run_remove_with` exit 1 when anything was refused or missing, `run_prune_with` exit 0) live in the same file so #52 only wires clap. 5 unit tests on a fake layout under a temp dir (cfg unix).
