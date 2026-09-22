//! Self-update against a local fake GitHub: release API and asset downloads are served from a
//! std TcpListener on 127.0.0.1, so nothing touches the network.

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use flate2::Compression;
use flate2::write::GzEncoder;
use merge_pipeline::selfupdate::download::{fetch_verified_binary_from, sha256_hex};
use merge_pipeline::selfupdate::layout::{ManagedInstall, host_target_for, locate_install_from};
use merge_pipeline::selfupdate::releases::{ReleaseError, resolve_latest_version_at};
use merge_pipeline::selfupdate::upgrade::{
    UninstallOptions, UpgradeContext, UpgradeOptions, read_check_record, run_uninstall_with,
    run_upgrade_with,
};

type Routes = Arc<Mutex<HashMap<String, (u16, Vec<u8>)>>>;

/// Minimal HTTP/1.1 server: one response per path, `Connection: close`.
struct FakeServer {
    base: String,
    routes: Routes,
}

impl FakeServer {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let base = format!("http://{}", listener.local_addr().unwrap());
        let routes: Routes = Arc::new(Mutex::new(HashMap::new()));
        let handler_routes = routes.clone();
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let routes = handler_routes.clone();
                thread::spawn(move || serve_one(stream, &routes));
            }
        });
        Self { base, routes }
    }

    fn route(&self, path: &str, status: u16, body: impl Into<Vec<u8>>) {
        self.routes
            .lock()
            .unwrap()
            .insert(path.to_owned(), (status, body.into()));
    }
}

fn serve_one(mut stream: std::net::TcpStream, routes: &Routes) {
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                buf.extend_from_slice(&chunk[..n]);
                if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    let request = String::from_utf8_lossy(&buf);
    let path = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/")
        .to_owned();
    let (status, body) = routes
        .lock()
        .unwrap()
        .get(&path)
        .cloned()
        .unwrap_or((404, b"not found".to_vec()));
    let reason = match status {
        200 => "OK",
        403 => "Forbidden",
        404 => "Not Found",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        _ => "Status",
    };
    let _ = write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(&body);
    let _ = stream.flush();
}

fn gz(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

/// A fake managed install rooted in a temp dir, running `running_version`.
fn fake_install(root: &Path, running_version: &str) -> ManagedInstall {
    fs::create_dir_all(root).unwrap();
    // Canonical from the start: macOS puts temp dirs under /var, which resolves to /private/var.
    let root = &fs::canonicalize(root).unwrap();
    let bin = root.join("versions").join(running_version).join("bin");
    fs::create_dir_all(&bin).unwrap();
    let exe = bin.join("merge-pipeline");
    fs::write(&exe, b"#!/bin/sh\necho old\n").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        root.join("versions").join(running_version),
        root.join("current"),
    )
    .unwrap();
    locate_install_from(&exe).expect("fake install has the managed shape")
}

fn context(install: ManagedInstall, current: &str, server: &FakeServer) -> UpgradeContext {
    UpgradeContext {
        install,
        current_version: current.to_owned(),
        host: host_target_for("linux", "x86_64").unwrap(),
        api_base: server.base.clone(),
        download_base: Some(format!("{}/download", server.base)),
        api_timeout: Duration::from_secs(5),
    }
}

