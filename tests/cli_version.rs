use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn version_flag_prints_the_cargo_version() {
    Command::cargo_bin("merge-pipeline")
        .expect("binary builds")
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn unknown_flag_is_rejected() {
    Command::cargo_bin("merge-pipeline")
        .expect("binary builds")
        .arg("--definitely-not-a-flag")
        .assert()
        .failure()
        .code(2);
}
