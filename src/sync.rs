//! Branch sync: bring local branches in line with `origin` before a workflow runs.
//!
//! The recurring problem this solves: a `Patch-*` branch is merged and deleted on origin, so the
//! next run either picks the dead local branch or cannot see the new one. Sync runs right after
//! `git fetch --prune origin`, before pattern → branch mapping, and does two things for every
//! pattern in the pipeline:
//!
//! 1. Local branches whose configured upstream is `[gone]` are *stale*. Each is offered for
//!    deletion (prompt key `sync:delete:<branch>`), or handled per [`StalePolicy`] when there is
//!    no human. A branch that never had an upstream is never stale — it was never pushed, so it
//!    is not ours to clean up.
//! 2. Remote branches with no local counterpart are *new*; a local tracking branch is created for
//!    each, without asking, because that is non-destructive.
//!
//! Planning ([`plan_sync`]) is pure so it can be unit-tested; applying ([`apply_sync`]) talks to
//! git and the prompter and reports through the runner's event sink.

use crate::git::{BranchTracking, Git, GitError, RemoteBranch};
use crate::pipeline::{PipelineError, pattern_matches};
use crate::prompt::{PromptError, Prompter, Question};

/// What to do with a stale local branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StalePolicy {
    /// Ask the prompter per branch (default yes). What the CLI does; `--yes` answers yes.
    #[default]
    Ask,
    /// Leave every stale branch alone, but still report it. The MCP default.
    Keep,
    /// Delete every stale branch without asking.
    Delete,
}

/// How sync should behave. `None` in [`crate::runner::Settings`] skips sync entirely.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncOptions {
    pub stale: StalePolicy,
    /// Create local tracking branches for new remote branches that match a pattern.
    pub fetch_new: bool,
}

impl Default for SyncOptions {
    fn default() -> Self {
        Self {
            stale: StalePolicy::Ask,
            fetch_new: true,
        }
    }
}

/// What sync would do, before doing it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SyncPlan {
    /// Local branches matching a pattern whose upstream is gone.
    pub stale: Vec<String>,
    /// Remote branches matching a pattern with no local branch of that name.
    pub new_remote: Vec<RemoteBranch>,
}

impl SyncPlan {
    pub fn is_empty(&self) -> bool {
        self.stale.is_empty() && self.new_remote.is_empty()
    }
}

/// What sync did.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SyncReport {
    /// Every stale branch found, deleted or not.
    pub stale: Vec<String>,
    pub deleted: Vec<String>,
    pub kept: Vec<String>,
    /// Local tracking branches created for new remote branches.
    pub created: Vec<String>,
}

