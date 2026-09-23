//! Thin wrapper over the `git` executable via std::process::Command.
//!
//! Ported from the baseline's `utl/GitApi.ts`. Command execution sits behind [`CommandRunner`] so
//! output parsing can be unit-tested with canned stdout, the way the baseline spied on `call`.

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Errors from running git.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GitError {
    /// git ran and exited non-zero.
    #[error("git {} failed{}: {}", render_args(args), status.map(|s| format!(" (exit {s})")).unwrap_or_default(), stderr.trim())]
    Command {
        args: Vec<String>,
        status: Option<i32>,
        stdout: String,
        stderr: String,
    },
    /// git could not be started at all.
    #[error("could not run git {}: {reason}", render_args(args))]
    Spawn { args: Vec<String>, reason: String },
    #[error("A branch name \"{0}\" already exists")]
    BranchExists(String),
}

impl GitError {
    /// True when stderr (or the message) says there is no upstream configured.
    pub fn is_missing_upstream(&self) -> bool {
        let haystack = match self {
            GitError::Command { stderr, .. } => stderr.to_lowercase(),
            other => other.to_string().to_lowercase(),
        };
        haystack.contains("no upstream configured")
    }
}

/// Raw result of one git invocation.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CommandOutput {
    pub status: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl CommandOutput {
    pub fn success(&self) -> bool {
        self.status == Some(0)
    }
}

/// Executes `git <args>` in a directory. Swap for a fake in tests.
pub trait CommandRunner {
    fn run(
        &mut self,
        cwd: &Path,
        args: &[String],
        envs: &[(String, String)],
    ) -> Result<CommandOutput, GitError>;
}

/// The real thing.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemRunner;

