//! The `Prompter` seam: every interactive question flows through here.
//!
//! The core library never talks to a terminal. Anything that needs a human decision asks a
//! [`Prompter`], identified by a stable [`Question::key`]. The CLI supplies an interactive
//! implementation; the MCP server supplies a [`ScriptedPrompter`] whose answers come from tool
//! arguments, so an unanswered question becomes a structured error rather than a hang.
//!
//! Question keys used across the crate (the MCP surface pre-answers by these):
//!
//! | key                          | kind    | asked by                                   |
//! |------------------------------|---------|--------------------------------------------|
//! | `cwd`                        | text    | CLI, when `--cwd` is absent                 |
//! | `workflow`                   | select  | CLI, when more than one workflow is enabled |
//! | `action`                     | select  | CLI, when `--action` is absent              |
//! | `auto_push`                  | confirm | CLI, for the `run` action                   |
//! | `branch:<pattern>`           | select  | pipeline mapping, when a regex is ambiguous |
//! | `step:<source>>><target>`    | confirm | runner, before each merge                   |
//! | `push:<branch>`              | confirm | runner, when auto-push is off               |
//! | `conflict:auto_resolve`      | confirm | package.json conflict resolver              |
//! | `conflict:version:<lo>\|<hi>` | select  | package.json conflict resolver              |

use std::collections::HashMap;
use std::fmt;

/// A question posed to a [`Prompter`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    /// Stable identifier, see the module docs for the vocabulary.
    pub key: String,
    /// Human-readable message. May contain ANSI colour codes when produced by the CLI layer.
    pub message: String,
}

impl Question {
    pub fn new(key: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            message: message.into(),
        }
    }
}

/// One option in a `select` question.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    /// What is displayed.
    pub label: String,
    /// What is returned / matched against scripted answers.
    pub value: String,
}

impl Choice {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
        }
    }

    /// A choice whose label and value are the same string.
    pub fn plain(value: impl Into<String>) -> Self {
        let value = value.into();
        Self {
            label: value.clone(),
            value,
        }
    }
}

/// Why a prompt could not produce an answer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PromptError {
    /// A non-interactive prompter had no answer for this question.
    #[error("no answer supplied for question `{key}`: {message}")]
    Unanswered {
        key: String,
        message: String,
        choices: Vec<String>,
    },
    /// A scripted answer did not fit the question (wrong type or not one of the choices).
    #[error("answer for question `{key}` is invalid: {reason}")]
    InvalidAnswer { key: String, reason: String },
    /// The user aborted (Ctrl-C / Esc) or the terminal failed.
    #[error("prompt `{key}` was interrupted: {reason}")]
    Interrupted { key: String, reason: String },
}

/// The one abstraction the core uses to ask a human (or an agent) anything.
pub trait Prompter {
    /// Yes / no.
    fn confirm(&mut self, question: &Question, default: bool) -> Result<bool, PromptError>;

    /// Pick one of `choices`; returns the index. `default` is pre-selected when present.
    fn select(
        &mut self,
        question: &Question,
        choices: &[Choice],
        default: Option<usize>,
    ) -> Result<usize, PromptError>;

    /// Free text, with an optional default.
    fn text(&mut self, question: &Question, default: Option<&str>) -> Result<String, PromptError>;
}

/// A pre-supplied answer for a [`ScriptedPrompter`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// Answers a `confirm`.
    Bool(bool),
    /// Answers a `text`, or a `select` by matching the choice value (then label).
    Text(String),
    /// Answers a `select` by position.
    Index(usize),
    /// Answers with whatever default the question offers; errors if it has none.
    Default,
}

impl From<bool> for Answer {
    fn from(value: bool) -> Self {
        Answer::Bool(value)
    }
}

impl From<&str> for Answer {
    fn from(value: &str) -> Self {
        Answer::Text(value.to_string())
    }
}

impl From<String> for Answer {
    fn from(value: String) -> Self {
        Answer::Text(value)
    }
}

/// A [`Prompter`] that answers from a script. Exact key matches win over prefix rules; prefix
/// rules are checked in insertion order. Anything unmatched is an [`PromptError::Unanswered`].
#[derive(Debug, Default, Clone)]
pub struct ScriptedPrompter {
    exact: HashMap<String, Answer>,
    prefixes: Vec<(String, Answer)>,
}

