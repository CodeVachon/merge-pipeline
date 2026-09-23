---
id: 42
title: "`config init` writes default workflows; `install` calls it"
state: Done
parent: 40
assignee: claude-code
labels: [cli]
blockedBy: [41]
created: 2026-09-22T19:13:21Z
updated: 2026-09-22T19:22:20Z
---

## Description

`merge-pipeline config init [--scope project|user] [--force]` creates the target directory (`<cwd>/.merge-pipeline/` or dirs::config_dir()/merge-pipeline) and writes patch.json, minor.json, canary.json, default.json from `include_str!("../../examples/config/<name>.json")` (so examples/ and the embedded defaults are one source). Existing files are skipped unless --force; each file is reported as created or kept. `merge-pipeline install [--scope …]` keeps wiring .mcp.json exactly as today and then runs the same init for the same scope, so a fresh repo is usable straight after `install`; add `--no-config` to install to opt out. Idempotent: running install twice changes nothing and says so.

## Plan

Implement in src/cli/config_init.rs with a pure `write_defaults(dir, force) -> Vec<Outcome>` plus the clap wiring in src/cli/mod.rs; call it from src/mcp/install.rs's run path (or from main.rs after run_install) — pick whichever keeps mcp/install.rs unaware of CLI scope types; note the choice. Tests: init into a temp dir creates four files identical to examples/config; second run keeps them; --force rewrites; install into a temp cwd produces both .mcp.json and .merge-pipeline/*.json and is idempotent; `config doctor` on the freshly created dir reports ok with 3 enabled workflows and no errors. Document in README (install section and a `config init` row), MIGRATING (this replaces copying the old config dir by hand), CHANGELOG Unreleased.

## Notes

### 2026-09-22T19:22:19Z — claude-code (agent)

Implemented in src/cli/config_init.rs: `DEFAULTS` embeds the four examples via include_str!, `target_dir(scope, cwd)`, pure `write_defaults(dir, force) -> InitReport{dir, files:[{path, outcome: created|kept|overwritten}]}`, `render`. CLI: `config init [--scope project|user] [-c DIR] [--force]`; `install` gained `--no-config` and, in main.rs after the .mcp.json merge, calls run_config_init for the same scope (kept in main.rs so mcp/install.rs stays unaware of workflow directories). Tests: 4 unit + 4 assert_cmd in tests/cli_init.rs (byte-identical to examples, kept vs --force, install idempotent + doctor passes on the result, --no-config). 165 tests green.

Real output, release binary, in a fresh `git init` dir:

$ merge-pipeline install
created …/.mcp.json with the merge-pipeline server
created …/.merge-pipeline/patch.json
created …/.merge-pipeline/minor.json
created …/.merge-pipeline/canary.json
created …/.merge-pipeline/default.json
workflow directory ready at …/.merge-pipeline (run `merge-pipeline config doctor` to check it)

$ merge-pipeline install   (again)
…/.mcp.json already wires merge-pipeline; nothing changed
kept    …/.merge-pipeline/patch.json  (×4)
…/.merge-pipeline already has every default workflow; nothing changed

$ merge-pipeline config doctor -c .
4 workflows, 3 enabled · 0 errors, 9 warnings   (every pattern warns "matches no local branch" — the repo has no branches yet, which is the right message)
✓ configuration is usable

README/MIGRATING/CHANGELOG updates for init, doctor and install --no-config are written together with #43's MCP table update.
