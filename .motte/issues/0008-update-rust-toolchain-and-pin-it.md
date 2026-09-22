---
id: 8
title: Update Rust toolchain and pin it
state: Done
parent: 1
assignee: claude-code
labels: [scaffold]
created: 2026-09-17T17:12:21Z
updated: 2026-09-17T17:45:28Z
---

## Description

rustc is 1.77.2 (2024-04); `rustup check` reports stable 1.98.1 and rustup 1.29.1 available. Modern clap/inquire/regex releases and edition 2024 need ≥1.85.

## Plan

rustup self update; rustup update stable. Add rust-toolchain.toml with channel = "stable" and components rustfmt, clippy. Set edition = "2024" and rust-version in Cargo.toml. Verify: cargo --version ≥ 1.85, cargo clippy runs.

## Notes

### 2026-09-17T17:45:25Z — claude-code (agent)

rustup self update succeeded (1.27.0 → 1.29.1; rustup was a standalone install, not a package manager). rustup update stable: rustc 1.77.2 → 1.98.1 (48a229cea 2026-09-01), cargo 1.98.1, rustfmt 1.9.0, clippy 0.1.98. rust-toolchain.toml pins channel = "stable" with rustfmt + clippy components. rust-version in Cargo.toml is set in #9.
