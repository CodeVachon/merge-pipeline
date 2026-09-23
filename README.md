# merge-pipeline

Merge git branches, one into the next, according to a configured workflow. Point it at a
repository, pick a workflow such as "Patch Release to Staging", and it fetches, checks out each
branch, pulls, merges, resolves `package.json` version conflicts for you, and pushes.

A single self-contained binary with `git` as its only prerequisite. It also serves the same
operations to agents over the Model Context Protocol and updates itself in place.

This is the Rust rewrite of `@codevachon/cli-merge-pipeline`. Coming from that tool? Read
[MIGRATING.md](MIGRATING.md).

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/CodeVachon/merge-pipeline/main/install.sh | sh
```

Windows (PowerShell):

```powershell
irm https://raw.githubusercontent.com/CodeVachon/merge-pipeline/main/install.ps1 | iex
```

The installer verifies the download against the release's `checksums.txt`, puts the binary in
`~/.merge-pipeline/versions/v<X.Y.Z>/bin/`, points `~/.merge-pipeline/current` at it, and links
`~/.local/bin/merge-pipeline`. It never edits your shell profile; it tells you if `~/.local/bin`
is not on your `PATH`. Installer environment variables:

| Variable                        | Purpose                                                |
| ------------------------------- | ------------------------------------------------------ |
| `MERGE_PIPELINE_VERSION`        | install a specific version instead of the latest       |
| `MERGE_PIPELINE_INSTALL_DIR`    | root instead of `~/.merge-pipeline`                    |
| `MERGE_PIPELINE_BIN_DIR`        | symlink directory instead of `~/.local/bin`            |
| `MERGE_PIPELINE_NO_MODIFY_PATH` | skip the PATH advice                                   |
| `MERGE_PIPELINE_DOWNLOAD_BASE`  | fetch assets from a mirror or a local directory server |

Builds are published for darwin-arm64, darwin-x64, linux-x64, linux-arm64 and windows-x64.

## Workflow files

A workflow is one JSON file. `pipeline` lists branch patterns in merge order: the first is merged
into the second, the second into the third, and so on.

```json
{
  "$schema": "https://raw.githubusercontent.com/CodeVachon/merge-pipeline/main/schema/config.json",
  "name": "Patch Release to Staging Pipeline",
  "description": "Deploy a Patch Release Branch to Staging",
  "order": 1,
  "pipeline": ["^Patch-*", "^staging-patch$"]
}
```

| Field         | Required | Meaning                                                                                                  |
| ------------- | -------- | -------------------------------------------------------------------------------------------------------- |
| `name`        | yes      | Shown in the selection list; the value for `--workflow`                                                  |
| `pipeline`    | yes      | Two or more patterns. Each is a case-insensitive regular expression searched against local branch names |
| `description` | no       | Free text                                                                                                |
| `order`       | no       | Lower numbers are listed first. Workflows without an order come after every ordered one, by file name    |
| `disabled`    | no       | `true` loads the file but never offers it                                                                |

How a pattern becomes a branch: every local branch that matches is a candidate, minus branches
already chosen earlier in the same pipeline. One candidate is used as is. Several candidates
produce a prompt. None is an error. So the canary example below lists `^Release-*` twice on
purpose: the second occurrence has to pick a different release branch than the first.

Four ready-made workflows live in [`examples/config/`](examples/config/): `patch.json`,
`minor.json`, `canary.json`, and a disabled `default.json`. Copy the ones you want into a
workflow directory.

### Where workflow files are looked for

The first of these that exists as a directory wins:

1. `--config <PATH>` (`-f`)
2. `$MERGE_PIPELINE_CONFIG`
3. `<cwd>/.merge-pipeline/`
4. the user config directory: `~/.config/merge-pipeline/` on Linux,
   `~/Library/Application Support/merge-pipeline/` on macOS
5. `<directory of the binary>/config` (the layout of the old Bun tool)

Inside the chosen directory, a `config/` sub-directory is used if present; otherwise the directory
itself. Every `*.json` file found, recursively, is a workflow. A file that fails to parse is
reported as a warning and skipped.

```sh
merge-pipeline config path        # prints the directory in effect and the rule that chose it
merge-pipeline config doctor      # validates every workflow file; exit 1 if a run would fail
merge-pipeline config init        # creates <cwd>/.merge-pipeline/ with the default workflows
```

`config doctor` checks each file's JSON, name, `pipeline` length and regular expressions, flags
duplicate names, unknown keys and mixed `order`, and with `-c <repo>` reports which local branch
each pattern matches right now (none is a warning, several is a note that a run will ask).
Warnings keep exit 0; errors exit 1. `--json` emits `{root, rule, workflows, problems, ok}`, the
same shape the MCP `doctor_config` tool returns.

`config init` writes `patch.json`, `minor.json`, `canary.json` and a disabled `default.json`,
identical to the files in [`examples/config/`](examples/config/). Existing files are kept unless
you pass `--force`; `--scope user` targets the user config directory instead. `install` runs it
for you (see below).

## Usage

Run it inside a repository and answer the questions:

```
$ merge-pipeline
```

It prints a title, asks for the working directory (default: current), refuses to continue if the
tree has uncommitted or staged changes, asks which workflow (skipped if only one is enabled),
what to do, and, for `run`, whether to push automatically. Then, for each step, it confirms
"Merge A into B?", checks out and pulls both branches, merges, and pushes B if it has an upstream.

Every question can be answered up front:

| Flag                              | Effect                                                                 |
| --------------------------------- | ---------------------------------------------------------------------- |
| `-c, --cwd <DIR>`                 | Working directory of the repository                                    |
| `-w, --workflow <NAME>`           | Workflow name, exactly as in the file                                  |
| `-a, --action <ACTION>`           | `run`, `dry-run` or `test`                                             |
| `-p, --auto-push`                 | Push each target without asking (`--auto_push` also accepted)          |
| `-f, --config <PATH>`             | Workflow directory                                                     |
| `-y, --yes`                       | Answer every confirmation with its default (merge each step, push, delete stale branches) |
| `--no-sync`                       | Skip the branch sync below and fetch without pruning, as the original tool did |

So a fully scripted run is:

```sh
merge-pipeline -c ~/src/app -w "Patch Release to Staging Pipeline" -a run -p -y
```

`--yes` only answers questions that have a default. A pattern matching several branches still
stops with an error, because guessing which branch to merge is not acceptable. Unknown flags
exit 2; any failure exits 1 and prints an `Error` block. A merge conflict that could not be
resolved lists the conflicted files and leaves the merge in progress for you to finish by hand.

### Actions

| Action    | What it does                                                                                    |
| --------- | ----------------------------------------------------------------------------------------------- |
| `run`     | Merges every step and pushes each target branch (asking per branch unless `--auto-push`)        |
| `dry-run` | Merges every step locally. It **does merge**; it only turns off automatic pushing, and still asks "Would you like to push" for each target with an upstream |
| `test`    | Fetches, resolves the pipeline to branches, prints the steps. Changes nothing                   |

The `dry-run` name is kept from the original tool for compatibility. If you want to see what
would happen without touching branches, use `test`.

### Branch sync

Release branches get merged and deleted on origin, and the next run used to trip over the dead
local copy or fail to see the new one. So before mapping patterns to branches, every action
(including `test`) syncs the local branches that the workflow cares about:

1. `git fetch --prune origin`, so remote-tracking refs for deleted branches disappear.
2. Every local branch that matches a pipeline pattern **and** whose upstream git reports as
   `[gone]` is *stale*. For each one you are asked "Local branch X no longer exists on origin.
   Delete it?" (default yes; `--yes` answers yes). Deleting uses `git branch -D`, and if the
   stale branch is checked out you are moved to origin's default branch (or a local `main`)
   first. A local branch that never had an upstream is never offered for deletion: it was never
   pushed, so it is not the tool's to clean up.
3. Every branch on origin that matches a pattern and has no local counterpart gets a local
   tracking branch (`git branch --track X origin/X`), without asking, so a new `Patch-*` branch
   is available to the very next step.

```
Syncing branches with origin...
  ✗ deleted Patch-v0.1.1 (gone from origin)
  ✓ created Patch-v0.1.2 tracking origin/Patch-v0.1.2

