//! `init_config`: write the default workflow files, exactly as `merge-pipeline config init` does.

use std::path::PathBuf;

use serde_json::{Map, Value, json};

use super::{Tool, ToolError, bool_or, optional_string};
use crate::cli::{InstallScope, config_init};

pub struct InitConfig;

impl Tool for InitConfig {
    fn name(&self) -> &'static str {
        "init_config"
    }

    fn description(&self) -> &'static str {
        "Create the workflow directory with the default workflow files (patch, minor, canary and \
         a disabled default), so a repository without configuration becomes usable. Project scope \
         writes <cwd>/.merge-pipeline/; user scope writes the user config directory. Existing \
         files are kept unless `force` is true. Same behaviour as `merge-pipeline config init`."
    }

    fn input_schema(&self) -> Value {
        let mut properties = Map::new();
        properties.insert(
            "cwd".into(),
            json!({
                "type": "string",
                "description": "Repository whose .merge-pipeline/ directory is created (project scope). Defaults to the server's working directory."
            }),
        );
        properties.insert(
            "scope".into(),
            json!({
                "type": "string",
                "enum": ["project", "user"],
                "default": "project",
                "description": "project: <cwd>/.merge-pipeline/; user: the user config directory."
            }),
        );
        properties.insert(
            "force".into(),
            json!({
                "type": "boolean",
                "default": false,
                "description": "Overwrite files that already exist."
            }),
        );
        json!({
            "type": "object",
            "properties": properties,
            "required": [],
            "additionalProperties": false
        })
    }

    fn call(&self, arguments: &Value) -> Result<Value, ToolError> {
        let cwd = match optional_string(arguments, "cwd")? {
            Some(cwd) => PathBuf::from(cwd),
            None => std::env::current_dir()
                .map_err(|error| ToolError::new(format!("could not determine cwd: {error}")))?,
        };
        if !cwd.is_dir() {
            return Err(ToolError::new(format!(
                "cwd {} is not a directory",
                cwd.display()
            )));
        }
        let scope = match optional_string(arguments, "scope")?.as_deref() {
            None | Some("project") => InstallScope::Project,
            Some("user") => InstallScope::User,
            Some(other) => {
                return Err(ToolError::new(format!(
                    "argument `scope` must be \"project\" or \"user\", got {other:?}"
                )));
            }
        };
        let force = bool_or(arguments, "force", false)?;

        let dir = config_init::target_dir(scope, &cwd)
            .map_err(|error| ToolError::new(error.to_string()))?;
        let report = config_init::write_defaults(&dir, force)
            .map_err(|error| ToolError::new(format!("{error:#}")))?;
        serde_json::to_value(&report).map_err(ToolError::from)
    }
}
