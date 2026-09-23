---
id: 58
title: README documentation link and final cross-checks
state: In Progress
parent: 55
assignee: claude-code
labels: [docs]
blockedBy: [56, 57]
created: 2026-09-23T18:33:37Z
updated: 2026-09-23T18:43:19Z
---

## Description

Add a short "Documentation" line near the top of README.md pointing at https://codevachon.github.io/merge-pipeline/, without pulling detailed content back into the README (the split is: README stays a fast-start reference, the book is the full manual). Re-read every book page once more against the live `--help` output of the actual built binary from this branch (including `versions`, `use`, `upgrade`, `config doctor`, `config init`) and fix any drift before opening the PR.

## Plan

Diff each page's command examples against `target/release/merge-pipeline <cmd> --help` output one more time as a final pass.
