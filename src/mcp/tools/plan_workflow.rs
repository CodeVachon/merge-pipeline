//! `plan_workflow`: resolve a workflow's patterns to concrete branches and steps, reporting
//! anything ambiguous or unmatched instead of prompting.

use serde_json::{Map, Value, json};

use super::{CommonArgs, Tool, ToolError, bool_or, object_schema, required_string, string_map};
use crate::git::Git;
use crate::pipeline::{Resolution, make_steps, map_pipeline_report, resolved_branches};

pub struct PlanWorkflow;

/// The `selections` schema, shared with `run_workflow`.
pub fn selections_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": { "type": "string" },
        "description": "Pattern → branch choices for pipeline entries that match more than one branch, e.g. {\"^Release-*\": \"Release-0.2.0\"}. Take the keys from a previous plan_workflow result's `ambiguous` list."
    })
}

/// `resolutions` and the derived `ambiguous` / `no_match` / `branches` / `steps` fields.
pub fn plan_json(report: &[Resolution]) -> Value {
    let resolutions: Vec<Value> = report
        .iter()
        .map(|resolution| match resolution {
            Resolution::Resolved { pattern, branch } => {
                json!({ "pattern": pattern, "status": "resolved", "branch": branch })
            }
            Resolution::Ambiguous {
                pattern,
                candidates,
            } => json!({ "pattern": pattern, "status": "ambiguous", "candidates": candidates }),
            Resolution::NoMatch { pattern } => {
                json!({ "pattern": pattern, "status": "no_match" })
            }
        })
        .collect();

    let ambiguous: Vec<Value> = report
        .iter()
        .filter_map(|resolution| match resolution {
            Resolution::Ambiguous {
                pattern,
                candidates,
            } => Some(json!({ "pattern": pattern, "candidates": candidates })),
            _ => None,
        })
        .collect();
    let no_match: Vec<&str> = report
        .iter()
        .filter_map(|resolution| match resolution {
            Resolution::NoMatch { pattern } => Some(pattern.as_str()),
            _ => None,
        })
        .collect();

    let mut out = Map::new();
    out.insert("resolutions".into(), Value::Array(resolutions));
    match resolved_branches(report) {
        Some(branches) => {
            let steps: Vec<Value> = make_steps(&branches)
                .into_iter()
                .map(|step| json!({ "source": step.source, "target": step.target }))
                .collect();
            out.insert("complete".into(), json!(true));
            out.insert("branches".into(), json!(branches));
            out.insert("steps".into(), Value::Array(steps));
        }
        None => {
            out.insert("complete".into(), json!(false));
            out.insert("ambiguous".into(), Value::Array(ambiguous));
            out.insert("no_match".into(), json!(no_match));
        }
    }
    Value::Object(out)
}

impl Tool for PlanWorkflow {
    fn name(&self) -> &'static str {
        "plan_workflow"
    }

    fn description(&self) -> &'static str {
        "Resolve a workflow's pipeline of regex patterns to the actual local branches and list the \
         merge steps (source → target) that run_workflow would perform. Never prompts: a pattern \
         matching several branches is reported under `ambiguous` with its candidates so you can \
         call again with `selections`; a pattern matching nothing is reported under `no_match`. \
         `complete` is true only when every pattern resolved."
    }

    fn input_schema(&self) -> Value {
        let mut extra = Map::new();
        extra.insert(
            "workflow".into(),
            json!({ "type": "string", "description": "Exact name of an enabled workflow (see list_workflows)." }),
        );
        extra.insert("selections".into(), selections_schema());
        extra.insert(
            "fetch".into(),
            json!({
                "type": "boolean",
                "default": true,
                "description": "Run `git fetch` first, as the CLI does. Set false to plan offline."
            }),
        );
        object_schema(extra, &["workflow"])
    }

    fn call(&self, arguments: &Value) -> Result<Value, ToolError> {
        let common = CommonArgs::parse(arguments)?;
        let name = required_string(arguments, "workflow")?;
        let selections = string_map(arguments, "selections")?;
        let fetch = bool_or(arguments, "fetch", true)?;

        let loaded = common.load_workflows()?;
        loaded.require_enabled()?;
        let workflow = loaded.find(&name)?;

        let mut git = Git::new(&common.cwd);
        if fetch {
            git.fetch()?;
        }
        let branches = git.branch_list()?;
        let report = map_pipeline_report(&workflow.pipeline, &branches, &selections)?;

        let mut payload = plan_json(&report);
        let object = payload
            .as_object_mut()
            .expect("plan_json returns an object");
        object.insert("workflow".into(), json!(workflow.name));
        object.insert("pipeline".into(), json!(workflow.pipeline));
        Ok(payload)
    }
}
