//! The interactive command driven fully by flags against a hermetic fixture repo.
//! Additions: the baseline had no CLI-level tests.

mod common;

use std::path::Path;

use assert_cmd::Command;
use common::Fixture;
use merge_pipeline::ui::strip_ansi;
use predicates::prelude::*;

fn write_configs(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(
        dir.join("patch.json"),
        r#"{"name":"Patch Release to Staging Pipeline","order":1,"pipeline":["^Patch-v0.1.1$","^staging-patch$"]}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("release.json"),
        r#"{"name":"Release to Main","order":2,"pipeline":["^Release-0.1.0$","^staging-release$","^main$"]}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("default.json"),
        r#"{"disabled":true,"name":"Default Pipeline","order":100,"pipeline":["staging","main"]}"#,
    )
    .unwrap();
}

fn bin(fixture: &Fixture) -> Command {
    let mut command = Command::cargo_bin("merge-pipeline").expect("binary builds");
    command.env_remove("MERGE_PIPELINE_CONFIG");
    command.env("NO_COLOR", "1");
    command.env("MERGE_PIPELINE_NO_UPDATE_CHECK", "1");
    for (key, value) in common::isolation_env(&fixture.hooks) {
        command.env(key, value);
    }
    command
}

fn stdout_of(assert: &assert_cmd::assert::Assert) -> String {
    strip_ansi(&String::from_utf8_lossy(&assert.get_output().stdout))
}

fn stderr_of(assert: &assert_cmd::assert::Assert) -> String {
    strip_ansi(&String::from_utf8_lossy(&assert.get_output().stderr))
}

#[test]
fn test_action_prints_the_title_pipeline_and_steps_without_touching_branches() {
    let fixture = Fixture::new();
    let config = fixture.temp.path().join("config");
    write_configs(&config);
    let before = fixture.local_head("staging-release");

    let assert = bin(&fixture)
        .args(["-c"])
        .arg(&fixture.work)
        .args(["-f"])
        .arg(&config)
        .args(["-w", "Release to Main", "-a", "test"])
        .assert()
        .success();
    let out = stdout_of(&assert);

    assert!(out.contains("+------------+\n| Git Branch |\n| Workflow   |\n+------------+"));
    assert!(out.contains("Test: Release to Main"));
    assert!(out.contains("Pipeline: ^Release-0.1.0$ > ^staging-release$ > ^main$"));
    assert!(out.contains("$ git fetch"));
    assert!(out.contains("Step 1: Merge Release-0.1.0 into staging-release"));
    assert!(out.contains("Step 2: Merge staging-release into main"));
    assert!(out.contains("Task Complete\nWork Complete\n"));
    assert_eq!(fixture.local_head("staging-release"), before);
}

#[test]
fn run_with_auto_push_and_yes_merges_every_step_and_pushes_to_origin() {
    let fixture = Fixture::new();
    let config = fixture.temp.path().join("config");
    write_configs(&config);
    fixture.commit_on(
        "Release-0.1.0",
        "feature.txt",
        "release work\n",
        "release work",
    );
    fixture.raw(&fixture.work, &["push", "origin", "Release-0.1.0"]);
    fixture.raw(&fixture.work, &["checkout", "main"]);
    let release_head = fixture.local_head("Release-0.1.0");

    let assert = bin(&fixture)
        .args(["-c"])
        .arg(&fixture.work)
        .args(["-f"])
        .arg(&config)
        .args(["-w", "Release to Main", "-a", "run", "-p", "-y"])
        .assert()
        .success();
    let out = stdout_of(&assert);

    assert!(out.contains("Run: Release to Main"));
    assert!(out.contains("$ git merge Release-0.1.0 --no-verify"));
    assert!(out.contains("$ git push --no-verify"));
    assert!(out.ends_with("Task Complete\nWork Complete\n"));

    // Fast-forward merges: every target now points at the release commit, locally and on origin.
    assert_eq!(fixture.local_head("staging-release"), release_head);
    assert_eq!(fixture.local_head("main"), release_head);
    assert_eq!(fixture.origin_head("staging-release"), release_head);
    assert_eq!(fixture.origin_head("main"), release_head);
}

