# Config Diagnostics

Two commands for setting up and checking a workflow directory.

## `config doctor`

```
{{#include help/config_doctor.txt}}
```

Checks each file's JSON, name, `pipeline` length and regular expressions, flags duplicate names,
unknown keys and mixed `order`. With `-c <repo>` it also reports which local branch each pattern
matches right now — none is a warning (the next run would fail with "no matching branch"),
several is a note that a run will ask.

Warnings keep exit code 0; errors exit 1. `--json` emits `{root, rule, workflows, problems, ok}`
— the same shape the MCP [`doctor_config`](mcp-server.md#doctor_config) tool returns, so a script
or an agent gets the same answer as the terminal.

```sh
merge-pipeline config doctor -c ~/src/app
```

## `config init`

```
{{#include help/config_init.txt}}
```

Writes `patch.json`, `minor.json`, `canary.json` and a disabled `default.json` — identical to the
files in [`examples/config/`](https://github.com/CodeVachon/merge-pipeline/tree/main/examples/config).
Existing files are kept unless you pass `--force`. `--scope user` targets the user config
directory instead of the current repository.

`merge-pipeline install` runs `config init` for you as part of wiring the MCP server (unless you
pass `--no-config`), so a fresh repository has workflows to run immediately. See
[MCP Server](mcp-server.md).

## `config path`

```
{{#include help/config_path.txt}}
```

Prints the directory that the [search order](workflow-files.md#where-workflow-files-are-looked-for)
resolved to, and which rule chose it — the fastest way to find out why a workflow file is not
being picked up.
