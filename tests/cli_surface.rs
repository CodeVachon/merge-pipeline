//! Command surface checks. The baseline had no CLI-level tests; these are additions.

use assert_cmd::Command;
use predicates::prelude::*;

fn bin() -> Command {
    Command::cargo_bin("merge-pipeline").expect("binary builds")
}

#[test]
fn version_prints_exactly_name_and_version() {
    bin()
        .arg("--version")
        .assert()
        .success()
        .stdout(format!("merge-pipeline {}\n", env!("CARGO_PKG_VERSION")));
}

#[test]
fn help_lists_every_flag_and_subcommand() {
    let assert = bin().arg("--help").assert().success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    for needle in [
        "-c, --cwd <DIR>",
        "-w, --workflow <NAME>",
        "-p, --auto-push",
        "-f, --config <PATH>",
        "-a, --action <ACTION>",
        "-y, --yes",
        "--no-sync",
        "- run:",
        "- dry-run:",
        "- test:",
        "mcp ",
        "upgrade ",
        "uninstall ",
        "install ",
        "config ",
        "completion ",
    ] {
        assert!(out.contains(needle), "help is missing {needle:?}:\n{out}");
    }
}

#[test]
fn unknown_flag_exits_2() {
    bin().arg("--nope").assert().failure().code(2);
}

#[test]
fn invalid_action_exits_2_and_names_the_choices() {
    bin()
        .args(["-a", "explode"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("dry-run"));
}

#[test]
fn completion_emits_a_script_for_each_shell() {
    for shell in ["bash", "zsh", "fish", "powershell"] {
        bin()
            .args(["completion", shell])
            .assert()
            .success()
            .stdout(predicate::str::contains("merge-pipeline"));
    }
}

#[test]
fn upgrade_help_shows_the_motte_style_flags() {
    bin().args(["upgrade", "--help"]).assert().success().stdout(
        predicate::str::contains("--check")
            .and(predicate::str::contains("--keep <N>"))
            .and(predicate::str::contains("--force"))
            .and(predicate::str::contains("--json"))
            .and(predicate::str::contains("[TARGET]")),
    );
}
