//! The interactive [`Prompter`], backed by `inquire`.
//!
//! `select` uses inquire's built-in fuzzy filter, which replaces the baseline's
//! `inquirer-search-list` plugin for the workflow picker and every other list.

use inquire::{Confirm, InquireError, Select, Text};

use crate::prompt::{Choice, PromptError, Prompter, Question};

/// Asks a human at the terminal.
#[derive(Debug, Default, Clone, Copy)]
pub struct InteractivePrompter;

fn map_error(question: &Question, error: InquireError) -> PromptError {
    let reason = match error {
        InquireError::OperationCanceled => "cancelled".to_string(),
        InquireError::OperationInterrupted => "interrupted".to_string(),
        InquireError::NotTTY => "stdin is not a terminal".to_string(),
        other => other.to_string(),
    };
    PromptError::Interrupted {
        key: question.key.clone(),
        reason,
    }
}

impl Prompter for InteractivePrompter {
    fn confirm(&mut self, question: &Question, default: bool) -> Result<bool, PromptError> {
        Confirm::new(&question.message)
            .with_default(default)
            .prompt()
            .map_err(|error| map_error(question, error))
    }

    fn select(
        &mut self,
        question: &Question,
        choices: &[Choice],
        default: Option<usize>,
    ) -> Result<usize, PromptError> {
        let labels: Vec<String> = choices.iter().map(|choice| choice.label.clone()).collect();
        let mut select = Select::new(&question.message, labels);
        if let Some(index) = default.filter(|index| *index < choices.len()) {
            select = select.with_starting_cursor(index);
        }
        select
            .raw_prompt()
            .map(|option| option.index)
            .map_err(|error| map_error(question, error))
    }

    fn text(&mut self, question: &Question, default: Option<&str>) -> Result<String, PromptError> {
        let mut text = Text::new(&question.message);
        if let Some(default) = default {
            text = text.with_default(default);
        }
        text.prompt().map_err(|error| map_error(question, error))
    }
}
