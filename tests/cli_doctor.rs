//! `config doctor` end to end. Additions; the baseline had no such command.

mod common;

use assert_cmd::Command;
use common::Fixture;
use merge_pipeline::ui::strip_ansi;
use predicates::prelude::*;
use std::fs;
use std::path::Path;

fn bin() -> Command {
    let mut command = Command::cargo_bin("merge-pipeline").expect("binary builds");
    command.env_remove("MERGE_PIPELINE_CONFIG");
    command
}

fn write(dir: &Path, name: &str, json: &str) {
    fs::write(dir.join(name), json).unwrap();
}

fn examples() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/config")
}

#[test]
fn shipped_examples_pass_with_exit_0() {
    let assert = bin()
        .args(["config", "doctor", "-f"])
        .arg(examples())
        .assert()
        .success();
    let out = strip_ansi(&String::from_utf8_lossy(&assert.get_output().stdout));
    assert!(out.contains("chosen by: --config flag"), "{out}");
    assert!(
        out.contains("✓ patch.json  Patch Release to Staging Pipeline"),
        "{out}"
    );
    assert!(
        out.contains("default.json  Default Pipeline (disabled)"),
        "{out}"
    );
    assert!(
        out.contains("4 workflows, 3 enabled · 0 errors, 0 warnings"),
        "{out}"
    );
    assert!(out.contains("✓ configuration is usable"), "{out}");
}

#[test]
fn bad_regex_exits_1_and_names_the_pattern() {
    let temp = tempfile::tempdir().unwrap();
    write(
        temp.path(),
        "bad.json",
        r#"{"$schema":"x","name":"Bad","pipeline":["^Patch-*","(unclosed"]}"#,
    );
    bin()
        .args(["config", "doctor", "-f"])
        .arg(temp.path())
        .assert()
        .failure()
        .code(1)
        .stdout(
            predicate::str::contains("pipeline[1] \"(unclosed\" is not a valid regular expression")
                .and(predicate::str::contains("configuration has errors")),
        );
}

#[test]
fn only_disabled_workflows_is_an_error() {
    let temp = tempfile::tempdir().unwrap();
    write(
        temp.path(),
        "off.json",
        r#"{"$schema":"x","disabled":true,"name":"Off","pipeline":["a","b"]}"#,
    );
    bin()
        .args(["config", "doctor", "-f"])
        .arg(temp.path())
        .assert()
        .failure()
        .code(1)
        .stdout(predicate::str::contains("no enabled workflows"));
}

#[test]
fn json_output_has_the_documented_shape_and_warnings_keep_exit_0() {
    let temp = tempfile::tempdir().unwrap();
    write(
        temp.path(),
        "one.json",
        r#"{"name":"One","pipeline":["a","b"],"extra":1}"#,
    );
    let assert = bin()
        .args(["config", "doctor", "--json", "-f"])
        .arg(temp.path())
        .assert()
        .success();
    let json: serde_json::Value =
        serde_json::from_slice(&assert.get_output().stdout).expect("valid JSON");
    assert_eq!(json["ok"], true);
    assert_eq!(json["rule"], "--config flag");
    assert_eq!(json["root"], temp.path().display().to_string());
    assert_eq!(json["workflows"][0]["name"], "One");
    assert_eq!(json["workflows"][0]["enabled"], true);
    let levels: Vec<&str> = json["problems"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["level"].as_str().unwrap())
        .collect();
    assert!(levels.contains(&"warning"), "{json}");
    assert!(levels.contains(&"info"), "{json}");
    assert!(!levels.contains(&"error"), "{json}");
}

#[test]
fn missing_directory_is_an_error_exit_1() {
    let temp = tempfile::tempdir().unwrap();
    bin()
        .args(["config", "doctor", "-f"])
        .arg(temp.path().join("absent"))
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("is not a directory"));
}

#[test]
fn cwd_repo_reports_branch_matches_for_each_pattern() {
    let fixture = Fixture::new();
    let config = fixture.work.join(".merge-pipeline");
    fs::create_dir_all(&config).unwrap();
    for name in ["patch.json", "canary.json"] {
        fs::copy(examples().join(name), config.join(name)).unwrap();
    }
    write(
        &config,
        "ghost.json",
        r#"{"$schema":"x","name":"Ghost","order":9,"pipeline":["^nothing-here$","main"]}"#,
    );

    let assert = bin()
        .args(["config", "doctor", "-c"])
        .arg(&fixture.work)
        .assert()
        .success(); // no-match is a warning, not an error
    let out = strip_ansi(&String::from_utf8_lossy(&assert.get_output().stdout));
    assert!(out.contains("chosen by: <cwd>/.merge-pipeline"), "{out}");
    assert!(out.contains("^staging-patch$ → staging-patch"), "{out}");
    assert!(
        out.contains("^Patch-* → Patch-v0.1.1, Patch-v0.1.2"),
        "{out}"
    );
    assert!(out.contains("\"^Patch-*\" matches 2 branches"), "{out}");
    assert!(out.contains("^nothing-here$ → no match"), "{out}");
    assert!(
        out.contains("\"^nothing-here$\" matches no local branch right now"),
        "{out}"
    );
    assert!(out.contains("0 errors, 1 warning"), "{out}");
}
