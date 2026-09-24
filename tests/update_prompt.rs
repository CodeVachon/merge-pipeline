//! The once-a-day update offer, driven through the binary from a fake managed layout: the test
//! binary is copied into `versions/v<crate version>/bin` so `locate_install` recognises it, a
//! local fake server plays GitHub, and the "new version" it serves is a shell script that
//! records how it was re-executed. Nothing touches the network.
#![cfg(unix)]

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use flate2::Compression;
use flate2::write::GzEncoder;
use merge_pipeline::selfupdate::download::sha256_hex;
use merge_pipeline::selfupdate::host_target;

const RUNNING: &str = concat!("v", env!("CARGO_PKG_VERSION"));
const NEWER: &str = "v99.0.0";
/// What the fake newer binary prints when it runs.
const MARKER: &str = "REEXEC_MARKER";

struct Layout {
    _temp: tempfile::TempDir,
    root: PathBuf,
    exe: PathBuf,
}

/// versions/<RUNNING>/bin holds a copy of the real test binary, which is also `current`.
fn layout() -> Layout {
    let temp = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(temp.path()).unwrap().join("root");
    let bin = root.join("versions").join(RUNNING).join("bin");
    fs::create_dir_all(&bin).unwrap();
    let exe = bin.join("merge-pipeline");
    fs::copy(env!("CARGO_BIN_EXE_merge-pipeline"), &exe).unwrap();
    std::os::unix::fs::symlink(root.join("versions").join(RUNNING), root.join("current")).unwrap();
    Layout {
        _temp: temp,
        root,
        exe,
    }
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

/// A fake GitHub whose latest release is [`NEWER`], serving a shell script as its asset. The
/// script prints its arguments and the re-exec marker variable, so a run that continued on the
/// "new version" is visible in stdout.
fn github_offering_newer() -> String {
    let target = host_target().expect("test host is a built target");
    let script = format!(
        "#!/bin/sh\necho {MARKER} \"$@\"\necho REEXEC=${{MERGE_PIPELINE_REEXEC:-unset}}\nexit 0\n"
    );
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(script.as_bytes()).unwrap();
    let archive = encoder.finish().unwrap();

    let mut routes = HashMap::new();
    routes.insert(
        "/repos/CodeVachon/merge-pipeline/releases/latest".to_owned(),
        format!("{{\"tag_name\":\"{NEWER}\",\"draft\":false,\"prerelease\":false}}").into_bytes(),
    );
    routes.insert(format!("/download/{}", target.asset_name), archive.clone());
    routes.insert(
        "/download/checksums.txt".to_owned(),
        format!("{}  {}\n", sha256_hex(&archive), target.asset_name).into_bytes(),
    );
    fake_server(routes)
}

/// Run `exe` as the interactive command with `--yes`, pointed at the fake GitHub. `CI` is
/// removed because the offer is off under it, and GitHub Actions sets it.
fn run_interactive(exe: &Path, base: &str, extra_env: &[(&str, &str)]) -> (i32, String, String) {
    let cwd = tempfile::tempdir().unwrap();
    let mut command = Command::new(exe);
    command
        .args(["--yes", "-a", "test", "-c"])
        .arg(cwd.path())
        .env_remove("CI")
        .env_remove("MERGE_PIPELINE_NO_UPDATE_CHECK")
        .env_remove("MERGE_PIPELINE_REEXEC")
        .env("MERGE_PIPELINE_API_BASE", base)
        .env("MERGE_PIPELINE_DOWNLOAD_BASE", format!("{base}/download"));
    for (k, v) in extra_env {
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

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[test]
fn accepted_offer_installs_repoints_and_continues_the_run_on_the_new_version() {
    let layout = layout();
    let base = github_offering_newer();

    let (code, out, err) = run_interactive(&layout.exe, &base, &[]);
    assert_eq!(code, 0, "stdout:\n{out}\nstderr:\n{err}");

    // The upgrade ran with its usual lines...
    assert!(
        out.contains(&format!("downloading merge-pipeline {NEWER}")),
        "{out}"
    );
    assert!(out.contains("✓ checksum verified"), "{out}");
    assert!(out.contains(&format!("✓ installed {NEWER}")), "{out}");
    assert!(
        out.contains(&format!("now using {NEWER} (was {RUNNING})")),
        "{out}"
    );
    assert_eq!(current_of(&layout.root), NEWER);

    // ...and the process re-executed on the new binary with the same arguments and the guard set.
    let marker_line = out
        .lines()
        .find(|l| l.starts_with(MARKER))
        .unwrap_or_else(|| panic!("no re-exec marker in:\n{out}"));
    assert!(marker_line.contains("--yes -a test -c "), "{marker_line}");
    assert!(out.contains("REEXEC=1"), "{out}");
    // The rest of the original run never happened in this process.
    assert!(!out.contains("Task Complete"), "{out}");
    assert!(!out.contains("Work Complete"), "{out}");

    let record: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(layout.root.join("update-check.json")).unwrap())
            .unwrap();
    assert_eq!(record["latest"], NEWER);
}

#[test]
fn within_a_day_of_the_last_attempt_nothing_is_offered() {
    let layout = layout();
    let base = github_offering_newer();
    fs::write(
        layout.root.join("update-check.json"),
        format!(
            "{{\"lastAttemptAt\":{},\"latest\":\"{NEWER}\"}}\n",
            now_millis() - 60_000
        ),
    )
    .unwrap();

    let (_, out, _) = run_interactive(&layout.exe, &base, &[]);
    assert!(!out.contains("downloading"), "{out}");
    assert!(!out.contains(MARKER), "{out}");
    // The run itself proceeded (it reaches config discovery and fails there, on this version).
    assert!(out.contains("Work Complete"), "{out}");
    assert_eq!(current_of(&layout.root), RUNNING);
}

#[test]
fn opt_out_variable_and_reexec_guard_suppress_the_offer() {
    let layout = layout();
    let base = github_offering_newer();

    for env in [
        ("MERGE_PIPELINE_NO_UPDATE_CHECK", "1"),
        ("MERGE_PIPELINE_REEXEC", "1"),
    ] {
        let (_, out, _) = run_interactive(&layout.exe, &base, &[env]);
        assert!(!out.contains("downloading"), "{env:?}: {out}");
        assert!(out.contains("Work Complete"), "{env:?}: {out}");
        assert!(
            !layout.root.join("update-check.json").exists(),
            "{env:?} must not even look"
        );
    }
    assert_eq!(current_of(&layout.root), RUNNING);
}

#[test]
fn an_unmanaged_binary_is_never_offered_an_update() {
    let base = github_offering_newer();
    let (_, out, _) = run_interactive(Path::new(env!("CARGO_BIN_EXE_merge-pipeline")), &base, &[]);
    assert!(!out.contains("downloading"), "{out}");
    assert!(!out.contains(MARKER), "{out}");
    assert!(out.contains("Work Complete"), "{out}");
}

#[test]
fn an_unreachable_release_api_is_silent_and_the_run_proceeds() {
    let layout = layout();
    // A bound-then-dropped port: connection refused, quickly.
    let refused = {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        format!("http://{}", listener.local_addr().unwrap())
    };

    let (_, out, err) = run_interactive(&layout.exe, &refused, &[]);
    assert!(!out.contains("downloading"), "{out}");
    assert!(!err.contains("warning"), "{err}");
    assert!(out.contains("Work Complete"), "{out}");
    // The failed attempt still counts for the day.
    let record: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(layout.root.join("update-check.json")).unwrap())
            .unwrap();
    assert!(record["lastAttemptAt"].as_u64().unwrap() > 0);
    assert!(record["latest"].is_null());
}
