---
id: 50
title: "Epic: nvm-style version management (versions, use, remove, prune)"
state: Done
labels: [self-update, cli]
created: 2026-09-23T15:15:25Z
updated: 2026-09-23T15:29:59Z
---

## Description

Requested 2026-09-23. Manage installed versions under ~/.merge-pipeline/versions like nvm manages node: list them, switch the active one, delete old ones, and update, all taking effect on the next invocation with no shell restart.

Facts that shape the design:
- The install layout already makes this cheap: `~/.local/bin/merge-pipeline -> ~/.merge-pipeline/current/bin/merge-pipeline` and `current -> versions/vX.Y.Z`. Switching versions is repointing `current`; the PATH entry never changes, so the shell needs no restart and no `hash -r`. The "Open a new shell" line printed by `upgrade` today is wrong and must go.
- src/selfupdate/ already has locate_install, installed_versions, fetch_verified_binary, install_binary (which repoints current), prune_versions. Reuse; do not duplicate.

Commands:
- `merge-pipeline versions [--json]` — installed versions newest first, marking `current` (what `current` points at) and `running` (the binary executing, from locate_install), plus the latest available if the update-check cache knows it (no network unless `--check`).
- `merge-pipeline use <version> [--json]` — normalise `0.1.0`/`v0.1.0`; if installed, repoint `current` (reuse the repoint code from install_binary, including the Windows junction/copy path); if not installed, download and verify it exactly like `upgrade <target>` then repoint. Print `now using vX.Y.Z (was vA.B.C)`; effective on the next invocation.
- `merge-pipeline versions remove <version>... [--json]` — delete version directories; refuse the one `current` points at ("use another version first") and the running one (Windows file lock; Unix would work but keep the rule uniform); `--json` lists removed/refused.
- `merge-pipeline versions prune [--keep N=2] [--json]` — the existing prune_versions exposed directly; never removes current or running.
- `upgrade` keeps its behaviour but its closing line becomes `now using vX.Y.Z; takes effect on your next merge-pipeline command` (also for `use`). Not-managed installs (cargo target/) get the existing curl|sh hint for all of these.
- Shell completion for `use`/`versions remove` may list installed versions if clap_complete makes that cheap; otherwise skip.
- README "Keeping it up to date" section documents all four and states plainly that no restart is needed; CHANGELOG Unreleased.

## Notes

### 2026-09-23T15:29:59Z — claude-code (agent)

Record for the "restart" question: a shell restart was never required after `upgrade`. The command on PATH is a fixed path (~/.local/bin/merge-pipeline) that symlinks to <root>/current/bin/merge-pipeline, and `current` symlinks to a version directory; the shell resolves those links on every exec, and the PATH entry never changes so there is no command-hash to refresh. The "Open a new shell" line was copied from motte's upgrade output and was misleading; it is gone, and `upgrade`/`use` now say the change applies to the next command. Delivered on branch feat/version-management as one PR (stacked on #1 while it was still open).