#[test]
fn dry_run_merges_locally_but_a_declined_push_leaves_origin_alone() {
    let fixture = Fixture::new();
    let config = fixture.temp.path().join("config");
    write_configs(&config);
    fixture.commit_on("Patch-v0.1.1", "patch.txt", "patch\n", "patch work");
    fixture.raw(&fixture.work, &["checkout", "main"]);
    let patch_head = fixture.local_head("Patch-v0.1.1");
    let origin_before = fixture.origin_head("staging-patch");

    // `--yes` answers the push confirmation with its default (yes), so origin moves too. That is
    // the baseline's dry-run behaviour: no *automatic* push, but every push is offered.
    bin(&fixture)
        .args(["-c"])
        .arg(&fixture.work)
        .args(["-f"])
        .arg(&config)
        .args([
            "-w",
            "Patch Release to Staging Pipeline",
            "-a",
            "dry-run",
            "-y",
        ])
        .assert()
        .success();

    assert_eq!(fixture.local_head("staging-patch"), patch_head);
    assert_ne!(fixture.origin_head("staging-patch"), origin_before);
}

#[test]
fn dirty_repository_exits_1_with_the_baseline_message() {
    let fixture = Fixture::new();
    let config = fixture.temp.path().join("config");
    write_configs(&config);
    fixture.write("README.md", "# changed but not committed\n");

    let assert = bin(&fixture)
        .args(["-c"])
        .arg(&fixture.work)
        .args(["-f"])
        .arg(&config)
        .args(["-w", "Release to Main", "-a", "test"])
        .assert()
        .failure()
        .code(1);
    let err = stderr_of(&assert);
    assert!(err.contains("Error\n====================\nGit is in a Dirty State\n"));
    assert!(stdout_of(&assert).ends_with("Work Complete\n"));
}