/// Progress events, forwarded to the runner's sink.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncEvent {
    Started,
    /// A stale branch was found; `deleted` says what happened to it.
    StaleBranch {
        branch: String,
        deleted: bool,
    },
    /// A local tracking branch was created for a new remote branch.
    BranchCreated {
        branch: String,
        upstream: String,
    },
    /// Something non-fatal that the user should know about.
    Warning {
        message: String,
    },
    /// Sync did not run.
    Skipped {
        reason: String,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error(transparent)]
    Git(#[from] GitError),
    #[error(transparent)]
    Pipeline(#[from] PipelineError),
    #[error(transparent)]
    Prompt(#[from] PromptError),
}

/// The prompt key for deleting `branch`.
pub fn delete_question_key(branch: &str) -> String {
    format!("sync:delete:{branch}")
}

fn matches_any(patterns: &[String], name: &str) -> Result<bool, PipelineError> {
    for pattern in patterns {
        if pattern_matches(pattern, name)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Decide what to sync. Pure: takes git's answers, returns the plan.
pub fn plan_sync(
    patterns: &[String],
    tracking: &[BranchTracking],
    remote: &[RemoteBranch],
) -> Result<SyncPlan, PipelineError> {
    let mut plan = SyncPlan::default();

    for local in tracking {
        if local.gone && local.upstream.is_some() && matches_any(patterns, &local.name)? {
            plan.stale.push(local.name.clone());
        }
    }

    for candidate in remote {
        let exists_locally = tracking.iter().any(|local| local.name == candidate.name);
        if !exists_locally && matches_any(patterns, &candidate.name)? {
            plan.new_remote.push(candidate.clone());
        }
    }

    Ok(plan)
}

/// Where to go before deleting the checked-out branch: origin's default branch if known, else a
/// local `main` or `master`.
fn safe_branch(git: &mut Git, avoid: &str) -> Result<Option<String>, GitError> {
    let local = git.branch_list()?;
    let mut candidates: Vec<String> = Vec::new();
    if let Some(default) = git.remote_default_branch()? {
        candidates.push(default);
    }
    candidates.push("main".to_string());
    candidates.push("master".to_string());
    Ok(candidates
        .into_iter()
        .find(|name| name != avoid && local.iter().any(|b| b == name)))
}

fn should_delete(
    branch: &str,
    policy: StalePolicy,
    prompter: &mut dyn Prompter,
) -> Result<bool, PromptError> {
    match policy {
        StalePolicy::Keep => Ok(false),
        StalePolicy::Delete => Ok(true),
        StalePolicy::Ask => prompter.confirm(
            &Question::new(
                delete_question_key(branch),
                format!("Local branch {branch} no longer exists on origin. Delete it?"),
            ),
            true,
        ),
    }
}

/// Carry out `plan`. Assumes `git fetch --prune origin` has already run.
pub fn apply_sync(
    git: &mut Git,
    plan: &SyncPlan,
    options: SyncOptions,
    prompter: &mut dyn Prompter,
    mut emit: impl FnMut(SyncEvent),
) -> Result<SyncReport, SyncError> {
    emit(SyncEvent::Started);
    let mut report = SyncReport::default();

    for branch in &plan.stale {
        report.stale.push(branch.clone());
        let mut delete = should_delete(branch, options.stale, prompter)?;

        if delete && git.current_branch()? == *branch {
            match safe_branch(git, branch)? {
                Some(target) => {
                    git.checkout(&target)?;
                }
                None => {
                    emit(SyncEvent::Warning {
                        message: format!(
                            "{branch} is checked out and no default branch was found to switch to; keeping it"
                        ),
                    });
                    delete = false;
                }
            }
        }

        if delete {
            git.delete_branch(branch, true)?;
            report.deleted.push(branch.clone());
        } else {
            report.kept.push(branch.clone());
        }
        emit(SyncEvent::StaleBranch {
            branch: branch.clone(),
            deleted: delete,
        });
    }

    if options.fetch_new {
        for remote in &plan.new_remote {
            git.create_tracking_branch(&remote.name, &remote.remote_ref)?;
            report.created.push(remote.name.clone());
            emit(SyncEvent::BranchCreated {
                branch: remote.name.clone(),
                upstream: remote.remote_ref.clone(),
            });
        }
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tracked(name: &str, upstream: Option<&str>, gone: bool) -> BranchTracking {
        BranchTracking {
            name: name.into(),
            upstream: upstream.map(str::to_string),
            gone,
        }
    }

    fn remote(name: &str) -> RemoteBranch {
        RemoteBranch {
            name: name.into(),
            remote_ref: format!("origin/{name}"),
        }
    }

    fn patterns(list: &[&str]) -> Vec<String> {
        list.iter().map(|p| p.to_string()).collect()
    }

    #[test]
    fn stale_means_matching_a_pattern_and_upstream_gone() {
        let tracking = vec![
            tracked("main", Some("origin/main"), false),
            tracked("staging-patch", Some("origin/staging-patch"), false),
            tracked("Patch-v0.1.1", Some("origin/Patch-v0.1.1"), true),
            tracked("Patch-v0.1.2", None, false),
            tracked("feature/x", Some("origin/feature/x"), true),
        ];
        let plan = plan_sync(&patterns(&["^Patch-*", "^staging-patch$"]), &tracking, &[]).unwrap();
        assert_eq!(plan.stale, vec!["Patch-v0.1.1"]);
        assert!(plan.new_remote.is_empty());
    }

    #[test]
    fn never_pushed_branches_are_never_stale_even_if_marked_gone() {
        // Defensive: `gone` without an upstream cannot come from git, but guard it anyway.
        let tracking = vec![tracked("Patch-v0.1.2", None, true)];
        let plan = plan_sync(&patterns(&["^Patch-*"]), &tracking, &[]).unwrap();
        assert!(plan.stale.is_empty());
    }

    #[test]
    fn new_remote_branches_are_those_matching_with_no_local_counterpart() {
        let tracking = vec![
            tracked("main", Some("origin/main"), false),
            tracked("Patch-v0.1.2", None, false),
        ];
        let remote = vec![
            remote("main"),
            remote("Patch-v0.1.2"),
            remote("Patch-v0.1.3"),
            remote("PATCH-v0.1.4"),
            remote("Release-9.9.9"),
        ];
        let plan = plan_sync(&patterns(&["^Patch-*"]), &tracking, &remote).unwrap();
        assert_eq!(
            plan.new_remote
                .iter()
                .map(|r| r.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Patch-v0.1.3", "PATCH-v0.1.4"],
            "matching is case-insensitive and existing locals are skipped"
        );
    }

    #[test]
    fn invalid_pattern_is_an_error_and_empty_inputs_are_an_empty_plan() {
        assert!(
            plan_sync(
                &patterns(&["(unclosed"]),
                &[tracked("a", Some("o/a"), true)],
                &[]
            )
            .is_err()
        );
        let plan = plan_sync(&patterns(&["^Patch-*"]), &[], &[]).unwrap();
        assert!(plan.is_empty());
    }

    #[test]
    fn delete_question_key_follows_the_vocabulary() {
        assert_eq!(
            delete_question_key("Patch-v0.1.1"),
            "sync:delete:Patch-v0.1.1"
        );
    }
}
