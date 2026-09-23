//! `config doctor`: validate a workflow directory and explain what is wrong with it.
//!
//! The interactive run only tells you about a problem when it hits it (a bad regex fails on the
//! first step, a missing branch fails when its pattern is reached). Doctor runs every check up
//! front, over every file, and reports them together. Errors are things that will make a run
//! fail; warnings are things that will probably surprise; info is advice.

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

use crate::config::{LoadResult, load_workflows};
use crate::pipeline::{Resolution, map_pipeline_report};
use crate::ui::Palette;

/// Keys the schema allows on a workflow file.
const KNOWN_KEYS: &[&str] = &[
    "$schema",
    "disabled",
    "name",
    "description",
    "order",
    "pipeline",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Info,
    Warning,
    Error,
}

/// One finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Problem {
    pub level: Level,
    /// The file it concerns, relative to the root when possible; `None` for directory-level
    /// findings such as "no enabled workflows".
    pub file: Option<String>,
    pub message: String,
}

/// How one pattern matches the branches of `--cwd`, when that was given.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BranchMatch {
    pub pattern: String,
    /// Branches still available for this pattern after earlier patterns took theirs.
    pub matches: Vec<String>,
}

/// One workflow as doctor saw it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkflowReport {
    pub file: String,
    pub name: String,
    pub enabled: bool,
    pub order: Option<f64>,
    pub pipeline: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branches: Option<Vec<BranchMatch>>,
}

/// The whole diagnosis. `ok` is false when any problem is an error.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    pub root: PathBuf,
    pub rule: String,
    pub workflows: Vec<WorkflowReport>,
    pub problems: Vec<Problem>,
    pub ok: bool,
}

impl Report {
    pub fn errors(&self) -> usize {
        self.count(Level::Error)
    }

    pub fn warnings(&self) -> usize {
        self.count(Level::Warning)
    }

