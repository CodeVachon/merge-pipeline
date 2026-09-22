//! Where workflow files come from.
//!
//! The baseline looked beside the executable, which does not fit a managed install under
//! `~/.merge-pipeline/versions/<v>/bin`. The search order is now:
//!
//! 1. `--config <PATH>`
//! 2. `$MERGE_PIPELINE_CONFIG`
//! 3. `<cwd>/.merge-pipeline/`
//! 4. `<user config dir>/merge-pipeline/` (`~/.config/merge-pipeline` on Linux and macOS)
//! 5. `<directory of the executable>/config` (the baseline layout, kept for compatibility)
//!
//! The first that exists as a directory wins. An explicit flag or env var that names a
//! missing directory is an error on its own rather than silently falling through.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// Environment variable consulted second.
pub const ENV_VAR: &str = "MERGE_PIPELINE_CONFIG";
/// Repo-local directory name consulted third.
pub const REPO_LOCAL_DIR: &str = ".merge-pipeline";
/// Sub-directory of the user config dir consulted fourth.
pub const USER_DIR_NAME: &str = "merge-pipeline";

/// Which rule produced (or was tried for) a location.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    Flag,
    EnvVar,
    RepoLocal,
    UserConfig,
    BesideExecutable,
}

impl Rule {
    pub fn describe(self) -> &'static str {
        match self {
            Rule::Flag => "--config flag",
            Rule::EnvVar => "$MERGE_PIPELINE_CONFIG",
            Rule::RepoLocal => "<cwd>/.merge-pipeline",
            Rule::UserConfig => "user config directory",
            Rule::BesideExecutable => "config directory beside the executable",
        }
    }
}

/// The winning location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub path: PathBuf,
    pub rule: Rule,
}

/// Nothing matched.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigDirError {
    #[error("{} names {} which is not a directory", rule.describe(), path.display())]
    NotADirectory { rule: Rule, path: PathBuf },
    #[error("No workflow directory found. Looked in:\n{}", describe_tried(tried))]
    NotFound { tried: Vec<(Rule, PathBuf)> },
}

