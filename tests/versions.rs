//! `versions`, `use`, `versions remove` and `versions prune` driven through the binary from a
//! fake managed layout: the test binary is copied into `versions/v<crate version>/bin` so
//! `locate_install` recognises it. Downloads come from a local fake server; nothing touches the
//! network.
#![cfg(unix)]

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use flate2::Compression;
use flate2::write::GzEncoder;
use merge_pipeline::selfupdate::download::sha256_hex;
use merge_pipeline::selfupdate::host_target;

const RUNNING: &str = concat!("v", env!("CARGO_PKG_VERSION"));

struct Layout {
    _temp: tempfile::TempDir,
    root: PathBuf,
    exe: PathBuf,
}

/// versions/<RUNNING>/bin holds a copy of the real test binary; `others` are dummies.
fn layout(others: &[&str], current: &str) -> Layout {
    let temp = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(temp.path()).unwrap().join("root");
    for v in others {
        let bin = root.join("versions").join(v).join("bin");
        fs::create_dir_all(&bin).unwrap();
        fs::write(bin.join("merge-pipeline"), format!("#!/bin/sh\necho {v}\n")).unwrap();
    }
    let bin = root.join("versions").join(RUNNING).join("bin");
    fs::create_dir_all(&bin).unwrap();
    let exe = bin.join("merge-pipeline");
    fs::copy(env!("CARGO_BIN_EXE_merge-pipeline"), &exe).unwrap();
    std::os::unix::fs::symlink(root.join("versions").join(current), root.join("current")).unwrap();
    Layout {
        _temp: temp,
        root,
        exe,
    }
}

fn run(layout: &Layout, args: &[&str]) -> (i32, String, String) {
    run_env(layout, args, &[])
}

