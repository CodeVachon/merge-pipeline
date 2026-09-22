//! `list_workflows`: every workflow file the config search finds, enabled or not.

use serde_json::{Map, Value, json};

use super::{CommonArgs, Tool, ToolError, object_schema, workflow_json};

pub struct ListWorkflows;

impl Tool for ListWorkflows {
    fn name(&self) -> &'static str {
        "list_workflows"
    }

    fn description(&self) -> &'static str {
        "List every workflow file merge-pipeline can see for a repository: name, description, \
         order, whether it is disabled, its pipeline of branch patterns, and the file it came from. \
         Also reports the config directory that was searched and any files that failed to parse."
    }

    fn input_schema(&self) -> Value {
        object_schema(Map::new(), &[])
    }

    fn call(&self, arguments: &Value) -> Result<Value, ToolError> {
        let common = CommonArgs::parse(arguments)?;
        let loaded = common.load_workflows()?;
        Ok(json!({
            "root": loaded.root.display().to_string(),
            "workflows": loaded.workflows.iter().map(workflow_json).collect::<Vec<_>>(),
            "enabled": loaded.names(),
            "errors": loaded.errors.iter().map(|error| json!({
                "path": error.path.display().to_string(),
                "reason": error.reason,
            })).collect::<Vec<_>>(),
        }))
    }
}