/// Publish a fake `version` release on the server: API responses plus asset and checksums.
fn publish(server: &FakeServer, version: &str, binary: &[u8]) {
    let archive = gz(binary);
    let asset = "merge-pipeline-linux-x64.gz";
    server.route(
        "/repos/CodeVachon/merge-pipeline/releases/latest",
        200,
        format!(r#"{{"tag_name":"{version}","draft":false,"prerelease":false}}"#),
    );
    server.route(&format!("/download/{asset}"), 200, archive.clone());
    server.route(
        "/download/checksums.txt",
        200,
        format!(
            "{}  {asset}\nffff  merge-pipeline-darwin-arm64.gz\n",
            sha256_hex(&archive)
        ),
    );
}

fn current_target(root: &Path) -> PathBuf {
    fs::read_link(root.join("current")).unwrap()
}

#[test]
fn resolve_latest_prefers_the_stable_release() {
    let server = FakeServer::start();
    server.route(
        "/repos/CodeVachon/merge-pipeline/releases/latest",
        200,
        r#"{"tag_name":"v1.4.0"}"#,
    );
    let tag = resolve_latest_version_at(&server.base, Duration::from_secs(5)).unwrap();
    assert_eq!(tag, "v1.4.0");
}

#[test]
fn resolve_latest_falls_back_to_the_newest_non_draft() {
    let server = FakeServer::start();
    server.route(
        "/repos/CodeVachon/merge-pipeline/releases/latest",
        404,
        r#"{"message":"Not Found"}"#,
    );
    server.route(
        "/repos/CodeVachon/merge-pipeline/releases",
        200,
        r#"[{"tag_name":"v0.3.0","draft":true},{"tag_name":"v0.2.0","draft":false,"prerelease":true}]"#,
    );
    let tag = resolve_latest_version_at(&server.base, Duration::from_secs(5)).unwrap();
    assert_eq!(tag, "v0.2.0");
}

#[test]
fn resolve_latest_reports_rate_limiting_and_missing_releases() {
    let server = FakeServer::start();
    server.route(
        "/repos/CodeVachon/merge-pipeline/releases/latest",
        403,
        r#"{"message":"API rate limit exceeded for 1.2.3.4."}"#,
    );
    let err = resolve_latest_version_at(&server.base, Duration::from_secs(5)).unwrap_err();
    assert!(matches!(err, ReleaseError::RateLimited), "{err}");

    let server = FakeServer::start();
    server.route(
        "/repos/CodeVachon/merge-pipeline/releases/latest",
        404,
        "{}",
    );
    server.route("/repos/CodeVachon/merge-pipeline/releases", 200, "[]");
    let err = resolve_latest_version_at(&server.base, Duration::from_secs(5)).unwrap_err();
    assert!(matches!(err, ReleaseError::NoRelease), "{err}");

    let server = FakeServer::start();
    server.route(
        "/repos/CodeVachon/merge-pipeline/releases/latest",
        404,
        "{}",
    );
    server.route("/repos/CodeVachon/merge-pipeline/releases", 500, "boom");
    let err = resolve_latest_version_at(&server.base, Duration::from_secs(5)).unwrap_err();
    assert!(matches!(err, ReleaseError::Status(500)), "{err}");
}

#[test]
fn fetch_verified_binary_refuses_a_bad_checksum() {
    let server = FakeServer::start();
    let target = host_target_for("linux", "x86_64").unwrap();
    server.route("/download/merge-pipeline-linux-x64.gz", 200, gz(b"payload"));
    server.route(
        "/download/checksums.txt",
        200,
        "0000  merge-pipeline-linux-x64.gz\n",
    );
    let err = fetch_verified_binary_from(&format!("{}/download", server.base), "v1.0.0", &target)
        .unwrap_err();
    assert!(err.to_string().contains("checksum mismatch"), "{err}");

    server.route("/download/checksums.txt", 404, "gone");
    let err = fetch_verified_binary_from(&format!("{}/download", server.base), "v1.0.0", &target)
        .unwrap_err();
    assert!(err.to_string().contains("HTTP 404"), "{err}");
}

#[cfg(unix)]
#[test]
fn upgrade_installs_the_new_version_repoints_current_and_prunes() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    // Three older versions already on disk; v0.2.0 is the one "running".
    for old in ["v0.0.1", "v0.1.0"] {
        fs::create_dir_all(root.join("versions").join(old).join("bin")).unwrap();
    }
    let install = fake_install(&root, "v0.2.0");
    let root = install.root.clone();

    let server = FakeServer::start();
    publish(&server, "v0.3.0", b"#!/bin/sh\necho new\n");

    let mut out = Vec::new();
    let code = run_upgrade_with(
        &UpgradeOptions {
            keep: Some(1),
            json: true,
            ..Default::default()
        },
        &context(install, "0.2.0", &server),
        &mut out,
    )
    .unwrap();
    assert_eq!(code, 0);
    let text = String::from_utf8(out).unwrap();

    assert!(
        text.contains("downloading merge-pipeline v0.3.0 for linux-x64..."),
        "{text}"
    );
    assert!(text.contains("✓ checksum verified"), "{text}");
    assert!(text.contains("✓ installed v0.3.0"), "{text}");
    assert!(text.contains("pruned v0.1.0, v0.0.1"), "{text}");
    assert!(
        text.contains("kept v0.2.0 — it is the binary currently running"),
        "{text}"
    );

    let json_start = text.find('{').unwrap();
    let report: serde_json::Value = serde_json::from_str(&text[json_start..]).unwrap();
    assert_eq!(report["from"], "v0.2.0");
    assert_eq!(report["to"], "v0.3.0");
    assert_eq!(report["pruned"], serde_json::json!(["v0.1.0", "v0.0.1"]));
    assert_eq!(report["keptRunning"], "v0.2.0");
    assert_eq!(report["installRoot"], root.to_str().unwrap());

    assert_eq!(current_target(&root), root.join("versions/v0.3.0"));
    assert_eq!(
        fs::read(root.join("current/bin/merge-pipeline")).unwrap(),
        b"#!/bin/sh\necho new\n"
    );
    assert!(
        root.join("versions/v0.2.0").exists(),
        "running version kept"
    );
    assert!(!root.join("versions/v0.1.0").exists());
    assert!(!root.join("versions/v0.0.1").exists());

    let record = read_check_record(&root).expect("update-check.json written");
    assert_eq!(record.latest.as_deref(), Some("v0.3.0"));
}

