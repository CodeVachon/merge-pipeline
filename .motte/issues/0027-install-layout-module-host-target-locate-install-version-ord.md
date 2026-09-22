---
id: 27
title: "Install layout module: host target, locate install, version ordering"
state: Done
parent: 5
assignee: claude-code
labels: [self-update]
blockedBy: [9]
created: 2026-09-17T17:13:55Z
updated: 2026-09-21T20:39:34Z
---

## Description

Port install/layout.ts. `host_target()` → {platform: darwin|linux|windows, arch: x64|arm64, binary_name, asset_name} from std::env::consts, erroring for unbuilt combos. `locate_install(exe = std::env::current_exe())` → canonicalize; require `…/versions/<vX.Y.Z>/bin/<exe>`; return {root, version, binary, versions_dir, current_link}; None when running from cargo target/. `installed_versions(dir)` newest first via semver. `normalize_version` adds the v. `download_base(version)` honours MERGE_PIPELINE_DOWNLOAD_BASE.

## Plan

Tests including one that reads install.sh from the repo and asserts every asset name and the `versions/<v>/bin` shape appear in it verbatim, so the two implementations cannot drift (motte's layout.test.ts does this).

## Notes

### 2026-09-21T20:39:09Z — claude-code (agent)

Done in src/selfupdate/layout.rs (module converted to a directory: mod.rs, layout.rs, releases.rs, download.rs, upgrade.rs, nudge.rs). host_target_for(os, arch) maps std::env::consts values; five built targets; locate_install_from canonicalizes and requires …/versions/<semver>/bin/<exe>, so `cargo run` (target/debug) is correctly "not managed". Version ordering uses the `semver` crate (prerelease sorts below release). install_sh_agrees_with_this_layout reads install.sh and asserts REPO, .merge-pipeline, versions/, bin, current, checksums.txt, every MERGE_PIPELINE_* override, each os/arch token and the 'merge-pipeline-%s-%s' asset template, mirroring motte's layout.test.ts. 9 unit tests.
