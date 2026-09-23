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

#[test]
fn fetch_prune_marks_a_branch_deleted_on_origin_as_gone() {
    let fixture = Fixture::new();
    fixture.origin_delete_branch("Patch-v0.1.1");
    let mut git = fixture.git();
    git.fetch_prune().unwrap();

    let tracking = git.local_branch_tracking().unwrap();
    let find = |name: &str| tracking.iter().find(|t| t.name == name).unwrap().clone();

    let stale = find("Patch-v0.1.1");
    assert_eq!(stale.upstream.as_deref(), Some("origin/Patch-v0.1.1"));
    assert!(stale.gone, "deleted on origin must show as gone");

    let live = find("staging-patch");
    assert_eq!(live.upstream.as_deref(), Some("origin/staging-patch"));
    assert!(!live.gone);

    let never_pushed = find("Patch-v0.1.2");
    assert_eq!(never_pushed.upstream, None);
    assert!(!never_pushed.gone, "no upstream is not the same as gone");
}

#[test]
fn a_new_origin_branch_is_listed_and_can_be_tracked_locally() {
    let fixture = Fixture::new();
    fixture.origin_add_branch("Patch-v0.1.3", "main");
    let mut git = fixture.git();

    assert!(
        !git.remote_branches()
            .unwrap()
            .iter()
            .any(|r| r.name == "Patch-v0.1.3"),
        "not visible before fetching"
    );
    git.fetch_prune().unwrap();
    let remote = git.remote_branches().unwrap();
    let new = remote.iter().find(|r| r.name == "Patch-v0.1.3").unwrap();
    assert_eq!(new.remote_ref, "origin/Patch-v0.1.3");
    assert!(
        !git.branch_list()
            .unwrap()
            .iter()
            .any(|b| b == "Patch-v0.1.3")
    );

    git.create_tracking_branch(&new.name, &new.remote_ref)
        .unwrap();
    assert!(
        git.branch_list()
            .unwrap()
            .iter()
            .any(|b| b == "Patch-v0.1.3")
    );
    let tracking = git.local_branch_tracking().unwrap();
    let created = tracking.iter().find(|t| t.name == "Patch-v0.1.3").unwrap();
    assert_eq!(created.upstream.as_deref(), Some("origin/Patch-v0.1.3"));
    assert!(!created.gone);
    assert_eq!(
        git.current_branch().unwrap(),
        "main",
        "no checkout happened"
    );
}

#[test]
fn delete_branch_needs_force_for_unmerged_work() {
    let fixture = Fixture::new();
    fixture.commit_on("Patch-v0.1.2", "extra.txt", "x\n", "unmerged work");
    fixture.raw(&fixture.work, &["checkout", "main"]);
    let mut git = fixture.git();

    assert!(git.delete_branch("Patch-v0.1.2", false).is_err());
    assert!(
        git.branch_list()
            .unwrap()
            .iter()
            .any(|b| b == "Patch-v0.1.2")
    );

    git.delete_branch("Patch-v0.1.2", true).unwrap();
    assert!(
        !git.branch_list()
            .unwrap()
            .iter()
            .any(|b| b == "Patch-v0.1.2")
    );

    git.delete_branch("banana", false).unwrap();
    assert!(!git.branch_list().unwrap().iter().any(|b| b == "banana"));
}

#[test]
fn remote_default_branch_follows_origin_head() {
    let fixture = Fixture::new();
    let mut git = fixture.git();
    assert_eq!(git.remote_default_branch().unwrap(), None);
    fixture.set_origin_head("main");
    assert_eq!(
        git.remote_default_branch().unwrap(),
        Some("main".to_string())
    );
}