impl ScriptedPrompter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Answer the question whose key is exactly `key`.
    pub fn answer(mut self, key: impl Into<String>, answer: impl Into<Answer>) -> Self {
        self.exact.insert(key.into(), answer.into());
        self
    }

    /// Answer every question whose key starts with `prefix`.
    pub fn answer_prefix(mut self, prefix: impl Into<String>, answer: impl Into<Answer>) -> Self {
        self.prefixes.push((prefix.into(), answer.into()));
        self
    }

    fn lookup(&self, key: &str) -> Option<&Answer> {
        if let Some(answer) = self.exact.get(key) {
            return Some(answer);
        }
        self.prefixes
            .iter()
            .find(|(prefix, _)| key.starts_with(prefix.as_str()))
            .map(|(_, answer)| answer)
    }

    fn unanswered(question: &Question, choices: &[Choice]) -> PromptError {
        PromptError::Unanswered {
            key: question.key.clone(),
            message: question.message.clone(),
            choices: choices.iter().map(|choice| choice.value.clone()).collect(),
        }
    }

    fn invalid(question: &Question, reason: impl fmt::Display) -> PromptError {
        PromptError::InvalidAnswer {
            key: question.key.clone(),
            reason: reason.to_string(),
        }
    }
}

impl Prompter for ScriptedPrompter {
    fn confirm(&mut self, question: &Question, default: bool) -> Result<bool, PromptError> {
        match self.lookup(&question.key) {
            None => Err(Self::unanswered(question, &[])),
            Some(Answer::Bool(value)) => Ok(*value),
            Some(Answer::Default) => Ok(default),
            Some(Answer::Text(text)) => match text.trim().to_ascii_lowercase().as_str() {
                "y" | "yes" | "true" => Ok(true),
                "n" | "no" | "false" => Ok(false),
                other => Err(Self::invalid(
                    question,
                    format!("expected yes/no, got {other:?}"),
                )),
            },
            Some(Answer::Index(_)) => Err(Self::invalid(question, "expected yes/no, got an index")),
        }
    }

    fn select(
        &mut self,
        question: &Question,
        choices: &[Choice],
        default: Option<usize>,
    ) -> Result<usize, PromptError> {
        let Some(answer) = self.lookup(&question.key) else {
            return Err(Self::unanswered(question, choices));
        };

        match answer {
            Answer::Default => default.ok_or_else(|| {
                Self::invalid(question, "asked for the default but the question has none")
            }),
            Answer::Index(index) if *index < choices.len() => Ok(*index),
            Answer::Index(index) => Err(Self::invalid(
                question,
                format!(
                    "index {index} is out of range for {} choices",
                    choices.len()
                ),
            )),
            Answer::Text(text) => choices
                .iter()
                .position(|choice| choice.value == *text)
                .or_else(|| choices.iter().position(|choice| choice.label == *text))
                .ok_or_else(|| {
                    Self::invalid(
                        question,
                        format!(
                            "{text:?} is not one of [{}]",
                            choices
                                .iter()
                                .map(|choice| format!("{:?}", choice.value))
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                    )
                }),
            Answer::Bool(_) => Err(Self::invalid(question, "expected a choice, got yes/no")),
        }
    }

    fn text(&mut self, question: &Question, default: Option<&str>) -> Result<String, PromptError> {
        match self.lookup(&question.key) {
            None => Err(Self::unanswered(question, &[])),
            Some(Answer::Text(text)) => Ok(text.clone()),
            Some(Answer::Default) => default.map(str::to_string).ok_or_else(|| {
                Self::invalid(question, "asked for the default but the question has none")
            }),
            Some(Answer::Bool(value)) => Ok(value.to_string()),
            Some(Answer::Index(_)) => Err(Self::invalid(question, "expected text, got an index")),
        }
    }
}

/// What kind of prompt was issued, for [`RecordingPrompter`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptKind {
    Confirm {
        default: bool,
    },
    Select {
        choices: Vec<Choice>,
        default: Option<usize>,
    },
    Text {
        default: Option<String>,
    },
}

/// One prompt seen by a [`RecordingPrompter`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recorded {
    pub question: Question,
    pub kind: PromptKind,
}

/// Wraps another prompter and records everything asked, for assertions in tests.
pub struct RecordingPrompter<P: Prompter> {
    inner: P,
    pub recorded: Vec<Recorded>,
}

impl<P: Prompter> RecordingPrompter<P> {
    pub fn new(inner: P) -> Self {
        Self {
            inner,
            recorded: Vec::new(),
        }
    }

    pub fn into_inner(self) -> P {
        self.inner
    }

    /// Keys of every question asked, in order.
    pub fn keys(&self) -> Vec<&str> {
        self.recorded
            .iter()
            .map(|record| record.question.key.as_str())
            .collect()
    }
}

impl<P: Prompter> Prompter for RecordingPrompter<P> {
    fn confirm(&mut self, question: &Question, default: bool) -> Result<bool, PromptError> {
        self.recorded.push(Recorded {
            question: question.clone(),
            kind: PromptKind::Confirm { default },
        });
        self.inner.confirm(question, default)
    }

    fn select(
        &mut self,
        question: &Question,
        choices: &[Choice],
        default: Option<usize>,
    ) -> Result<usize, PromptError> {
        self.recorded.push(Recorded {
            question: question.clone(),
            kind: PromptKind::Select {
                choices: choices.to_vec(),
                default,
            },
        });
        self.inner.select(question, choices, default)
    }

