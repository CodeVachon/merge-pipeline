//! `config init`: write the default workflow files so a fresh repository works immediately.
//!
//! The defaults are the four example workflows shipped in `examples/config/`, embedded at compile
//! time so the binary carries them and the examples cannot drift from what init writes. Existing
//! files are never overwritten unless asked, so re-running is safe.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, anyhow};
use serde::Serialize;

use super::InstallScope;
use super::config_dir::{REPO_LOCAL_DIR, USER_DIR_NAME};

/// The embedded defaults, in the order they are written.
pub const DEFAULTS: &[(&str, &str)] = &[
    (
        "patch.json",
        include_str!("../../examples/config/patch.json"),
    ),
    (
        "minor.json",
        include_str!("../../examples/config/minor.json"),
    ),
    (
        "canary.json",
        include_str!("../../examples/config/canary.json"),
    ),
    (
        "default.json",
        include_str!("../../examples/config/default.json"),
    ),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    Created,
    Kept,
    Overwritten,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileReport {
    pub path: PathBuf,
    pub outcome: Outcome,
}

/// What `write_defaults` did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InitReport {
    pub dir: PathBuf,
    pub files: Vec<FileReport>,
}

impl InitReport {
    /// True when nothing on disk changed.
    pub fn unchanged(&self) -> bool {
        self.files.iter().all(|file| file.outcome == Outcome::Kept)
    }
}

/// The directory `scope` initialises: `<cwd>/.merge-pipeline` or the user config directory.
pub fn target_dir(scope: InstallScope, cwd: &Path) -> anyhow::Result<PathBuf> {
    match scope {
        InstallScope::Project => Ok(cwd.join(REPO_LOCAL_DIR)),
        InstallScope::User => Ok(dirs::config_dir()
            .ok_or_else(|| anyhow!("could not determine the user config directory"))?
            .join(USER_DIR_NAME)),
    }
}

/// Create `dir` and write every default into it. Files already present are kept unless `force`.
pub fn write_defaults(dir: &Path, force: bool) -> anyhow::Result<InitReport> {
    fs::create_dir_all(dir).with_context(|| format!("could not create {}", dir.display()))?;

    let mut files = Vec::with_capacity(DEFAULTS.len());
    for (name, contents) in DEFAULTS {
        let path = dir.join(name);
        let existed = path.exists();
        let outcome = match (existed, force) {
            (true, false) => Outcome::Kept,
            (true, true) => Outcome::Overwritten,
            (false, _) => Outcome::Created,
        };
        if outcome != Outcome::Kept {
            fs::write(&path, contents)
                .with_context(|| format!("could not write {}", path.display()))?;
        }
        files.push(FileReport { path, outcome });
    }

    Ok(InitReport {
        dir: dir.to_path_buf(),
        files,
    })
}

/// Human-readable lines for the report.
pub fn render(report: &InitReport) -> String {
    let mut out = String::new();
    for file in &report.files {
        let verb = match file.outcome {
            Outcome::Created => "created",
            Outcome::Kept => "kept   ",
            Outcome::Overwritten => "rewrote",
        };
        out.push_str(&format!("{verb} {}\n", file.path.display()));
    }
    if report.unchanged() {
        out.push_str(&format!(
            "{} already has every default workflow; nothing changed\n",
            report.dir.display()
        ));
    } else {
        out.push_str(&format!(
            "workflow directory ready at {} (run `merge-pipeline config doctor` to check it)\n",
            report.dir.display()
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example(name: &str) -> String {
        fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("examples/config")
                .join(name),
        )
        .unwrap()
    }

    #[test]
    fn creates_the_four_examples_byte_for_byte() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("nested").join(".merge-pipeline");
        let report = write_defaults(&dir, false).unwrap();

        assert_eq!(report.dir, dir);
        assert_eq!(report.files.len(), 4);
        assert!(
            report
                .files
                .iter()
                .all(|file| file.outcome == Outcome::Created)
        );
        for (name, _) in DEFAULTS {
            assert_eq!(fs::read_to_string(dir.join(name)).unwrap(), example(name));
        }
        assert!(!report.unchanged());
    }

    #[test]
    fn second_run_keeps_files_and_force_rewrites_them() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().to_path_buf();
        write_defaults(&dir, false).unwrap();
        fs::write(
            dir.join("patch.json"),
            "{\"name\":\"mine\",\"pipeline\":[\"a\",\"b\"]}",
        )
        .unwrap();

        let report = write_defaults(&dir, false).unwrap();
        assert!(report.unchanged());
        assert!(
            fs::read_to_string(dir.join("patch.json"))
                .unwrap()
                .contains("mine")
        );

        let report = write_defaults(&dir, true).unwrap();
        assert!(
            report
                .files
                .iter()
                .all(|file| file.outcome == Outcome::Overwritten)
        );
        assert_eq!(
            fs::read_to_string(dir.join("patch.json")).unwrap(),
            example("patch.json")
        );
    }

    #[test]
    fn project_scope_targets_the_repo_local_directory() {
        let cwd = Path::new("/some/repo");
        assert_eq!(
            target_dir(InstallScope::Project, cwd).unwrap(),
            PathBuf::from("/some/repo/.merge-pipeline")
        );
    }

    #[test]
    fn render_names_each_file_and_the_verdict() {
        let temp = tempfile::tempdir().unwrap();
        let report = write_defaults(temp.path(), false).unwrap();
        let text = render(&report);
        assert!(text.contains("created "));
        assert!(text.contains("patch.json"));
        assert!(text.contains("workflow directory ready at"));

        let report = write_defaults(temp.path(), false).unwrap();
        let text = render(&report);
        assert!(text.contains("kept    "));
        assert!(text.contains("nothing changed"));
    }
}
