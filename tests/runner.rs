//! End-to-end runner tests against a hermetic git fixture, driven by a scripted prompter.
mod common;

use std::path::PathBuf;

use common::Fixture;
use merge_pipeline::config::WorkflowConfig;
use merge_pipeline::pipeline::Step;
use merge_pipeline::prompt::{Answer, RecordingPrompter, ScriptedPrompter};
use merge_pipeline::runner::{
    Action, CollectingSink, Event, PushSkipReason, RunError, Settings, run_workflow,
};
use merge_pipeline::sync::{StalePolicy, SyncEvent, SyncOptions};

fn workflow(patterns: &[&str]) -> WorkflowConfig {
    WorkflowConfig {
        disabled: false,
        name: "Test Workflow".into(),
        description: String::new(),
        order: None,
        pipeline: patterns.iter().map(|p| p.to_string()).collect(),
        source: PathBuf::from("test.json"),
    }
}

fn settings(fixture: &Fixture, action: Action, auto_push: bool) -> Settings {
    Settings {
        cwd: fixture.work.clone(),
        action,
        auto_push,
        sync: Some(SyncOptions::default()),
    }
}

fn step(source: &str, target: &str) -> Step {
    Step {
        source: source.into(),
        target: target.into(),
    }
}

fn yes_to_everything() -> ScriptedPrompter {
    ScriptedPrompter::new()
        .answer_prefix("step:", true)
        .answer_prefix("push:", true)
}

#[test]
fn run_merges_each_step_and_pushes_to_origin_with_auto_push() {
    let fixture = Fixture::new();
    fixture.commit_on("Patch-v0.1.1", "fix.txt", "fixed\n", "a fix");
    let before_patch = fixture.origin_head("staging-patch");
    let before_release = fixture.origin_head("staging-release");

    let mut git = fixture.git();
    let mut prompter = RecordingPrompter::new(yes_to_everything());
    let mut sink = CollectingSink::default();

    let report = run_workflow(
        &settings(&fixture, Action::Run, true),
        &workflow(&["^Patch-v0.1.1$", "^staging-patch$", "^staging-release$"]),
        &mut git,
        &mut prompter,
        &mut sink,
    )
    .unwrap();

    assert_eq!(
        report.branches,
        vec!["Patch-v0.1.1", "staging-patch", "staging-release"]
    );
    assert_eq!(report.steps.len(), 2);
    assert!(report.steps.iter().all(|s| s.merged && s.pushed));

    // Only the two step confirmations were asked; auto_push suppressed the push prompts.
    assert_eq!(
        prompter.keys(),
        vec![
            "step:Patch-v0.1.1>>staging-patch",
            "step:staging-patch>>staging-release"
        ]
    );

    assert_ne!(fixture.origin_head("staging-patch"), before_patch);
    assert_ne!(fixture.origin_head("staging-release"), before_release);
    assert_eq!(
        fixture.origin_head("staging-release"),
        fixture.local_head("staging-release")
    );

    assert!(sink.0.contains(&Event::StepsPlanned {
        steps: vec![
            step("Patch-v0.1.1", "staging-patch"),
            step("staging-patch", "staging-release")
        ]
    }));
    assert!(sink.0.contains(&Event::Pushed {
        branch: "staging-release".into()
    }));
}

#[test]
fn test_action_only_plans_and_touches_nothing() {
    let fixture = Fixture::new();
    fixture.commit_on("Patch-v0.1.1", "fix.txt", "fixed\n", "a fix");
    let before = fixture.local_head("staging-patch");

    let mut git = fixture.git();
    let mut prompter = RecordingPrompter::new(ScriptedPrompter::new());
    let mut sink = CollectingSink::default();

    let report = run_workflow(
        &settings(&fixture, Action::Test, true),
        &workflow(&["^Patch-v0.1.1$", "^staging-patch$"]),
        &mut git,
        &mut prompter,
        &mut sink,
    )
    .unwrap();

    assert_eq!(report.branches, vec!["Patch-v0.1.1", "staging-patch"]);
    assert!(report.steps.is_empty());
    assert!(prompter.recorded.is_empty());
    assert_eq!(fixture.local_head("staging-patch"), before);
    assert!(matches!(sink.0.last(), Some(Event::StepsPlanned { .. })));
    assert!(!sink.0.iter().any(|e| matches!(e, Event::Merged { .. })));
}

