---
id: 29
title: Download, verify, install, prune
state: Done
parent: 5
labels: [self-update]
blockedBy: [27]
created: 2026-09-17T17:13:55Z
updated: 2026-09-21T20:39:37Z
---

## Description

Port install/download.ts: fetch `<base>/<asset>.gz` and `<base>/checksums.txt`; find the line ending in ` <asset>` (tolerate one or two spaces); sha256 (sha2) of the compressed bytes must match before anything is written; gunzip (flate2); write to versions/<v>/bin/<exe>, chmod 755 on unix; replace `current` symlink (remove then symlink; on Windows fall back to a junction or a copy into current/bin — record whichever works in a note). `prune(versions_dir, ordered, keep, running)` removes beyond `max(keep,1)`, never the running version, reporting kept_running.

## Plan

Tests against a local http server (tiny_http dev-dep or a std TcpListener responding to two GETs) serving a fake asset + checksums; assert mismatch refuses to write.

## Notes

### 2026-09-21T20:39:15Z — claude-code (agent)

Done in src/selfupdate/download.rs. fetch_verified_binary_from(base, version, target) downloads <asset>.gz and checksums.txt (ureq body limit raised to 256MB from ureq's 10MB default), matches the line ending in " <asset>" tolerating one or two spaces and uppercase hex, sha256 (sha2) must match before anything is written, then gunzips with flate2. install_binary writes to versions/<v>/bin/.merge-pipeline.partial then renames, chmod 755 on unix, and repoints `current` (remove symlink/dir, then symlink; on Windows symlink_dir with a fallback that copies bin/ into current/bin — untested here, no Windows machine). prune_versions keeps max(keep,1) newest and never the running version, reporting kept_running. Tests: 5 unit (checksum parsing, round trip, mismatch/unlisted refuse, install+repoint+mode, prune) + fake-server integration tests for mismatch and a 404 checksums.txt.