fn describe_tried(tried: &[(Rule, PathBuf)]) -> String {
    tried
        .iter()
        .map(|(rule, path)| format!("  {} ({})", path.display(), rule.describe()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Inputs for [`resolve`], so tests can supply every source explicitly.
#[derive(Debug, Clone, Default)]
pub struct Sources<'a> {
    pub flag: Option<&'a Path>,
    pub env: Option<&'a OsStr>,
    pub cwd: Option<&'a Path>,
    pub user_config_dir: Option<&'a Path>,
    pub exe_dir: Option<&'a Path>,
}

/// Apply the search order to `sources`.
pub fn resolve(sources: &Sources<'_>) -> Result<Resolved, ConfigDirError> {
    if let Some(flag) = sources.flag {
        return require_dir(Rule::Flag, flag.to_path_buf());
    }
    if let Some(env) = sources.env.filter(|value| !value.is_empty()) {
        return require_dir(Rule::EnvVar, PathBuf::from(env));
    }

    let mut tried = Vec::new();
    if let Some(cwd) = sources.cwd {
        tried.push((Rule::RepoLocal, cwd.join(REPO_LOCAL_DIR)));
    }
    if let Some(user) = sources.user_config_dir {
        tried.push((Rule::UserConfig, user.join(USER_DIR_NAME)));
    }
    if let Some(exe_dir) = sources.exe_dir {
        tried.push((Rule::BesideExecutable, exe_dir.join("config")));
    }

    for (rule, path) in &tried {
        if path.is_dir() {
            return Ok(Resolved {
                path: path.clone(),
                rule: *rule,
            });
        }
    }
    Err(ConfigDirError::NotFound { tried })
}

fn require_dir(rule: Rule, path: PathBuf) -> Result<Resolved, ConfigDirError> {
    if path.is_dir() {
        Ok(Resolved { path, rule })
    } else {
        Err(ConfigDirError::NotADirectory { rule, path })
    }
}

/// [`resolve`] using the real process environment, current directory, user config dir and
/// executable location. `cwd` defaults to the process cwd.
pub fn resolve_from_process(
    flag: Option<&Path>,
    cwd: Option<&Path>,
) -> Result<Resolved, ConfigDirError> {
    let env = std::env::var_os(ENV_VAR);
    let process_cwd = std::env::current_dir().ok();
    let user = dirs::config_dir();
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf));

    resolve(&Sources {
        flag,
        env: env.as_deref(),
        cwd: cwd.or(process_cwd.as_deref()),
        user_config_dir: user.as_deref(),
        exe_dir: exe_dir.as_deref(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::fs;

    #[test]
    fn flag_wins_and_must_exist() {
        let temp = tempfile::tempdir().unwrap();
        let flag = temp.path().join("flag");
        fs::create_dir(&flag).unwrap();
        let cwd = temp.path().join("cwd");
        fs::create_dir_all(cwd.join(REPO_LOCAL_DIR)).unwrap();

        let resolved = resolve(&Sources {
            flag: Some(&flag),
            cwd: Some(&cwd),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(
            resolved,
            Resolved {
                path: flag.clone(),
                rule: Rule::Flag
            }
        );

        let missing = temp.path().join("nope");
        let err = resolve(&Sources {
            flag: Some(&missing),
            cwd: Some(&cwd),
            ..Default::default()
        })
        .unwrap_err();
        assert_eq!(
            err,
            ConfigDirError::NotADirectory {
                rule: Rule::Flag,
                path: missing
            }
        );
    }

    #[test]
    fn env_var_is_second_and_empty_value_is_ignored() {
        let temp = tempfile::tempdir().unwrap();
        let env_dir = temp.path().join("from-env");
        fs::create_dir(&env_dir).unwrap();
        let cwd = temp.path().join("cwd");
        fs::create_dir_all(cwd.join(REPO_LOCAL_DIR)).unwrap();

        let env = OsString::from(&env_dir);
        let resolved = resolve(&Sources {
            env: Some(&env),
            cwd: Some(&cwd),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(resolved.rule, Rule::EnvVar);
        assert_eq!(resolved.path, env_dir);

        let empty = OsString::new();
        let resolved = resolve(&Sources {
            env: Some(&empty),
            cwd: Some(&cwd),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(resolved.rule, Rule::RepoLocal);
    }

    #[test]
    fn falls_through_repo_local_user_then_exe_dir() {
        let temp = tempfile::tempdir().unwrap();
        let cwd = temp.path().join("cwd");
        let user = temp.path().join("user");
        let exe_dir = temp.path().join("bin");
        fs::create_dir_all(&cwd).unwrap();
        fs::create_dir_all(&user).unwrap();
        fs::create_dir_all(&exe_dir).unwrap();

        let sources = |cwd: &Path, user: &Path, exe: &Path| -> Sources<'static> {
            // Leak for the closure's convenience in tests only.
            Sources {
                cwd: Some(Box::leak(cwd.to_path_buf().into_boxed_path())),
                user_config_dir: Some(Box::leak(user.to_path_buf().into_boxed_path())),
                exe_dir: Some(Box::leak(exe.to_path_buf().into_boxed_path())),
                ..Default::default()
            }
        };

        let err = resolve(&sources(&cwd, &user, &exe_dir)).unwrap_err();
        let text = err.to_string();
        assert!(text.contains("No workflow directory found"));
        assert!(text.contains(&cwd.join(REPO_LOCAL_DIR).display().to_string()));
        assert!(text.contains(&user.join(USER_DIR_NAME).display().to_string()));
        assert!(text.contains(&exe_dir.join("config").display().to_string()));

        fs::create_dir_all(exe_dir.join("config")).unwrap();
        assert_eq!(
            resolve(&sources(&cwd, &user, &exe_dir)).unwrap().rule,
            Rule::BesideExecutable
        );

        fs::create_dir_all(user.join(USER_DIR_NAME)).unwrap();
        assert_eq!(
            resolve(&sources(&cwd, &user, &exe_dir)).unwrap().rule,
            Rule::UserConfig
        );

        fs::create_dir_all(cwd.join(REPO_LOCAL_DIR)).unwrap();
        let resolved = resolve(&sources(&cwd, &user, &exe_dir)).unwrap();
        assert_eq!(resolved.rule, Rule::RepoLocal);
        assert_eq!(resolved.path, cwd.join(REPO_LOCAL_DIR));
    }
}
