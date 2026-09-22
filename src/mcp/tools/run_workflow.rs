//! `run_workflow`: execute a workflow with every decision supplied up front.
//!
//! The runner asks questions through a [`ScriptedPrompter`] built from the arguments. Anything it
//! cannot answer aborts with an error naming the question and its choices, so an agent can supply
//! the missing answer and call again. Nothing here ever waits for a human.

use serde_json::{Map, Value, json};

use super::plan_workflow::selections_schema;
use super::{
    CommonArgs, Tool, ToolError, bool_or, object_schema, optional_string, required_string,
    string_map,
};
use crate::conflicts::AUTO_RESOLVE_KEY;
use crate::git::Git;
use crate::pipeline::{Step, branch_question_key};
use crate::prompt::{Answer, PromptError, ScriptedPrompter};
use crate::runner::{
    Action, CollectingSink, Event, PushSkipReason, RunError, RunReport, Settings, run_workflow,
};

pub struct RunWorkflow;

/// How package.json `"version"` conflicts are decided without a human.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionStrategy {
    /// Take the higher version (the CLI's default choice).
    Higher,
    /// Take the lower version.
    Lower,
    /// Do not auto-resolve; the step fails and reports the conflicted paths.
    Fail,
}

impl VersionStrategy {
    pub const NAMES: [&'static str; 3] = ["higher", "lower", "fail"];

    fn parse(value: Option<String>) -> Result<Self, ToolError> {
        match value.as_deref().map(str::trim) {
            None | Some("") | Some("fail") => Ok(VersionStrategy::Fail),
            Some("higher") => Ok(VersionStrategy::Higher),
            Some("lower") => Ok(VersionStrategy::Lower),
            Some(other) => Err(ToolError::new(format!(
                "version_strategy must be one of {:?}, got {other:?}",
                Self::NAMES
            ))),
        }
    }
}

/// Build the prompter from the tool arguments.
fn scripted_prompter(
    auto_push: bool,
    selections: &std::collections::HashMap<String, String>,
    strategy: VersionStrategy,
    answers: &Map<String, Value>,
) -> Result<ScriptedPrompter, ToolError> {
    let mut prompter = ScriptedPrompter::new()
        .answer_prefix("step:", true)
        .answer_prefix("push:", auto_push)
        .answer(AUTO_RESOLVE_KEY, strategy != VersionStrategy::Fail);

    match strategy {
        VersionStrategy::Higher => {
            prompter = prompter.answer_prefix("conflict:version:", Answer::Index(0));
        }
        VersionStrategy::Lower => {
            prompter = prompter.answer_prefix("conflict:version:", Answer::Index(1));
        }
        VersionStrategy::Fail => {}
    }

    for (pattern, branch) in selections {
        prompter = prompter.answer(branch_question_key(pattern), branch.as_str());
    }

    // Explicit answers win over everything derived above.
    for (key, value) in answers {
        let answer = match value {
            Value::Bool(b) => Answer::Bool(*b),
            Value::String(s) => Answer::Text(s.clone()),
            Value::Number(n) => match n.as_u64() {
                Some(index) => Answer::Index(index as usize),
                None => {
                    return Err(ToolError::new(format!(
                        "answers.{key} must be a bool, string or non-negative integer"
                    )));
                }
            },
            other => {
                return Err(ToolError::new(format!(
                    "answers.{key} must be a bool, string or integer, got {other}"
                )));
            }
        };
        prompter = prompter.answer(key.clone(), answer);
    }
    Ok(prompter)
}

fn step_json(step: &Step) -> Value {
    json!({ "source": step.source, "target": step.target })
}

/// Events → JSON, for the agent's benefit.
pub fn event_json(event: &Event) -> Value {
    match event {
        Event::Pipeline { patterns } => json!({ "type": "pipeline", "patterns": patterns }),
        Event::Fetched => json!({ "type": "fetched" }),
        Event::BranchesMapped { branches } => {
            json!({ "type": "branches_mapped", "branches": branches })
        }
        Event::StepsPlanned { steps } => json!({
            "type": "steps_planned",
            "steps": steps.iter().map(step_json).collect::<Vec<_>>()
        }),
        Event::StepStarted { index, total, step } => json!({
            "type": "step_started", "index": index + 1, "total": total, "step": step_json(step)
        }),
        Event::CheckedOut { branch } => json!({ "type": "checked_out", "branch": branch }),
        Event::Pulled { branch } => json!({ "type": "pulled", "branch": branch }),
        Event::Merged { step } => json!({ "type": "merged", "step": step_json(step) }),
        Event::ConflictsDetected { step, paths } => json!({
            "type": "conflicts_detected", "step": step_json(step), "paths": paths
        }),
        Event::ConflictsResolved {
            step,
            rewritten,
            committed,
        } => json!({
            "type": "conflicts_resolved", "step": step_json(step),
            "rewritten": rewritten, "committed": committed
        }),
        Event::Pushed { branch } => json!({ "type": "pushed", "branch": branch }),
        Event::PushSkipped { branch, reason } => json!({
            "type": "push_skipped", "branch": branch,
            "reason": match reason {
                PushSkipReason::NoUpstream => "no_upstream",
                PushSkipReason::Declined => "declined",
            }
        }),
    }
}

fn report_json(report: &RunReport) -> Value {
    json!({
        "branches": report.branches,
        "steps": report.steps.iter().map(|outcome| json!({
            "source": outcome.step.source,
            "target": outcome.step.target,
            "merged": outcome.merged,
            "pushed": outcome.pushed,
            "conflicts_resolved": outcome.conflicts_resolved,
        })).collect::<Vec<_>>(),
    })
}

/// Turn a runner failure into a tool error with enough structure to act on.
fn run_error(
    error: RunError,
    events: &[Event],
    git: &mut Git,
    abort_on_conflict: bool,
) -> ToolError {
    let events_json: Vec<Value> = events.iter().map(event_json).collect();
    let mut data = Map::new();
    data.insert("events".into(), Value::Array(events_json));

    match error {
        RunError::Prompt(PromptError::Unanswered {
            key,
            message,
            choices,
        })
        | RunError::Pipeline(crate::pipeline::PipelineError::Prompt(PromptError::Unanswered {
            key,
            message,
            choices,
        })) => {
            data.insert(
                "question".into(),
                json!({ "key": key, "message": message, "choices": choices }),
            );
            ToolError::new(format!(
                "run_workflow needs an answer for `{key}` ({message}). Supply it via `selections` (for branch:* keys) or `answers`."
            ))
            .with_data(Value::Object(data))
        }
        RunError::Merge(merge) => {
            let mut aborted = false;
            if abort_on_conflict {
                aborted = git.call(&["merge", "--abort"]).is_ok();
            }
            data.insert("step".into(), step_json(&merge.step));
            data.insert("conflicted_paths".into(), json!(merge.files));
            data.insert("merge_aborted".into(), json!(aborted));
            data.insert(
                "merge_in_progress".into(),
                json!(!aborted && !merge.files.is_empty()),
            );
            if let Some(cause) = &merge.cause {
                data.insert("cause".into(), json!(cause));
            }
            ToolError::new(merge.to_string()).with_data(Value::Object(data))
        }
        other => ToolError::new(other.to_string()).with_data(Value::Object(data)),
    }
}

impl Tool for RunWorkflow {
    fn name(&self) -> &'static str {
        "run_workflow"
    }

