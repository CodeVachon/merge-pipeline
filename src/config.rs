//! Workflow configuration files: discovery, parsing, ordering, and lookup.
//!
//! Ported from the baseline's `utl/Workflows.ts` + `utl/readJSONFile.ts`. A workflow is one JSON
//! file; a directory of them is a config set. Discovery uses `<dir>/config` when that is a
//! directory (the baseline's layout), otherwise `<dir>` itself, and walks recursively.

use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::text::array_to_string_list;

/// One workflow definition.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowConfig {
    /// Disabled workflows are loaded but never offered or runnable.
    pub disabled: bool,
    /// Display name; also the lookup key for `--workflow`.
    pub name: String,
    pub description: String,
    /// Lower values are listed first. Workflows without an order come after every ordered one.
    pub order: Option<f64>,
    /// Regex patterns, in merge order. Each maps to one branch; consecutive pairs become steps.
    pub pipeline: Vec<String>,
    /// Where it was read from.
    pub source: PathBuf,
}

impl WorkflowConfig {
    pub fn enabled(&self) -> bool {
        !self.disabled
    }
}

/// The on-disk shape, lenient the way `{...defaults, ...JSON.parse()}` was.
#[derive(Debug, Deserialize)]
struct RawWorkflow {
    #[serde(default)]
    disabled: bool,
    name: Option<String>,
    #[serde(default)]
    description: String,
    order: Option<f64>,
    #[serde(default)]
    pipeline: Vec<String>,
}

/// A file that could not become a workflow. Collected, not fatal.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("An Error occurred while trying to read or parse {}: {reason}", path.display())]
pub struct LoadError {
    pub path: PathBuf,
    pub reason: String,
}

/// Errors from looking workflows up.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    #[error(
        "Could not find Workflow named \"{name}\". Expected one of [{}]",
        array_to_string_list(available)
    )]
    UnknownWorkflow {
        name: String,
        available: Vec<String>,
    },
    #[error("Expected to find enabled workflows at {}. found 0", root.display())]
    NoEnabledWorkflows { root: PathBuf },
}

/// Everything `load_workflows` found.
#[derive(Debug, Clone, Default)]
pub struct LoadResult {
    /// The directory actually walked (`<dir>/config` or `<dir>`).
    pub root: PathBuf,
    /// Sorted: ordered first ascending, then unordered in path order.
    pub workflows: Vec<WorkflowConfig>,
    pub errors: Vec<LoadError>,
}

impl LoadResult {
    pub fn enabled(&self) -> impl Iterator<Item = &WorkflowConfig> {
        self.workflows.iter().filter(|workflow| workflow.enabled())
    }

    /// Names of enabled workflows, trimmed, blanks dropped.
    pub fn names(&self) -> Vec<String> {
        self.enabled()
            .map(|workflow| workflow.name.trim().to_string())
            .filter(|name| !name.is_empty())
            .collect()
    }

    /// Exact-name lookup among enabled workflows.
    pub fn find(&self, name: &str) -> Result<&WorkflowConfig, ConfigError> {
        self.enabled()
            .find(|workflow| workflow.name == name)
            .ok_or_else(|| ConfigError::UnknownWorkflow {
                name: name.to_string(),
                available: self.names(),
            })
    }

    /// Error unless at least one workflow is enabled.
    pub fn require_enabled(&self) -> Result<(), ConfigError> {
        if self.names().is_empty() {
            Err(ConfigError::NoEnabledWorkflows {
                root: self.root.clone(),
            })
        } else {
            Ok(())
        }
    }
}

/// `<dir>/config` if it is a directory, otherwise `<dir>`.
pub fn workflow_root(dir: &Path) -> PathBuf {
    let candidate = dir.join("config");
    if candidate.is_dir() {
        candidate
    } else {
        dir.to_path_buf()
    }
}

fn compare_paths(left: &Path, right: &Path) -> Ordering {
    // Approximates JS localeCompare: case-insensitive first, then bytes as a tiebreak.
    let l = left.to_string_lossy();
    let r = right.to_string_lossy();
    l.to_lowercase()
        .cmp(&r.to_lowercase())
        .then_with(|| l.as_bytes().cmp(r.as_bytes()))
}

/// Every `*.json` under `root`, recursively, sorted by path.
fn find_workflow_files(root: &Path, errors: &mut Vec<LoadError>) -> Vec<PathBuf> {
    let mut matches = Vec::new();
    let mut pending = vec![root.to_path_buf()];

    while let Some(directory) = pending.pop() {
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                errors.push(LoadError {
                    path: directory.clone(),
                    reason: error.to_string(),
                });
                continue;
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.is_file()
                && path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
            {
                matches.push(path);
            }
        }
    }

    matches.sort_by(|left, right| compare_paths(left, right));
    matches
}

fn parse_workflow(path: &Path) -> Result<WorkflowConfig, LoadError> {
    let contents = fs::read_to_string(path).map_err(|error| LoadError {
        path: path.to_path_buf(),
        reason: error.to_string(),
    })?;
    let raw: RawWorkflow = serde_json::from_str(&contents).map_err(|error| LoadError {
        path: path.to_path_buf(),
        reason: error.to_string(),
    })?;

    if raw.pipeline.len() < 2 {
        return Err(LoadError {
            path: path.to_path_buf(),
            reason: format!(
                "pipeline must have at least 2 entries, found {}",
                raw.pipeline.len()
            ),
        });
    }

    Ok(WorkflowConfig {
        disabled: raw.disabled,
        name: raw.name.unwrap_or_else(|| "not named".to_string()),
        description: raw.description,
        order: raw.order.filter(|order| order.is_finite()),
        pipeline: raw.pipeline,
        source: path.to_path_buf(),
    })
}