#[test]
fn declining_a_step_aborts_before_any_merge() {
    let fixture = Fixture::new();
    fixture.commit_on("Patch-v0.1.1", "fix.txt", "fixed\n", "a fix");
    let before = fixture.local_head("staging-patch");

    let mut git = fixture.git();
    let mut prompter = ScriptedPrompter::new().answer_prefix("step:", false);
    let mut sink = CollectingSink::default();

    let error = run_workflow(
        &settings(&fixture, Action::Run, true),
        &workflow(&["^Patch-v0.1.1$", "^staging-patch$"]),
        &mut git,
        &mut prompter,
        &mut sink,
    )
    .unwrap_err();

    assert_eq!(
        error.to_string(),
        "User chose not to proceed with merging \"Patch-v0.1.1\" into \"staging-patch\""
    );
    assert!(matches!(error, RunError::UserDeclined { .. }));
    assert_eq!(fixture.local_head("staging-patch"), before);
}

#[test]
fn dry_run_forces_auto_push_off_and_asks_per_branch() {
    let fixture = Fixture::new();
    fixture.commit_on("Patch-v0.1.1", "fix.txt", "fixed\n", "a fix");
    let before = fixture.origin_head("staging-patch");

    let mut git = fixture.git();
    let mut prompter = RecordingPrompter::new(
        ScriptedPrompter::new()
            .answer_prefix("step:", true)
            .answer_prefix("push:", false),
    );
    let mut sink = CollectingSink::default();

    let report = run_workflow(
        &settings(&fixture, Action::DryRun, true),
        &workflow(&["^Patch-v0.1.1$", "^staging-patch$"]),
        &mut git,
        &mut prompter,
        &mut sink,
    )
    .unwrap();

    assert_eq!(
        prompter.keys(),
        vec!["step:Patch-v0.1.1>>staging-patch", "push:staging-patch"]
    );
    assert!(report.steps[0].merged);
    assert!(!report.steps[0].pushed);
    // Merged locally, not pushed.
    assert_ne!(fixture.local_head("staging-patch"), before);
    assert_eq!(fixture.origin_head("staging-patch"), before);
    assert!(sink.0.contains(&Event::PushSkipped {
        branch: "staging-patch".into(),
        reason: PushSkipReason::Declined
    }));
}

#[test]
fn local_only_target_is_neither_pulled_nor_pushed() {
    let fixture = Fixture::new();
    fixture.commit_on("Patch-v0.1.1", "fix.txt", "fixed\n", "a fix");

    let mut git = fixture.git();
    let mut prompter = RecordingPrompter::new(yes_to_everything());
    let mut sink = CollectingSink::default();

    let report = run_workflow(
        &settings(&fixture, Action::Run, false),
        &workflow(&["^Patch-v0.1.1$", "^Release-0.2.0$"]),
        &mut git,
        &mut prompter,
        &mut sink,
    )
    .unwrap();

    assert!(report.steps[0].merged);
    assert!(!report.steps[0].pushed);
    // No push prompt, because there is no upstream to push to.
    assert_eq!(prompter.keys(), vec!["step:Patch-v0.1.1>>Release-0.2.0"]);
    assert!(sink.0.contains(&Event::PushSkipped {
        branch: "Release-0.2.0".into(),
        reason: PushSkipReason::NoUpstream
    }));
    assert!(!sink.0.contains(&Event::Pulled {
        branch: "Release-0.2.0".into()
    }));
    assert!(sink.0.contains(&Event::Pulled {
        branch: "Patch-v0.1.1".into()
    }));
    assert_eq!(fixture.read("fix.txt"), "fixed\n");
}