    fn description(&self) -> &'static str {
        "Execute a workflow: for each step check out and pull both branches, merge source into \
         target, and push the target when auto_push is true. Requires confirm=true and a clean \
         working tree. Never prompts — resolve ambiguous patterns with `selections` (use \
         plan_workflow first) and decide package.json version conflicts with `version_strategy`. \
         Any question it still cannot answer is returned as an error naming the question. A merge \
         that stops on conflicts is aborted (unless abort_on_conflict=false) and the conflicted \
         paths are reported."
    }

    fn input_schema(&self) -> Value {
        let mut extra = Map::new();
        extra.insert(
            "workflow".into(),
            json!({ "type": "string", "description": "Exact name of an enabled workflow (see list_workflows)." }),
        );
        extra.insert(
            "confirm".into(),
            json!({ "type": "boolean", "description": "Must be true. Merging and pushing branches is not reversible from here." }),
        );
        extra.insert("selections".into(), selections_schema());
        extra.insert(
            "auto_push".into(),
            json!({ "type": "boolean", "default": false, "description": "Push each target branch that has an upstream after merging. false performs the merges locally only (the CLI's dry-run)." }),
        );
        extra.insert(
            "version_strategy".into(),
            json!({
                "type": "string",
                "enum": VersionStrategy::NAMES,
                "default": "fail",
                "description": "How to settle conflicting \"version\" fields in package.json files: higher, lower, or fail (leave them and report the conflict)."
            }),
        );
        extra.insert(
            "abort_on_conflict".into(),
            json!({ "type": "boolean", "default": true, "description": "Run `git merge --abort` when a step stops on unresolved conflicts, so the repository is left clean. false leaves the merge in progress for a human to finish." }),
        );
        extra.insert(
            "answers".into(),
            json!({
                "type": "object",
                "additionalProperties": { "type": ["string", "boolean", "integer"] },
                "description": "Explicit answers by question key (e.g. {\"conflict:version:1.9.0|1.10.0\": \"1.9.0\", \"push:main\": false}); these override every derived answer."
            }),
        );
        object_schema(extra, &["workflow", "confirm"])
    }

    fn call(&self, arguments: &Value) -> Result<Value, ToolError> {
        let common = CommonArgs::parse(arguments)?;
        let name = required_string(arguments, "workflow")?;
        if !bool_or(arguments, "confirm", false)? {
            return Err(ToolError::new(
                "run_workflow requires confirm=true; call plan_workflow first to see what it would do",
            ));
        }
        let selections = string_map(arguments, "selections")?;
        let auto_push = bool_or(arguments, "auto_push", false)?;
        let abort_on_conflict = bool_or(arguments, "abort_on_conflict", true)?;
        let strategy = VersionStrategy::parse(optional_string(arguments, "version_strategy")?)?;
        let answers = match arguments.get("answers") {
            None | Some(Value::Null) => Map::new(),
            Some(Value::Object(map)) => map.clone(),
            Some(other) => {
                return Err(ToolError::new(format!(
                    "argument `answers` must be an object, got {other}"
                )));
            }
        };

        let loaded = common.load_workflows()?;
        loaded.require_enabled()?;
        let workflow = loaded.find(&name)?;

        let mut git = Git::new(&common.cwd);
        if git.is_dirty()? {
            return Err(ToolError::new("Git is in a Dirty State"));
        }
        let starting_branch = git.current_branch()?;

        let mut prompter = scripted_prompter(auto_push, &selections, strategy, &answers)?;
        let mut sink = CollectingSink::default();
        let settings = Settings {
            cwd: common.cwd.clone(),
            action: Action::Run,
            auto_push,
        };

        match run_workflow(&settings, workflow, &mut git, &mut prompter, &mut sink) {
            Ok(report) => {
                let mut payload = report_json(&report);
                let object = payload.as_object_mut().expect("object");
                object.insert("workflow".into(), json!(workflow.name));
                object.insert("auto_push".into(), json!(auto_push));
                object.insert("starting_branch".into(), json!(starting_branch));
                object.insert(
                    "current_branch".into(),
                    json!(git.current_branch().unwrap_or_default()),
                );
                object.insert(
                    "events".into(),
                    Value::Array(sink.0.iter().map(event_json).collect()),
                );
                Ok(payload)
            }
            Err(error) => Err(run_error(error, &sink.0, &mut git, abort_on_conflict)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompt::{Prompter, Question};

    #[test]
    fn version_strategy_parses_and_defaults_to_fail() {
        assert_eq!(VersionStrategy::parse(None).unwrap(), VersionStrategy::Fail);
        assert_eq!(
            VersionStrategy::parse(Some("higher".into())).unwrap(),
            VersionStrategy::Higher
        );
        assert!(VersionStrategy::parse(Some("ours".into())).is_err());
    }

    #[test]
    fn scripted_prompter_answers_from_arguments_and_explicit_answers_win() {
        let selections =
            std::collections::HashMap::from([("^Patch-*".to_string(), "Patch-v0.1.2".to_string())]);
        let mut answers = Map::new();
        answers.insert("push:main".into(), Value::Bool(false));
        let mut prompter =
            scripted_prompter(true, &selections, VersionStrategy::Higher, &answers).unwrap();

        assert!(
            prompter
                .confirm(&Question::new("step:a>>b", ""), true)
                .unwrap()
        );
        assert!(
            prompter
                .confirm(&Question::new("push:staging", ""), true)
                .unwrap()
        );
        assert!(
            !prompter
                .confirm(&Question::new("push:main", ""), true)
                .unwrap()
        );
        assert!(
            prompter
                .confirm(&Question::new(AUTO_RESOLVE_KEY, ""), true)
                .unwrap()
        );
        let choices = [
            crate::prompt::Choice::new("1.10.0 (higher)", "1.10.0"),
            crate::prompt::Choice::plain("1.9.0"),
        ];
        assert_eq!(
            prompter
                .select(
                    &Question::new("conflict:version:1.9.0|1.10.0", ""),
                    &choices,
                    Some(0)
                )
                .unwrap(),
            0
        );
        assert_eq!(
            prompter
                .select(
                    &Question::new("branch:^Patch-*", ""),
                    &[
                        crate::prompt::Choice::plain("Patch-v0.1.1"),
                        crate::prompt::Choice::plain("Patch-v0.1.2")
                    ],
                    None
                )
                .unwrap(),
            1
        );

        let mut failing = scripted_prompter(
            false,
            &Default::default(),
            VersionStrategy::Fail,
            &Map::new(),
        )
        .unwrap();
        assert!(
            !failing
                .confirm(&Question::new(AUTO_RESOLVE_KEY, ""), true)
                .unwrap()
        );
        assert!(matches!(
            failing.select(&Question::new("branch:x", ""), &[], None),
            Err(PromptError::Unanswered { .. })
        ));
    }
}
