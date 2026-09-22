//! The workflow runner: prepare steps, then merge (and push) them one by one.
//!
//! Ported from `actions/runWorkflow.ts`, `actions/testWorkflow.ts` and `actions/shared.ts`.
//! Performs no console output — progress is reported as [`Event`]s through an [`EventSink`], so
//! the CLI can render them and the MCP server can collect them.

use std::path::PathBuf;
use std::str::FromStr;

use crate::config::WorkflowConfig;
use crate::conflicts::{ConflictError, try_resolve_package_json_versions};
use crate::git::{Git, GitError};
use crate::pipeline::{PipelineError, Step, make_steps, map_pipeline};
use crate::prompt::{PromptError, Prompter, Question};
use crate::text::{action_to_string, array_to_string_list, plural};

/// What the user asked for. Note `dry-run` still merges locally; it only stops automatic pushes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Run,
    DryRun,
    Test,
}

impl Action {
    pub const ALL: [Action; 3] = [Action::Run, Action::DryRun, Action::Test];

    pub fn as_str(self) -> &'static str {
        match self {
            Action::Run => "run",
            Action::DryRun => "dry-run",
            Action::Test => "test",
        }
    }

    /// "Run", "Dry Run", "Test".
    pub fn label(self) -> String {
        action_to_string(self.as_str())
    }
}

impl FromStr for Action {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "run" => Ok(Action::Run),
            "dry-run" | "dry_run" | "dryrun" => Ok(Action::DryRun),
            "test" => Ok(Action::Test),
            other => Err(format!(
                "Unexpected Action \"{other}\". Expected one of [{}]",
                array_to_string_list(&Action::ALL.map(Action::as_str))
            )),
        }
    }
}

impl std::fmt::Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Per-run settings (the baseline's `ISettings`, minus the workflow name).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub cwd: PathBuf,
    pub action: Action,
    /// Push each target without asking. Forced off for `dry-run`.
    pub auto_push: bool,
}

/// Why a push did not happen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PushSkipReason {
    NoUpstream,
    Declined,
}

/// Progress, in order of occurrence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// The workflow's raw patterns.
    Pipeline {
        patterns: Vec<String>,
    },
    Fetched,
    /// Patterns resolved to concrete branches.
    BranchesMapped {
        branches: Vec<String>,
    },
    StepsPlanned {
        steps: Vec<Step>,
    },
    StepStarted {
        index: usize,
        total: usize,
        step: Step,
    },
    CheckedOut {
        branch: String,
    },
    Pulled {
        branch: String,
    },
    Merged {
        step: Step,
    },
    ConflictsDetected {
        step: Step,
        paths: Vec<String>,
    },
    ConflictsResolved {
        step: Step,
        rewritten: Vec<String>,
        committed: bool,
    },
    Pushed {
        branch: String,
    },
    PushSkipped {
        branch: String,
        reason: PushSkipReason,
    },
}

/// Receives events.
pub trait EventSink {
    fn event(&mut self, event: &Event);
}

impl<F: FnMut(&Event)> EventSink for F {
    fn event(&mut self, event: &Event) {
        self(event)
    }
}

/// Collects events into a Vec.
#[derive(Debug, Default)]
pub struct CollectingSink(pub Vec<Event>);

impl EventSink for CollectingSink {
    fn event(&mut self, event: &Event) {
        self.0.push(event.clone());
    }
}

/// A merge that stopped on conflicts the resolver could not clear.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("Merge Error: {} into {}.{}", step.source, step.target, describe_files(files))]
pub struct MergeError {
    pub step: Step,
    /// Conflicted paths left behind; the merge is still in progress in the working tree.
    pub files: Vec<String>,
    /// git's own message when there were no conflicted paths to show.
    pub cause: Option<String>,
}

