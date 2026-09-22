//! `config path` end to end. Additions; the baseline had no CLI tests.

use assert_cmd::Command;
use predicates::prelude::*;

fn bin() -> Command {
    let mut command = Command::cargo_bin("merge-pipeline").expect("binary builds");
    command.env_remove("MERGE_PIPELINE_CONFIG");
    command
}

#[test]
fn flag_is_reported_as_the_rule() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("cfg");
    std::fs::create_dir(&dir).unwrap();
    bin()
        .args(["config", "path", "-f"])
        .arg(&dir)
        .assert()
        .success()
        .stdout(
            predicate::str::contains(dir.display().to_string())
                .and(predicate::str::contains("chosen by: --config flag")),
        );
}

#[test]
fn env_var_is_honoured() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("from-env");
    std::fs::create_dir(&dir).unwrap();
    bin()
        .env("MERGE_PIPELINE_CONFIG", &dir)
        .args(["config", "path"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "chosen by: $MERGE_PIPELINE_CONFIG",
        ));
}

#[test]
fn repo_local_directory_wins_over_nothing() {
    let temp = tempfile::tempdir().unwrap();
    let local = temp.path().join(".merge-pipeline");
    std::fs::create_dir(&local).unwrap();
    bin()
        .args(["config", "path", "-c"])
        .arg(temp.path())
        .assert()
        .success()
        .stdout(
            predicate::str::contains(local.display().to_string())
                .and(predicate::str::contains("chosen by: <cwd>/.merge-pipeline")),
        );
}

#[test]
fn missing_flag_directory_is_an_error_naming_it() {
    let temp = tempfile::tempdir().unwrap();
    let missing = temp.path().join("absent");
    bin()
        .args(["config", "path", "-f"])
        .arg(&missing)
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("is not a directory"));
}
