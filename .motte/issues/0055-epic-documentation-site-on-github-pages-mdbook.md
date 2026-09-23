---
id: 55
title: "Epic: documentation site on GitHub Pages (mdBook)"
state: Todo
labels: [docs]
created: 2026-09-23T18:33:17Z
updated: 2026-09-23T18:33:17Z
---

## Description

Requested 2026-09-23: a docs site covering everything the tool does, published to GitHub Pages.

Decisions:
- **mdBook**, not a JS site generator. This project's whole premise is a single small Rust binary with no npm/node toolchain (that is why cli-merge-pipeline was rewritten in the first place); mdBook is a single static binary, markdown source, built-in search and theme toggle, and is what the Rust ecosystem itself uses for exactly this (rustup, cargo, the Rust book). motte's own site is a bespoke Bun/Vite app under apps/site, which fits motte's existing bun monorepo — not a reason to add a JS toolchain here.
- Source lives at `docs/` (mdBook convention: `docs/book.toml`, `docs/src/*.md`, `docs/src/SUMMARY.md`). The CHANGELOG page uses mdBook's `{{#include ../../CHANGELOG.md}}` to embed the real file rather than duplicate it, so it can never drift.
- GitHub Pages is already enabled for CodeVachon/merge-pipeline, build type "workflow" (Actions-based deployment, not a gh-pages branch), serving at https://codevachon.github.io/merge-pipeline/.
- Two workflow jobs, mirroring the project's existing "verify what's actually live" philosophy (see release.yml's verify jobs, and motte's own pages.yml which checks the deployed schema URL, not just the build):
  - `.github/workflows/ci.yml` gains a `book` job (runs on every PR/push touching `docs/`): install a pinned mdbook release, `mdbook build`, `mdbook test` if applicable, and a link-check pass (mdbook-linkcheck or a small script) so a broken internal link fails CI before merge.
  - `.github/workflows/pages.yml` (new): on push to main, build the book, configure-pages, upload-pages-artifact, deploy-pages, then curl the live URL and grep for a known string (e.g. the book title) so a deploy that "succeeded" but served nothing is caught, same as motte does for its schema.
- Content covers every shipped and in-flight feature, written from the actual README/MIGRATING/CHANGELOG/CLI --help output and code, not re-derived from memory: introduction & quick start, install (curl one-liner + cargo build), workflow files & the config schema, usage & the three actions (honest dry-run note), branch sync, config doctor/init and install's auto-config, the MCP server & its six tools, keeping up to date (upgrade/versions/use/uninstall/the daily update offer from the still-open PR stack), migrating from cli-merge-pipeline, and a changelog page.
- README gets a short "Documentation" link near the top pointing at the Pages URL, without duplicating the content that now lives there.
