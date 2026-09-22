//! Integration tests for the git wrapper against a real, hermetic repository.
mod common;

use common::{Fixture, LOCAL_ONLY_BRANCHES, REMOTE_BRANCHES};

#[test]
fn branch_list_contains_the_bootstrap_branch_set() {
    let fixture = Fixture::new();
    let mut git = fixture.git();
    let branches = git.branch_list().unwrap();
    for branch in REMOTE_BRANCHES.iter().chain(LOCAL_ONLY_BRANCHES) {
        assert!(branches.iter().any(|b| b == branch), "missing {branch}");
    }
    assert_eq!(git.current_branch().unwrap(), "main");
}

#[test]
fn has_upstream_is_true_for_pushed_and_false_for_local_only_branches() {
    let fixture = Fixture::new();
    let mut git = fixture.git();
    assert!(git.branch_has_upstream("staging-patch").unwrap());
    assert!(!git.branch_has_upstream("Patch-v0.1.2").unwrap());
    assert_eq!(git.current_branch().unwrap(), "Patch-v0.1.2");
}

#[test]
fn is_dirty_sees_unstaged_and_staged_changes_but_not_untracked_files() {
    let fixture = Fixture::new();
    let mut git = fixture.git();
    assert!(!git.is_dirty().unwrap());

    fixture.write("untracked.txt", "new");
    assert!(!git.is_dirty().unwrap(), "untracked files do not count");

    fixture.write("README.md", "# changed\n");
    assert!(git.is_dirty().unwrap(), "unstaged change counts");

    fixture.raw(&fixture.work, &["add", "README.md"]);
    assert!(
        git.is_dirty().unwrap(),
        "staged change counts (baseline missed this)"
    );
}

#[test]
fn merge_and_push_land_commits_on_origin() {
    let fixture = Fixture::new();
    fixture.commit_on("Patch-v0.1.1", "fix.txt", "fixed\n", "a fix");
    let mut git = fixture.git();

    git.checkout("staging-patch").unwrap();
    git.merge("Patch-v0.1.1").unwrap();
    assert_eq!(fixture.read("fix.txt"), "fixed\n");
    assert_ne!(
        fixture.local_head("staging-patch"),
        fixture.origin_head("staging-patch")
    );

    git.push().unwrap();
    assert_eq!(
        fixture.local_head("staging-patch"),
        fixture.origin_head("staging-patch")
    );
}

#[test]
fn conflicting_merge_fails_and_reports_the_conflicted_path() {
    let fixture = Fixture::new();
    fixture.commit_on("Patch-v0.1.1", "README.md", "# patch side\n", "patch");
    fixture.commit_on("staging-patch", "README.md", "# staging side\n", "staging");
    let mut git = fixture.git();

    git.checkout("staging-patch").unwrap();
    let error = git.merge("Patch-v0.1.1").unwrap_err();
    assert!(error.to_string().contains("merge"), "{error}");
    assert_eq!(git.conflicted_paths().unwrap(), vec!["README.md"]);
}

#[test]
fn checkout_new_refuses_an_existing_branch() {
    let fixture = Fixture::new();
    let mut git = fixture.git();
    let error = git.checkout_new("main").unwrap_err();
    assert_eq!(error.to_string(), "A branch name \"main\" already exists");
    git.checkout_new("fresh").unwrap();
    assert_eq!(git.current_branch().unwrap(), "fresh");
}
