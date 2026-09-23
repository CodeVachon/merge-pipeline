# Migrating from cli-merge-pipeline

`merge-pipeline` is a from-scratch Rust rewrite of the Bun/TypeScript tool
`@codevachon/cli-merge-pipeline`. The workflow files, the interactive flow, and the three actions
are the same. What changed is where things live, one flag's spelling, and one safety check.

## 1. Install the binary

The old tool was built locally with `bun run compile` and kept beside its `config/` directory.
The new one is a single binary installed by a script and updated in place:

```sh
curl -fsSL https://raw.githubusercontent.com/CodeVachon/merge-pipeline/main/install.sh | sh
```

It lands in `~/.merge-pipeline/versions/v<X.Y.Z>/bin/merge-pipeline` with a symlink at
`~/.local/bin/merge-pipeline`. Update later with `merge-pipeline upgrade`. Remove it with
`merge-pipeline uninstall`. You can delete the old `bin/cli-merge-pipeline` build.

## 2. Move your workflow files

Workflow JSON files are no longer looked for beside the executable first. If your old files were
the stock patch/minor/canary set, you do not need to copy anything: run
`merge-pipeline config init` in the repository (or `merge-pipeline install`, which does it as part
of wiring the MCP server) and the same four files are written to `<repo>/.merge-pipeline/`.
Otherwise copy the `config/*.json` files from your old checkout to one of these locations:

| Location                                              | When to use it                          |
| ----------------------------------------------------- | --------------------------------------- |
| `<repo>/.merge-pipeline/`                             | workflows specific to one repository    |
| `~/.config/merge-pipeline/` (Linux)                   | workflows you use everywhere            |
| `~/Library/Application Support/merge-pipeline/` (mac) | same, on macOS                          |

The full search order is `--config`, then `$MERGE_PIPELINE_CONFIG`, then `<cwd>/.merge-pipeline/`,
then the user config directory above, then `<directory of the binary>/config` (the old layout,
kept so an unmoved directory still works). The first directory that exists wins. Run
`merge-pipeline config path` to see which one was chosen and why, and `merge-pipeline config doctor`
to check every file before the first run.

The file format is unchanged. Optionally add a `$schema` line so your editor validates the file:

```json
{
  "$schema": "https://raw.githubusercontent.com/CodeVachon/merge-pipeline/main/schema/config.json",
  "name": "Patch Release to Staging Pipeline",
  "description": "Deploy a Patch Release Branch to Staging",
  "order": 1,
  "pipeline": ["^Patch-*", "^staging-patch$"]
}
```

A `pipeline` with fewer than two entries is now reported as a load error (the schema always
required two; the old loader silently accepted one).

## 3. Update any scripts that pass flags

| Old                    | New                                       |
| ---------------------- | ----------------------------------------- |
| `--auto_push` / `-p`   | `--auto-push` / `-p` (`--auto_push` still accepted) |
| `--cwd` / `-c`         | unchanged                                 |
| `--workflow` / `-w`    | unchanged                                 |
| `--config` / `-f`      | unchanged                                 |
| `--action` / `-a`      | unchanged: `run`, `dry-run`, `test`       |
| (none)                 | `--yes` / `-y`: answer every confirmation with its default |

`--yes` is new. The old tool always asked "Merge X into Y?" and "Would you like to push Y?" per
step, which made it impossible to script. With `--yes` those confirmations take their default
(yes), and any question that has no default, such as picking between several branches that match
one pattern, still stops with an error rather than guessing.

Unknown flags are still rejected (exit code 2).

## 4. Behaviour changes to know about

- **Dirty-tree check catches staged changes.** The old tool ran `git diff --stat`, which ignores
  changes that are staged but not committed. The new tool runs
  `git status --porcelain --untracked-files=no`, so a staged change is also "Git is in a Dirty
  State". Untracked files are still ignored, as before.
- **Regex semantics are unchanged.** Each `pipeline` entry is a case-insensitive regular
  expression *searched* (not anchored) against local branch names, excluding branches already
  chosen earlier in the pipeline. Note that `^Patch-*` means "Patch" followed by zero or more
  hyphens, which matches `Patch-v0.1.1` and also a branch called `Patchwork`. If you want "Patch-
  followed by anything", write `^Patch-.*`. Existing configs keep working as they did.
- **`dry-run` is unchanged, and still merges.** It performs every merge locally and only stops
  *automatic* pushing; it will still ask "Would you like to push <branch>" for each target that
  has an upstream. Use `test` to see the resolved steps without touching anything.
- **Branches are synced with origin before the workflow runs.** The old tool ran a plain
  `git fetch`. The new one runs `git fetch --prune origin`, then, for the branches that match the
  selected workflow's patterns, offers to delete local branches whose upstream is gone (default
  yes, so `--yes` deletes them) and creates local tracking branches for new remote ones. Local
  branches that were never pushed are never touched. Pass `--no-sync` for the old behaviour.
  See "Branch sync" in the README.
- **Workflow-file load errors are warnings.** A file that is not valid JSON is reported on stderr
  and skipped; the rest still load. Zero enabled workflows is still a hard error.

## 5. New things you did not have before

- `merge-pipeline mcp` serves the tool over the Model Context Protocol for agents, and
  `merge-pipeline install` writes the wiring into `.mcp.json`. See the README.
- `merge-pipeline upgrade` / `uninstall` manage the installation.
- `merge-pipeline completion <shell>` prints a completion script.
- `merge-pipeline config path` shows which workflow directory is in effect.

## 6. Things that went away

- `bootstrap.sh` (it cloned a real GitHub repository for manual testing). The Rust test suite
  builds its own git fixtures under a temporary directory.
- The `data/` directory, `fallow`, `vitest`, and every Node/Bun dependency.
