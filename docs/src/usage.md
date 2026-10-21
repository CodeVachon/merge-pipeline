# Usage

Run it inside a repository and answer the questions:

```
$ merge-pipeline
```

It prints a title, asks for the working directory (default: current), refuses to continue if the
tree has uncommitted or staged changes, asks which workflow (skipped if only one is enabled),
what to do, and, for `run`, whether to push automatically. Then, for each step, it confirms
"Merge A into B?", checks out and pulls both branches, merges, and pushes B if it has an upstream.

## Flags

Every question can be answered up front:

```
{{#include help/root.txt}}
```

So a fully scripted run is:

```sh
merge-pipeline -c ~/src/app -w "Patch Release to Staging Pipeline" -a run -p -y
```

`--yes` only answers questions that *have* a default. A pattern matching several branches still
stops with an error, because guessing which branch to merge is not acceptable. Unknown flags
exit 2; any failure exits 1 and prints an `Error` block. A merge conflict that could not be
resolved lists the conflicted files and leaves the merge in progress for you to finish by hand.

The old flag spelling `--auto_push` still works; `--auto-push` is preferred.

## Actions

| Action | What it does |
| --- | --- |
| `run` | Merges every step and pushes each target branch (asking per branch unless `--auto-push`) |
| `dry-run` | Merges every step locally. It **does merge**; it only turns off automatic pushing, and still asks "Would you like to push" for each target with an upstream |
| `test` | Fetches, resolves the pipeline to branches, prints the steps. Changes nothing |

The `dry-run` name is kept from the original tool for compatibility. If you want to see what
would happen without touching branches, use `test`.

## package.json version conflicts

When a merge stops on conflicts and every conflict is a `"version"` line in a `package.json`,
merge-pipeline offers to resolve them: conflicts with the same pair of versions are grouped (so a
monorepo with twenty `package.json` files asks once), the higher version is offered first and is
the default, the files are rewritten, staged, and the merge is committed with `--no-verify`.
Anything else conflicting is left for you.

## Conflict command

A workflow can name a command that knows how to fix its conflicts, typically a code generator
for generated files:

```json
{
  "name": "Patch Release to Staging Pipeline",
  "pipeline": ["^Patch-*", "^staging-patch$"],
  "conflictCommand": "resolve-merge-conflicts -a claude -m sonnet",
  "conflictNoVerify": true,
  "conflictFiles": ["^generated/", "\\.lock$"]
}
```

| Field | Default | Meaning |
| --- | --- | --- |
| `conflictCommand` | none | Shell command to offer |
| `conflictNoVerify` | `true` | Commit the finished merge with `--no-verify`; `false` lets git hooks run |
| `conflictFiles` | any conflict | Regular expressions searched (case-sensitively) against conflicted paths; the command is offered only when at least one conflicted path matches |

When a merge stops on conflicts that remain after the `package.json` resolution above,
merge-pipeline asks whether to run the command (`--yes` answers yes). It runs through the shell
in the repository root. Afterwards every conflicted file with no conflict markers left is staged;
if git then reports no conflicts, the merge is committed with `--no-verify` and the run carries
on to the push. If conflicts remain, or the command exits non-zero, the run stops with the merge
left in progress. Answering no stops the run the same way.

The command can be interactive: it inherits the terminal. It suits
[`resolve-merge-conflicts`](https://github.com/CodeVachon/resolve-merge-conflict), which resolves
conflicts with an agent, stages what it really resolved, and never commits; merge-pipeline then
commits and pushes. That tool exits 1 when files are still conflicted, which stops the run. Pass
its own flags (`-y`, `-a`, `-m`, ...) in `conflictCommand`.

## Before a run starts: branch sync

Every action, `test` included, first syncs local branches with origin — a stale local branch no
longer trips up the next run, and a newly-pushed branch is picked up immediately. See
[Branch Sync](branch-sync.md).
