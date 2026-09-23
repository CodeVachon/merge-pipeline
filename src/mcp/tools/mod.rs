//! The tool registry: what `tools/list` advertises and `tools/call` dispatches to.
//!
//! Schemas are hand-written `json!` values to keep the dependency footprint small.

use std::path::PathBuf;

use serde_json::{Map, Value, json};

use crate::cli::config_dir;
use crate::config::{LoadResult, load_workflows};
use crate::sync::{StalePolicy, SyncOptions, SyncReport};

mod doctor_config;
mod init_config;
mod inspect_repo;
mod list_workflows;
mod plan_workflow;
mod run_workflow;

pub use doctor_config::DoctorConfig;
pub use init_config::InitConfig;
pub use inspect_repo::InspectRepo;
pub use list_workflows::ListWorkflows;
pub use plan_workflow::PlanWorkflow;
pub use run_workflow::RunWorkflow;

/// A tool call that did not produce a result. Rendered as `isError: true`, never as a protocol
/// error, so the agent sees what went wrong and can adjust its arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolError {
    pub message: String,
    pub data: Option<Value>,
}

impl ToolError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            data: None,
        }
    }

    pub fn with_data(mut self, data: Value) -> Self {
        self.data = Some(data);
        self
    }

    /// The `tools/call` result for this error.
    pub fn into_result(self) -> Value {
        let mut structured = Map::new();
        structured.insert("error".into(), Value::String(self.message.clone()));
        if let Some(Value::Object(extra)) = self.data {
            structured.extend(extra);
        } else if let Some(other) = self.data {
            structured.insert("details".into(), other);
        }
        let structured = Value::Object(structured);
        json!({
            "content": [{ "type": "text", "text": pretty(&structured) }],
            "structuredContent": structured,
            "isError": true
        })
    }
}

impl<E: std::error::Error> From<E> for ToolError {
    fn from(error: E) -> Self {
        ToolError::new(error.to_string())
    }
}

/// Wrap a successful JSON payload as a `tools/call` result.
pub fn success(payload: Value) -> Value {
    json!({
        "content": [{ "type": "text", "text": pretty(&payload) }],
        "structuredContent": payload,
        "isError": false
    })
}

fn pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

/// One MCP tool.
pub trait Tool {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn input_schema(&self) -> Value;
    fn call(&self, arguments: &Value) -> Result<Value, ToolError>;
}

/// Every tool the server offers, in listing order.
pub struct Registry {
    tools: Vec<Box<dyn Tool>>,
}

impl Default for Registry {
    fn default() -> Self {
        Self::with_tools(vec![
            Box::new(ListWorkflows),
            Box::new(InspectRepo),
            Box::new(PlanWorkflow),
            Box::new(RunWorkflow),
            Box::new(DoctorConfig),
            Box::new(InitConfig),
        ])
    }
}

impl Registry {
    pub fn with_tools(tools: Vec<Box<dyn Tool>>) -> Self {
        Self { tools }
    }

    pub fn names(&self) -> Vec<&'static str> {
        self.tools.iter().map(|tool| tool.name()).collect()
    }

    /// The `tools/list` payload.
    pub fn list(&self) -> Value {
        json!({
            "tools": self.tools.iter().map(|tool| json!({
                "name": tool.name(),
                "description": tool.description(),
                "inputSchema": tool.input_schema(),
            })).collect::<Vec<_>>()
        })
    }

    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools
            .iter()
            .find(|tool| tool.name() == name)
            .map(|tool| tool.as_ref())
    }

    /// Run `name` with `arguments`, producing the `tools/call` result, or `None` if unknown.
    pub fn call(&self, name: &str, arguments: &Value) -> Option<Value> {
        let tool = self.get(name)?;
        Some(match tool.call(arguments) {
            Ok(payload) => success(payload),
            Err(error) => error.into_result(),
        })
    }
}

/// Arguments every tool accepts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonArgs {
    pub cwd: PathBuf,
    pub config: Option<PathBuf>,
}

/// The `cwd` and `config` properties, spliced into each tool's schema.
pub fn common_properties() -> Map<String, Value> {
    let mut map = Map::new();
    map.insert(
        "cwd".into(),
        json!({
            "type": "string",
            "description": "Path of the git repository to operate on. Defaults to the server's working directory."
        }),
    );
    map.insert(
        "config".into(),
        json!({
            "type": "string",
            "description": "Directory containing workflow JSON files. When omitted the usual search order applies: $MERGE_PIPELINE_CONFIG, <cwd>/.merge-pipeline, the user config directory, then the directory beside the executable."
        }),
    );
    map
}