Step 1: Merge Patch-v0.1.2 into staging-patch
```

`--no-sync` turns all of this off and runs a plain `git fetch`, which is what the original tool
did. Branches matching no pattern in the selected workflow are never touched either way.

### package.json version conflicts

When a merge stops on conflicts and every conflict is a `"version"` line in a `package.json`,
merge-pipeline offers to resolve them: conflicts with the same pair of versions are grouped (so a
monorepo with twenty `package.json` files asks once), the higher version is offered first and is
the default, the files are rewritten, staged, and the merge is committed with `--no-verify`.
Anything else conflicting is left for you.

## Agents: MCP server

`merge-pipeline mcp` speaks the Model Context Protocol over stdio (JSON-RPC 2.0, one message per
line), so Claude Code, Codex, Cursor and similar agents can drive the same operations as the CLI.
Wire it up with:

```sh
merge-pipeline install                 # writes .mcp.json and creates .merge-pipeline/ in the current directory
merge-pipeline install --scope user    # writes the mcpServers map of ~/.claude.json and the user config directory
merge-pipeline install --no-config     # only wire the server; do not create workflow files
```

Both merge into an existing file and leave other servers alone, and both then run
`config init` for the same scope so a fresh repository has workflows to run (files already
present are kept). Running `install` twice changes nothing. The entry written is:

```json
{
  "mcpServers": {
    "merge-pipeline": { "command": "merge-pipeline", "args": ["mcp"] }
  }
}
```

The server implements `initialize`, `ping`, `tools/list` and `tools/call`. Every tool accepts
`cwd` (the repository, default: the server's working directory) and `config` (a workflow
directory; otherwise the search order above applies). Results carry the JSON both as text and as
`structuredContent`; failures come back as `isError: true` with a message, never as a protocol
error, so the agent can adjust and retry.

| Tool             | Arguments                                                                                                                              | Returns                                                                                                                                   |
| ---------------- | -------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| `list_workflows` | —                                                                                                                                      | `root`, every `workflows[]` entry (name, description, order, disabled, pipeline, source), the `enabled` names, and `errors[]` for files that failed to parse |
| `inspect_repo`   | `fetch` (default false)                                                                                                                | `clean`, `current_branch`, `branches[]` with `has_upstream`                                                                              |
| `plan_workflow`  | `workflow` (required), `selections` `{pattern: branch}`, `fetch` (default true), `sync` (see below)                                     | `resolutions[]`, `sync`, and either `complete: true` with `branches` and `steps[]`, or `complete: false` with `ambiguous[]` (pattern + candidates) and `no_match[]` |
| `run_workflow`   | `workflow` and `confirm: true` (both required), `selections`, `auto_push` (default false), `version_strategy` `higher\|lower\|fail` (default fail), `abort_on_conflict` (default true), `sync` (see below), `answers` `{question key: value}` | per-step outcomes, `sync`, and the event log; on unresolved conflicts, the conflicted paths                                               |
| `doctor_config`  | `check_branches` (default true)                                                                                                        | the `config doctor --json` report: `root`, `rule`, `workflows[]`, `problems[]` (`level`, `file`, `message`) and `ok`. Problems are a normal result, not `isError` |
| `init_config`    | `scope` `project\|user` (default project), `force` (default false); `cwd` only                                                          | `dir` and `files[]` with `path` and `outcome` `created\|kept\|overwritten`, exactly as `config init` behaves                              |

`run_workflow` never waits for a human. It refuses a dirty tree, and any question it cannot answer
from its arguments is returned as an error naming the question key and its choices. The usual
sequence is `plan_workflow`, supply `selections` for anything `ambiguous`, then `run_workflow`.
With the default `abort_on_conflict`, a step that stops on conflicts is `git merge --abort`ed so
the repository is left clean; pass `false` to leave the merge in progress for a person to finish.

`sync` controls the branch sync described under Usage. There is nobody to ask, so the default is
`{"stale": "keep", "fetch_new": true}`: stale local branches are reported under `sync.stale` and
`sync.kept` but never deleted, and new remote branches get local tracking branches. Pass
`{"stale": "delete"}` to remove them, `{"fetch_new": false}` to stop creating locals, or
`false` to skip sync and fetch without pruning. The result's `sync` object has `stale`,
`deleted`, `kept` and `created`; it is `null` when sync did not run.

Question keys, for `answers`: `step:<source>>><target>` (confirm a merge), `push:<branch>`,
`branch:<pattern>` (same as `selections`), `conflict:auto_resolve`,
`conflict:version:<lower>|<higher>`, and `sync:delete:<branch>` (CLI only; MCP uses `sync`).

## Keeping it up to date

```sh
merge-pipeline upgrade            # install the newest release and switch to it
merge-pipeline upgrade --check    # just report whether one exists
merge-pipeline upgrade 0.2.0      # install a specific version (downgrades warn, then proceed)
merge-pipeline upgrade --keep 3   # how many versions to leave on disk (default 2)
merge-pipeline upgrade --json     # machine-readable output
merge-pipeline uninstall          # show what would be removed
merge-pipeline uninstall --yes    # remove ~/.merge-pipeline and the symlink it owns
```

### Managing installed versions

Several versions can sit side by side under `~/.merge-pipeline/versions/`, and you switch
between them the way `nvm use` switches Node versions:

| Command | What it does |
|---|---|
| `merge-pipeline versions` | List installed versions, newest first. `*` marks the active one; `running` marks the binary that is executing. `--check` asks GitHub for the newest release; `--json` for scripts. |
| `merge-pipeline use <version>` | Make `<version>` the active one. If it is not installed, it is downloaded and verified first, exactly like `upgrade <version>`. |
| `merge-pipeline versions remove <version>...` | Delete versions from disk. The active version and the one running are never removed; each refusal says why, and the command exits 1 if anything was refused or missing. |
| `merge-pipeline versions prune [--keep N]` | Delete all but the newest N (default 2), again never the active or running one. |

**No shell restart is ever needed.** The command on your PATH, `~/.local/bin/merge-pipeline`, is a
symlink to `~/.merge-pipeline/current/bin/merge-pipeline`, and `current` is a symlink to one
version directory. `upgrade` and `use` only move the `current` link, so the very next
`merge-pipeline` command runs the version you chose. Nothing on PATH changes, so there is no
`hash -r` and no new terminal.

`upgrade`, `versions` and `use` only work on a managed installation (one made by `install.sh`,
`install.ps1` or a previous `upgrade`). A binary built from source or copied by hand is told how
to install a managed copy instead. Your workflow files are never touched by any of these,
including `uninstall`.

At the end of an interactive run on a terminal, once a day, a dimmed line mentions a newer
release if one is known. Set `MERGE_PIPELINE_NO_UPDATE_CHECK=1` to turn that off; it is also
off when `CI` is set.

Shell completion:

```sh
merge-pipeline completion zsh > ~/.zfunc/_merge-pipeline     # bash, zsh, fish, elvish, powershell
```

The installer also writes completion scripts to `~/.merge-pipeline/completions/`.

## Development

```sh
cargo build            # or: cargo run -- --help
cargo test             # unit + integration tests; no network, fixtures are built in temp dirs
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo build --release  # target/release/merge-pipeline, a few MB
```

Integration tests create their own git repositories with a local bare `origin`
(`tests/common/mod.rs`), so there is no bootstrap script and nothing to clone. Release builds are
produced by `.github/workflows/release.yml` on a `v*` tag, verified with the installers against
the freshly built assets, and only then published.

## License

MIT. See [LICENSE](LICENSE).
