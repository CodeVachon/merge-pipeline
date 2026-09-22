---
id: 6
title: "Epic: Release pipeline (GitHub Actions)"
state: Todo
labels: [release]
blockedBy: [3, 5]
created: 2026-09-17T17:11:56Z
updated: 2026-09-21T20:55:14Z
---

## Description

Tag-driven release that builds, verifies, then publishes — never publishes before verifying, and the published bytes are the tested bytes. Modelled on motte's release.yml, adapted because Rust does not cross-compile from a single runner the way Bun does.

Jobs:
1. build (matrix): macos-14 → aarch64-apple-darwin and x86_64-apple-darwin; ubuntu-latest → x86_64-unknown-linux-musl (static, so it runs on any glibc); ubuntu-24.04-arm → aarch64-unknown-linux-musl; windows-latest → x86_64-pc-windows-msvc. `cargo build --release --locked`, gzip to `merge-pipeline-<os>-<arch>[.exe].gz`, upload artifact per target.
2. gate: tag must equal `v$(cargo metadata version)`; cargo fmt --check, clippy -D warnings, cargo test on ubuntu + macos.
3. verify: download all artifacts, write checksums.txt, then on ubuntu serve dist/ with python http.server and run install.sh with MERGE_PIPELINE_DOWNLOAD_BASE pointed at it; smoke: `merge-pipeline --version` equals the Cargo version, `merge-pipeline --help`, `merge-pipeline mcp` answers an `initialize` request piped on stdin, and `test` action against a fixture repo prints the expected steps. Same for install.ps1 on windows-latest.
4. publish: softprops/action-gh-release with *.gz + checksums.txt, generate_release_notes, prerelease while tag is v0.x.

Also: Dependabot/renovate for cargo is optional; do not add until asked.

## Plan

Children: build matrix + artifact naming; checksums + installer verification; publish job with tag/version gate; a CHANGELOG.md convention and first tag v0.1.0 once CLI parity is reached.

## Notes

### 2026-09-21T20:55:14Z — claude-code (agent)

Epic status 2026-09-21: all workflow code is complete (#33, #34, #35 Done; release.yml has gate, build matrix, verify, verify-windows, publish). The epic stays open only because #36 requires the human release step: create the GitHub remote, commit, `git tag v0.1.0`, push the tag. The verify job was replayed locally end to end (install.sh into a temp root from locally served assets, smoke.sh all checks green). Unverified until first tag: musl builds of ureq/rustls, install.ps1 on Windows, actionlint.
