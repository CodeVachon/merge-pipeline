//! Hermetic git fixtures for integration tests. Nothing here touches the network.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

use merge_pipeline::git::Git;
use tempfile::TempDir;

/// Env that isolates git from the user's global/system config, hooks and signing.
pub fn isolation_env(hooks_dir: &Path) -> Vec<(String, String)> {
    vec![
        ("GIT_CONFIG_GLOBAL".into(), "/dev/null".into()),
        ("GIT_CONFIG_NOSYSTEM".into(), "1".into()),
        ("GIT_AUTHOR_NAME".into(), "Fixture".into()),
        ("GIT_AUTHOR_EMAIL".into(), "fixture@example.com".into()),
        ("GIT_COMMITTER_NAME".into(), "Fixture".into()),
        ("GIT_COMMITTER_EMAIL".into(), "fixture@example.com".into()),
        ("GIT_TERMINAL_PROMPT".into(), "0".into()),
        ("GIT_CONFIG_COUNT".into(), "3".into()),
        ("GIT_CONFIG_KEY_0".into(), "commit.gpgsign".into()),
        ("GIT_CONFIG_VALUE_0".into(), "false".into()),
        ("GIT_CONFIG_KEY_1".into(), "core.hooksPath".into()),
        (
            "GIT_CONFIG_VALUE_1".into(),
            hooks_dir.to_string_lossy().into_owned(),
        ),
        ("GIT_CONFIG_KEY_2".into(), "init.defaultBranch".into()),
        ("GIT_CONFIG_VALUE_2".into(), "main".into()),
    ]
}

/// A work repo with a local bare `origin` and the baseline `bootstrap.sh` branch set.
pub struct Fixture {
    pub temp: TempDir,
    pub work: PathBuf,
    pub origin: PathBuf,
    pub hooks: PathBuf,
}

/// Branches pushed to origin with an upstream configured.
pub const REMOTE_BRANCHES: &[&str] = &[
    "main",
    "staging-patch",
    "staging-release",
    "staging-canary",
    "Release-0.1.0",
    "Patch-v0.1.1",
];

/// Branches that exist only locally.
pub const LOCAL_ONLY_BRANCHES: &[&str] = &["Patch-v0.1.2", "Release-0.2.0", "banana"];

impl Fixture {
    pub fn new() -> Self {
        let temp = tempfile::tempdir().expect("temp dir");
        let work = temp.path().join("work");
        let origin = temp.path().join("origin.git");
        let hooks = temp.path().join("no-hooks");
        std::fs::create_dir_all(&work).unwrap();
        std::fs::create_dir_all(&hooks).unwrap();

        let fixture = Self {
            temp,
            work,
            origin,
            hooks,
        };

        fixture.raw(fixture.temp.path(), &["init", "--bare", "origin.git"]);
        fixture.raw(&fixture.work, &["init", "--initial-branch=main"]);
        fixture.raw(
            &fixture.work,
            &["remote", "add", "origin", fixture.origin.to_str().unwrap()],
        );
        fixture.write("README.md", "# fixture\n");
        fixture.commit_all("initial");
        fixture.raw(&fixture.work, &["push", "-u", "origin", "main"]);

        for branch in REMOTE_BRANCHES.iter().skip(1) {
            fixture.raw(&fixture.work, &["checkout", "-b", branch, "main"]);
            fixture.raw(&fixture.work, &["push", "-u", "origin", branch]);
        }
        for branch in LOCAL_ONLY_BRANCHES {
            fixture.raw(&fixture.work, &["branch", branch, "main"]);
        }
        fixture.raw(&fixture.work, &["checkout", "main"]);
        fixture
    }

    /// Run git directly (fixture setup), panicking on failure.
    pub fn raw(&self, cwd: &Path, args: &[&str]) -> String {
        let mut command = Command::new("git");
        command.args(args).current_dir(cwd);
        for (key, value) in isolation_env(&self.hooks) {
            command.env(key, value);
        }
        let output = command.output().expect("git runs");
        assert!(
            output.status.success(),
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout)
            .trim_end()
            .to_string()
    }

    /// A [`Git`] on the work repo with isolation env applied.
    pub fn git(&self) -> Git {
        let mut git = Git::new(&self.work);
        for (key, value) in isolation_env(&self.hooks) {
            git = git.env(key, value);
        }
        git
    }

    pub fn write(&self, relative: &str, contents: &str) {
        let path = self.work.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, contents).unwrap();
    }

    pub fn read(&self, relative: &str) -> String {
        std::fs::read_to_string(self.work.join(relative)).unwrap()
    }

    pub fn commit_all(&self, message: &str) {
        self.raw(&self.work, &["add", "-A"]);
        self.raw(&self.work, &["commit", "-m", message]);
    }

    /// Check out `branch`, write `relative`, commit. Returns to the previous branch? No — stays.
    pub fn commit_on(&self, branch: &str, relative: &str, contents: &str, message: &str) {
        self.raw(&self.work, &["checkout", branch]);
        self.write(relative, contents);
        self.commit_all(message);
    }

    pub fn current_branch(&self) -> String {
        self.raw(&self.work, &["rev-parse", "--abbrev-ref", "HEAD"])
    }

    /// Commit hash of `branch` as origin sees it.
    pub fn origin_head(&self, branch: &str) -> String {
        self.raw(&self.origin, &["rev-parse", branch])
    }

    pub fn local_head(&self, branch: &str) -> String {
        self.raw(&self.work, &["rev-parse", branch])
    }
}