/// An object schema with the common properties plus `extra`.
pub fn object_schema(extra: Map<String, Value>, required: &[&str]) -> Value {
    let mut properties = common_properties();
    properties.extend(extra);
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
}

impl CommonArgs {
    pub fn parse(arguments: &Value) -> Result<Self, ToolError> {
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
        let config = optional_string(arguments, "config")?.map(PathBuf::from);
        Ok(Self { cwd, config })
    }

    /// Resolve the config directory and load every workflow in it.
    pub fn load_workflows(&self) -> Result<LoadResult, ToolError> {
        let resolved = config_dir::resolve_from_process(self.config.as_deref(), Some(&self.cwd))?;
        Ok(load_workflows(&resolved.path))
    }
}

/// `arguments[key]` as a string, if present and not null.
pub fn optional_string(arguments: &Value, key: &str) -> Result<Option<String>, ToolError> {
    match arguments.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(other) => Err(ToolError::new(format!(
            "argument `{key}` must be a string, got {other}"
        ))),
    }
}

/// `arguments[key]` as a string, required.
pub fn required_string(arguments: &Value, key: &str) -> Result<String, ToolError> {
    optional_string(arguments, key)?
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| ToolError::new(format!("argument `{key}` is required")))
}

/// `arguments[key]` as a bool, defaulting when absent.
pub fn bool_or(arguments: &Value, key: &str, default: bool) -> Result<bool, ToolError> {
    match arguments.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(Value::Bool(value)) => Ok(*value),
        Some(other) => Err(ToolError::new(format!(
            "argument `{key}` must be a boolean, got {other}"
        ))),
    }
}

/// `arguments[key]` as a string→string map, defaulting to empty.
pub fn string_map(
    arguments: &Value,
    key: &str,
) -> Result<std::collections::HashMap<String, String>, ToolError> {
    match arguments.get(key) {
        None | Some(Value::Null) => Ok(Default::default()),
        Some(Value::Object(map)) => map
            .iter()
            .map(|(k, v)| match v {
                Value::String(s) => Ok((k.clone(), s.clone())),
                other => Err(ToolError::new(format!(
                    "argument `{key}.{k}` must be a string, got {other}"
                ))),
            })
            .collect(),
        Some(other) => Err(ToolError::new(format!(
            "argument `{key}` must be an object, got {other}"
        ))),
    }
}

/// The `sync` argument shared by plan_workflow and run_workflow.
///
/// `false` skips branch sync (the baseline's plain `git fetch`); absent, `true` or an object runs
/// it. Without a human there is nobody to ask, so `stale` is `keep` (report only) unless the agent
/// says `delete`.
pub fn sync_schema() -> Value {
    json!({
        "oneOf": [
            { "type": "boolean" },
            {
                "type": "object",
                "properties": {
                    "stale": {
                        "type": "string",
                        "enum": ["keep", "delete"],
                        "default": "keep",
                        "description": "What to do with local branches matching a pipeline pattern whose upstream is gone from origin: keep (report only) or delete (git branch -D)."
                    },
                    "fetch_new": {
                        "type": "boolean",
                        "default": true,
                        "description": "Create local tracking branches for new origin branches that match a pattern."
                    }
                },
                "additionalProperties": false
            }
        ],
        "description": "Branch sync before the workflow: `git fetch --prune origin`, then report/delete stale local branches and create locals for new remote ones. false skips it; the default is {stale: keep, fetch_new: true}."
    })
}

/// Parse the `sync` argument. `Ok(None)` means skip sync.
pub fn sync_options(arguments: &Value) -> Result<Option<SyncOptions>, ToolError> {
    let default = SyncOptions {
        stale: StalePolicy::Keep,
        fetch_new: true,
    };
    match arguments.get("sync") {
        None | Some(Value::Null) | Some(Value::Bool(true)) => Ok(Some(default)),
        Some(Value::Bool(false)) => Ok(None),
        Some(Value::Object(map)) => {
            let stale = match map.get("stale") {
                None | Some(Value::Null) => StalePolicy::Keep,
                Some(Value::String(value)) => match value.trim() {
                    "" | "keep" => StalePolicy::Keep,
                    "delete" => StalePolicy::Delete,
                    other => {
                        return Err(ToolError::new(format!(
                            "sync.stale must be \"keep\" or \"delete\", got {other:?}"
                        )));
                    }
                },
                Some(other) => {
                    return Err(ToolError::new(format!(
                        "sync.stale must be a string, got {other}"
                    )));
                }
            };
            let fetch_new = match map.get("fetch_new") {
                None | Some(Value::Null) => true,
                Some(Value::Bool(value)) => *value,
                Some(other) => {
                    return Err(ToolError::new(format!(
                        "sync.fetch_new must be a boolean, got {other}"
                    )));
                }
            };
            Ok(Some(SyncOptions { stale, fetch_new }))
        }
        Some(other) => Err(ToolError::new(format!(
            "argument `sync` must be a boolean or an object, got {other}"
        ))),
    }
}

