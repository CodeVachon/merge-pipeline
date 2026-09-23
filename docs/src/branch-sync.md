# Branch Sync

Release branches get merged and deleted on origin, and a local checkout used to trip over the
dead local copy, or fail to see the new one, on the very next run. So before mapping patterns to
branches, every action — `run`, `dry-run`, and `test` — syncs the local branches that the
selected workflow cares about.

## What it does

1. `git fetch --prune origin`, so remote-tracking refs for deleted branches disappear.
2. Every local branch that matches a pipeline pattern **and** whose upstream git reports as
   `[gone]` is *stale*. For each one you are asked:

   > Local branch X no longer exists on origin. Delete it? (default yes; `--yes` answers yes)

   Deleting uses `git branch -D`. If the stale branch is checked out, you are moved to origin's
   default branch (or a local `main`) first. A local branch that never had an upstream is never
   offered for deletion — it was never pushed, so it is not the tool's to clean up.
3. Every branch on origin that matches a pattern and has no local counterpart gets a local
   tracking branch (`git branch --track X origin/X`), without asking, so a new `Patch-*` branch
   is available to the very next step.

```
Syncing branches with origin...
  ✗ deleted Patch-v0.1.1 (gone from origin)
  ✓ created Patch-v0.1.2 tracking origin/Patch-v0.1.2

Step 1: Merge Patch-v0.1.2 into staging-patch
```

## Turning it off

```sh
merge-pipeline --no-sync ...
```

Runs a plain `git fetch` instead, which is what the original Bun tool did. Branches matching no
pattern in the selected workflow are never touched either way, on or off.

## From an agent

The MCP tools `plan_workflow` and `run_workflow` take a `sync` argument with the same effect —
see [MCP Server](mcp-server.md#branch-sync-argument). The default there is `{"stale": "keep",
"fetch_new": true}`: stale branches are reported, not deleted, because there is nobody to ask.
