---
id: 31
title: install.sh and install.ps1
state: Done
parent: 5
labels: [self-update]
blockedBy: [18, 27]
created: 2026-09-17T17:13:55Z
updated: 2026-09-21T20:39:40Z
---

## Description

POSIX sh (dash/busybox-safe) port of motte's install.sh with the names swapped: REPO=CodeVachon/merge-pipeline, root ~/.merge-pipeline, env MERGE_PIPELINE_{VERSION,INSTALL_DIR,BIN_DIR,NO_MODIFY_PATH,DOWNLOAD_BASE}. Steps: detect target, resolve version (latest then any), fetch asset + checksums, verify with sha256sum/shasum (refuse if neither), gunzip into versions/<v>/bin, ln -sfn current and ~/.local/bin link, run `--version` as a smoke test, generate completions via `completion <shell>` into ~/.merge-pipeline/completions (fish auto-installed only), PATH advice. install.ps1 equivalent for windows-x64.

## Plan

shellcheck clean. Verified in the release workflow against locally served assets (release epic).

## Notes

### 2026-09-21T20:39:29Z — claude-code (agent)

Done: install.sh (POSIX port of motte's installer, names swapped: REPO CodeVachon/merge-pipeline, root ~/.merge-pipeline, env MERGE_PIPELINE_{VERSION,INSTALL_DIR,BIN_DIR,NO_MODIFY_PATH,DOWNLOAD_BASE}; completions generated only if `merge-pipeline completion bash` succeeds, so an older binary cannot fail the install; smoke `--version` after install) and install.ps1 (windows-x64 only; junction for `current` with a copy fallback since symlinks need Developer Mode; adds current\bin to the user PATH unless MERGE_PIPELINE_NO_MODIFY_PATH). Validation: `sh -n`, `dash -n`, `bash -n` all pass; shellcheck and pwsh are not installed locally so neither was linted/parsed here — the release verify job (#35) exercises both against served assets. The layout unit test cross-checks install.sh's asset names and directory shape against the Rust module.