fn compare_order(left: &WorkflowConfig, right: &WorkflowConfig) -> Ordering {
    match (left.order, right.order) {
        (Some(l), Some(r)) => l.partial_cmp(&r).unwrap_or(Ordering::Equal),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

/// Load every workflow under `dir` (see [`workflow_root`]). Never fails as a whole; per-file
/// problems land in [`LoadResult::errors`].
pub fn load_workflows(dir: &Path) -> LoadResult {
    let root = workflow_root(dir);
    let mut errors = Vec::new();
    let files = find_workflow_files(&root, &mut errors);

    let mut workflows = Vec::new();
    for path in files {
        match parse_workflow(&path) {
            Ok(workflow) => workflows.push(workflow),
            Err(error) => errors.push(error),
        }
    }

    // Stable sort keeps the path order as the fallback for equal / missing `order`.
    workflows.sort_by(compare_order);

    LoadResult {
        root,
        workflows,
        errors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(dir: &Path, name: &str, json: &str) {
        fs::write(dir.join(name), json).unwrap();
    }

    // Ported from utl/Workflows.test.ts
    #[test]
    fn sorts_workflows_by_order_and_keeps_file_order_as_the_fallback() {
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("config");
        fs::create_dir_all(&config).unwrap();

        write(
            &config,
            "z-last.json",
            r#"{"name":"Unordered Last","pipeline":["staging","main"]}"#,
        );
        write(
            &config,
            "a-first.json",
            r#"{"name":"Unordered First","pipeline":["staging","main"]}"#,
        );
        write(
            &config,
            "ordered-two.json",
            r#"{"name":"Ordered Two","order":2,"pipeline":["staging","main"]}"#,
        );
        write(
            &config,
            "ordered-one.json",
            r#"{"name":"Ordered One","order":1,"pipeline":["staging","main"]}"#,
        );

        let result = load_workflows(temp.path());

        assert_eq!(result.root, config);
        assert!(result.errors.is_empty());
        assert_eq!(
            result.names(),
            vec![
                "Ordered One",
                "Ordered Two",
                "Unordered First",
                "Unordered Last"
            ]
        );
    }

    #[test]
    fn walks_nested_directories_and_uses_dir_itself_when_no_config_subdir() {
        let temp = tempfile::tempdir().unwrap();
        let nested = temp.path().join("team/alpha");
        fs::create_dir_all(&nested).unwrap();
        write(
            temp.path(),
            "top.json",
            r#"{"name":"Top","pipeline":["a","b"]}"#,
        );
        write(
            &nested,
            "deep.json",
            r#"{"name":"Deep","pipeline":["a","b"]}"#,
        );
        write(&nested, "notes.txt", "not a workflow");

        let result = load_workflows(temp.path());
        assert_eq!(result.root, temp.path());
        assert_eq!(result.names(), vec!["Deep", "Top"]);
    }

    #[test]
    fn bad_json_and_short_pipelines_are_collected_as_errors_not_failures() {
        let temp = tempfile::tempdir().unwrap();
        write(temp.path(), "broken.json", "{ not json");
        write(
            temp.path(),
            "short.json",
            r#"{"name":"Short","pipeline":["only-one"]}"#,
        );
        write(
            temp.path(),
            "good.json",
            r#"{"$schema":"x","name":"Good","pipeline":["a","b"],"extra":true}"#,
        );

        let result = load_workflows(temp.path());
        assert_eq!(result.names(), vec!["Good"]);
        assert_eq!(result.errors.len(), 2);
        assert!(
            result
                .errors
                .iter()
                .any(|error| error.path.ends_with("broken.json"))
        );
        assert!(
            result
                .errors
                .iter()
                .any(|error| error.path.ends_with("short.json")
                    && error.reason.contains("at least 2"))
        );
    }

    #[test]
    fn disabled_workflows_are_loaded_but_not_listed_or_findable() {
        let temp = tempfile::tempdir().unwrap();
        write(
            temp.path(),
            "default.json",
            r#"{"disabled":true,"name":"Default Pipeline","order":100,"pipeline":["staging","main"]}"#,
        );
        write(
            temp.path(),
            "patch.json",
            r#"{"disabled":false,"name":"Patch","order":1,"pipeline":["^Patch-*","^staging-patch$"]}"#,
        );

        let result = load_workflows(temp.path());
        assert_eq!(result.workflows.len(), 2);
        assert_eq!(result.names(), vec!["Patch"]);
        assert!(result.find("Patch").is_ok());
        assert_eq!(
            result.find("Default Pipeline").unwrap_err().to_string(),
            "Could not find Workflow named \"Default Pipeline\". Expected one of [\"Patch\"]"
        );
        assert!(result.require_enabled().is_ok());
    }

    #[test]
    fn defaults_match_the_baseline() {
        let temp = tempfile::tempdir().unwrap();
        write(temp.path(), "bare.json", r#"{"pipeline":["a","b"]}"#);
        let result = load_workflows(temp.path());
        let workflow = &result.workflows[0];
        assert_eq!(workflow.name, "not named");
        assert_eq!(workflow.description, "");
        assert!(!workflow.disabled);
        assert_eq!(workflow.order, None);
    }

    #[test]
    fn require_enabled_reports_the_root() {
        let temp = tempfile::tempdir().unwrap();
        let result = load_workflows(temp.path());
        let error = result.require_enabled().unwrap_err();
        assert_eq!(
            error.to_string(),
            format!(
                "Expected to find enabled workflows at {}. found 0",
                temp.path().display()
            )
        );
    }
}
