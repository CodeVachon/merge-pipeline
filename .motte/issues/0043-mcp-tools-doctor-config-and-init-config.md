---
id: 43
title: MCP tools `doctor_config` and `init_config`
state: Done
parent: 40
assignee: claude-code
labels: [mcp]
blockedBy: [41, 42]
created: 2026-09-22T19:18:02Z
updated: 2026-09-22T19:24:58Z
---

## Description

Expose the new config commands to agents over MCP, reusing the exact same code paths as `config doctor` and `config init` (no duplicated checks).

- `doctor_config {cwd?, config?, check_branches?: bool (default true)}` → the same JSON the CLI's `--json` emits: {root, rule, workflows, problems:[{level,file,message}], ok}. When `ok` is false the result is still a normal (non-isError) result, because "here are the problems" is the successful answer; isError is reserved for not being able to run at all (e.g. cwd does not exist).
- `init_config {cwd?, scope?: "project"|"user" (default project), force?: bool (default false)}` → {dir, files:[{path, outcome: created|kept|overwritten}]}. Writes the embedded defaults; never overwrites without force.
- Register both in the tool registry, update README's MCP tool table, CHANGELOG Unreleased, and add end-to-end cases to tests/mcp.rs: doctor_config on the fixture with a good config (ok true) and a bad-regex config (ok false, problems non-empty, not isError); init_config into a temp cwd then doctor_config on it reports 3 enabled workflows; second init_config reports all kept.

## Plan

Implement after #41/#42 in src/mcp/tools/doctor_config.rs and init_config.rs, calling the pure functions from src/cli/doctor.rs and src/cli/config_init.rs (move them to a non-cli module if that dependency direction looks wrong; note the choice). Keep the JSON shapes identical between CLI --json and MCP so the README documents one shape.

## Notes

### 2026-09-22T19:24:57Z — claude-code (agent)

Implemented src/mcp/tools/doctor_config.rs and init_config.rs, registered after run_workflow (tools/list now: list_workflows, inspect_repo, plan_workflow, run_workflow, doctor_config, init_config). Both call the same pure functions the CLI uses (cli::doctor::diagnose, cli::config_init::{target_dir, write_defaults}) and serialize the same structs, so the JSON is byte-identical to `config doctor --json` / the init report; nothing duplicated. Dependency direction (mcp → cli::doctor/config_init) mirrors the existing mcp → cli::config_dir use, so the modules were left where they are. doctor_config: check_branches (default true) runs the branch report only when cwd has .git; a config dir that does not exist is isError, a config with problems is a normal result with ok=false. init_config: cwd/scope/force; unknown scope is isError. Tests: server unit test list updated, 2 new e2e tests in tests/mcp.rs (good/bad/absent config, check_branches false; init→doctor→kept→overwritten→bad scope). 167 tests green, fmt/clippy clean. README MCP table, config commands section, install section, MIGRATING §2 and CHANGELOG Unreleased updated.

Live probe, release binary over stdio: init_config on an empty temp dir → 4 files created; doctor_config on it → ok true, rule "<cwd>/.merge-pipeline", 4 workflows, 0 problems.
