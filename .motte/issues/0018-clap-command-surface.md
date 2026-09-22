---
id: 18
title: clap command surface
state: Done
parent: 3
assignee: claude-code
labels: [cli]
blockedBy: [9]
created: 2026-09-17T17:13:18Z
updated: 2026-09-21T20:31:48Z
---

## Description

Root invocation (no subcommand) is the interactive workflow run with flags -c/--cwd, -w/--workflow, -p/--auto-push (alias --auto_push, hidden), -f/--config, -a/--action <run|dry-run|test>. Subcommands reserved for other epics: `mcp`, `upgrade`, `uninstall`, `install`, `config path` (prints resolved config dir and which rule chose it), `completion <shell>` via clap_complete. `--version` prints CARGO_PKG_VERSION only (the release smoke test compares it to the tag).

## Plan

Derive structs; `cargo run -- --help` snapshot test with assert_cmd; unknown flag exits 2 (baseline used yargs strict).

## Notes

### 2026-09-21T20:31:47Z — claude-code (agent)

Done. cli.rs holds Cli/RunArgs/Command and the arg structs (UpgradeArgs, UninstallArgs, InstallArgs, ConfigCommand); main.rs dispatches to selfupdate::run_upgrade/run_uninstall and mcp::serve/run_install, which are `bail!("not implemented yet")` stubs appended to those modules for the owning agents to replace. Added clap_complete 4.6.11 for `completion <shell>`. `--auto_push` is a hidden clap alias of `--auto-push`. Action is a ValueEnum (CliAction) converted into runner::Action so --help lists the choices like yargs did. `args_conflicts_with_subcommands` keeps root flags and subcommands from mixing. Tests: 6 unit (parse shapes) + 6 assert_cmd (exact --version line, help contents, exit 2 on unknown flag/bad action, completion for 4 shells, upgrade --help flags). The baseline had no CLI tests; these are additions. Full about text comes from Cargo description; the boxed "Git Branch / Workflow" title is separate (#20).