    fn text(&mut self, question: &Question, default: Option<&str>) -> Result<String, PromptError> {
        self.recorded.push(Recorded {
            question: question.clone(),
            kind: PromptKind::Text {
                default: default.map(str::to_string),
            },
        });
        self.inner.text(question, default)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn choices() -> Vec<Choice> {
        vec![
            Choice::new("1.10.0 (higher)", "1.10.0"),
            Choice::plain("1.9.0"),
        ]
    }

    #[test]
    fn unanswered_question_reports_key_message_and_choices() {
        let mut prompter = ScriptedPrompter::new();
        let question = Question::new("branch:^Release-*", "Which branch?");
        let err = prompter
            .select(&question, &choices(), None)
            .expect_err("nothing scripted");
        assert_eq!(
            err,
            PromptError::Unanswered {
                key: "branch:^Release-*".into(),
                message: "Which branch?".into(),
                choices: vec!["1.10.0".into(), "1.9.0".into()],
            }
        );
    }

    #[test]
    fn exact_answer_beats_prefix_rule() {
        let mut prompter = ScriptedPrompter::new()
            .answer_prefix("step:", true)
            .answer("step:a>>b", false);
        assert!(
            !prompter
                .confirm(&Question::new("step:a>>b", "?"), true)
                .unwrap()
        );
        assert!(
            prompter
                .confirm(&Question::new("step:b>>c", "?"), false)
                .unwrap()
        );
    }

    #[test]
    fn select_matches_value_then_label_then_index() {
        let mut by_value = ScriptedPrompter::new().answer("v", "1.9.0");
        assert_eq!(
            by_value
                .select(&Question::new("v", ""), &choices(), None)
                .unwrap(),
            1
        );

        let mut by_label = ScriptedPrompter::new().answer("v", "1.10.0 (higher)");
        assert_eq!(
            by_label
                .select(&Question::new("v", ""), &choices(), None)
                .unwrap(),
            0
        );

        let mut by_index = ScriptedPrompter::new().answer("v", Answer::Index(1));
        assert_eq!(
            by_index
                .select(&Question::new("v", ""), &choices(), None)
                .unwrap(),
            1
        );

        let mut by_default = ScriptedPrompter::new().answer("v", Answer::Default);
        assert_eq!(
            by_default
                .select(&Question::new("v", ""), &choices(), Some(0))
                .unwrap(),
            0
        );
    }

    #[test]
    fn select_rejects_unknown_value_and_out_of_range_index() {
        let mut bad_value = ScriptedPrompter::new().answer("v", "2.0.0");
        assert!(matches!(
            bad_value.select(&Question::new("v", ""), &choices(), None),
            Err(PromptError::InvalidAnswer { .. })
        ));
        let mut bad_index = ScriptedPrompter::new().answer("v", Answer::Index(7));
        assert!(matches!(
            bad_index.select(&Question::new("v", ""), &choices(), None),
            Err(PromptError::InvalidAnswer { .. })
        ));
    }

    #[test]
    fn confirm_accepts_textual_yes_no_and_default() {
        let mut prompter = ScriptedPrompter::new()
            .answer("a", "yes")
            .answer("b", "No")
            .answer("c", Answer::Default);
        assert!(prompter.confirm(&Question::new("a", ""), false).unwrap());
        assert!(!prompter.confirm(&Question::new("b", ""), true).unwrap());
        assert!(prompter.confirm(&Question::new("c", ""), true).unwrap());
    }

    #[test]
    fn text_uses_default_when_asked() {
        let mut prompter = ScriptedPrompter::new()
            .answer("cwd", Answer::Default)
            .answer("name", "hello");
        assert_eq!(
            prompter
                .text(&Question::new("cwd", ""), Some("/tmp"))
                .unwrap(),
            "/tmp"
        );
        assert_eq!(
            prompter.text(&Question::new("name", ""), None).unwrap(),
            "hello"
        );
        assert!(matches!(
            prompter.text(&Question::new("missing", ""), None),
            Err(PromptError::Unanswered { .. })
        ));
    }

    #[test]
    fn recording_prompter_captures_every_question_in_order() {
        let inner = ScriptedPrompter::new()
            .answer("auto_push", true)
            .answer("workflow", "Patch")
            .answer("cwd", "/repo");
        let mut recorder = RecordingPrompter::new(inner);
        recorder
            .confirm(&Question::new("auto_push", "Push?"), true)
            .unwrap();
        recorder
            .select(
                &Question::new("workflow", "Which?"),
                &[Choice::plain("Patch"), Choice::plain("Minor")],
                None,
            )
            .unwrap();
        recorder
            .text(&Question::new("cwd", "Dir?"), Some("."))
            .unwrap();

        assert_eq!(recorder.keys(), vec!["auto_push", "workflow", "cwd"]);
        assert_eq!(
            recorder.recorded[1].kind,
            PromptKind::Select {
                choices: vec![Choice::plain("Patch"), Choice::plain("Minor")],
                default: None
            }
        );
    }
}
