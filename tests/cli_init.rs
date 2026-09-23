//! `config init` and the config half of `install`, end to end. Additions.

mod common;

use assert_cmd::Command;
use common::Fixture;
use predicates::prelude::*;
use std::fs;
use std::path::Path;

fn bin() -> Command {
    let mut command = Command::cargo_bin("merge-pipeline").expect("binary builds");
    command.env_remove("MERGE_PIPELINE_CONFIG");
    command
}

fn example(name: &str) -> String {
    fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples/config")
            .join(name),
    )
    .unwrap()
}

const NAMES: [&str; 4] = ["patch.json", "minor.json", "canary.json", "default.json"];

#[test]
fn init_creates_the_defaults_then_keeps_them_then_force_rewrites() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join(".merge-pipeline");

    bin()
        .args(["config", "init", "-c"])
        .arg(temp.path())
        .assert()
        .success()
        .stdout(
            predicate::str::contains("created ")
                .and(predicate::str::contains("workflow directory ready at")),
        );
    for name in NAMES {
        assert_eq!(fs::read_to_string(dir.join(name)).unwrap(), example(name));
    }

    fs::write(
        dir.join("patch.json"),
        "{\"name\":\"mine\",\"pipeline\":[\"a\",\"b\"]}",
    )
    .unwrap();
    bin()
        .args(["config", "init", "-c"])
        .arg(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("nothing changed"));
    assert!(
        fs::read_to_string(dir.join("patch.json"))
            .unwrap()
            .contains("mine")
    );

    bin()
        .args(["config", "init", "--force", "-c"])
        .arg(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("rewrote "));
    assert_eq!(
        fs::read_to_string(dir.join("patch.json")).unwrap(),
        example("patch.json")
    );
}

#[test]
fn init_output_passes_doctor_with_three_enabled_workflows() {
    let temp = tempfile::tempdir().unwrap();
    bin()
        .args(["config", "init", "-c"])
        .arg(temp.path())
        .assert()
        .success();
    bin()
        .args(["config", "doctor", "-c"])
        .arg(temp.path())
        .assert()
        .success()
        .stdout(
            predicate::str::contains("chosen by: <cwd>/.merge-pipeline").and(
                predicate::str::contains("4 workflows, 3 enabled · 0 errors, 0 warnings"),
            ),
        );
}

#[test]
fn install_writes_mcp_json_and_the_workflow_directory_and_is_idempotent() {
    let fixture = Fixture::new();
    let repo = fixture.work.clone();

    let first = bin().current_dir(&repo).arg("install").assert().success();
    let out = String::from_utf8_lossy(&first.get_output().stdout).to_string();
    assert!(out.contains(".mcp.json"), "{out}");
    assert!(out.contains("created "), "{out}");

    let mcp: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(repo.join(".mcp.json")).unwrap()).unwrap();
    assert_eq!(
        mcp["mcpServers"]["merge-pipeline"]["args"],
        serde_json::json!(["mcp"])
    );
    for name in NAMES {
        assert!(repo.join(".merge-pipeline").join(name).is_file(), "{name}");
    }

    let second = bin().current_dir(&repo).arg("install").assert().success();
    let out = String::from_utf8_lossy(&second.get_output().stdout).to_string();
    assert!(out.contains("nothing changed"), "{out}");
    assert!(out.contains("already wires merge-pipeline"), "{out}");

    // The fresh repo is immediately usable: doctor finds the directory by the repo-local rule and
    // every pattern of the patch workflow matches a fixture branch.
    bin()
        .args(["config", "doctor", "-c"])
        .arg(&repo)
        .assert()
        .success()
        .stdout(
            predicate::str::contains("chosen by: <cwd>/.merge-pipeline")
                .and(predicate::str::contains("4 workflows, 3 enabled"))
                .and(predicate::str::contains("^staging-patch$ → staging-patch")),
        );
}

#[test]
fn install_no_config_only_wires_the_agent() {
    let temp = tempfile::tempdir().unwrap();
    bin()
        .current_dir(temp.path())
        .args(["install", "--no-config"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains(".mcp.json")
                .and(predicate::str::contains("workflow directory").not()),
        );
    assert!(temp.path().join(".mcp.json").is_file());
    assert!(!temp.path().join(".merge-pipeline").exists());
}
