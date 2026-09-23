//! `doctor_config`: the same checks as `merge-pipeline config doctor --json`, for agents.

use serde_json::{Map, Value, json};

use super::{CommonArgs, Tool, ToolError, bool_or, object_schema};
use crate::cli::{config_dir, doctor};
use crate::git::Git;

pub struct DoctorConfig;

impl Tool for DoctorConfig {
    fn name(&self) -> &'static str {
        "doctor_config"
    }

    fn description(&self) -> &'static str {
        "Validate the workflow configuration a repository would use: JSON, names, regular \
         expressions, ordering, duplicates, and (by default) whether each pattern matches a local \
         branch right now. Returns the same report as `merge-pipeline config doctor --json`; `ok` \
         is false when a run would fail. Problems are the successful answer, not an error."
    }

    fn input_schema(&self) -> Value {
        let mut extra = Map::new();
        extra.insert(
            "check_branches".into(),
            json!({
                "type": "boolean",
                "default": true,
                "description": "Also report which local branches each pipeline pattern matches (cwd must be a git repository)."
            }),
        );
        object_schema(extra, &[])
    }

    fn call(&self, arguments: &Value) -> Result<Value, ToolError> {
        let common = CommonArgs::parse(arguments)?;
        let check_branches = bool_or(arguments, "check_branches", true)?;

        let resolved =
            config_dir::resolve_from_process(common.config.as_deref(), Some(&common.cwd))?;

        let branches = if check_branches && common.cwd.join(".git").exists() {
            Some(Git::new(common.cwd.clone()).branch_list()?)
        } else {
            None
        };

        let report = doctor::diagnose(
            &resolved.path,
            resolved.rule.describe(),
            branches.as_deref(),
        );
        serde_json::to_value(&report).map_err(ToolError::from)
    }
}
