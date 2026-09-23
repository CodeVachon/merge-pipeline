---
id: 40
title: "Epic: config doctor and default config on install"
state: Done
labels: [cli]
created: 2026-09-22T19:12:58Z
updated: 2026-09-22T19:24:59Z
---

## Description

Requested by the user on 2026-09-22 after the first push: (1) a `doctor` command that validates the workflow configuration, and (2) `merge-pipeline install` should also create the project-local `.merge-pipeline/` directory with the default workflow files so a fresh repo works immediately.

Design decisions:
- Doctor lives under the existing `config` subcommand: `merge-pipeline config doctor [-f/--config <dir>] [-c/--cwd <repo>] [--json]`, next to `config path`. Same resolution rules as everything else.
- Default config creation is its own command too, `merge-pipeline config init [--scope project|user] [--force]`, and `install` calls the same code so the behaviour is identical whether or not someone wants MCP wiring. `install --scope project` → `<cwd>/.merge-pipeline/`; `install --scope user` → the user config directory (dirs::config_dir()/merge-pipeline). Never overwrite an existing file unless --force; print each file created or skipped.
- The defaults are the four example workflows (patch, minor, canary, default) embedded with include_str! from examples/config/ so the binary carries them and the examples cannot drift from what init writes.
