//! `merge-pipeline install`: merge this server into an agent's MCP configuration.
//!
//! Project scope writes `.mcp.json` in the current directory (what Claude Code, Codex and Cursor
//! read for a repository). User scope writes the `mcpServers` map of `~/.claude.json`, Claude
//! Code's user-level configuration. Existing keys are preserved in both files.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, anyhow};
use serde_json::{Map, Value, json};

use crate::cli::InstallScope;

/// The key under `mcpServers`.
pub const SERVER_KEY: &str = "merge-pipeline";

/// The entry that is written.
pub fn server_entry() -> Value {
    json!({ "command": "merge-pipeline", "args": ["mcp"] })
}

/// What happened to the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Created(PathBuf),
    Added(PathBuf),
    Updated(PathBuf),
    Unchanged(PathBuf),
}

impl fmt::Display for Outcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Outcome::Created(path) => {
                write!(f, "created {} with the {SERVER_KEY} server", path.display())
            }
            Outcome::Added(path) => write!(f, "added {SERVER_KEY} to {}", path.display()),
            Outcome::Updated(path) => {
                write!(f, "updated the {SERVER_KEY} entry in {}", path.display())
            }
            Outcome::Unchanged(path) => {
                write!(
                    f,
                    "{} already wires {SERVER_KEY}; nothing changed",
                    path.display()
                )
            }
        }
    }
}

/// Where `scope` writes by default.
pub fn default_target(scope: InstallScope) -> anyhow::Result<PathBuf> {
    match scope {
        InstallScope::Project => Ok(std::env::current_dir()
            .context("could not determine the current directory")?
            .join(".mcp.json")),
        InstallScope::User => Ok(dirs::home_dir()
            .ok_or_else(|| anyhow!("could not determine the home directory"))?
            .join(".claude.json")),
    }
}

/// Merge the server entry into `target`, creating the file when absent.
pub fn install(_scope: InstallScope, target: &Path) -> anyhow::Result<Outcome> {
    let existed = target.exists();
    let mut root = if existed {
        let text = fs::read_to_string(target)
            .with_context(|| format!("could not read {}", target.display()))?;
        if text.trim().is_empty() {
            Value::Object(Map::new())
        } else {
            serde_json::from_str(&text)
                .with_context(|| format!("{} is not valid JSON", target.display()))?
        }
    } else {
        Value::Object(Map::new())
    };

    let object = root
        .as_object_mut()
        .ok_or_else(|| anyhow!("{} must contain a JSON object", target.display()))?;
    let servers = object
        .entry("mcpServers")
        .or_insert_with(|| Value::Object(Map::new()));
    let servers = servers
        .as_object_mut()
        .ok_or_else(|| anyhow!("`mcpServers` in {} must be an object", target.display()))?;

    let previous = servers.insert(SERVER_KEY.to_string(), server_entry());
    let outcome = match (existed, previous) {
        (false, _) => Outcome::Created(target.to_path_buf()),
        (true, None) => Outcome::Added(target.to_path_buf()),
        (true, Some(old)) if old == server_entry() => Outcome::Unchanged(target.to_path_buf()),
        (true, Some(_)) => Outcome::Updated(target.to_path_buf()),
    };

    if !matches!(outcome, Outcome::Unchanged(_)) {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("could not create {}", parent.display()))?;
        }
        let text = serde_json::to_string_pretty(&root)?;
        fs::write(target, format!("{text}\n"))
            .with_context(|| format!("could not write {}", target.display()))?;
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_adds_keeps_and_updates() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("nested").join(".mcp.json");

        assert_eq!(
            install(InstallScope::Project, &target).unwrap(),
            Outcome::Created(target.clone())
        );
        let written: Value = serde_json::from_str(&fs::read_to_string(&target).unwrap()).unwrap();
        assert_eq!(written["mcpServers"][SERVER_KEY], server_entry());

        assert_eq!(
            install(InstallScope::Project, &target).unwrap(),
            Outcome::Unchanged(target.clone())
        );

        fs::write(
            &target,
            r#"{"other": true, "mcpServers": {"motte": {"command": "motte", "args": ["mcp"]}}}"#,
        )
        .unwrap();
        assert_eq!(
            install(InstallScope::Project, &target).unwrap(),
            Outcome::Added(target.clone())
        );
        let written: Value = serde_json::from_str(&fs::read_to_string(&target).unwrap()).unwrap();
        assert_eq!(written["other"], json!(true));
        assert_eq!(written["mcpServers"]["motte"]["command"], json!("motte"));
        assert_eq!(written["mcpServers"][SERVER_KEY]["args"], json!(["mcp"]));

        fs::write(
            &target,
            r#"{"mcpServers": {"merge-pipeline": {"command": "/old/path", "args": []}}}"#,
        )
        .unwrap();
        assert_eq!(
            install(InstallScope::Project, &target).unwrap(),
            Outcome::Updated(target.clone())
        );
    }

    #[test]
    fn rejects_files_that_are_not_objects() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join(".mcp.json");
        fs::write(&target, "[]").unwrap();
        assert!(install(InstallScope::Project, &target).is_err());
        fs::write(&target, r#"{"mcpServers": 3}"#).unwrap();
        assert!(install(InstallScope::Project, &target).is_err());
        fs::write(&target, "{not json").unwrap();
        assert!(install(InstallScope::Project, &target).is_err());
    }
}