fn run_env(layout: &Layout, args: &[&str], env: &[(&str, &str)]) -> (i32, String, String) {
    let mut command = Command::new(&layout.exe);
    command
        .args(args)
        .env_remove("MERGE_PIPELINE_DOWNLOAD_BASE");
    for (k, v) in env {
        command.env(k, v);
    }
    let output = command.output().expect("binary runs");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn current_of(root: &Path) -> String {
    fs::read_link(root.join("current"))
        .unwrap()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

#[test]
fn versions_lists_newest_first_marking_current_and_running() {
    let layout = layout(&["v0.0.1", "v9.9.9"], "v0.0.1");

    let (code, out, _) = run(&layout, &["versions"]);
    assert_eq!(code, 0, "{out}");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "  v9.9.9");
    assert_eq!(lines[1], format!("  {RUNNING}  running"));
    assert_eq!(lines[2], "* v0.0.1  current");

    let (code, out, _) = run(&layout, &["versions", "--json"]);
    assert_eq!(code, 0);
    let report: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(report["current"], "v0.0.1");
    assert_eq!(report["running"], RUNNING);
    assert_eq!(report["installRoot"], layout.root.to_str().unwrap());
    let versions = report["versions"].as_array().unwrap();
    assert_eq!(versions.len(), 3);
    assert_eq!(versions[0]["version"], "v9.9.9");
    assert_eq!(versions[2]["current"], true);
    assert_eq!(versions[1]["running"], true);
    assert!(
        report["latest"].is_null(),
        "nothing newer than v9.9.9 is known"
    );
}

#[test]
fn use_switches_current_without_touching_path_and_reports_the_change() {
    let layout = layout(&["v0.0.1"], RUNNING);

    let (code, out, _) = run(&layout, &["use", "0.0.1"]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(
        out.trim(),
        format!(
            "now using v0.0.1 (was {RUNNING}); takes effect on your next merge-pipeline command"
        )
    );
    assert_eq!(current_of(&layout.root), "v0.0.1");
    // The binary we keep invoking is unchanged; only `current` moved.
    let (code, out, _) = run(&layout, &["versions"]);
    assert_eq!(code, 0);
    assert!(out.contains("* v0.0.1  current"), "{out}");
    assert!(out.contains(&format!("  {RUNNING}  running")), "{out}");

    let (code, out, _) = run(&layout, &["use", RUNNING, "--json"]);
    assert_eq!(code, 0, "{out}");
    let report: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(report["from"], "v0.0.1");
    assert_eq!(report["to"], RUNNING);
    assert_eq!(report["downloaded"], false);
    assert_eq!(current_of(&layout.root), RUNNING);

    let (code, _, err) = run(&layout, &["use", "latest"]);
    assert_eq!(code, 1);
    assert!(err.contains("is not a version"), "{err}");
}

#[test]
fn remove_refuses_current_and_running_then_removes_when_allowed() {
    let layout = layout(&["v0.0.1", "v0.0.2"], "v0.0.1");

    let (code, out, _) = run(
        &layout,
        &["versions", "remove", "v0.0.1", RUNNING, "v0.0.2"],
    );
    assert_eq!(code, 1, "refusals are an error exit\n{out}");
    assert!(
        out.contains("! kept v0.0.1: it is the active version; `merge-pipeline use <other>` first"),
        "{out}"
    );
    assert!(
        out.contains(&format!(
            "! kept {RUNNING}: it is the binary currently running"
        )),
        "{out}"
    );
    assert!(out.contains("✓ removed v0.0.2"), "{out}");
    assert!(!layout.root.join("versions/v0.0.2").exists());
    assert!(layout.root.join("versions/v0.0.1").exists());

    run(&layout, &["use", RUNNING]);
    let (code, out, _) = run(&layout, &["versions", "remove", "0.0.1", "--json"]);
    assert_eq!(code, 0, "{out}");
    let report: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(report["removed"], serde_json::json!(["v0.0.1"]));
    assert_eq!(report["kept"], serde_json::json!([]));
    assert!(!layout.root.join("versions/v0.0.1").exists());

    let (code, out, _) = run(&layout, &["versions", "remove", "v0.0.1"]);
    assert_eq!(code, 1);
    assert!(out.contains("! v0.0.1 is not installed"), "{out}");
}

#[test]
fn prune_keeps_the_newest_n_plus_current_and_running() {
    // current is the oldest (chosen with `use`); running is the crate version; v9.9.9 is newest.
    let layout = layout(&["v0.0.1", "v0.0.2", "v0.0.3", "v9.9.9"], "v0.0.1");

    let (code, out, _) = run(&layout, &["versions", "prune", "--keep", "1"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("✓ removed v0.0.3"), "{out}");
    assert!(out.contains("✓ removed v0.0.2"), "{out}");
    assert!(
        out.contains(&format!(
            "! kept {RUNNING}: it is the binary currently running"
        )),
        "{out}"
    );
    assert!(
        out.contains("! kept v0.0.1: it is the active version"),
        "{out}"
    );
    let mut left: Vec<String> = fs::read_dir(layout.root.join("versions"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    assert_eq!(left, vec!["v0.0.1", RUNNING, "v9.9.9"]);

    let (code, out, _) = run(&layout, &["versions", "prune", "--json"]);
    assert_eq!(code, 0);
    let report: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(report["removed"], serde_json::json!([]));
    assert_eq!(report["kept"].as_array().unwrap().len(), 1, "{out}");
}

type Routes = Arc<Mutex<HashMap<String, Vec<u8>>>>;

/// Serves the routes it is given with 200, everything else 404. One request per connection.
fn fake_server(routes: HashMap<String, Vec<u8>>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let routes: Routes = Arc::new(Mutex::new(routes));
    thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let routes = routes.clone();
            thread::spawn(move || {
                stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
                let mut buf = Vec::new();
                let mut chunk = [0u8; 1024];
                while let Ok(n) = stream.read(&mut chunk) {
                    if n == 0 {
                        break;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                    if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                }
                let request = String::from_utf8_lossy(&buf);
                let path = request
                    .lines()
                    .next()
                    .and_then(|l| l.split_whitespace().nth(1))
                    .unwrap_or("/")
                    .to_owned();
                let (status, body) = match routes.lock().unwrap().get(&path) {
                    Some(body) => ("200 OK", body.clone()),
                    None => ("404 Not Found", b"not found".to_vec()),
                };
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(&body);
            });
        }
    });
    base
}

#[test]
fn use_downloads_and_verifies_a_version_that_is_not_installed() {
    let layout = layout(&[], RUNNING);
    let target = host_target().expect("test host is a built target");

    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(b"#!/bin/sh\necho v0.0.5\n").unwrap();
    let archive = encoder.finish().unwrap();
    let mut routes = HashMap::new();
    routes.insert(format!("/download/{}", target.asset_name), archive.clone());
    routes.insert(
        "/download/checksums.txt".to_owned(),
        format!("{}  {}\n", sha256_hex(&archive), target.asset_name).into_bytes(),
    );
    let base = fake_server(routes);

    let (code, out, err) = run_env(
        &layout,
        &["use", "v0.0.5"],
        &[("MERGE_PIPELINE_DOWNLOAD_BASE", &format!("{base}/download"))],
    );
    assert_eq!(code, 0, "stdout:\n{out}\nstderr:\n{err}");
    assert!(
        out.contains(&format!(
            "downloading merge-pipeline v0.0.5 for {}-{}...",
            target.platform.as_str(),
            target.arch.as_str()
        )),
        "{out}"
    );
    assert!(out.contains("✓ checksum verified"), "{out}");
    assert!(out.contains("✓ installed v0.0.5"), "{out}");
    assert!(
        out.contains(&format!("now using v0.0.5 (was {RUNNING})")),
        "{out}"
    );
    assert_eq!(current_of(&layout.root), "v0.0.5");
    assert_eq!(
        fs::read(layout.root.join("current/bin/merge-pipeline")).unwrap(),
        b"#!/bin/sh\necho v0.0.5\n"
    );

    // A bad checksum must write nothing and leave `current` alone.
    let mut routes = HashMap::new();
    routes.insert(format!("/download/{}", target.asset_name), archive);
    routes.insert(
        "/download/checksums.txt".to_owned(),
        format!("0000  {}\n", target.asset_name).into_bytes(),
    );
    let base = fake_server(routes);
    let (code, _, err) = run_env(
        &layout,
        &["use", "v0.0.6"],
        &[("MERGE_PIPELINE_DOWNLOAD_BASE", &format!("{base}/download"))],
    );
    assert_eq!(code, 1);
    assert!(err.contains("checksum mismatch"), "{err}");
    assert!(!layout.root.join("versions/v0.0.6").exists());
    assert_eq!(current_of(&layout.root), "v0.0.5");
}

#[test]
fn unmanaged_binary_is_told_how_to_install_a_managed_copy() {
    for args in [
        vec!["versions"],
        vec!["use", "0.1.0"],
        vec!["versions", "remove", "0.1.0"],
        vec!["versions", "prune"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_merge-pipeline"))
            .args(&args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1), "{args:?}");
        let err = String::from_utf8_lossy(&output.stderr);
        assert!(err.contains("install.sh | sh"), "{args:?}: {err}");
    }
}

#[test]
fn help_lists_versions_and_use() {
    let output = Command::new(env!("CARGO_BIN_EXE_merge-pipeline"))
        .arg("--help")
        .output()
        .unwrap();
    let out = String::from_utf8_lossy(&output.stdout);
    assert!(out.contains("versions "), "{out}");
    assert!(out.contains("use "), "{out}");
    let output = Command::new(env!("CARGO_BIN_EXE_merge-pipeline"))
        .args(["versions", "--help"])
        .output()
        .unwrap();
    let out = String::from_utf8_lossy(&output.stdout);
    assert!(
        out.contains("remove") && out.contains("prune") && out.contains("--check"),
        "{out}"
    );
}
