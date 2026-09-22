//! `inspect_repo`: is the tree clean, what branch is checked out, which branches exist and which
//! of them track a remote. Read-only; `fetch` is opt-in because it touches the network.

use serde_json::{Map, Value, json};

use super::{CommonArgs, Tool, ToolError, bool_or, object_schema};
use crate::git::Git;

pub struct InspectRepo;

/// Whether `branch` has an upstream, without checking it out.
fn branch_tracks_remote(git: &mut Git, branch: &str) -> Result<bool, ToolError> {
    match git.call(&[
        "rev-parse",
        "--abbrev-ref",
        "--symbolic-full-name",
        &format!("{branch}@{{u}}"),
    ]) {
        Ok(output) => Ok(!output.is_empty()),
        Err(error) if error.is_missing_upstream() => Ok(false),
        Err(error) => Err(error.into()),
    }
}

impl Tool for InspectRepo {
    fn name(&self) -> &'static str {
        "inspect_repo"
    }

    fn description(&self) -> &'static str {
        "Describe the git repository merge-pipeline would operate on: whether the working tree is \
         clean (run_workflow refuses a dirty tree), the current branch, and every local branch with \
         whether it tracks a remote. Set fetch=true to `git fetch` first."
    }

    fn input_schema(&self) -> Value {
        let mut extra = Map::new();
        extra.insert(
            "fetch".into(),
            json!({
                "type": "boolean",
                "default": false,
                "description": "Run `git fetch` before inspecting, so remote-tracking state is current."
            }),
        );
        object_schema(extra, &[])
    }

    fn call(&self, arguments: &Value) -> Result<Value, ToolError> {
        let common = CommonArgs::parse(arguments)?;
        let fetch = bool_or(arguments, "fetch", false)?;

        let mut git = Git::new(&common.cwd);
        if fetch {
            git.fetch()?;
        }
        let clean = !git.is_dirty()?;
        let current = git.current_branch()?;
        let mut branches = Vec::new();
        for name in git.branch_list()? {
            let has_upstream = branch_tracks_remote(&mut git, &name)?;
            branches.push(json!({ "name": name, "has_upstream": has_upstream }));
        }

        Ok(json!({
            "cwd": common.cwd.display().to_string(),
            "clean": clean,
            "current_branch": current,
            "fetched": fetch,
            "branches": branches,
        }))
    }
}
