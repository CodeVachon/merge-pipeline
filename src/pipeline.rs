//! Mapping pipeline patterns to branches and generating merge steps.
//!
//! Ported from `utl/mapPipelineValueToBranch.ts` and `utl/makeWorkflowSteps.ts`. Each pipeline
//! entry is a regular expression matched case-insensitively (unanchored search) against the local
//! branch names; a branch chosen for an earlier entry is excluded from later ones, which is how
//! `["^Patch-*", "^Release-*", "^Release-*", "^staging-canary$"]` yields two distinct releases.

use std::collections::HashMap;

use regex::Regex;

use crate::prompt::{Choice, PromptError, Prompter, Question};

/// One merge: `source` into `target`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub source: String,
    pub target: String,
}

impl Step {
    /// The prompt key used to confirm this step.
    pub fn question_key(&self) -> String {
        format!("step:{}>>{}", self.source, self.target)
    }
}

/// Consecutive pairs: `[a, b, c]` → `a→b`, `b→c`.
pub fn make_steps<S: AsRef<str>>(branches: &[S]) -> Vec<Step> {
    branches
        .windows(2)
        .map(|pair| Step {
            source: pair[0].as_ref().to_string(),
            target: pair[1].as_ref().to_string(),
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PipelineError {
    #[error("No matching branch found for \"{0}\"")]
    NoMatch(String),
    #[error("Invalid pattern \"{pattern}\": {reason}")]
    InvalidPattern { pattern: String, reason: String },
    #[error(transparent)]
    Prompt(#[from] PromptError),
}

/// How one pattern resolved, for non-interactive reporting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    Resolved {
        pattern: String,
        branch: String,
    },
    Ambiguous {
        pattern: String,
        candidates: Vec<String>,
    },
    NoMatch {
        pattern: String,
    },
}

/// The prompt key for choosing a branch for `pattern`.
pub fn branch_question_key(pattern: &str) -> String {
    format!("branch:{pattern}")
}

fn compile(pattern: &str) -> Result<Regex, PipelineError> {
    Regex::new(&format!("(?i){pattern}")).map_err(|error| PipelineError::InvalidPattern {
        pattern: pattern.to_string(),
        reason: error.to_string(),
    })
}

fn candidates_for<S: AsRef<str>>(
    pattern: &str,
    branches: &[S],
    taken: &[String],
) -> Result<Vec<String>, PipelineError> {
    let regex = compile(pattern)?;
    Ok(branches
        .iter()
        .map(|branch| branch.as_ref())
        .filter(|branch| regex.is_match(branch))
        .filter(|branch| !taken.iter().any(|t| t == branch))
        .map(str::to_string)
        .collect())
}

/// Map every pattern to a branch, asking `prompter` (key `branch:<pattern>`) when several match.
pub fn map_pipeline<S: AsRef<str>>(
    pipeline: &[String],
    branches: &[S],
    prompter: &mut dyn Prompter,
) -> Result<Vec<String>, PipelineError> {
    let mut mapped: Vec<String> = Vec::new();

    for pattern in pipeline {
        let found = candidates_for(pattern, branches, &mapped)?;
        let chosen = match found.len() {
            0 => return Err(PipelineError::NoMatch(pattern.clone())),
            1 => found[0].clone(),
            _ => {
                let question = Question::new(
                    branch_question_key(pattern),
                    format!("Which branch should we use for \"{pattern}\""),
                );
                let choices: Vec<Choice> = found.iter().map(Choice::plain).collect();
                let index = prompter.select(&question, &choices, None)?;
                found[index].clone()
            }
        };
        mapped.push(chosen);
    }

    Ok(mapped)
}

/// Non-interactive mapping: `selections` (pattern → branch) stand in for prompts; anything still
/// ambiguous or unmatched is reported instead of failing. Only an invalid regex is an error.
pub fn map_pipeline_report<S: AsRef<str>>(
    pipeline: &[String],
    branches: &[S],
    selections: &HashMap<String, String>,
) -> Result<Vec<Resolution>, PipelineError> {
    let mut taken: Vec<String> = Vec::new();
    let mut report = Vec::new();

    for pattern in pipeline {
        let found = candidates_for(pattern, branches, &taken)?;
        let resolution = match found.len() {
            0 => Resolution::NoMatch {
                pattern: pattern.clone(),
            },
            1 => Resolution::Resolved {
                pattern: pattern.clone(),
                branch: found[0].clone(),
            },
            _ => match selections.get(pattern) {
                Some(branch) if found.iter().any(|candidate| candidate == branch) => {
                    Resolution::Resolved {
                        pattern: pattern.clone(),
                        branch: branch.clone(),
                    }
                }
                _ => Resolution::Ambiguous {
                    pattern: pattern.clone(),
                    candidates: found,
                },
            },
        };
        if let Resolution::Resolved { branch, .. } = &resolution {
            taken.push(branch.clone());
        }
        report.push(resolution);
    }

    Ok(report)
}

/// Collapse a report into branches when every pattern resolved.
pub fn resolved_branches(report: &[Resolution]) -> Option<Vec<String>> {
    report
        .iter()
        .map(|resolution| match resolution {
            Resolution::Resolved { branch, .. } => Some(branch.clone()),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompt::{RecordingPrompter, ScriptedPrompter};

    fn branches() -> Vec<&'static str> {
        vec![
            "main",
            "staging-patch",
            "staging-release",
            "staging-canary",
            "Release-0.1.0",
            "Patch-v0.1.1",
            "Patch-v0.1.2",
            "Release-0.2.0",
        ]
    }

    fn pipeline(patterns: &[&str]) -> Vec<String> {
        patterns.iter().map(|p| p.to_string()).collect()
    }

    // Ported from utl/makeWorkflowSteps.test.ts
    #[test]
    fn make_steps_returns_an_array_of_objects() {
        let data = ["a", "b", "c", "d"];
        let steps = make_steps(&data);
        assert_eq!(steps.len(), data.len() - 1);
        assert_eq!(
            steps,
            vec![
                Step {
                    source: "a".into(),
                    target: "b".into()
                },
                Step {
                    source: "b".into(),
                    target: "c".into()
                },
                Step {
                    source: "c".into(),
                    target: "d".into()
                },
            ]
        );
        assert!(make_steps::<&str>(&[]).is_empty());
        assert!(make_steps(&["only"]).is_empty());
    }

    #[test]
    fn single_matches_are_taken_without_prompting_and_are_case_insensitive() {
        let mut prompter = RecordingPrompter::new(ScriptedPrompter::new());
        let mapped = map_pipeline(
            &pipeline(&["^patch-v0.1.1$", "^STAGING-PATCH$"]),
            &branches(),
            &mut prompter,
        )
        .unwrap();
        assert_eq!(mapped, vec!["Patch-v0.1.1", "staging-patch"]);
        assert!(prompter.recorded.is_empty());
    }

    #[test]
    fn ambiguous_patterns_prompt_with_key_branch_pattern() {
        let scripted = ScriptedPrompter::new().answer("branch:^Patch-*", "Patch-v0.1.2");
        let mut prompter = RecordingPrompter::new(scripted);
        let mapped = map_pipeline(
            &pipeline(&["^Patch-*", "^staging-patch$"]),
            &branches(),
            &mut prompter,
        )
        .unwrap();
        assert_eq!(mapped, vec!["Patch-v0.1.2", "staging-patch"]);
        assert_eq!(prompter.keys(), vec!["branch:^Patch-*"]);
        assert_eq!(
            prompter.recorded[0].question.message,
            "Which branch should we use for \"^Patch-*\""
        );
    }

    #[test]
    fn already_mapped_branches_are_excluded_so_repeated_patterns_pick_distinct_branches() {
        // The canary config lists ^Release-* twice on purpose.
        let scripted = ScriptedPrompter::new()
            .answer("branch:^Patch-*", "Patch-v0.1.1")
            .answer("branch:^Release-*", "Release-0.1.0");
        let mut prompter = RecordingPrompter::new(scripted);
        let mapped = map_pipeline(
            &pipeline(&["^Patch-*", "^Release-*", "^Release-*", "^staging-canary$"]),
            &branches(),
            &mut prompter,
        )
        .unwrap();
        // Second ^Release-* has only one candidate left, so it is taken without a prompt.
        assert_eq!(
            mapped,
            vec![
                "Patch-v0.1.1",
                "Release-0.1.0",
                "Release-0.2.0",
                "staging-canary"
            ]
        );
        assert_eq!(
            prompter.keys(),
            vec!["branch:^Patch-*", "branch:^Release-*"]
        );
    }

    #[test]
    fn no_match_and_invalid_regex_are_errors_naming_the_pattern() {
        let mut prompter = ScriptedPrompter::new();
        let error =
            map_pipeline(&pipeline(&["^nope$", "main"]), &branches(), &mut prompter).unwrap_err();
        assert_eq!(error.to_string(), "No matching branch found for \"^nope$\"");

        let error = map_pipeline(
            &pipeline(&["(unclosed", "main"]),
            &branches(),
            &mut prompter,
        )
        .unwrap_err();
        assert!(
            matches!(error, PipelineError::InvalidPattern { ref pattern, .. } if pattern == "(unclosed")
        );
    }

    #[test]
    fn unanswered_prompt_surfaces_as_a_prompt_error() {
        let mut prompter = ScriptedPrompter::new();
        let error =
            map_pipeline(&pipeline(&["^Patch-*", "main"]), &branches(), &mut prompter).unwrap_err();
        assert!(matches!(
            error,
            PipelineError::Prompt(PromptError::Unanswered { ref key, ref choices, .. })
                if key == "branch:^Patch-*" && choices == &vec!["Patch-v0.1.1".to_string(), "Patch-v0.1.2".to_string()]
        ));
    }

    #[test]
    fn report_lists_ambiguous_candidates_and_honours_selections() {
        let report = map_pipeline_report(
            &pipeline(&[
                "^Patch-*",
                "^Release-*",
                "^Release-*",
                "^staging-canary$",
                "^zzz$",
            ]),
            &branches(),
            &HashMap::from([("^Patch-*".to_string(), "Patch-v0.1.2".to_string())]),
        )
        .unwrap();
        assert_eq!(
            report,
            vec![
                Resolution::Resolved {
                    pattern: "^Patch-*".into(),
                    branch: "Patch-v0.1.2".into()
                },
                Resolution::Ambiguous {
                    pattern: "^Release-*".into(),
                    candidates: vec!["Release-0.1.0".into(), "Release-0.2.0".into()]
                },
                Resolution::Ambiguous {
                    pattern: "^Release-*".into(),
                    candidates: vec!["Release-0.1.0".into(), "Release-0.2.0".into()]
                },
                Resolution::Resolved {
                    pattern: "^staging-canary$".into(),
                    branch: "staging-canary".into()
                },
                Resolution::NoMatch {
                    pattern: "^zzz$".into()
                },
            ]
        );
        assert_eq!(resolved_branches(&report), None);

        let full = map_pipeline_report(
            &pipeline(&["^Patch-*", "^staging-patch$"]),
            &branches(),
            &HashMap::from([("^Patch-*".to_string(), "Patch-v0.1.1".to_string())]),
        )
        .unwrap();
        assert_eq!(
            resolved_branches(&full),
            Some(vec![
                "Patch-v0.1.1".to_string(),
                "staging-patch".to_string()
            ])
        );
    }
}
