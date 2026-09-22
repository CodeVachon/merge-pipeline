---
id: 30
title: "`upgrade` and `uninstall` subcommands"
state: Done
parent: 5
labels: [self-update]
blockedBy: [18, 28, 29]
created: 2026-09-17T17:13:55Z
updated: 2026-09-21T20:39:38Z
---

## Description

upgrade [target] --check --keep <2> --force --json: not-managed → print the curl|sh hint, exit 1. Resolve wanted (latest or target), record update-check.json, compare with semver: equal → "already on vX (pass --force)"; wanted older → warn downgrade then proceed; print downloading/checksum verified/installed/pruned/kept lines like motte; --json emits {from,to,pruned,keptRunning?,installRoot} or for --check {current,latest,upToDate,isDowngrade,installRoot,installed}. uninstall [--yes] [--json]: without --yes list what would be removed and exit 1; with it remove root and any ~/.local/bin, ~/bin, $MERGE_PIPELINE_BIN_DIR link whose realpath is inside root.

## Notes

### 2026-09-21T20:39:25Z — claude-code (agent)

Done in src/selfupdate/upgrade.rs + adapters in mod.rs. #18's clap types are cli::UpgradeArgs{target, check, keep: usize=2, force, json} and cli::UninstallArgs{yes, json}; main.rs calls selfupdate::run_upgrade(&UpgradeArgs)/run_uninstall(&UninstallArgs) -> anyhow::Result<()>, which convert to UpgradeOptions/UninstallOptions and call upgrade::run_upgrade_with(options, UpgradeContext, &mut dyn Write) -> Result<i32 exit code>. UpgradeContext::detect() reads the real install/version/host; tests build one pointing at a temp install and the fake server. Output lines and --json shapes copy motte's upgrade.ts (not-managed INSTALL_HINT with the curl|sh line; "already on vX — pass --force"; "! vX is older than the running vY"; downloading/✓ checksum verified/✓ installed/pruned/kept … currently running; --check JSON {current,latest,upToDate,isDowngrade,installRoot,installed}; upgrade JSON {from,to,pruned,keptRunning?,installRoot}). update-check.json {lastAttemptAt,lastSuccessAt?,latest} is recorded on every latest lookup, tolerating corruption. uninstall: without --yes prints the plan and exits 1 (via std::process::exit after flushing, since a non-error exit code cannot travel through anyhow::Result — noted in code); with --yes removes root and only PATH links whose canonical target is inside root ($MERGE_PIPELINE_BIN_DIR, ~/.local/bin, ~/bin). No colour: ui.rs was empty when written; plain ✓/! prefixes. End-to-end tests in tests/selfupdate.rs: install+repoint+prune with JSON, --check with and without JSON changes nothing, up-to-date/--force/explicit downgrade, uninstall confirm-then-remove keeping a foreign link.