#[test]
fn package_json_version_conflict_is_auto_resolved_committed_and_pushed() {
    let fixture = Fixture::new();
    let pkg = |version: &str| {
        format!(
            "{{\n  \"name\": \"example\",\n  \"version\": \"{version}\",\n  \"private\": true\n}}\n"
        )
    };
    fixture.commit_on("main", "package.json", &pkg("1.0.0"), "add package.json");
    fixture.raw(&fixture.work, &["push", "origin", "main"]);
    // Both sides branch from main's package.json and bump differently.
    fixture.raw(&fixture.work, &["checkout", "Patch-v0.1.1"]);
    fixture.raw(&fixture.work, &["merge", "main", "--no-verify"]);
    fixture.commit_on(
        "Patch-v0.1.1",
        "package.json",
        &pkg("1.10.0"),
        "bump patch side",
    );
    fixture.raw(&fixture.work, &["checkout", "staging-patch"]);
    fixture.raw(&fixture.work, &["merge", "main", "--no-verify"]);
    fixture.commit_on(
        "staging-patch",
        "package.json",
        &pkg("1.9.0"),
        "bump staging side",
    );
    fixture.raw(&fixture.work, &["push", "origin", "staging-patch"]);

    let mut git = fixture.git();
    let mut prompter = RecordingPrompter::new(
        yes_to_everything()
            .answer("conflict:auto_resolve", true)
            .answer_prefix("conflict:version:", Answer::Index(0)),
    );
    let mut sink = CollectingSink::default();

    let report = run_workflow(
        &settings(&fixture, Action::Run, true),
        &workflow(&["^Patch-v0.1.1$", "^staging-patch$"]),
        &mut git,
        &mut prompter,
        &mut sink,
    )
    .unwrap();

    assert_eq!(report.steps[0].conflicts_resolved, vec!["package.json"]);
    assert!(report.steps[0].pushed);
    assert_eq!(
        prompter.keys(),
        vec![
            "step:Patch-v0.1.1>>staging-patch",
            "conflict:auto_resolve",
            "conflict:version:1.9.0|1.10.0"
        ]
    );
    assert!(
        fixture
            .read("package.json")
            .contains("\"version\": \"1.10.0\"")
    );
    assert!(git.conflicted_paths().unwrap().is_empty());
    assert_eq!(
        fixture.origin_head("staging-patch"),
        fixture.local_head("staging-patch")
    );
    assert!(sink.0.iter().any(|e| matches!(
        e,
        Event::ConflictsResolved { committed: true, rewritten, .. } if rewritten == &vec!["package.json".to_string()]
    )));
}

#[test]
fn unresolvable_conflict_is_a_merge_error_listing_files_and_leaves_the_merge_in_progress() {
    let fixture = Fixture::new();
    fixture.commit_on("Patch-v0.1.1", "README.md", "# patch side\n", "patch");
    fixture.commit_on("staging-patch", "README.md", "# staging side\n", "staging");
    fixture.raw(&fixture.work, &["push", "origin", "staging-patch"]);

    let mut git = fixture.git();
    let mut prompter = yes_to_everything();
    let mut sink = CollectingSink::default();

    let error = run_workflow(
        &settings(&fixture, Action::Run, true),
        &workflow(&["^Patch-v0.1.1$", "^staging-patch$"]),
        &mut git,
        &mut prompter,
        &mut sink,
    )
    .unwrap_err();

    match error {
        RunError::Merge(merge) => {
            assert_eq!(merge.files, vec!["README.md"]);
            assert_eq!(
                merge.to_string(),
                "Merge Error: Patch-v0.1.1 into staging-patch. 1 conflicted file"
            );
        }
        other => panic!("expected MergeError, got {other:?}"),
    }
    // Left for a human to finish, as the baseline did.
    assert_eq!(fixture.current_branch(), "staging-patch");
    assert_eq!(git.conflicted_paths().unwrap(), vec!["README.md"]);
    assert!(sink.0.contains(&Event::ConflictsDetected {
        step: step("Patch-v0.1.1", "staging-patch"),
        paths: vec!["README.md".into()]
    }));
}

/// Choice values offered by a recorded select prompt.
fn choices_of(recorded: &merge_pipeline::prompt::Recorded) -> Vec<String> {
    match &recorded.kind {
        merge_pipeline::prompt::PromptKind::Select { choices, .. } => {
            choices.iter().map(|c| c.value.clone()).collect()
        }
        other => panic!("expected a select prompt, got {other:?}"),
    }
}

fn has(branches: &[String], name: &str) -> bool {
    branches.iter().any(|b| b == name)
}

/// origin has moved on: Patch-v0.1.1 was merged and deleted, Patch-v0.1.3 is new.
fn drift_origin(fixture: &Fixture) {
    fixture.origin_delete_branch("Patch-v0.1.1");
    fixture.origin_add_branch("Patch-v0.1.3", "main");
}