impl CommandRunner for SystemRunner {
    fn run(
        &mut self,
        cwd: &Path,
        args: &[String],
        envs: &[(String, String)],
    ) -> Result<CommandOutput, GitError> {
        let mut command = Command::new("git");
        command.args(args).current_dir(cwd);
        for (key, value) in envs {
            command.env(key, value);
        }
        let output = command.output().map_err(|error| GitError::Spawn {
            args: args.to_vec(),
            reason: error.to_string(),
        })?;
        Ok(CommandOutput {
            status: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

/// Receives the rendered `git ...` line for every call when verbose.
pub trait CommandLog {
    fn command(&mut self, rendered: &str);
}

impl<F: FnMut(&str)> CommandLog for F {
    fn command(&mut self, rendered: &str) {
        self(rendered)
    }
}

fn escape_arg(value: &str) -> String {
    if value.chars().any(char::is_whitespace) || value.contains('"') {
        serde_json::to_string(value).unwrap_or_else(|_| format!("{value:?}"))
    } else {
        value.to_string()
    }
}

/// `git a "b c"` — how a command is shown to the user.
pub fn render_args(args: &[String]) -> String {
    args.iter()
        .map(|arg| escape_arg(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

/// A git working directory.
pub struct Git {
    cwd: PathBuf,
    runner: Box<dyn CommandRunner>,
    envs: Vec<(String, String)>,
    log: Option<Box<dyn CommandLog>>,
}

impl fmt::Debug for Git {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Git")
            .field("cwd", &self.cwd)
            .field("envs", &self.envs)
            .field("verbose", &self.log.is_some())
            .finish()
    }
}

impl Git {
    /// Real git in `cwd`, quiet.
    pub fn new(cwd: impl Into<PathBuf>) -> Self {
        Self::with_runner(cwd, Box::new(SystemRunner))
    }

    /// Git in `cwd` with a custom [`CommandRunner`].
    pub fn with_runner(cwd: impl Into<PathBuf>, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            cwd: cwd.into(),
            runner,
            envs: Vec::new(),
            log: None,
        }
    }

    /// Log every command through `log` (the baseline's `verbose: true`).
    pub fn verbose(mut self, log: impl CommandLog + 'static) -> Self {
        self.log = Some(Box::new(log));
        self
    }

    /// Extra environment for every git call (tests use this to isolate global config).
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.envs.push((key.into(), value.into()));
        self
    }

    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    /// Run `git <args>`; returns trimmed stdout, or [`GitError::Command`] on non-zero exit.
    pub fn call<S: AsRef<str>>(&mut self, args: &[S]) -> Result<String, GitError> {
        let args: Vec<String> = args.iter().map(|arg| arg.as_ref().to_string()).collect();
        if let Some(log) = self.log.as_mut() {
            log.command(&format!("git {}", render_args(&args)));
        }
        let output = self.runner.run(&self.cwd, &args, &self.envs)?;
        if !output.success() {
            return Err(GitError::Command {
                args,
                status: output.status,
                stdout: output.stdout,
                stderr: output.stderr,
            });
        }
        Ok(output.stdout.trim_end().to_string())
    }

    pub fn fetch(&mut self) -> Result<String, GitError> {
        self.call(&["fetch"])
    }

    /// Local branch names, `*` marker stripped.
    pub fn branch_list(&mut self) -> Result<Vec<String>, GitError> {
        let list = self.call(&["branch"])?;
        Ok(list
            .lines()
            .map(|line| line.trim_start_matches('*').trim())
            .filter(|line| !line.is_empty() && *line != "*")
            .map(str::to_string)
            .collect())
    }

    pub fn current_branch(&mut self) -> Result<String, GitError> {
        self.call(&["rev-parse", "--abbrev-ref", "HEAD"])
    }

    pub fn checkout(&mut self, branch: &str) -> Result<String, GitError> {
        self.call(&["checkout", branch])
    }

    /// `git checkout -b`, refusing if the branch already exists.
    pub fn checkout_new(&mut self, branch: &str) -> Result<String, GitError> {
        if self
            .branch_list()?
            .iter()
            .any(|existing| existing == branch)
        {
            return Err(GitError::BranchExists(branch.to_string()));
        }
        self.call(&["checkout", "-b", branch])
    }

    /// Uncommitted changes, staged or not. Untracked files do not count.
    ///
    /// Deliberate change from the baseline, which used `git diff --stat` and therefore missed
    /// staged-but-uncommitted changes.
    pub fn is_dirty(&mut self) -> Result<bool, GitError> {
        let status = self.call(&["status", "--porcelain", "--untracked-files=no"])?;
        Ok(!status.trim().is_empty())
    }

    pub fn pull(&mut self) -> Result<String, GitError> {
        self.call(&["pull"])
    }

    pub fn merge(&mut self, source: &str) -> Result<String, GitError> {
        self.call(&["merge", source, "--no-verify"])
    }

    /// Whether the current branch has an upstream.
    pub fn has_upstream(&mut self) -> Result<bool, GitError> {
        match self.call(&["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]) {
            Ok(result) => Ok(!result.is_empty()),
            Err(error) if error.is_missing_upstream() => Ok(false),
            Err(error) => Err(error),
        }
    }

    /// Whether `branch` has an upstream; checks it out first (as the baseline did).
    pub fn branch_has_upstream(&mut self, branch: &str) -> Result<bool, GitError> {
        self.checkout(branch)?;
        self.has_upstream()
    }

    /// `git push --no-verify` on the current branch.
    pub fn push(&mut self) -> Result<String, GitError> {
        self.call(&["push", "--no-verify"])
    }

    /// `git push -u <remote> <current branch> --no-verify`.
    pub fn push_upstream(&mut self, remote: &str) -> Result<String, GitError> {
        let branch = self.current_branch()?;
        self.call(&["push", "-u", remote, &branch, "--no-verify"])
    }

    /// Paths with unresolved conflicts, deduplicated, in first-seen order.
    pub fn conflicted_paths(&mut self) -> Result<Vec<String>, GitError> {
        let output = self.call(&["ls-files", "--unmerged"])?;
        Ok(parse_unmerged(&output))
    }

    /// `git fetch --prune origin`: learn origin's state and drop remote-tracking refs for
    /// branches it no longer has, which is what makes stale local branches show as `[gone]`.
    pub fn fetch_prune(&mut self) -> Result<String, GitError> {
        self.call(&["fetch", "--prune", "origin"])
    }

    /// Branches on `origin`, from `git branch -r`. `origin/HEAD -> ...` and other remotes are skipped.
    pub fn remote_branches(&mut self) -> Result<Vec<RemoteBranch>, GitError> {
        let output = self.call(&["branch", "-r"])?;
        Ok(parse_remote_branches(&output))
    }

    /// Every local branch with its configured upstream and whether that upstream is gone.
    pub fn local_branch_tracking(&mut self) -> Result<Vec<BranchTracking>, GitError> {
        let output = self.call(&[
            "for-each-ref",
            "--format=%(refname:short)%09%(upstream:short)%09%(upstream:track)",
            "refs/heads/",
        ])?;
        Ok(parse_branch_tracking(&output))
    }

    /// `git branch --track <name> <remote_ref>` without checking it out.
    pub fn create_tracking_branch(
        &mut self,
        name: &str,
        remote_ref: &str,
    ) -> Result<String, GitError> {
        self.call(&["branch", "--track", name, remote_ref])
    }

    /// `git branch -D <name>` when `force`, else `-d` (refuses unmerged branches).
    pub fn delete_branch(&mut self, name: &str, force: bool) -> Result<String, GitError> {
        let flag = if force { "-D" } else { "-d" };
        self.call(&["branch", flag, name])
    }

    /// The branch `origin/HEAD` points at, or `None` when the symbolic ref is not set.
    pub fn remote_default_branch(&mut self) -> Result<Option<String>, GitError> {
        match self.call(&["symbolic-ref", "refs/remotes/origin/HEAD"]) {
            Ok(full) => Ok(full
                .trim()
                .strip_prefix("refs/remotes/origin/")
                .filter(|name| !name.is_empty())
                .map(str::to_string)),
            Err(GitError::Command { stderr, .. })
                if stderr.contains("not a symbolic ref") || stderr.contains("No such file") =>
            {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }
}

/// Parse `git ls-files --unmerged` output: `<mode> <sha> <stage>\t<path>` per line.
pub fn parse_unmerged(output: &str) -> Vec<String> {
    let mut paths: Vec<String> = Vec::new();
    for line in output.lines() {
        if line.is_empty() {
            continue;
        }
        let Some((meta, path)) = line.split_once('\t') else {
            continue;
        };
        if meta.split_whitespace().count() != 3 || path.is_empty() {
            continue;
        }
        if !paths.iter().any(|existing| existing == path) {
            paths.push(path.to_string());
        }
    }
    paths
}

/// One entry of `git branch -r` on origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteBranch {
    /// Branch name as it would be called locally, e.g. `Patch-v0.1.3`.
    pub name: String,
    /// The remote-tracking ref, e.g. `origin/Patch-v0.1.3`.
    pub remote_ref: String,
}

/// A local branch and the state of its configured upstream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchTracking {
    pub name: String,
    /// `origin/<name>` when an upstream is configured, `None` when the branch was never pushed.
    pub upstream: Option<String>,
    /// True when git reports the upstream as `[gone]`: it was configured but no longer exists.
    pub gone: bool,
}

/// Parse `git branch -r` output, keeping `origin/<name>` entries only.
pub fn parse_remote_branches(output: &str) -> Vec<RemoteBranch> {
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.contains(" -> "))
        .filter_map(|line| {
            let name = line.strip_prefix("origin/")?;
            (!name.is_empty()).then(|| RemoteBranch {
                name: name.to_string(),
                remote_ref: line.to_string(),
            })
        })
        .collect()
}

/// Parse `for-each-ref --format='%(refname:short)%09%(upstream:short)%09%(upstream:track)'`.
pub fn parse_branch_tracking(output: &str) -> Vec<BranchTracking> {
    output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let mut parts = line.splitn(3, '\t');
            let name = parts.next().unwrap_or_default().trim().to_string();
            let upstream = parts
                .next()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let track = parts.next().unwrap_or_default();
            BranchTracking {
                name,
                upstream,
                gone: track.contains("[gone]"),
            }
        })
        .filter(|entry| !entry.name.is_empty())
        .collect()
}

/// Test double: canned stdout keyed by the first git argument, every call recorded.
#[cfg(test)]
pub(crate) mod fakes {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;

    /// Answers `git <sub> ...` with the canned output registered for `sub` (or empty success)
    /// and records every argument vector in order.
    #[derive(Default)]
    pub(crate) struct ScriptedRunner {
        responses: HashMap<String, Vec<CommandOutput>>,
        pub(crate) calls: Rc<RefCell<Vec<Vec<String>>>>,
    }

    impl ScriptedRunner {
        pub(crate) fn new() -> Self {
            Self::default()
        }

        /// Queue a successful stdout for the next `git <sub>` call.
        pub(crate) fn stdout(mut self, sub: &str, stdout: &str) -> Self {
            self.responses
                .entry(sub.to_string())
                .or_default()
                .push(CommandOutput {
                    status: Some(0),
                    stdout: stdout.to_string(),
                    stderr: String::new(),
                });
            self
        }

        /// Queue a failure for the next `git <sub>` call.
        pub(crate) fn fail(mut self, sub: &str, status: i32, stderr: &str) -> Self {
            self.responses
                .entry(sub.to_string())
                .or_default()
                .push(CommandOutput {
                    status: Some(status),
                    stdout: String::new(),
                    stderr: stderr.to_string(),
                });
            self
        }

        pub(crate) fn calls_handle(&self) -> Rc<RefCell<Vec<Vec<String>>>> {
            Rc::clone(&self.calls)
        }

        pub(crate) fn into_git(self) -> Git {
            Git::with_runner("/nowhere", Box::new(self))
        }
    }

    impl CommandRunner for ScriptedRunner {
        fn run(
            &mut self,
            _cwd: &Path,
            args: &[String],
            _envs: &[(String, String)],
        ) -> Result<CommandOutput, GitError> {
            self.calls.borrow_mut().push(args.to_vec());
            let sub = args.first().cloned().unwrap_or_default();
            let queued = self.responses.get_mut(&sub).and_then(|queue| {
                if queue.is_empty() {
                    None
                } else {
                    Some(queue.remove(0))
                }
            });
            Ok(queued.unwrap_or(CommandOutput {
                status: Some(0),
                ..Default::default()
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fakes::ScriptedRunner;
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    // Ported from utl/GitApi.test.ts — "returns an empty list when there are no conflicted files"
    #[test]
    fn conflicted_paths_returns_an_empty_list_when_there_are_no_conflicted_files() {
        let runner = ScriptedRunner::new().stdout("ls-files", "");
        let calls = runner.calls_handle();
        let mut git = runner.into_git();
        assert_eq!(git.conflicted_paths().unwrap(), Vec::<String>::new());
        assert_eq!(calls.borrow()[0], vec!["ls-files", "--unmerged"]);
    }

    // Ported — "returns unique conflicted paths and ignores trailing newlines"
    #[test]
    fn conflicted_paths_returns_unique_paths_and_ignores_trailing_newlines() {
        let mut git = ScriptedRunner::new()
            .stdout(
                "ls-files",
                &[
                    "100644 b8fbae9220c6b32ea09ae004e6d63d0f77bc73fd 1\tpackage.json",
                    "100644 935b2491ee062efa54ed025f9ceaff2c89336de7 2\tpackage.json",
                    "100644 88f49c1abba08aae22f0088db28a3e43636eb313 3\tapps/frontend/package.json",
                    "",
                ]
                .join("\n"),
            )
            .into_git();
        assert_eq!(
            git.conflicted_paths().unwrap(),
            vec!["package.json", "apps/frontend/package.json"]
        );
    }

    // Ported — "preserves conflicted paths containing spaces"
    #[test]
    fn conflicted_paths_preserves_paths_containing_spaces() {
        let mut git = ScriptedRunner::new()
            .stdout(
                "ls-files",
                "100644 88f49c1abba08aae22f0088db28a3e43636eb313 3\tapps/mobile/My Package/package.json",
            )
            .into_git();
        assert_eq!(
            git.conflicted_paths().unwrap(),
            vec!["apps/mobile/My Package/package.json"]
        );
    }

    #[test]
    fn branch_list_strips_the_current_marker_and_blank_lines() {
        let mut git = ScriptedRunner::new()
            .stdout("branch", "  main\n* staging-patch\n\n  Release-0.1.0\n")
            .into_git();
        assert_eq!(
            git.branch_list().unwrap(),
            vec!["main", "staging-patch", "Release-0.1.0"]
        );
    }

    #[test]
    fn verbose_log_quotes_arguments_with_whitespace() {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&seen);
        let mut git = ScriptedRunner::new()
            .into_git()
            .verbose(move |line: &str| sink.borrow_mut().push(line.to_string()));
        git.call(&["commit", "-m", "hello world", "--no-verify"])
            .unwrap();
        assert_eq!(
            seen.borrow()[0],
            "git commit -m \"hello world\" --no-verify"
        );
    }

    #[test]
    fn missing_upstream_maps_to_false_and_other_failures_are_errors() {
        let mut git = ScriptedRunner::new()
            .fail(
                "rev-parse",
                128,
                "fatal: no upstream configured for branch 'x'\n",
            )
            .into_git();
        assert!(!git.has_upstream().unwrap());

        let mut git = ScriptedRunner::new().fail("fetch", 1, "boom").into_git();
        let error = git.fetch().unwrap_err();
        assert!(matches!(
            error,
            GitError::Command {
                status: Some(1),
                ..
            }
        ));
        assert!(error.to_string().contains("boom"));
    }
    #[test]
    fn remote_branches_skips_the_head_pointer_and_other_remotes() {
        let mut git = ScriptedRunner::new()
            .stdout(
                "branch",
                "  origin/HEAD -> origin/main\n  origin/main\n  origin/Patch-v0.1.3\n  upstream/main\n\n",
            )
            .into_git();
        assert_eq!(
            git.remote_branches().unwrap(),
            vec![
                RemoteBranch {
                    name: "main".into(),
                    remote_ref: "origin/main".into()
                },
                RemoteBranch {
                    name: "Patch-v0.1.3".into(),
                    remote_ref: "origin/Patch-v0.1.3".into()
                },
            ]
        );
    }

    #[test]
    fn branch_tracking_reads_upstream_and_gone_from_for_each_ref() {
        let runner = ScriptedRunner::new().stdout(
            "for-each-ref",
            "main\torigin/main\t\nPatch-v0.1.1\torigin/Patch-v0.1.1\t[gone]\nPatch-v0.1.2\t\t\nRelease-0.1.0\torigin/Release-0.1.0\t[ahead 1]\n",
        );
        let calls = runner.calls_handle();
        let mut git = runner.into_git();
        let tracking = git.local_branch_tracking().unwrap();
        assert_eq!(
            tracking,
            vec![
                BranchTracking {
                    name: "main".into(),
                    upstream: Some("origin/main".into()),
                    gone: false
                },
                BranchTracking {
                    name: "Patch-v0.1.1".into(),
                    upstream: Some("origin/Patch-v0.1.1".into()),
                    gone: true
                },
                BranchTracking {
                    name: "Patch-v0.1.2".into(),
                    upstream: None,
                    gone: false
                },
                BranchTracking {
                    name: "Release-0.1.0".into(),
                    upstream: Some("origin/Release-0.1.0".into()),
                    gone: false
                },
            ]
        );
        assert_eq!(
            calls.borrow()[0],
            vec![
                "for-each-ref",
                "--format=%(refname:short)%09%(upstream:short)%09%(upstream:track)",
                "refs/heads/"
            ]
        );
    }

    #[test]
    fn sync_primitives_issue_the_expected_git_commands() {
        let runner = ScriptedRunner::new().stdout("symbolic-ref", "refs/remotes/origin/main\n");
        let calls = runner.calls_handle();
        let mut git = runner.into_git();
        git.fetch_prune().unwrap();
        git.create_tracking_branch("Patch-v0.1.3", "origin/Patch-v0.1.3")
            .unwrap();
        git.delete_branch("Patch-v0.1.1", true).unwrap();
        git.delete_branch("banana", false).unwrap();
        assert_eq!(git.remote_default_branch().unwrap(), Some("main".into()));
        let calls = calls.borrow();
        assert_eq!(calls[0], vec!["fetch", "--prune", "origin"]);
        assert_eq!(
            calls[1],
            vec!["branch", "--track", "Patch-v0.1.3", "origin/Patch-v0.1.3"]
        );
        assert_eq!(calls[2], vec!["branch", "-D", "Patch-v0.1.1"]);
        assert_eq!(calls[3], vec!["branch", "-d", "banana"]);
    }

    #[test]
    fn remote_default_branch_is_none_when_origin_head_is_unset() {
        let mut git = ScriptedRunner::new()
            .fail(
                "symbolic-ref",
                128,
                "fatal: ref refs/remotes/origin/HEAD is not a symbolic ref\n",
            )
            .into_git();
        assert_eq!(git.remote_default_branch().unwrap(), None);
    }
}
