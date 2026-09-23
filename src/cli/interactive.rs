//! The interactive run: the baseline's `src/index.ts` `setup()` + `main()`.
//!
//! Order of operations is preserved exactly: title, config discovery, workflow loading, working
//! directory, clean-tree guard, workflow selection, action selection, auto-push question (run
//! only), then the runner. Every question goes through the [`Prompter`] so `--yes` and tests can
//! answer without a terminal.

use std::path::{Path, PathBuf};

use anyhow::{Context, anyhow};

use super::config_dir;
use super::prompter::InteractivePrompter;
use super::{CliAction, RunArgs};
use crate::config::{LoadResult, WorkflowConfig, load_workflows};
use crate::git::Git;
use crate::prompt::{Choice, PromptError, Prompter, Question};
use crate::runner::{Action, RunReport, Settings, run_workflow};
use crate::sync::SyncOptions;
use crate::ui::{self, Palette, StdoutRenderer};

/// Answers every confirmation with its default, and every select that has a default with that
/// default; anything else is delegated. This is what `--yes` does.
pub struct AssumeDefaults<P: Prompter>(pub P);

impl<P: Prompter> Prompter for AssumeDefaults<P> {
    fn confirm(&mut self, _question: &Question, default: bool) -> Result<bool, PromptError> {
        Ok(default)
    }

    fn select(
        &mut self,
        question: &Question,
        choices: &[Choice],
        default: Option<usize>,
    ) -> Result<usize, PromptError> {
        match default {
            Some(index) if index < choices.len() => Ok(index),
            _ => self.0.select(question, choices, default),
        }
    }

    fn text(&mut self, question: &Question, default: Option<&str>) -> Result<String, PromptError> {
        match default {
            Some(value) => Ok(value.to_string()),
            None => self.0.text(question, default),
        }
    }
}

fn process_cwd() -> anyhow::Result<PathBuf> {
    std::env::current_dir().context("could not determine the current directory")
}

fn load(args: &RunArgs, cwd_hint: &Path, palette: Palette) -> anyhow::Result<LoadResult> {
    let resolved = config_dir::resolve_from_process(args.config.as_deref(), Some(cwd_hint))?;
    let loaded = load_workflows(&resolved.path);
    for error in &loaded.errors {
        eprintln!("{} {error}", palette.red("warning:"));
    }
    loaded.require_enabled()?;
    Ok(loaded)
}

fn working_directory(args: &RunArgs, prompter: &mut dyn Prompter) -> anyhow::Result<PathBuf> {
    if let Some(cwd) = &args.cwd {
        return Ok(cwd.clone());
    }
    let default = process_cwd()?;
    let answer = prompter.text(
        &Question::new("cwd", "Working Directory?"),
        Some(&default.to_string_lossy()),
    )?;
    let answer = answer.trim();
    Ok(if answer.is_empty() {
        default
    } else {
        PathBuf::from(answer)
    })
}

fn ensure_clean(cwd: &Path) -> anyhow::Result<()> {
    let mut git = Git::new(cwd);
    if git.is_dirty()? {
        return Err(anyhow!("Git is in a Dirty State"));
    }
    Ok(())
}

fn select_workflow<'a>(
    args: &RunArgs,
    loaded: &'a LoadResult,
    prompter: &mut dyn Prompter,
) -> anyhow::Result<&'a WorkflowConfig> {
    if let Some(name) = &args.workflow {
        return Ok(loaded.find(name)?);
    }
    let names = loaded.names();
    if names.len() == 1 {
        return Ok(loaded.find(&names[0])?);
    }
    let choices: Vec<Choice> = names.iter().map(Choice::plain).collect();
    let index = prompter.select(
        &Question::new("workflow", "Which Workflow would you like to run?"),
        &choices,
        None,
    )?;
    Ok(loaded.find(&names[index])?)
}

fn select_action(args: &RunArgs, prompter: &mut dyn Prompter) -> anyhow::Result<Action> {
    if let Some(action) = args.action {
        return Ok(action.into());
    }
    let choices: Vec<Choice> = Action::ALL
        .iter()
        .map(|action| Choice::new(action.label(), action.as_str()))
        .collect();
    let index = prompter.select(
        &Question::new("action", "What would you like to do?"),
        &choices,
        None,
    )?;
    Ok(Action::ALL[index])
}

fn select_auto_push(
    args: &RunArgs,
    action: Action,
    prompter: &mut dyn Prompter,
) -> anyhow::Result<bool> {
    if action != Action::Run {
        return Ok(false);
    }
    if args.auto_push {
        return Ok(true);
    }
    Ok(prompter.confirm(
        &Question::new(
            "auto_push",
            "Would you like to Automatically Push to Origin?",
        ),
        true,
    )?)
}

/// Everything up to and including the run. Errors propagate so `main` can render them.
pub fn execute(args: &RunArgs, palette: Palette) -> anyhow::Result<RunReport> {
    let mut prompter: Box<dyn Prompter> = if args.yes {
        Box::new(AssumeDefaults(InteractivePrompter))
    } else {
        Box::new(InteractivePrompter)
    };

    let cwd_hint = match &args.cwd {
        Some(cwd) => cwd.clone(),
        None => process_cwd()?,
    };
    let loaded = load(args, &cwd_hint, palette)?;
    let cwd = working_directory(args, prompter.as_mut())?;
    ensure_clean(&cwd)?;

    let workflow = select_workflow(args, &loaded, prompter.as_mut())?;
    let action = select_action(args, prompter.as_mut())?;
    let auto_push = select_auto_push(args, action, prompter.as_mut())?;

    println!();
    println!(
        "{}: {}",
        palette.cyan(&action.label()),
        palette.orange(&workflow.name)
    );
    println!();

    let settings = Settings {
        cwd: cwd.clone(),
        action,
        auto_push,
        sync: if args.no_sync {
            None
        } else {
            Some(SyncOptions::default())
        },
    };
    let mut git = Git::new(&cwd).verbose(ui::command_logger(palette));
    let mut sink = StdoutRenderer::new(palette);
    Ok(run_workflow(
        &settings,
        workflow,
        &mut git,
        prompter.as_mut(),
        &mut sink,
    )?)
}

/// The full interactive command, including the title and the closing lines. Returns the exit
/// code; errors are already rendered.
pub fn run(args: &RunArgs) -> i32 {
    let palette = Palette::detect();
    ui::print_title(palette, "Git Branch\nWorkflow");

    let code = match execute(args, palette) {
        Ok(_) => {
            println!("Task Complete");
            0
        }
        Err(error) => {
            ui::print_error(palette, &error);
            1
        }
    };
    println!("Work Complete");
    code
}

impl From<CliAction> for Choice {
    fn from(value: CliAction) -> Self {
        let action: Action = value.into();
        Choice::new(action.label(), action.as_str())
    }
}