#[test]
fn sync_deletes_the_stale_patch_branch_and_tracks_the_new_one_before_mapping() {
    let fixture = Fixture::new();
    drift_origin(&fixture);

    let mut git = fixture.git();
    let mut prompter = RecordingPrompter::new(
        ScriptedPrompter::new()
            .answer("sync:delete:Patch-v0.1.1", true)
            .answer("branch:^Patch-*", "Patch-v0.1.3"),
    );
    let mut sink = CollectingSink::default();

    let report = run_workflow(
        &settings(&fixture, Action::Test, false),
        &workflow(&["^Patch-*", "^staging-patch$"]),
        &mut git,
        &mut prompter,
        &mut sink,
    )
    .unwrap();

    let sync = report.sync.expect("sync ran");
    assert_eq!(sync.stale, vec!["Patch-v0.1.1"]);
    assert_eq!(sync.deleted, vec!["Patch-v0.1.1"]);
    assert!(sync.kept.is_empty());
    assert_eq!(sync.created, vec!["Patch-v0.1.3"]);

    let branches = git.branch_list().unwrap();
    assert!(
        !has(&branches, "Patch-v0.1.1"),
        "stale branch is gone locally"
    );
    assert!(has(&branches, "Patch-v0.1.3"), "new branch exists locally");
    assert!(
        has(&branches, "Patch-v0.1.2"),
        "never-pushed local branch is untouched"
    );
    let tracking = git.local_branch_tracking().unwrap();
    let created = tracking.iter().find(|t| t.name == "Patch-v0.1.3").unwrap();
    assert_eq!(created.upstream.as_deref(), Some("origin/Patch-v0.1.3"));

    // The delete was asked before mapping, and mapping only offered the surviving branches.
    assert_eq!(
        prompter.keys(),
        vec!["sync:delete:Patch-v0.1.1", "branch:^Patch-*"]
    );
    assert_eq!(
        prompter.recorded[0].question.message,
        "Local branch Patch-v0.1.1 no longer exists on origin. Delete it?"
    );
    assert_eq!(
        choices_of(&prompter.recorded[1]),
        vec!["Patch-v0.1.2", "Patch-v0.1.3"]
    );
    assert_eq!(report.branches, vec!["Patch-v0.1.3", "staging-patch"]);

    assert!(sink.0.contains(&Event::Sync(SyncEvent::StaleBranch {
        branch: "Patch-v0.1.1".into(),
        deleted: true
    })));
    assert!(sink.0.contains(&Event::Sync(SyncEvent::BranchCreated {
        branch: "Patch-v0.1.3".into(),
        upstream: "origin/Patch-v0.1.3".into()
    })));
}

#[test]
fn declining_the_delete_keeps_the_stale_branch_and_reports_it() {
    let fixture = Fixture::new();
    drift_origin(&fixture);

    let mut git = fixture.git();
    let mut prompter = ScriptedPrompter::new()
        .answer("sync:delete:Patch-v0.1.1", false)
        .answer("branch:^Patch-*", "Patch-v0.1.2");
    let mut sink = CollectingSink::default();

    let report = run_workflow(
        &settings(&fixture, Action::Test, false),
        &workflow(&["^Patch-*", "^staging-patch$"]),
        &mut git,
        &mut prompter,
        &mut sink,
    )
    .unwrap();

    let sync = report.sync.unwrap();
    assert_eq!(sync.stale, vec!["Patch-v0.1.1"]);
    assert!(sync.deleted.is_empty());
    assert_eq!(sync.kept, vec!["Patch-v0.1.1"]);
    assert_eq!(sync.created, vec!["Patch-v0.1.3"]);
    assert!(has(&git.branch_list().unwrap(), "Patch-v0.1.1"));
    assert!(sink.0.contains(&Event::Sync(SyncEvent::StaleBranch {
        branch: "Patch-v0.1.1".into(),
        deleted: false
    })));
}

#[test]
fn deleting_the_checked_out_stale_branch_moves_to_the_default_branch_first() {
    let fixture = Fixture::new();
    fixture.raw(&fixture.work, &["checkout", "Patch-v0.1.1"]);
    fixture.set_origin_head("main");
    fixture.origin_delete_branch("Patch-v0.1.1");

    let mut git = fixture.git();
    let mut prompter = ScriptedPrompter::new().answer_prefix("sync:delete:", true);
    let mut sink = CollectingSink::default();

    let report = run_workflow(
        &settings(&fixture, Action::Test, false),
        &workflow(&["^Patch-v0.1.1$|^Patch-v0.1.2$", "^staging-patch$"]),
        &mut git,
        &mut prompter,
        &mut sink,
    )
    .unwrap();

    assert_eq!(report.sync.unwrap().deleted, vec!["Patch-v0.1.1"]);
    assert!(!has(&git.branch_list().unwrap(), "Patch-v0.1.1"));
    // Mapping then resolved to the only remaining candidate, and HEAD was moved off the deleted
    // branch before the delete.
    assert_eq!(report.branches, vec!["Patch-v0.1.2", "staging-patch"]);
    assert_eq!(fixture.current_branch(), "main");
}

