---
id: 9
title: Cargo project skeleton with lib + bin and release profile
state: Done
parent: 1
assignee: claude-code
labels: [scaffold]
blockedBy: [8]
created: 2026-09-17T17:12:21Z
updated: 2026-09-17T17:47:06Z
---

## Description

Crate `merge-pipeline`, binary name `merge-pipeline`, library `merge_pipeline`. Module stubs: config, pipeline, git, conflicts, prompt, ui, cli, mcp, selfupdate. Dependencies: clap (derive), serde, serde_json, regex, anyhow, thiserror, inquire, owo-colors, semver, ureq (rustls, no default features beyond tls), sha2, flate2, dirs. Dev: tempfile, assert_cmd, predicates.

## Plan

cargo init; wire [lib] and [[bin]]; [profile.release] opt-level=3, lto="fat", codegen-units=1, strip=true, panic="abort". `cargo build --release` produces a binary; record its size in a note as the baseline to keep small (Bun compile was ~58MB; target well under 10MB).

## Notes

### 2026-09-17T17:47:04Z — claude-code (agent)

Skeleton built. Versions cargo resolved on 2026-09-17 with rustc 1.98.1: clap 4.6.7 (derive), serde 1.0.229, serde_json 1.0.151, regex 1.13.1, anyhow 1.0.104, thiserror 2.0.20, inquire 0.9.4, owo-colors 4.4.0, semver 1.0.28, ureq 3.4.2 (default-features off, feature "rustls" → ring + webpki-roots, no native-tls), sha2 0.11.0, flate2 1.1.10 (pure-Rust miniz_oxide backend), dirs 7.0.0; dev: tempfile 3.27.0, assert_cmd 2.2.2, predicates 3.1.4. rust-version = "1.98", edition 2024.

Size baseline: release binary (opt-level 3, lto fat, codegen-units 1, strip, panic abort) with only clap wired = 501,376 bytes (492K) on darwin-arm64. Only clap is actually linked yet, so expect growth when ureq/rustls/inquire get used; the target from the epic (well under 10MB) has plenty of headroom. Bun baseline was ~58MB.

Deviations: dropped the `readme = "README.md"` manifest key until the docs epic creates the file. Added tests/cli_version.rs (two assert_cmd smoke tests: --version prints CARGO_PKG_VERSION; unknown flag exits 2) so the test gate is exercised from day one. main.rs is a clap derive with no options yet; #18 replaces it.
