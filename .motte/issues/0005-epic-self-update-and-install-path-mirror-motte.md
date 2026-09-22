---
id: 5
title: "Epic: Self-update and install path (mirror motte)"
state: Done
labels: [self-update]
blockedBy: [1]
created: 2026-09-17T17:11:47Z
updated: 2026-09-21T20:39:41Z
---

## Description

Replicate motte's install/upgrade contract so the two tools behave identically on a machine. Reference implementation read on 2026-09-17: CodeVachon/motte install.sh, packages/cli/src/commands/upgrade.ts, install/{layout,releases,download}.ts, version.ts.

On-disk contract:
  ~/.merge-pipeline/versions/v<X.Y.Z>/bin/merge-pipeline   the binary
  ~/.merge-pipeline/current -> versions/v<X.Y.Z>            active version (symlink, replaced with ln -sfn semantics)
  ~/.local/bin/merge-pipeline -> ~/.merge-pipeline/current/bin/merge-pipeline
  ~/.merge-pipeline/update-check.json                       {lastAttemptAt, lastSuccessAt?, latest?}
Env overrides: MERGE_PIPELINE_VERSION, MERGE_PIPELINE_INSTALL_DIR, MERGE_PIPELINE_BIN_DIR, MERGE_PIPELINE_NO_MODIFY_PATH, MERGE_PIPELINE_DOWNLOAD_BASE (lets CI verify the installer against a local http.server before publishing).

Release assets: merge-pipeline-{darwin-arm64,darwin-x64,linux-x64,linux-arm64}.gz, merge-pipeline-windows-x64.exe.gz, checksums.txt (sha256 of the .gz, `<hex>  <asset>` lines). Checksum is verified before anything is written.

`upgrade [target] --check --keep <n=2> --force --json`:
- Locate the managed install from the running binary's real path (…/versions/vX/bin/<exe>); if not managed, print the curl|sh hint and exit 1.
- Resolve latest: GET /releases/latest, fall back to /releases newest non-draft; detect rate limiting (403/429 + "rate limit") and say so. Normalise `0.1.0` → `v0.1.0`; compare with the `semver` crate so a downgrade is reported as one.
- Download + verify + gunzip + write to versions/<v>/bin, chmod 755, repoint `current`. Prune to `--keep` newest, never the running version (report it as keptRunning).
- Record update-check.json on every latest lookup.
`uninstall [--yes] [--json]`: remove root and only those PATH symlinks whose realpath resolves inside root. Backlogs/config untouched.
Version string comes from CARGO_PKG_VERSION; release workflow refuses if the git tag differs from Cargo.toml.

## Plan

Children: layout module (target detection, locate install, version ordering) with a test that parses install.sh and asserts asset names + directory shape agree, as motte's layout.test.ts does; releases lookup; download/verify/install/prune; upgrade + uninstall subcommands; install.sh (POSIX sh) and install.ps1; optional passive nudge (at most once per 24h, stderr, never blocks, opt out via MERGE_PIPELINE_NO_UPDATE_CHECK).