#[test]
fn checked_out_stale_branch_falls_back_to_a_local_main_when_origin_head_is_unset() {
    let fixture = Fixture::new();
    fixture.raw(&fixture.work, &["checkout", "Patch-v0.1.1"]);
    fixture.origin_delete_branch("Patch-v0.1.1");

    let mut git = fixture.git();
    let mut prompter = ScriptedPrompter::new().answer_prefix("sync:delete:", true);
    let mut sink = CollectingSink::default();

    run_workflow(
        &settings(&fixture, Action::Test, false),
        &workflow(&["^Patch-v0.1.1$|^Patch-v0.1.2$", "^staging-patch$"]),
        &mut git,
        &mut prompter,
        &mut sink,
    )
    .unwrap();

    assert!(!has(&git.branch_list().unwrap(), "Patch-v0.1.1"));
    assert_eq!(fixture.current_branch(), "main");
}

#[test]
fn sync_disabled_restores_the_baseline_plain_fetch_and_changes_nothing() {
    let fixture = Fixture::new();
    drift_origin(&fixture);

    let mut git = fixture.git();
    let mut prompter = RecordingPrompter::new(ScriptedPrompter::new());
    let mut sink = CollectingSink::default();
    let settings = Settings {
        sync: None,
        ..settings(&fixture, Action::Test, false)
    };

    let report = run_workflow(
        &settings,
        &workflow(&["^Patch-v0.1.1$", "^staging-patch$"]),
        &mut git,
        &mut prompter,
        &mut sink,
    )
    .unwrap();

    assert!(report.sync.is_none());
    assert!(prompter.recorded.is_empty());
    let branches = git.branch_list().unwrap();
    assert!(has(&branches, "Patch-v0.1.1"), "stale branch still present");
    assert!(
        !has(&branches, "Patch-v0.1.3"),
        "new branch not fetched into a local branch"
    );
    assert_eq!(report.branches, vec!["Patch-v0.1.1", "staging-patch"]);
    assert!(sink.0.contains(&Event::Sync(SyncEvent::Skipped {
        reason: "disabled".into()
    })));
    assert!(!sink.0.contains(&Event::Sync(SyncEvent::Started)));
}

#[test]
fn keep_policy_never_prompts_and_never_deletes_but_still_creates_new_branches() {
    let fixture = Fixture::new();
    drift_origin(&fixture);

    let mut git = fixture.git();
    let mut prompter =
        RecordingPrompter::new(ScriptedPrompter::new().answer("branch:^Patch-*", "Patch-v0.1.3"));
    let mut sink = CollectingSink::default();
    let settings = Settings {
        sync: Some(SyncOptions {
            stale: StalePolicy::Keep,
            fetch_new: true,
        }),
        ..settings(&fixture, Action::Test, false)
    };

    let report = run_workflow(
        &settings,
        &workflow(&["^Patch-*", "^staging-patch$"]),
        &mut git,
        &mut prompter,
        &mut sink,
    )
    .unwrap();

    let sync = report.sync.unwrap();
    assert_eq!(sync.stale, vec!["Patch-v0.1.1"]);
    assert_eq!(sync.kept, vec!["Patch-v0.1.1"]);
    assert!(sync.deleted.is_empty());
    assert_eq!(sync.created, vec!["Patch-v0.1.3"]);
    assert_eq!(prompter.keys(), vec!["branch:^Patch-*"], "no delete prompt");
    assert_eq!(
        choices_of(&prompter.recorded[0]),
        vec!["Patch-v0.1.1", "Patch-v0.1.2", "Patch-v0.1.3"]
    );
}

#[test]
fn delete_policy_removes_stale_branches_without_prompting() {
    let fixture = Fixture::new();
    drift_origin(&fixture);

    let mut git = fixture.git();
    let mut prompter = RecordingPrompter::new(ScriptedPrompter::new());
    let mut sink = CollectingSink::default();
    let settings = Settings {
        sync: Some(SyncOptions {
            stale: StalePolicy::Delete,
            fetch_new: false,
        }),
        ..settings(&fixture, Action::Test, false)
    };

    let report = run_workflow(
        &settings,
        &workflow(&["^Patch-v0.1.1$|^Patch-v0.1.2$", "^staging-patch$"]),
        &mut git,
        &mut prompter,
        &mut sink,
    )
    .unwrap();

    let sync = report.sync.unwrap();
    assert_eq!(sync.deleted, vec!["Patch-v0.1.1"]);
    assert!(sync.created.is_empty(), "fetch_new=false creates nothing");
    assert!(prompter.recorded.is_empty());
    let branches = git.branch_list().unwrap();
    assert!(!has(&branches, "Patch-v0.1.1"));
    assert!(!has(&branches, "Patch-v0.1.3"));
}
