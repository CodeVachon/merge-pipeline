---
id: 34
title: "Gate job: tag equals Cargo version, fmt, clippy, tests"
state: Done
parent: 6
assignee: claude-code
labels: [release]
blockedBy: [10]
created: 2026-09-17T17:14:06Z
updated: 2026-09-21T20:19:27Z
---

## Description

Fail fast if `v$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[0].version')` != tag. Run fmt --check, clippy -D warnings, cargo test on ubuntu and macos. Build jobs `needs` this gate.

## Notes

### 2026-09-21T20:19:18Z — claude-code (agent)

`gate` job in release.yml runs on ubuntu-latest and macos-latest with fail-fast: resolves the tag (workflow_dispatch input or github.ref_name) and the Cargo version via `cargo metadata --no-deps --format-version 1 | jq -r '.packages[0].version'`, fails if tag != v<version>, then fmt --check, clippy -D warnings, cargo test --locked. `build` needs gate. Tag and version are exported as job outputs for #35/#36 to consume. Checkout uses `ref: inputs.tag || github.ref` so a dispatched run builds the tag it names, not the default branch.
