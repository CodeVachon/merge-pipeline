---
id: 10
title: "CI workflow: fmt, clippy, test on ubuntu and macos"
state: Done
parent: 1
assignee: claude-code
labels: [scaffold]
blockedBy: [9]
created: 2026-09-17T17:12:21Z
updated: 2026-09-17T17:47:37Z
---

## Description

`.github/workflows/ci.yml` on push and pull_request. Uses dtolnay/rust-toolchain@stable and Swatinem/rust-cache. Steps: cargo fmt --all --check; cargo clippy --all-targets -- -D warnings; cargo test --locked. Windows CI can wait until the release epic adds the windows target.

## Notes

### 2026-09-17T17:47:35Z — claude-code (agent)

.github/workflows/ci.yml: push to main + pull_request, matrix ubuntu-latest/macos-latest, dtolnay/rust-toolchain@stable (reads rust-toolchain.toml, adds rustfmt+clippy), Swatinem/rust-cache@v2, then fmt --check, clippy --all-targets -D warnings, test --locked. Added concurrency cancel-in-progress per ref and contents: read permissions. YAML validated with ruby's YAML.load_file. Cargo.lock is committed-intent (it is a binary crate) so --locked works; verified locally. Windows deliberately deferred to the release epic (#33).