/// Sync report → JSON, shared by plan and run. `None` (sync skipped) becomes `null`.
pub fn sync_report_json(report: Option<&SyncReport>) -> Value {
    match report {
        None => Value::Null,
        Some(report) => json!({
            "stale": report.stale,
            "deleted": report.deleted,
            "kept": report.kept,
            "created": report.created,
        }),
    }
}

/// Workflow config → JSON, shared by list and plan.
pub fn workflow_json(workflow: &crate::config::WorkflowConfig) -> Value {
    json!({
        "name": workflow.name,
        "description": workflow.description,
        "order": workflow.order,
        "disabled": workflow.disabled,
        "pipeline": workflow.pipeline,
        "source": workflow.source.display().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_lists_every_tool_with_object_schemas() {
        let registry = Registry::default();
        assert_eq!(
            registry.names(),
            vec![
                "list_workflows",
                "inspect_repo",
                "plan_workflow",
                "run_workflow",
                "doctor_config",
                "init_config"
            ]
        );
        let listed = registry.list();
        for tool in listed["tools"].as_array().unwrap() {
            assert_eq!(tool["inputSchema"]["type"], json!("object"));
            assert!(tool["inputSchema"]["properties"]["cwd"].is_object());
            assert!(!tool["description"].as_str().unwrap().is_empty());
        }
    }

    #[test]
    fn unknown_tool_is_none_and_errors_render_as_is_error() {
        let registry = Registry::default();
        assert!(registry.call("nope", &json!({})).is_none());

        let rendered = ToolError::new("boom")
            .with_data(json!({"question": {"key": "k"}}))
            .into_result();
        assert_eq!(rendered["isError"], json!(true));
        assert_eq!(rendered["structuredContent"]["error"], json!("boom"));
        assert_eq!(rendered["structuredContent"]["question"]["key"], json!("k"));
        assert!(
            rendered["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("boom")
        );
    }

    #[test]
    fn sync_argument_parses_every_form() {
        let keep = SyncOptions {
            stale: StalePolicy::Keep,
            fetch_new: true,
        };
        assert_eq!(sync_options(&json!({})).unwrap(), Some(keep));
        assert_eq!(sync_options(&json!({"sync": true})).unwrap(), Some(keep));
        assert_eq!(sync_options(&json!({"sync": {}})).unwrap(), Some(keep));
        assert_eq!(sync_options(&json!({"sync": false})).unwrap(), None);
        assert_eq!(
            sync_options(&json!({"sync": {"stale": "delete", "fetch_new": false}})).unwrap(),
            Some(SyncOptions {
                stale: StalePolicy::Delete,
                fetch_new: false,
            })
        );
        assert!(sync_options(&json!({"sync": {"stale": "ask"}})).is_err());
        assert!(sync_options(&json!({"sync": "yes"})).is_err());
        assert_eq!(sync_report_json(None), Value::Null);
    }

    #[test]
    fn common_args_default_cwd_and_reject_wrong_types() {
        let args = CommonArgs::parse(&json!({})).unwrap();
        assert_eq!(args.cwd, std::env::current_dir().unwrap());
        assert!(CommonArgs::parse(&json!({"cwd": 5})).is_err());
        assert!(CommonArgs::parse(&json!({"cwd": "/definitely/not/here"})).is_err());
        assert!(bool_or(&json!({"x": "yes"}), "x", false).is_err());
        assert!(bool_or(&json!({}), "x", true).unwrap());
        assert!(string_map(&json!({"s": {"a": 1}}), "s").is_err());
        assert_eq!(
            string_map(&json!({"s": {"a": "b"}}), "s").unwrap()["a"],
            "b"
        );
    }
}
