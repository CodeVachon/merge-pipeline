---
id: 53
title: "Docs: README version management section, CHANGELOG, install.sh wording"
state: Todo
parent: 50
labels: [self-update, cli]
blockedBy: [52]
created: 2026-09-23T15:16:17Z
updated: 2026-09-23T15:16:17Z
---

## Description

README "Keeping it up to date" gains a table of `upgrade`, `versions`, `use`, `versions remove`, `versions prune` and a sentence that switching takes effect on the next command because PATH points at the stable `current` link. install.sh's closing advice keeps the PATH note (that one is genuinely first-install only) but must not imply a restart is needed after upgrades. CHANGELOG Unreleased entries. Motte note recording that no shell restart was ever required and why.
