# Workflow Files

A workflow is one JSON file. `pipeline` lists branch patterns in merge order: the first is merged
into the second, the second into the third, and so on.

```json
{{#include ../../examples/config/patch.json}}
```

| Field | Required | Meaning |
| --- | --- | --- |
| `name` | yes | Shown in the selection list; the value for `--workflow` |
| `pipeline` | yes | Two or more patterns. Each is a case-insensitive regular expression searched against local branch names |
| `description` | no | Free text |
| `order` | no | Lower numbers are listed first. Workflows without an order come after every ordered one, by file name |
| `disabled` | no | `true` loads the file but never offers it |
| `$schema` | no | Points editors at the [published schema](https://raw.githubusercontent.com/CodeVachon/merge-pipeline/main/schema/config.json) so they can validate the file as you type |

## How a pattern becomes a branch

Every local branch that matches a pattern is a candidate, minus branches already chosen earlier
in the same pipeline.

- **One candidate** is used as is.
- **Several candidates** produce a prompt (or, non-interactively, an error — see
  [Usage](usage.md)).
- **None** is an error.

So the canary example below lists `^Release-*` twice on purpose: the second occurrence has to
pick a different release branch than the first.

## The four example workflows

Four ready-made workflows live in
[`examples/config/`](https://github.com/CodeVachon/merge-pipeline/tree/main/examples/config).
Copy the ones you want into a workflow directory, or run `merge-pipeline config init` to write
all four at once (see [Config Diagnostics](config-diagnostics.md)).

**patch.json** — order 1, enabled:

```json
{{#include ../../examples/config/patch.json}}
```

**minor.json** — order 2, enabled:

```json
{{#include ../../examples/config/minor.json}}
```

**canary.json** — order 3, enabled, shows the repeated-pattern case:

```json
{{#include ../../examples/config/canary.json}}
```

**default.json** — order 100, disabled by default; a starting point to copy and edit:

```json
{{#include ../../examples/config/default.json}}
```

## Where workflow files are looked for

The first of these that exists as a directory wins:

1. `--config <PATH>` (`-f`)
2. `$MERGE_PIPELINE_CONFIG`
3. `<cwd>/.merge-pipeline/`
4. the user config directory: `~/.config/merge-pipeline/` on Linux,
   `~/Library/Application Support/merge-pipeline/` on macOS
5. `<directory of the binary>/config` (the layout of the old Bun tool)

Inside the chosen directory, a `config/` sub-directory is used if present; otherwise the directory
itself. Every `*.json` file found, recursively, is a workflow. A file that fails to parse is
reported as a warning and skipped — it does not stop the rest from loading.

```sh
merge-pipeline config path        # prints the directory in effect and the rule that chose it
```

See [Config Diagnostics](config-diagnostics.md) for `config doctor` and `config init`.

## The JSON schema

The full schema, `schema/config.json` in the repository:

```json
{{#include ../../schema/config.json}}
```
