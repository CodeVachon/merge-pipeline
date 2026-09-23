---
id: 57
title: "CI: book job on PRs, pages.yml deploy-and-verify on main"
state: Todo
parent: 55
labels: [docs]
blockedBy: [56]
created: 2026-09-23T18:33:37Z
updated: 2026-09-23T18:33:37Z
---

## Description

Add a `book` job to .github/workflows/ci.yml, paths-filtered to `docs/**` and workflow changes, or just always-on if that is simpler and the cost is acceptable (an mdbook build is seconds): install a pinned mdbook version (curl the release tarball the way install.sh fetches assets, or a maintained setup action — pick one and note the choice), `mdbook build docs`, fail on any warning, and a link-check step (peaceiris/actions-mdbook has a linkcheck backend, or a small script using `mdbook-linkcheck` if trivial to add, or grep for `href="http` 404s with a lightweight local check — pick the cheapest option that actually catches a broken internal link and say why). Add .github/workflows/pages.yml: on push to main (and workflow_dispatch), build the book, actions/configure-pages, actions/upload-pages-artifact with docs/book (mdbook's output dir), actions/deploy-pages, then curl the deployed URL and grep for the book title so a deploy that served nothing is caught — same pattern release.yml already uses for install.sh/install.ps1 and the pattern motte's own pages.yml uses for its schema.

## Plan

Validate both workflow YAMLs with ruby -ryaml. This job cannot be exercised end-to-end until the branch reaches main (pages.yml only triggers on push to main), so the PR description must say so plainly — the CI `book` job on the PR is what can be verified now.
