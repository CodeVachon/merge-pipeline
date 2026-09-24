---
id: 49
title: "Epic: pull-request workflow and useful release notes"
state: Todo
labels: [release, docs]
created: 2026-09-23T15:15:25Z
updated: 2026-09-23T15:15:25Z
---

## Description

User decision 2026-09-23: all changes go through pull requests from now on, so GitHub's generated release notes (generate_release_notes: true in release.yml) have PR titles to list instead of an empty "Full Changelog" link.

Deliverables:
- `.github/release.yml` release-notes config with categories by label: `feature`/`enhancement` → Features, `bug` → Fixes, `docs` → Documentation, `release`/`ci` → Internal; exclude `skip-changelog`.
- `.github/pull_request_template.md`: one-line summary, motte issue refs (`Closes .motte #NN` style, since issues live in motte not GitHub), verification checklist (fmt/clippy/test, CHANGELOG Unreleased updated).
- AGENTS.md: a "Branching and pull requests" section: never commit to main; branch `feat/…`, `fix/…`, `docs/…`; open the PR with `gh pr create`, PR title is the release-note line, apply a label; squash-merge is the default so one PR = one commit = one release-note line; CHANGELOG Unreleased entry in the same PR.
- Motte notes record the decision; agents keep tracking work in motte.