#[test]
fn staged_changes_also_count_as_dirty() {
    let fixture = Fixture::new();
    let config = fixture.temp.path().join("config");
    write_configs(&config);
    fixture.write("staged.txt", "staged\n");
    fixture.raw(&fixture.work, &["add", "staged.txt"]);

    bin(&fixture)
        .args(["-c"])
        .arg(&fixture.work)
        .args(["-f"])
        .arg(&config)
        .args(["-w", "Release to Main", "-a", "test"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Git is in a Dirty State"));
}

#[test]
fn unknown_workflow_exits_1_and_lists_the_enabled_names_quoted() {
    let fixture = Fixture::new();
    let config = fixture.temp.path().join("config");
    write_configs(&config);

    let assert = bin(&fixture)
        .args(["-c"])
        .arg(&fixture.work)
        .args(["-f"])
        .arg(&config)
        .args(["-w", "Nope", "-a", "test"])
        .assert()
        .failure()
        .code(1);
    assert!(stderr_of(&assert).contains(
        "Could not find Workflow named \"Nope\". Expected one of [\"Patch Release to Staging Pipeline\", \"Release to Main\"]"
    ));
}

#[test]
fn zero_enabled_workflows_is_an_error_naming_the_directory() {
    let fixture = Fixture::new();
    let config = fixture.temp.path().join("only-disabled");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(
        config.join("default.json"),
        r#"{"disabled":true,"name":"Default","pipeline":["a","b"]}"#,
    )
    .unwrap();

    let assert = bin(&fixture)
        .args(["-c"])
        .arg(&fixture.work)
        .args(["-f"])
        .arg(&config)
        .args(["-a", "test"])
        .assert()
        .failure()
        .code(1);
    assert!(stderr_of(&assert).contains(&format!(
        "Expected to find enabled workflows at {}. found 0",
        config.display()
    )));
}

#[test]
fn unreadable_workflow_files_warn_but_do_not_stop_the_run() {
    let fixture = Fixture::new();
    let config = fixture.temp.path().join("config");
    write_configs(&config);
    std::fs::write(config.join("broken.json"), "{ nope").unwrap();

    let assert = bin(&fixture)
        .args(["-c"])
        .arg(&fixture.work)
        .args(["-f"])
        .arg(&config)
        .args(["-w", "Release to Main", "-a", "test"])
        .assert()
        .success();
    assert!(stderr_of(&assert).contains("warning:"));
    assert!(stderr_of(&assert).contains("broken.json"));
}

#[test]
fn a_conflict_outside_package_json_stops_with_the_conflicted_paths_listed() {
    let fixture = Fixture::new();
    let config = fixture.temp.path().join("config");
    write_configs(&config);
    fixture.commit_on("Patch-v0.1.1", "README.md", "# from patch\n", "patch edit");
    fixture.commit_on(
        "staging-patch",
        "README.md",
        "# from staging\n",
        "staging edit",
    );
    fixture.raw(&fixture.work, &["push", "origin", "staging-patch"]);
    fixture.raw(&fixture.work, &["checkout", "main"]);

    let assert = bin(&fixture)
        .args(["-c"])
        .arg(&fixture.work)
        .args(["-f"])
        .arg(&config)
        .args([
            "-w",
            "Patch Release to Staging Pipeline",
            "-a",
            "run",
            "-p",
            "-y",
        ])
        .assert()
        .failure()
        .code(1);
    let err = stderr_of(&assert);
    assert!(err.contains("Merge Error: Patch-v0.1.1 into staging-patch. 1 conflicted file"));
    assert!(err.contains("Error Details\n====================\nREADME.md\n"));
    // The merge is left in progress for a human to finish, as the baseline did.
    assert_eq!(fixture.current_branch(), "staging-patch");
    assert!(fixture.work.join(".git/MERGE_HEAD").exists());
}

#[test]
fn package_json_version_conflict_is_auto_resolved_with_the_higher_version_and_committed() {
    let fixture = Fixture::new();
    let config = fixture.temp.path().join("config");
    write_configs(&config);
    fixture.commit_on(
        "main",
        "package.json",
        "{\n  \"name\": \"example\",\n  \"version\": \"1.0.0\"\n}\n",
        "add package.json",
    );
    fixture.raw(&fixture.work, &["push", "origin", "main"]);
    for branch in ["Patch-v0.1.1", "staging-patch"] {
        fixture.raw(&fixture.work, &["checkout", branch]);
        fixture.raw(&fixture.work, &["merge", "--ff-only", "main"]);
    }
    fixture.commit_on(
        "Patch-v0.1.1",
        "package.json",
        "{\n  \"name\": \"example\",\n  \"version\": \"1.10.0\"\n}\n",
        "bump to 1.10.0",
    );
    fixture.commit_on(
        "staging-patch",
        "package.json",
        "{\n  \"name\": \"example\",\n  \"version\": \"1.9.0\"\n}\n",
        "bump to 1.9.0",
    );
    fixture.raw(&fixture.work, &["push", "origin", "staging-patch"]);
    fixture.raw(&fixture.work, &["checkout", "main"]);

    let assert = bin(&fixture)
        .args(["-c"])
        .arg(&fixture.work)
        .args(["-f"])
        .arg(&config)
        .args([
            "-w",
            "Patch Release to Staging Pipeline",
            "-a",
            "run",
            "-p",
            "-y",
        ])
        .assert()
        .success();
    let out = stdout_of(&assert);
    assert!(out.contains("Resolved package.json versions in package.json (committed)"));
    assert!(
        fixture
            .read("package.json")
            .contains("\"version\": \"1.10.0\"")
    );
    assert!(!fixture.work.join(".git/MERGE_HEAD").exists());
    assert_eq!(
        fixture.origin_head("staging-patch"),
        fixture.local_head("staging-patch")
    );
}
