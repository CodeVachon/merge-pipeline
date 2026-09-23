# Keeping Up to Date

```sh
merge-pipeline upgrade            # install the newest release and switch to it
merge-pipeline upgrade --check    # just report whether one exists
merge-pipeline upgrade 0.2.0      # install a specific version (downgrades warn, then proceed)
merge-pipeline upgrade --keep 3   # how many versions to leave on disk (default 2)
merge-pipeline upgrade --json     # machine-readable output
merge-pipeline uninstall          # show what would be removed
merge-pipeline uninstall --yes    # remove ~/.merge-pipeline and the symlink it owns
```

```
{{#include help/upgrade.txt}}
```

```
{{#include help/uninstall.txt}}
```

## Managing installed versions

Several versions can sit side by side under `~/.merge-pipeline/versions/`, and you switch between
them the way `nvm use` switches Node versions:

```
{{#include help/versions.txt}}
```

| Command | What it does |
| --- | --- |
| `merge-pipeline versions` | List installed versions, newest first. `*` marks the active one; `running` marks the binary that is executing. `--check` asks GitHub for the newest release; `--json` for scripts. |
| `merge-pipeline use <version>` | Make `<version>` the active one. If it is not installed, it is downloaded and verified first, exactly like `upgrade <version>`. |
| `merge-pipeline versions remove <version>...` | Delete versions from disk. The active version and the one running are never removed; each refusal says why, and the command exits 1 if anything was refused or missing. |
| `merge-pipeline versions prune [--keep N]` | Delete all but the newest N (default 2), again never the active or running one. |

```
{{#include help/use.txt}}
```

```
{{#include help/versions_remove.txt}}
```

```
{{#include help/versions_prune.txt}}
```

### No shell restart is ever needed

The command on your `PATH`, `~/.local/bin/merge-pipeline`, is a symlink to
`~/.merge-pipeline/current/bin/merge-pipeline`, and `current` is a symlink to one version
directory. `upgrade` and `use` only move the `current` link, so the very next `merge-pipeline`
command runs the version you chose. Nothing on `PATH` changes, so there is no `hash -r` and no new
terminal.

`upgrade`, `versions` and `use` only work on a managed installation (one made by `install.sh`,
`install.ps1` or a previous `upgrade`). A binary built from source or copied by hand is told how
to install a managed copy instead. Your workflow files are never touched by any of these,
including `uninstall`.

## The daily update offer

At most once a day, an interactive run starts by checking whether a newer release exists and, if
so, asks:

```
? merge-pipeline v0.3.0 is available (you have v0.2.0). Update now? (Y/n)
```

- **Yes** (the default, and what `--yes` answers): the release is downloaded, verified and
  installed exactly as `merge-pipeline upgrade` would, `current` is repointed, and the run you
  started continues on the new version with the same arguments. Nothing to retype.
- **No**: `skipping; run merge-pipeline upgrade any time`, and the run continues on this version.
  You will not be asked again until tomorrow.

Any attempt counts towards the day, including a declined offer or a lookup that could not reach
GitHub (those are silent; the lookup gives up after 1.5 seconds). The offer is only made where it
can act: a managed installation, on a terminal (or with `--yes`). A binary built from source, a
piped run, or any subcommand is never interrupted by it.

Turn it off with `MERGE_PIPELINE_NO_UPDATE_CHECK=1`. It is also off when `CI` is set.
