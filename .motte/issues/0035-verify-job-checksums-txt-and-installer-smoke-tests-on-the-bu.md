---
id: 35
title: "Verify job: checksums.txt and installer smoke tests on the built bytes"
state: Done
parent: 6
assignee: claude-code
labels: [release]
blockedBy: [21, 22, 31, 33]
created: 2026-09-17T17:14:06Z
updated: 2026-09-23T15:06:29Z
---

## Description

Download all artifacts into dist/, write checksums.txt (`sha256sum *.gz`). On ubuntu: python3 -m http.server serving dist/; MERGE_PIPELINE_VERSION=<tag> MERGE_PIPELINE_DOWNLOAD_BASE=http://127.0.0.1:8123 MERGE_PIPELINE_INSTALL_DIR=$RUNNER_TEMP/root MERGE_PIPELINE_BIN_DIR=$RUNNER_TEMP/bin sh install.sh; then `--version` == Cargo version, `--help` exits 0, `printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}\n' | merge-pipeline mcp` returns serverInfo, and `-a test` against a scripted fixture repo prints the expected steps. Same on windows-latest with install.ps1. Upload dist/ (assets + checksums) as one artifact for publish.

## Notes

### 2026-09-21T20:48:25Z — claude-code (agent)

Implemented in .github/workflows/release.yml (jobs `verify`, `verify-windows`) and scripts/smoke.sh.

verify (ubuntu): downloads release-* artifacts merged into dist/, writes checksums.txt with `sha256sum` (two-space format; download.rs `expected_checksum` trims and matches ` <asset>` suffix, install.sh greps ` <asset>$`, so one or two spaces both parse), serves dist/ with python http.server, runs install.sh with MERGE_PIPELINE_{VERSION,DOWNLOAD_BASE,INSTALL_DIR,BIN_DIR,NO_MODIFY_PATH}, then scripts/smoke.sh. A static file at dist/repos/CodeVachon/merge-pipeline/releases/latest stands in for the GitHub API (MERGE_PIPELINE_API_BASE) so `upgrade --check` can resolve "latest" before the release exists; it is deleted before dist/ is uploaded as `release-dist`.

smoke.sh checks: --version == "merge-pipeline <v>", --help and upgrade --help exit 0, `mcp` initialize reply contains "serverInfo" (SMOKE_ALLOW_MCP_STUB=1 downgrades to a warning for local runs until #22 lands), a throwaway repo with bare origin + main/staging-patch/Patch-v0.1.1 where `-a test --yes` prints "Step 1: Merge Patch-v0.1.1 into staging-patch" and `-a run --auto-push --yes` lands the merge on origin/staging-patch (checked with merge-base --is-ancestor), and from a managed install `upgrade v<v> --check --json` plus the latest-lookup variant report upToDate true. Upgrade checks are skipped with a warning when the binary is not under versions/<v>/bin, so the script also runs against target/release locally.

verify-windows: install.ps1 against the same served dist/, then --version equality and --help in pwsh. Windows has no git fixture smoke; noted as a gap.

Verified locally on darwin-arm64: release.yml parses (ruby YAML), `sh -n scripts/smoke.sh`, smoke.sh against target/release passes, and a full replay of the verify job (gzip asset, shasum checksums.txt, http.server, install.sh into a temp root, smoke.sh on the installed symlink) passes all 8 checks including both upgrade checks. Not verified locally: the mcp handshake (subcommand still a stub, #22 in progress), install.ps1 (no pwsh here), actionlint/shellcheck (not installed). CI runs the real thing on the first tag.

### 2026-09-23T15:06:29Z — claude-code (agent)

2026-09-23, user request: verify (linux) and verify-windows now run in parallel. A new `checksums` job (needs build) downloads the release-* artifacts, writes checksums.txt and uploads `release-dist` once; both verify jobs need [gate, checksums] and download that bundle; publish still needs both verifies. The releases-API stand-in stays inside the linux job and is no longer removed from dist because that job no longer uploads anything. Job graph checked with ruby YAML.
