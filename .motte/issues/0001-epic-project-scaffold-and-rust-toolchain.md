---
id: 1
title: "Epic: Project scaffold and Rust toolchain"
state: Done
labels: [scaffold]
created: 2026-09-17T17:10:57Z
updated: 2026-09-17T17:48:18Z
---

## Description

Stand up the Rust project that replaces ../cli-merge-pipeline (Bun/TypeScript). Goal for the whole rewrite: one small, fast, self-contained binary named `merge-pipeline` with no runtime prerequisites other than `git` on PATH.

Baseline being replaced: /Users/cvachon/Repositories/Github/CodeVachon/cli-merge-pipeline (src/ ~1,100 lines TS; deps: yargs, inquirer + inquirer-search-list, chalk). Tests: vitest, 8 spec files, the meaningful coverage is in resolvePackageJsonVersionConflicts.test.ts, Workflows.test.ts and GitApi.test.ts.

Architecture decisions (recorded here so children don't re-litigate):
- Single crate, `src/lib.rs` (core, no terminal I/O) + `src/main.rs` (thin CLI). Modules: `config`, `pipeline`, `git`, `conflicts`, `ui`, `cli`, `mcp`, `selfupdate`.
- Synchronous throughout. No tokio. Git is shelled out via std::process::Command (mirrors baseline; keeps binary small and behaviour identical). HTTP via `ureq` (rustls). MCP is a hand-rolled JSON-RPC 2.0 stdio loop over serde_json rather than the `rmcp` SDK, which would drag in tokio. Revisit only if MCP scope grows beyond tools/list + tools/call.
- Prompts via `inquire` (Select has built-in fuzzy filtering, replacing inquirer-search-list). Colors via `owo-colors` (truecolor hex #ff6700 orange / #00d5ff cyan preserved).
- Every interactive decision goes through a `Prompter` trait so the same core serves the CLI (interactive impl) and the MCP server (non-interactive impl that takes pre-supplied answers or returns the open question). Baseline already injects `askQuestion` into the conflict resolver; this generalises that.
- Local toolchain is rustc 1.77.2 (2024-04). `rustup check` shows stable 1.98.1 available. Update before anything else; pin with rust-toolchain.toml.

## Plan

1. rustup update; add rust-toolchain.toml (stable) and `rust-version` in Cargo.toml.
2. cargo init --name merge-pipeline with lib + bin targets; add clap, serde, serde_json, regex, anyhow, thiserror, inquire, owo-colors, semver, tempfile (dev), assert_cmd (dev).
3. Release profile: opt-level 3, lto = "fat", codegen-units = 1, strip = true, panic = "abort".
4. GitHub Actions ci.yml: fmt --check, clippy -D warnings, test, on ubuntu + macos.
5. Update AGENTS.md with the cargo commands agents must run before finishing (fmt, clippy, test), keeping the motte block.
6. .gitignore for target/, and a `config/` example directory that is NOT gitignored this time (baseline gitignored config/*.json which hid the example workflows).