fn describe_files(files: &[String]) -> String {
    if files.is_empty() {
        String::new()
    } else {
        format!(
            " {} conflicted {}",
            files.len(),
            plural("file", files.len())
        )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error(transparent)]
    Git(#[from] GitError),
    #[error(transparent)]
    Pipeline(#[from] PipelineError),
    #[error(transparent)]
    Prompt(#[from] PromptError),
    #[error(transparent)]
    Conflict(#[from] ConflictError),
    #[error(transparent)]
    Merge(#[from] MergeError),
    #[error("User chose not to proceed with merging \"{}\" into \"{}\"", step.source, step.target)]
    UserDeclined { step: Step },
}

/// What happened to one step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepOutcome {
    pub step: Step,
    pub merged: bool,
    pub pushed: bool,
    /// package.json files the resolver rewrote.
    pub conflicts_resolved: Vec<String>,
}

/// Result of a completed run.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RunReport {
    pub branches: Vec<String>,
    pub steps: Vec<StepOutcome>,
}

/// Fetch, map the pipeline to branches, and produce the steps. This is all `test` does.
pub fn prepare(
    workflow: &WorkflowConfig,
    git: &mut Git,
    prompter: &mut dyn Prompter,
    sink: &mut dyn EventSink,
) -> Result<(Vec<String>, Vec<Step>), RunError> {
    sink.event(&Event::Pipeline {
        patterns: workflow.pipeline.clone(),
    });
    git.fetch()?;
    sink.event(&Event::Fetched);

    let branches = git.branch_list()?;
    let mapped = map_pipeline(&workflow.pipeline, &branches, prompter)?;
    sink.event(&Event::BranchesMapped {
        branches: mapped.clone(),
    });

    let steps = make_steps(&mapped);
    sink.event(&Event::StepsPlanned {
        steps: steps.clone(),
    });
    Ok((mapped, steps))
}

fn confirm_step(step: &Step, prompter: &mut dyn Prompter) -> Result<(), RunError> {
    let proceed = prompter.confirm(
        &Question::new(
            step.question_key(),
            format!("Merge {} into {}?", step.source, step.target),
        ),
        true,
    )?;
    if proceed {
        Ok(())
    } else {
        Err(RunError::UserDeclined { step: step.clone() })
    }
}

fn update_branch(git: &mut Git, branch: &str, sink: &mut dyn EventSink) -> Result<bool, RunError> {
    git.checkout(branch)?;
    sink.event(&Event::CheckedOut {
        branch: branch.to_string(),
    });
    let has_upstream = git.has_upstream()?;
    if has_upstream {
        git.pull()?;
        sink.event(&Event::Pulled {
            branch: branch.to_string(),
        });
    }
    Ok(has_upstream)
}

fn merge_step(
    settings: &Settings,
    step: &Step,
    git: &mut Git,
    prompter: &mut dyn Prompter,
    sink: &mut dyn EventSink,
) -> Result<Vec<String>, RunError> {
    let merge_error = match git.merge(&step.source) {
        Ok(_) => {
            sink.event(&Event::Merged { step: step.clone() });
            return Ok(Vec::new());
        }
        Err(error) => error,
    };

    let conflicted = git.conflicted_paths()?;
    sink.event(&Event::ConflictsDetected {
        step: step.clone(),
        paths: conflicted.clone(),
    });

    let outcome = try_resolve_package_json_versions(&settings.cwd, &conflicted, git, prompter)?;
    if outcome.attempted {
        sink.event(&Event::ConflictsResolved {
            step: step.clone(),
            rewritten: outcome.rewritten.clone(),
            committed: outcome.committed,
        });
    }

    if outcome.attempted && outcome.remaining.is_empty() {
        sink.event(&Event::Merged { step: step.clone() });
        return Ok(outcome.rewritten);
    }

    let files = outcome.remaining;
    Err(MergeError {
        step: step.clone(),
        cause: if files.is_empty() {
            Some(merge_error.to_string())
        } else {
            None
        },
        files,
    }
    .into())
}

fn should_push(
    settings: &Settings,
    branch: &str,
    has_upstream: bool,
    prompter: &mut dyn Prompter,
) -> Result<Result<(), PushSkipReason>, RunError> {
    if !has_upstream {
        return Ok(Err(PushSkipReason::NoUpstream));
    }
    if settings.auto_push {
        return Ok(Ok(()));
    }
    let push = prompter.confirm(
        &Question::new(
            format!("push:{branch}"),
            format!("Would you like to push {branch}"),
        ),
        true,
    )?;
    Ok(if push {
        Ok(())
    } else {
        Err(PushSkipReason::Declined)
    })
}

fn run_step(
    settings: &Settings,
    index: usize,
    total: usize,
    step: &Step,
    git: &mut Git,
    prompter: &mut dyn Prompter,
    sink: &mut dyn EventSink,
) -> Result<StepOutcome, RunError> {
    sink.event(&Event::StepStarted {
        index,
        total,
        step: step.clone(),
    });
    confirm_step(step, prompter)?;
    update_branch(git, &step.source, sink)?;
    let target_has_upstream = update_branch(git, &step.target, sink)?;
    let conflicts_resolved = merge_step(settings, step, git, prompter, sink)?;

    let pushed = match should_push(settings, &step.target, target_has_upstream, prompter)? {
        Ok(()) => {
            git.push()?;
            sink.event(&Event::Pushed {
                branch: step.target.clone(),
            });
            true
        }
        Err(reason) => {
            sink.event(&Event::PushSkipped {
                branch: step.target.clone(),
                reason,
            });
            false
        }
    };

    Ok(StepOutcome {
        step: step.clone(),
        merged: true,
        pushed,
        conflicts_resolved,
    })
}

/// Execute `workflow` according to `settings.action`.
///
/// `test` stops after planning. `run` and `dry-run` confirm, update, merge and (maybe) push each
/// step; `dry-run` forces `auto_push` off, so every push is asked about — exactly as the baseline.
pub fn run_workflow(
    settings: &Settings,
    workflow: &WorkflowConfig,
    git: &mut Git,
    prompter: &mut dyn Prompter,
    sink: &mut dyn EventSink,
) -> Result<RunReport, RunError> {
    let settings = Settings {
        auto_push: settings.auto_push && settings.action != Action::DryRun,
        ..settings.clone()
    };

    let (branches, steps) = prepare(workflow, git, prompter, sink)?;
    let mut report = RunReport {
        branches,
        steps: Vec::new(),
    };

    if settings.action == Action::Test {
        return Ok(report);
    }

    let total = steps.len();
    for (index, step) in steps.iter().enumerate() {
        let outcome = run_step(&settings, index, total, step, git, prompter, sink)?;
        report.steps.push(outcome);
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_parses_and_labels_like_the_baseline() {
        assert_eq!("run".parse::<Action>().unwrap(), Action::Run);
        assert_eq!("dry-run".parse::<Action>().unwrap(), Action::DryRun);
        assert_eq!("dry_run".parse::<Action>().unwrap(), Action::DryRun);
        assert_eq!("TEST".parse::<Action>().unwrap(), Action::Test);
        assert_eq!(Action::DryRun.label(), "Dry Run");
        assert_eq!(
            "nope".parse::<Action>().unwrap_err(),
            "Unexpected Action \"nope\". Expected one of [\"run\", \"dry-run\", \"test\"]"
        );
    }

    #[test]
    fn merge_error_message_counts_files() {
        let step = Step {
            source: "a".into(),
            target: "b".into(),
        };
        let one = MergeError {
            step: step.clone(),
            files: vec!["x".into()],
            cause: None,
        };
        assert_eq!(one.to_string(), "Merge Error: a into b. 1 conflicted file");
        let none = MergeError {
            step,
            files: vec![],
            cause: Some("boom".into()),
        };
        assert_eq!(none.to_string(), "Merge Error: a into b.");
    }
}