#[cfg(unix)]
#[test]
fn upgrade_check_reports_without_changing_anything() {
    let temp = tempfile::tempdir().unwrap();
    let install = fake_install(&temp.path().join("root"), "v0.2.0");
    let root = install.root.clone();
    let server = FakeServer::start();
    publish(&server, "v0.3.0", b"new");

    let mut out = Vec::new();
    let code = run_upgrade_with(
        &UpgradeOptions {
            check: true,
            ..Default::default()
        },
        &context(install.clone(), "v0.2.0", &server),
        &mut out,
    )
    .unwrap();
    assert_eq!(code, 0);
    let text = String::from_utf8(out).unwrap();
    assert!(
        text.contains("✓ merge-pipeline v0.3.0 is available (you have v0.2.0)"),
        "{text}"
    );
    assert!(text.contains("  merge-pipeline upgrade"), "{text}");
    assert!(
        !root.join("versions/v0.3.0").exists(),
        "check must not install"
    );
    assert_eq!(current_target(&root), root.join("versions/v0.2.0"));

    let mut out = Vec::new();
    run_upgrade_with(
        &UpgradeOptions {
            check: true,
            json: true,
            ..Default::default()
        },
        &context(install, "v0.2.0", &server),
        &mut out,
    )
    .unwrap();
    let report: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(report["current"], "v0.2.0");
    assert_eq!(report["latest"], "v0.3.0");
    assert_eq!(report["upToDate"], false);
    assert_eq!(report["isDowngrade"], false);
    assert_eq!(report["installed"], serde_json::json!(["v0.2.0"]));
}

