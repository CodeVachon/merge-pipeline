# MCP Server

`merge-pipeline mcp` speaks the [Model Context Protocol](https://modelcontextprotocol.io) over
stdio (JSON-RPC 2.0, one message per line), so Claude Code, Codex, Cursor and similar agents can
drive the same operations as the CLI.

## Wiring it up

```sh
merge-pipeline install                 # writes .mcp.json and creates .merge-pipeline/ in the current directory
merge-pipeline install --scope user    # writes the mcpServers map of ~/.claude.json and the user config directory
merge-pipeline install --no-config     # only wire the server; do not create workflow files
```

Both merge into an existing file and leave other servers alone, and both then run `config init`
for the same scope so a fresh repository has workflows to run (files already present are kept).
Running `install` twice changes nothing. The entry written:

```json
{
  "mcpServers": {
    "merge-pipeline": { "command": "merge-pipeline", "args": ["mcp"] }
  }
}
```

## The protocol

The server implements `initialize`, `ping`, `tools/list` and `tools/call`. Every tool accepts
`cwd` (the repository, default: the server's working directory) and `config` (a workflow
directory; otherwise the [search order](workflow-files.md#where-workflow-files-are-looked-for)
applies). Results carry the JSON both as text and as `structuredContent`; failures come back as
`isError: true` with a message, never as a protocol error, so the agent can adjust and retry.

## Tools

| Tool | Purpose |
| --- | --- |
| [`list_workflows`](#list_workflows) | See every workflow file and any that failed to parse |
| [`inspect_repo`](#inspect_repo) | Check whether the tree is clean and what branches exist |
| [`plan_workflow`](#plan_workflow) | Resolve patterns to branches without touching anything |
| [`run_workflow`](#run_workflow) | Actually merge and push |
| [`doctor_config`](#doctor_config) | Validate the workflow files |
| [`init_config`](#init_config) | Create the default workflow files |

The usual sequence for `run_workflow` is: `plan_workflow` first, supply `selections` for anything
`ambiguous`, then `run_workflow` with `confirm: true`. `run_workflow` never waits for a human —
any question it cannot answer from its arguments is returned as an error naming the question key
and its choices (see [Question keys](#question-keys)).

### `list_workflows`

> List every workflow file merge-pipeline can see for a repository: name, description, order,
> whether it is disabled, its pipeline of branch patterns, and the file it came from. Also
> reports the config directory that was searched and any files that failed to parse.

| Argument | Type | Default | Meaning |
| --- | --- | --- | --- |
| `cwd` | string | server's cwd | Repository to operate on |
| `config` | string | search order | Workflow directory |

### `inspect_repo`

> Describe the git repository merge-pipeline would operate on: whether the working tree is clean
> (`run_workflow` refuses a dirty tree), the current branch, and every local branch with whether
> it tracks a remote. Set `fetch=true` to `git fetch` first.

| Argument | Type | Default | Meaning |
| --- | --- | --- | --- |
| `cwd` / `config` | | | as above |
| `fetch` | boolean | `false` | Run `git fetch` before inspecting |

### `plan_workflow`

> Resolve a workflow's pipeline of regex patterns to the actual local branches and list the merge
> steps (source → target) that `run_workflow` would perform. Never prompts: a pattern matching
> several branches is reported under `ambiguous` with its candidates so you can call again with
> `selections`; a pattern matching nothing is reported under `no_match`. `complete` is true only
> when every pattern resolved. Before mapping it fetches with `--prune` and syncs branches per
> `sync` (default: report stale local branches, create locals for new remote ones), so the plan
> reflects origin's current branches.

| Argument | Type | Default | Meaning |
| --- | --- | --- | --- |
| `workflow` | string | **required** | Exact name of an enabled workflow |
| `selections` | object | `{}` | `{pattern: branch}` for ambiguous patterns |
| `fetch` | boolean | `true` | Fetch from origin first; `false` also skips branch sync |
| `sync` | bool or object | see below | [Branch sync argument](#branch-sync-argument) |
| `cwd` / `config` | | | as above |

Returns `resolutions[]`, `sync`, and either `complete: true` with `branches` and `steps[]`, or
`complete: false` with `ambiguous[]` (pattern + candidates) and `no_match[]`.

### `run_workflow`

> Execute a workflow: for each step check out and pull both branches, merge source into target,
> and push the target when `auto_push` is true. Requires `confirm=true` and a clean working tree.
> Never prompts — resolve ambiguous patterns with `selections` (use `plan_workflow` first) and
> decide `package.json` version conflicts with `version_strategy`. Any question it still cannot
> answer is returned as an error naming the question. A merge that stops on conflicts is aborted
> (unless `abort_on_conflict=false`) and the conflicted paths are reported. Before mapping,
> branches are synced with origin per `sync`.

| Argument | Type | Default | Meaning |
| --- | --- | --- | --- |
| `workflow` | string | **required** | Exact name of an enabled workflow |
| `confirm` | boolean | **required** | Must be `true` — merging and pushing is not reversible from here |
| `selections` | object | `{}` | as in `plan_workflow` |
| `auto_push` | boolean | `false` | Push each target with an upstream after merging; `false` is the CLI's `dry-run` |
| `version_strategy` | `higher\|lower\|fail` | `fail` | How to settle a conflicting `package.json` `"version"` |
| `abort_on_conflict` | boolean | `true` | `git merge --abort` on unresolved conflicts, leaving the repo clean |
| `sync` | bool or object | see below | [Branch sync argument](#branch-sync-argument) |
| `answers` | object | `{}` | Explicit answers by [question key](#question-keys); override every derived answer |
| `cwd` / `config` | | | as above |

Returns per-step outcomes, `sync`, and the event log; on unresolved conflicts, the conflicted
paths.

### `doctor_config`

> Validate the workflow configuration a repository would use: JSON, names, regular expressions,
> ordering, duplicates, and (by default) whether each pattern matches a local branch right now.
> Returns the same report as `merge-pipeline config doctor --json`; `ok` is false when a run
> would fail. Problems are the successful answer, not an error.

| Argument | Type | Default | Meaning |
| --- | --- | --- | --- |
| `check_branches` | boolean | `true` | Also report which local branches each pattern matches (`cwd` must be a git repository) |
| `cwd` / `config` | | | as above |

Returns `{root, rule, workflows[], problems[] ({level, file, message}), ok}`. See
[Config Diagnostics](config-diagnostics.md).

### `init_config`

> Create the workflow directory with the default workflow files (patch, minor, canary and a
> disabled default), so a repository without configuration becomes usable. Project scope writes
> `<cwd>/.merge-pipeline/`; user scope writes the user config directory. Existing files are kept
> unless `force` is true. Same behaviour as `merge-pipeline config init`.

| Argument | Type | Default | Meaning |
| --- | --- | --- | --- |
| `scope` | `project\|user` | `project` | Where to write |
| `force` | boolean | `false` | Overwrite files that already exist |
| `cwd` | string | server's cwd | Repository whose `.merge-pipeline/` is created (project scope) |

Returns `dir` and `files[]` with `path` and `outcome` (`created`, `kept`, or `overwritten`).

## Branch sync argument

`plan_workflow` and `run_workflow` share a `sync` argument controlling the
[branch sync](branch-sync.md) that runs before mapping. There is nobody to ask, so the default is
`{"stale": "keep", "fetch_new": true}`:

| Value | Effect |
| --- | --- |
| omitted, or `{"stale": "keep"}` | Report stale local branches under `sync.stale` and `sync.kept`, but never delete them; still create locals for new remote branches |
| `{"stale": "delete"}` | Delete stale local branches (`git branch -D`) |
| `{"fetch_new": false}` | Stop creating local tracking branches for new remote ones |
| `false` | Skip sync entirely and fetch without pruning, as `--no-sync` does |

The result's `sync` object has `stale`, `deleted`, `kept` and `created`; it is `null` when sync
did not run.

## Question keys

For `run_workflow`'s `answers`: `step:<source>>><target>` (confirm a merge), `push:<branch>`,
`branch:<pattern>` (same as `selections`), `conflict:auto_resolve`,
`conflict:version:<lower>|<higher>`. (`sync:delete:<branch>` exists on the CLI side only — MCP
callers use the `sync` argument instead.)
