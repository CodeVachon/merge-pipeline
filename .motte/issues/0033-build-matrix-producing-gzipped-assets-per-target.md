---
id: 33
title: Build matrix producing gzipped assets per target
state: Done
parent: 6
assignee: claude-code
labels: [release]
blockedBy: [9]
created: 2026-09-17T17:14:06Z
updated: 2026-09-21T20:19:24Z
---

## Description

release.yml on tags v* and workflow_dispatch(tag). Matrix: macos-14 aarch64-apple-darwin + x86_64-apple-darwin; ubuntu-latest x86_64-unknown-linux-musl; ubuntu-24.04-arm aarch64-unknown-linux-musl; windows-latest x86_64-pc-windows-msvc. `cargo build --release --locked --target`, rename to merge-pipeline-<os>-<arch>[.exe], gzip -9, upload-artifact per target. Musl for Linux so the binary runs on any distro; if a crate refuses musl, fall back to gnu and note it.

## Notes

### 2026-09-21T20:19:14Z — claude-code (agent)

release.yml written with a 5-target build matrix (macos-14 darwin-arm64/x64, ubuntu-latest linux-x64 musl, ubuntu-24.04-arm linux-arm64 musl, windows-latest windows-x64 msvc). Assets are merge-pipeline-<os>-<arch>[.exe].gz produced with `gzip -9 -n` (no timestamp, so checksums are reproducible); uploaded as artifacts named release-<os>-<arch>, 1-day retention. `verify` and `publish` exist as `if: false` placeholders so the file is complete YAML now; #35 and #36 fill them in. Unverified until the first tag: ureq with the rustls feature builds on musl in principle (pure Rust, ring/aws-lc-rs have musl support), but no musl build has been run yet. If aws-lc-rs fails to compile on musl, switch ureq's TLS backend to the `rustls` feature with `ring` crypto or add the `rustls-no-provider` feature and pick ring explicitly. YAML validated with ruby; actionlint not installed locally so not run.