#[cfg(unix)]
#[test]
fn upgrade_recognises_up_to_date_force_and_downgrades() {
    let temp = tempfile::tempdir().unwrap();
    let install = fake_install(&temp.path().join("root"), "v0.3.0");
    let root = install.root.clone();
    let server = FakeServer::start();
    publish(&server, "v0.3.0", b"same");

    let mut out = Vec::new();
    run_upgrade_with(
        &UpgradeOptions::default(),
        &context(install.clone(), "0.3.0", &server),
        &mut out,
    )
    .unwrap();
    assert_eq!(
        String::from_utf8(out).unwrap().trim(),
        "✓ already on v0.3.0 — pass --force to reinstall"
    );

    // --force reinstalls the same version.
    let mut out = Vec::new();
    run_upgrade_with(
        &UpgradeOptions {
            force: true,
            ..Default::default()
        },
        &context(install.clone(), "0.3.0", &server),
        &mut out,
    )
    .unwrap();
    assert_eq!(
        fs::read(root.join("versions/v0.3.0/bin/merge-pipeline")).unwrap(),
        b"same"
    );

    // An explicit older target is a downgrade: warned about, then installed.
    let archive = gz(b"older");
    server.route(
        "/download/merge-pipeline-linux-x64.gz",
        200,
        archive.clone(),
    );
    server.route(
        "/download/checksums.txt",
        200,
        format!("{}  merge-pipeline-linux-x64.gz\n", sha256_hex(&archive)),
    );
    let mut out = Vec::new();
    run_upgrade_with(
        &UpgradeOptions {
            target: Some("0.1.0".into()),
            ..Default::default()
        },
        &context(install, "0.3.0", &server),
        &mut out,
    )
    .unwrap();
    let text = String::from_utf8(out).unwrap();
    assert!(
        text.contains("! v0.1.0 is older than the running v0.3.0"),
        "{text}"
    );
    assert_eq!(current_target(&root), root.join("versions/v0.1.0"));
    // No API lookup happens for an explicit target, so the record still holds what the earlier
    // latest-lookups found rather than v0.1.0.
    assert_eq!(
        read_check_record(&root).unwrap().latest.as_deref(),
        Some("v0.3.0")
    );
}

#[cfg(unix)]
#[test]
fn uninstall_requires_yes_then_removes_the_root_and_owned_links() {
    let temp = tempfile::tempdir().unwrap();
    let install = fake_install(&temp.path().join("root"), "v0.2.0");
    let root = install.root.clone();
    let bin_dir = temp.path().join("local-bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let ours = bin_dir.join("merge-pipeline");
    std::os::unix::fs::symlink(root.join("current/bin/merge-pipeline"), &ours).unwrap();
    // A merge-pipeline.exe link pointing elsewhere must be left alone.
    let foreign_target = temp.path().join("elsewhere");
    fs::write(&foreign_target, b"x").unwrap();
    let foreign = bin_dir.join("merge-pipeline.exe");
    std::os::unix::fs::symlink(&foreign_target, &foreign).unwrap();

    // The bin dir is discovered through this variable; set it for the duration of the test.
    // SAFETY: tests in this binary that read MERGE_PIPELINE_BIN_DIR all run through this
    // function, and cargo runs integration test functions on separate threads only within one
    // process; the variable is scoped to this test's lifetime.
    unsafe { std::env::set_var("MERGE_PIPELINE_BIN_DIR", &bin_dir) };

    let mut out = Vec::new();
    let code = run_uninstall_with(&UninstallOptions::default(), &install, &mut out).unwrap();
    assert_eq!(code, 1);
    let text = String::from_utf8(out).unwrap();
    assert!(
        text.contains(&format!("! this will remove {}", root.display())),
        "{text}"
    );
    assert!(
        text.contains(&format!("and the symlink {}", ours.display())),
        "{text}"
    );
    assert!(text.contains("Re-run with --yes to proceed."), "{text}");
    assert!(root.exists(), "nothing removed without --yes");

    let mut out = Vec::new();
    let code = run_uninstall_with(
        &UninstallOptions {
            yes: true,
            json: true,
        },
        &install,
        &mut out,
    )
    .unwrap();
    assert_eq!(code, 0);
    let report: serde_json::Value = serde_json::from_slice(&out).unwrap();
    let removed = report["removed"].as_array().unwrap();
    assert_eq!(removed[0], root.to_str().unwrap());
    assert_eq!(removed[1], ours.to_str().unwrap());
    assert!(!root.exists());
    assert!(fs::symlink_metadata(&ours).is_err(), "our link removed");
    assert!(fs::symlink_metadata(&foreign).is_ok(), "foreign link kept");

    unsafe { std::env::remove_var("MERGE_PIPELINE_BIN_DIR") };
}
