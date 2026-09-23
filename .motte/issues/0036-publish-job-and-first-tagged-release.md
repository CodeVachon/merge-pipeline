---
id: 36
title: Publish job and first tagged release
state: Done
parent: 6
labels: [release]
blockedBy: [21, 30, 34, 35]
created: 2026-09-17T17:14:06Z
updated: 2026-09-23T14:28:29Z
---

## Description

softprops/action-gh-release@v2 with dist/*.gz and checksums.txt, generate_release_notes: true, prerelease while the tag starts with v0. Cut v0.1.0 once the CLI epic's end-to-end tests pass; verify `curl -fsSL …/install.sh | sh` on this Mac installs it and `merge-pipeline upgrade --check` reports up to date.

## Notes

### 2026-09-21T20:17:59Z — claude-code (agent)

Blocked on the human, not on code: this repo has no git remote and no commits yet (checked 2026-09-21). Cutting v0.1.0 means creating github.com/CodeVachon/merge-pipeline, committing, and pushing a tag, which publishes binaries and is not something an agent should do unasked. Everything else in the epic (workflow files, verify job, installer smoke tests) can be finished and left ready so the release is a single `git tag v0.1.0 && git push --tags` once the user has reviewed.

### 2026-09-21T20:48:29Z — claude-code (agent)

Workflow half done: `publish` in release.yml needs [gate, verify, verify-windows], downloads the `release-dist` artifact and runs softprops/action-gh-release@v2 with dist/*.gz + dist/checksums.txt, generate_release_notes: true, prerelease when the tag starts with v0. Nothing was tagged or pushed. What remains is human-only: create the GitHub repo CodeVachon/merge-pipeline, commit, `git tag v0.1.0 && git push origin main --tags`, then confirm `curl -fsSL https://raw.githubusercontent.com/CodeVachon/merge-pipeline/main/install.sh | sh` installs it and `merge-pipeline upgrade --check` reports up to date. Left in Todo deliberately.

### 2026-09-22T15:37:56Z — claude-code (agent)

2026-09-22: remote created by the user and the full tree pushed as commit 776995a on main (github.com/CodeVachon/merge-pipeline, public). Commits use the CodeVachon noreply identity, set repo-locally to match motte. Remaining human step is unchanged: once CI is green on main, `git tag v0.1.0 && git push origin v0.1.0` triggers release.yml.

### 2026-09-23T14:28:04Z — claude-code (agent)

Released by the user: tag v0.1.0 pushed 2026-09-22 from commit 776995a; release.yml run 35752625232 succeeded in 6m30s (gate, five-target build, verify, verify-windows, publish) and the pre-release is live at github.com/CodeVachon/merge-pipeline/releases/tag/v0.1.0. Note: v0.1.0 predates commit 6ff1bfd (config doctor/init, default config on install, branch sync); those need a v0.1.1 or v0.2.0 tag to ship.