    fn count(&self, level: Level) -> usize {
        self.problems
            .iter()
            .filter(|problem| problem.level == level)
            .count()
    }
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

/// Read the file again as plain JSON so keys the lenient loader ignores can be reported.
fn raw_object(path: &Path) -> Option<serde_json::Map<String, Value>> {
    let text = fs::read_to_string(path).ok()?;
    match serde_json::from_str::<Value>(&text).ok()? {
        Value::Object(map) => Some(map),
        _ => None,
    }
}

fn check_raw_file(root: &Path, path: &Path, problems: &mut Vec<Problem>) {
    let Some(raw) = raw_object(path) else {
        return; // Parse failures were already reported by the loader.
    };
    let file = Some(relative(root, path));

    match raw.get("name") {
        Some(Value::String(name)) if !name.trim().is_empty() => {}
        Some(Value::String(_)) => problems.push(Problem {
            level: Level::Error,
            file: file.clone(),
            message: "\"name\" is empty; the workflow cannot be selected by name".to_string(),
        }),
        Some(_) => problems.push(Problem {
            level: Level::Error,
            file: file.clone(),
            message: "\"name\" must be a string".to_string(),
        }),
        None => problems.push(Problem {
            level: Level::Error,
            file: file.clone(),
            message: "\"name\" is missing; the workflow would be listed as \"not named\""
                .to_string(),
        }),
    }

    let unknown: Vec<&String> = raw
        .keys()
        .filter(|key| !KNOWN_KEYS.contains(&key.as_str()))
        .collect();
    if !unknown.is_empty() {
        problems.push(Problem {
            level: Level::Warning,
            file: file.clone(),
            message: format!(
                "unknown key{} {} (ignored)",
                if unknown.len() == 1 { "" } else { "s" },
                unknown
                    .iter()
                    .map(|key| format!("\"{key}\""))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        });
    }

    if !raw.contains_key("$schema") {
        problems.push(Problem {
            level: Level::Info,
            file,
            message: "no \"$schema\" key; add one so editors can validate the file".to_string(),
        });
    }
}

fn check_patterns(root: &Path, loaded: &LoadResult, problems: &mut Vec<Problem>) {
    for workflow in &loaded.workflows {
        for (index, pattern) in workflow.pipeline.iter().enumerate() {
            if let Err(error) = regex::Regex::new(&format!("(?i){pattern}")) {
                let reason = error.to_string();
                let reason = reason.lines().last().unwrap_or("").trim();
                problems.push(Problem {
                    level: Level::Error,
                    file: Some(relative(root, &workflow.source)),
                    message: format!(
                        "pipeline[{index}] \"{pattern}\" is not a valid regular expression: {reason}"
                    ),
                });
            }
        }
    }
}

fn check_names_and_order(root: &Path, loaded: &LoadResult, problems: &mut Vec<Problem>) {
    let mut seen: HashMap<&str, &Path> = HashMap::new();
    for workflow in loaded.enabled() {
        if let Some(first) = seen.insert(workflow.name.as_str(), &workflow.source) {
            problems.push(Problem {
                level: Level::Error,
                file: Some(relative(root, &workflow.source)),
                message: format!(
                    "duplicate name \"{}\" (also in {}); --workflow selects by exact name",
                    workflow.name,
                    relative(root, first)
                ),
            });
        }
    }

    if loaded.names().is_empty() {
        problems.push(Problem {
            level: Level::Error,
            file: None,
            message: format!(
                "no enabled workflows in {}; a run would fail with \"Expected to find enabled workflows\"",
                loaded.root.display()
            ),
        });
    }

    let ordered = loaded
        .workflows
        .iter()
        .filter(|w| w.order.is_some())
        .count();
    if ordered > 0 && ordered < loaded.workflows.len() {
        let missing: Vec<String> = loaded
            .workflows
            .iter()
            .filter(|w| w.order.is_none())
            .map(|w| relative(root, &w.source))
            .collect();
        problems.push(Problem {
            level: Level::Warning,
            file: None,
            message: format!(
                "\"order\" is set on some workflows but not {}; unordered ones are listed last",
                missing.join(", ")
            ),
        });
    }
}

fn branch_matches(
    root: &Path,
    workflow: &crate::config::WorkflowConfig,
    branches: &[String],
    problems: &mut Vec<Problem>,
) -> Vec<BranchMatch> {
    let file = Some(relative(root, &workflow.source));
    let report = match map_pipeline_report(&workflow.pipeline, branches, &HashMap::new()) {
        Ok(report) => report,
        // Invalid patterns are reported by check_patterns; nothing more to say here.
        Err(_) => return Vec::new(),
    };

    report
        .into_iter()
        .map(|resolution| match resolution {
            Resolution::Resolved { pattern, branch } => BranchMatch {
                pattern,
                matches: vec![branch],
            },
            Resolution::Ambiguous {
                pattern,
                candidates,
            } => {
                problems.push(Problem {
                    level: Level::Info,
                    file: file.clone(),
                    message: format!(
                        "\"{pattern}\" matches {} branches ({}); a run will ask which to use",
                        candidates.len(),
                        candidates.join(", ")
                    ),
                });
                BranchMatch {
                    pattern,
                    matches: candidates,
                }
            }
            Resolution::NoMatch { pattern } => {
                problems.push(Problem {
                    level: Level::Warning,
                    file: file.clone(),
                    message: format!(
                        "\"{pattern}\" matches no local branch right now; a run would fail with \"No matching branch found\""
                    ),
                });
                BranchMatch {
                    pattern,
                    matches: Vec::new(),
                }
            }
        })
        .collect()
}

/// Run every check over `dir`. `rule` is how the directory was chosen, for the report header.
/// `branches`, when given, are the local branches of a repository to check patterns against.
pub fn diagnose(dir: &Path, rule: &str, branches: Option<&[String]>) -> Report {
    let loaded = load_workflows(dir);
    let root = loaded.root.clone();
    let mut problems = Vec::new();

    for error in &loaded.errors {
        problems.push(Problem {
            level: Level::Error,
            file: Some(relative(&root, &error.path)),
            message: error.reason.clone(),
        });
    }
    for workflow in &loaded.workflows {
        check_raw_file(&root, &workflow.source, &mut problems);
    }
    check_patterns(&root, &loaded, &mut problems);
    check_names_and_order(&root, &loaded, &mut problems);

    let workflows = loaded
        .workflows
        .iter()
        .map(|workflow| WorkflowReport {
            file: relative(&root, &workflow.source),
            name: workflow.name.clone(),
            enabled: workflow.enabled(),
            order: workflow.order,
            pipeline: workflow.pipeline.clone(),
            branches: branches
                .filter(|_| workflow.enabled())
                .map(|branches| branch_matches(&root, workflow, branches, &mut problems)),
        })
        .collect();

    problems.sort_by(|a, b| a.file.cmp(&b.file).then(b.level.cmp(&a.level)));
    let ok = !problems.iter().any(|problem| problem.level == Level::Error);

    Report {
        root,
        rule: rule.to_string(),
        workflows,
        problems,
        ok,
    }
}

fn glyph(palette: Palette, level: Level) -> String {
    match level {
        Level::Error => palette.red("✗"),
        Level::Warning => palette.orange("!"),
        Level::Info => palette.dim("·"),
    }
}

/// Human-readable rendering, one block per workflow then the findings that have no file.
pub fn render(palette: Palette, report: &Report) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Workflow directory: {} {}\n",
        report.root.display(),
        palette.dim(&format!("(chosen by: {})", report.rule))
    ));

    let files: BTreeSet<&str> = report
        .workflows
        .iter()
        .map(|w| w.file.as_str())
        .chain(report.problems.iter().filter_map(|p| p.file.as_deref()))
        .collect();

    for file in files {
        let problems: Vec<&Problem> = report
            .problems
            .iter()
            .filter(|p| p.file.as_deref() == Some(file))
            .collect();
        let worst = problems.iter().map(|p| p.level).max();
        let status = match worst {
            Some(Level::Error) => palette.red("✗"),
            Some(Level::Warning) => palette.orange("!"),
            _ => palette.cyan("✓"),
        };

        out.push('\n');
        match report.workflows.iter().find(|w| w.file == file) {
            Some(workflow) => {
                let state = if workflow.enabled {
                    String::new()
                } else {
                    palette.dim(" (disabled)")
                };
                let order = workflow
                    .order
                    .map(|order| palette.dim(&format!(" order {order}")))
                    .unwrap_or_default();
                out.push_str(&format!(
                    "{status} {}  {}{state}{order}\n",
                    palette.cyan(file),
                    palette.orange(&workflow.name)
                ));
                out.push_str(&format!(
                    "    {}\n",
                    workflow.pipeline.join(&palette.dim(" > "))
                ));
                if let Some(branches) = &workflow.branches {
                    for m in branches {
                        let shown = if m.matches.is_empty() {
                            palette.dim("no match")
                        } else {
                            m.matches.join(", ")
                        };
                        out.push_str(&format!("    {} → {shown}\n", palette.dim(&m.pattern)));
                    }
                }
            }
            None => out.push_str(&format!("{status} {}\n", palette.cyan(file))),
        }
        for problem in problems {
            out.push_str(&format!(
                "    {} {}\n",
                glyph(palette, problem.level),
                problem.message
            ));
        }
    }

    let general: Vec<&Problem> = report
        .problems
        .iter()
        .filter(|p| p.file.is_none())
        .collect();
    if !general.is_empty() {
        out.push('\n');
        for problem in general {
            out.push_str(&format!(
                "{} {}\n",
                glyph(palette, problem.level),
                problem.message
            ));
        }
    }

    let enabled = report.workflows.iter().filter(|w| w.enabled).count();
    let verdict = if report.ok {
        palette.cyan("✓ configuration is usable")
    } else {
        palette.red("✗ configuration has errors")
    };
    out.push_str(&format!(
        "\n{} workflow{}, {} enabled · {} error{}, {} warning{}\n{verdict}\n",
        report.workflows.len(),
        if report.workflows.len() == 1 { "" } else { "s" },
        enabled,
        report.errors(),
        if report.errors() == 1 { "" } else { "s" },
        report.warnings(),
        if report.warnings() == 1 { "" } else { "s" },
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, json: &str) {
        fs::write(dir.join(name), json).unwrap();
    }

    fn messages(report: &Report, level: Level) -> Vec<String> {
        report
            .problems
            .iter()
            .filter(|p| p.level == level)
            .map(|p| p.message.clone())
            .collect()
    }

    #[test]
    fn the_shipped_examples_are_clean() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/config");
        let report = diagnose(&dir, "test", None);
        assert!(report.ok, "{:?}", report.problems);
        assert_eq!(report.errors(), 0);
        assert_eq!(report.warnings(), 0);
        assert_eq!(report.workflows.len(), 4);
        assert_eq!(report.workflows.iter().filter(|w| w.enabled).count(), 3);
    }

    #[test]
    fn bad_json_missing_name_and_short_pipeline_are_errors() {
        let temp = tempfile::tempdir().unwrap();
        write(temp.path(), "broken.json", "{ nope");
        write(temp.path(), "nameless.json", r#"{"pipeline":["a","b"]}"#);
        write(
            temp.path(),
            "short.json",
            r#"{"name":"Short","pipeline":["a"]}"#,
        );

        let report = diagnose(temp.path(), "test", None);
        assert!(!report.ok);
        let errors = messages(&report, Level::Error);
        assert_eq!(errors.len(), 3, "{errors:?}");
        assert!(errors.iter().any(|m| m.contains("\"name\" is missing")));
        assert!(errors.iter().any(|m| m.contains("at least 2 entries")));
        assert!(
            report
                .problems
                .iter()
                .any(|p| p.file.as_deref() == Some("broken.json"))
        );
    }

    #[test]
    fn invalid_regex_is_an_error_naming_the_index() {
        let temp = tempfile::tempdir().unwrap();
        write(
            temp.path(),
            "bad.json",
            r#"{"$schema":"x","name":"Bad","pipeline":["^Patch-*","(unclosed"]}"#,
        );
        let report = diagnose(temp.path(), "test", None);
        let errors = messages(&report, Level::Error);
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(
            errors[0].starts_with("pipeline[1] \"(unclosed\" is not a valid regular expression")
        );
    }

    #[test]
    fn duplicate_enabled_names_and_zero_enabled_are_errors() {
        let temp = tempfile::tempdir().unwrap();
        write(
            temp.path(),
            "a.json",
            r#"{"$schema":"x","name":"Same","pipeline":["a","b"]}"#,
        );
        write(
            temp.path(),
            "b.json",
            r#"{"$schema":"x","name":"Same","pipeline":["a","b"]}"#,
        );
        let report = diagnose(temp.path(), "test", None);
        let errors = messages(&report, Level::Error);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("duplicate name \"Same\" (also in a.json)"));

        let temp = tempfile::tempdir().unwrap();
        write(
            temp.path(),
            "off.json",
            r#"{"$schema":"x","disabled":true,"name":"Off","pipeline":["a","b"]}"#,
        );
        let report = diagnose(temp.path(), "test", None);
        assert!(!report.ok);
        assert!(messages(&report, Level::Error)[0].contains("no enabled workflows"));
    }

    #[test]
    fn unknown_keys_mixed_order_and_missing_schema_are_not_errors() {
        let temp = tempfile::tempdir().unwrap();
        write(
            temp.path(),
            "one.json",
            r#"{"name":"One","order":1,"pipeline":["a","b"],"colour":"blue"}"#,
        );
        write(
            temp.path(),
            "two.json",
            r#"{"$schema":"x","name":"Two","pipeline":["a","b"]}"#,
        );
        let report = diagnose(temp.path(), "test", None);
        assert!(report.ok);
        let warnings = messages(&report, Level::Warning);
        assert!(
            warnings
                .iter()
                .any(|m| m == "unknown key \"colour\" (ignored)")
        );
        assert!(
            warnings
                .iter()
                .any(|m| m.contains("\"order\" is set on some workflows but not two.json"))
        );
        let info = messages(&report, Level::Info);
        assert_eq!(info.len(), 1);
        assert!(info[0].contains("no \"$schema\" key"));
        assert_eq!(
            report
                .problems
                .iter()
                .find(|p| p.message.contains("colour"))
                .unwrap()
                .file
                .as_deref(),
            Some("one.json")
        );
    }

    #[test]
    fn branch_matches_report_no_match_as_warning_and_ambiguity_as_info() {
        let temp = tempfile::tempdir().unwrap();
        write(
            temp.path(),
            "canary.json",
            r#"{"$schema":"x","name":"Canary","pipeline":["^Patch-*","^Release-*","^Release-*","^staging-canary$"]}"#,
        );
        write(
            temp.path(),
            "off.json",
            r#"{"$schema":"x","disabled":true,"name":"Off","pipeline":["^nothing$","main"]}"#,
        );
        let branches: Vec<String> = ["main", "Patch-v0.1.1", "Release-0.1.0", "Release-0.2.0"]
            .iter()
            .map(|s| s.to_string())
            .collect();

        let report = diagnose(temp.path(), "test", Some(&branches));
        assert!(report.ok, "{:?}", report.problems);

        let canary = report
            .workflows
            .iter()
            .find(|w| w.name == "Canary")
            .unwrap();
        let matches = canary.branches.as_ref().unwrap();
        assert_eq!(matches[0].matches, vec!["Patch-v0.1.1"]);
        assert_eq!(matches[1].matches.len(), 2);
        assert!(matches[3].matches.is_empty());

        let warnings = messages(&report, Level::Warning);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("\"^staging-canary$\" matches no local branch"));
        assert!(
            messages(&report, Level::Info)
                .iter()
                .any(|m| m.contains("\"^Release-*\" matches 2 branches"))
        );

        // Disabled workflows are not checked against branches.
        let off = report.workflows.iter().find(|w| w.name == "Off").unwrap();
        assert!(off.branches.is_none());
    }

    #[test]
    fn render_summarises_counts_and_verdict() {
        let temp = tempfile::tempdir().unwrap();
        write(
            temp.path(),
            "bad.json",
            r#"{"$schema":"x","name":"Bad","pipeline":["(","b"]}"#,
        );
        let report = diagnose(temp.path(), "--config flag", None);
        let text = render(Palette::plain(), &report);
        assert!(text.starts_with("Workflow directory: "));
        assert!(text.contains("(chosen by: --config flag)"));
        assert!(text.contains("✗ bad.json  Bad"));
        assert!(text.contains("1 workflow, 1 enabled · 1 error, 0 warnings"));
        assert!(text.trim_end().ends_with("✗ configuration has errors"));

        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(json["ok"], false);
        assert_eq!(json["problems"][0]["level"], "error");
        assert_eq!(json["problems"][0]["file"], "bad.json");
    }
}
