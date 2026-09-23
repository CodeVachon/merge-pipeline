//! Manage the versions installed under `<root>/versions`: list, switch, remove, prune.
//!
//! The layout makes this cheap. `<bin>/merge-pipeline` on PATH points at `<root>/current`, and
//! `current` points at one version directory, so switching versions is repointing one link and
//! takes effect on the next invocation. Nothing here needs a shell restart, and nothing here
//! touches the network except `use` of a version that is not installed yet.

use std::fs;
use std::io::Write;
use std::path::Path;

use serde::Serialize;
use serde_json::json;
use thiserror::Error;

use super::download::{
    DownloadError, fetch_verified_binary_from, install_binary, point_current_at,
};
use super::layout::{
    ManagedInstall, compare_versions_descending, download_base, installed_versions,
    normalize_version, parse_version,
};
use super::releases::{ReleaseError, resolve_latest_version_at};
use super::upgrade::{UpgradeContext, read_check_record, record_check};

/// One installed version and its role.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VersionEntry {
    pub version: String,
    /// `current` points at this version: it is what the next invocation runs.
    pub current: bool,
    /// The binary executing right now lives in this version directory.
    pub running: bool,
}

/// Why a version could not be switched to, removed, or pruned.
#[derive(Debug, Error)]
pub enum VersionsError {
    #[error("{0} is not a version (expected something like 0.2.0 or v0.2.0)")]
    Invalid(String),
    #[error("{0} is not installed; `merge-pipeline use {0}` downloads and switches to it")]
    NotInstalled(String),
    #[error(transparent)]
    Download(#[from] DownloadError),
    #[error(transparent)]
    Release(#[from] ReleaseError),
    #[error("could not update the installation: {0}")]
    Io(#[from] std::io::Error),
    #[error("could not write output: {0}")]
    Output(std::io::Error),
}

/// The version `current` points at, when it is a link to a version directory.
///
/// `None` on a Windows install that fell back to copying `bin/` into `current`, and whenever
/// the link is missing or points somewhere that is not a version.
pub fn current_version(root: &Path) -> Option<String> {
    let target = fs::read_link(root.join("current")).ok()?;
    let name = target.file_name()?.to_str()?.to_owned();
    parse_version(&name).map(|_| name)
}

/// Installed versions, newest first, with `current` and `running` marked.
pub fn list(install: &ManagedInstall) -> Vec<VersionEntry> {
    let current = current_version(&install.root);
    installed_versions(&install.versions_dir)
        .into_iter()
        .map(|version| VersionEntry {
            current: current.as_deref() == Some(version.as_str()),
            running: version == install.version,
            version,
        })
        .collect()
}

/// What [`switch`] did.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Switched {
    /// What `current` pointed at before, if it was a version.
    pub from: Option<String>,
    pub to: String,
}

fn require_version(raw: &str) -> Result<String, VersionsError> {
    let normalized = normalize_version(raw);
    parse_version(&normalized)
        .map(|_| normalized)
        .ok_or_else(|| VersionsError::Invalid(raw.trim().to_owned()))
}

/// Point `current` at an installed `version`.
pub fn switch(install: &ManagedInstall, version: &str) -> Result<Switched, VersionsError> {
    let to = require_version(version)?;
    let version_dir = install.versions_dir.join(&to);
    if !version_dir.join("bin").is_dir() {
        return Err(VersionsError::NotInstalled(to));
    }
    let from = current_version(&install.root);
    point_current_at(&install.root, &version_dir)?;
    Ok(Switched { from, to })
}

/// A version that was asked about but left in place, and why.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Kept {
    pub version: String,
    pub reason: String,
}

/// What [`remove`] and [`prune`] did.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemoveReport {
    pub removed: Vec<String>,
    pub kept: Vec<Kept>,
    /// Asked for but not on disk (remove only).
    pub missing: Vec<String>,
}

const REASON_CURRENT: &str = "it is the active version; `merge-pipeline use <other>` first";
const REASON_RUNNING: &str = "it is the binary currently running";

fn protection_reason(
    install: &ManagedInstall,
    current: Option<&str>,
    version: &str,
) -> Option<&'static str> {
    if current == Some(version) {
        Some(REASON_CURRENT)
    } else if version == install.version {
        Some(REASON_RUNNING)
    } else {
        None
    }
}

/// Delete the named version directories, never the active or the running one.
pub fn remove(
    install: &ManagedInstall,
    versions: &[String],
) -> Result<RemoveReport, VersionsError> {
    let current = current_version(&install.root);
    let mut report = RemoveReport::default();
    for raw in versions {
        let version = require_version(raw)?;
        if report.removed.contains(&version) {
            continue;
        }
        let dir = install.versions_dir.join(&version);
        if !dir.exists() {
            report.missing.push(version);
            continue;
        }
        if let Some(reason) = protection_reason(install, current.as_deref(), &version) {
            report.kept.push(Kept {
                version,
                reason: reason.to_owned(),
            });
            continue;
        }
        fs::remove_dir_all(&dir)?;
        report.removed.push(version);
    }
    Ok(report)
}

/// Remove all but the newest `keep` versions, never the active or the running one.
///
/// Unlike `download::prune_versions`, which `upgrade` uses right after installing the newest
/// version, this also protects `current`, because here `current` may point at an older version
/// the user chose with `use`.
pub fn prune(install: &ManagedInstall, keep: usize) -> Result<RemoveReport, VersionsError> {
    let current = current_version(&install.root);
    let mut report = RemoveReport::default();
    for version in installed_versions(&install.versions_dir)
        .into_iter()
        .skip(keep.max(1))
    {
        if let Some(reason) = protection_reason(install, current.as_deref(), &version) {
            report.kept.push(Kept {
                version,
                reason: reason.to_owned(),
            });
            continue;
        }
        fs::remove_dir_all(install.versions_dir.join(&version))?;
        report.removed.push(version);
    }
    Ok(report)
}

/// The line every version change ends with. There is no restart to do: PATH points at the
/// stable `current` link, so the next command already runs the new version.
pub fn now_using_line(to: &str, from: Option<&str>) -> String {
    match from {
        Some(from) if from != to => {
            format!("now using {to} (was {from}); takes effect on your next merge-pipeline command")
        }
        _ => format!("now using {to}; takes effect on your next merge-pipeline command"),
    }
}

fn emit(out: &mut dyn Write, line: impl AsRef<str>) -> Result<(), VersionsError> {
    writeln!(out, "{}", line.as_ref()).map_err(VersionsError::Output)
}

fn emit_json(out: &mut dyn Write, value: serde_json::Value) -> Result<(), VersionsError> {
    emit(
        out,
        serde_json::to_string_pretty(&value).unwrap_or_default(),
    )
}

/// Flags for `versions` (the list).
#[derive(Debug, Clone, Default)]
pub struct ListOptions {
    /// Ask the releases API for the newest version instead of relying on the cache.
    pub check: bool,
    pub json: bool,
}

/// `merge-pipeline versions`: list installed versions. Returns the exit code.
pub fn run_list_with(
    options: &ListOptions,
    ctx: &UpgradeContext,
    out: &mut dyn Write,
) -> Result<i32, VersionsError> {
    let install = &ctx.install;
    let entries = list(install);

    let latest = if options.check {
        let found = resolve_latest_version_at(&ctx.api_base, ctx.api_timeout);
        record_check(&install.root, found.as_deref().ok());
        Some(normalize_version(&found?))
    } else {
        read_check_record(&install.root).and_then(|r| r.latest)
    };
    // Only worth mentioning when it is newer than everything on disk.
    let newer_available = latest.filter(|latest| {
        entries.first().is_none_or(|newest| {
            compare_versions_descending(latest, &newest.version) == std::cmp::Ordering::Less
        })
    });

    if options.json {
        emit_json(
            out,
            json!({
                "installRoot": install.root,
                "current": current_version(&install.root),
                "running": install.version,
                "versions": entries,
                "latest": newer_available,
            }),
        )?;
        return Ok(0);
    }

    if entries.is_empty() {
        emit(out, "no versions installed")?;
    }
    for entry in &entries {
        let marker = if entry.current { "*" } else { " " };
        let mut roles = Vec::new();
        if entry.current {
            roles.push("current");
        }
        if entry.running {
            roles.push("running");
        }
        let suffix = if roles.is_empty() {
            String::new()
        } else {
            format!("  {}", roles.join(", "))
        };
        emit(out, format!("{marker} {}{suffix}", entry.version))?;
    }
    if let Some(latest) = newer_available {
        emit(
            out,
            format!(
                "\n{latest} is available: merge-pipeline upgrade, or merge-pipeline use {latest}"
            ),
        )?;
    }
    Ok(0)
}

/// Flags for `use`.
#[derive(Debug, Clone, Default)]
pub struct UseOptions {
    pub version: String,
    pub json: bool,
}

/// `merge-pipeline use <version>`: switch to an installed version, downloading it first if it
/// is not on disk. Returns the exit code.
pub fn run_use_with(
    options: &UseOptions,
    ctx: &UpgradeContext,
    out: &mut dyn Write,
) -> Result<i32, VersionsError> {
    let install = &ctx.install;
    let wanted = require_version(&options.version)?;
    let from = current_version(&install.root);

    let downloaded = !install.versions_dir.join(&wanted).join("bin").is_dir();
    if downloaded {
        emit(
            out,
            format!(
                "downloading merge-pipeline {wanted} for {}-{}...",
                ctx.host.platform.as_str(),
                ctx.host.arch.as_str()
            ),
        )?;
        let base = ctx
            .download_base
            .clone()
            .unwrap_or_else(|| download_base(&wanted));
        let binary = fetch_verified_binary_from(&base, &wanted, &ctx.host)?;
        emit(out, "✓ checksum verified")?;
        install_binary(&install.root, &wanted, &binary, ctx.host.is_windows())?;
        emit(out, format!("✓ installed {wanted}"))?;
    } else {
        switch(install, &wanted)?;
    }

    if options.json {
        emit_json(
            out,
            json!({
                "from": from,
                "to": wanted,
                "downloaded": downloaded,
                "installRoot": install.root,
            }),
        )?;
        return Ok(0);
    }
    emit(out, now_using_line(&wanted, from.as_deref()))?;
    Ok(0)
}

/// Flags for `versions remove`.
#[derive(Debug, Clone, Default)]
pub struct RemoveOptions {
    pub versions: Vec<String>,
    pub json: bool,
}

fn render_remove_report(
    report: &RemoveReport,
    json: bool,
    install: &ManagedInstall,
    out: &mut dyn Write,
) -> Result<(), VersionsError> {
    if json {
        return emit_json(
            out,
            json!({
                "removed": report.removed,
                "kept": report.kept,
                "missing": report.missing,
                "installRoot": install.root,
            }),
        );
    }
    for version in &report.removed {
        emit(out, format!("✓ removed {version}"))?;
    }
    for kept in &report.kept {
        emit(out, format!("! kept {}: {}", kept.version, kept.reason))?;
    }
    for version in &report.missing {
        emit(out, format!("! {version} is not installed"))?;
    }
    if report.removed.is_empty() && report.kept.is_empty() && report.missing.is_empty() {
        emit(out, "nothing to remove")?;
    }
    Ok(())
}

/// `merge-pipeline versions remove <version>...`. Exit 1 when anything asked for was refused
/// or missing, so scripts notice.
pub fn run_remove_with(
    options: &RemoveOptions,
    install: &ManagedInstall,
    out: &mut dyn Write,
) -> Result<i32, VersionsError> {
    let report = remove(install, &options.versions)?;
    render_remove_report(&report, options.json, install, out)?;
    Ok(if report.kept.is_empty() && report.missing.is_empty() {
        0
    } else {
        1
    })
}

/// Flags for `versions prune`.
#[derive(Debug, Clone)]
pub struct PruneOptions {
    pub keep: usize,
    pub json: bool,
}

/// `merge-pipeline versions prune [--keep N]`. Keeping the active or running version past the
/// cutoff is expected, so it is reported but is not a failure.
pub fn run_prune_with(
    options: &PruneOptions,
    install: &ManagedInstall,
    out: &mut dyn Write,
) -> Result<i32, VersionsError> {
    let report = prune(install, options.keep)?;
    render_remove_report(&report, options.json, install, out)?;
    Ok(0)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A fake managed install: `versions` exist as directories with a `bin/`, `current` points
    /// at `current`, and the binary "runs" from `running`.
    fn fake(temp: &Path, versions: &[&str], current: &str, running: &str) -> ManagedInstall {
        let root = temp.join("root");
        for v in versions {
            fs::create_dir_all(root.join("versions").join(v).join("bin")).unwrap();
            fs::write(
                root.join("versions")
                    .join(v)
                    .join("bin")
                    .join("merge-pipeline"),
                v.as_bytes(),
            )
            .unwrap();
        }
        std::os::unix::fs::symlink(root.join("versions").join(current), root.join("current"))
            .unwrap();
        ManagedInstall {
            current_link: root.join("current"),
            versions_dir: root.join("versions"),
            binary: root
                .join("versions")
                .join(running)
                .join("bin")
                .join("merge-pipeline"),
            root,
            version: running.to_owned(),
        }
    }

    fn current_target(root: &Path) -> PathBuf {
        fs::read_link(root.join("current")).unwrap()
    }

    #[test]
    fn list_marks_current_and_running_newest_first() {
        let temp = tempfile::tempdir().unwrap();
        let install = fake(
            temp.path(),
            &["v0.1.0", "v0.3.0", "v0.2.0"],
            "v0.2.0",
            "v0.3.0",
        );
        let entries = list(&install);
        assert_eq!(
            entries,
            vec![
                VersionEntry {
                    version: "v0.3.0".into(),
                    current: false,
                    running: true
                },
                VersionEntry {
                    version: "v0.2.0".into(),
                    current: true,
                    running: false
                },
                VersionEntry {
                    version: "v0.1.0".into(),
                    current: false,
                    running: false
                },
            ]
        );
        assert_eq!(current_version(&install.root).as_deref(), Some("v0.2.0"));
    }

    #[test]
    fn switch_repoints_current_and_reports_the_previous_version() {
        let temp = tempfile::tempdir().unwrap();
        let install = fake(temp.path(), &["v0.1.0", "v0.2.0"], "v0.2.0", "v0.2.0");

        let switched = switch(&install, "0.1.0").unwrap();
        assert_eq!(
            switched,
            Switched {
                from: Some("v0.2.0".into()),
                to: "v0.1.0".into()
            }
        );
        assert_eq!(
            current_target(&install.root),
            install.root.join("versions/v0.1.0")
        );
        assert_eq!(
            fs::read(install.root.join("current/bin/merge-pipeline")).unwrap(),
            b"v0.1.0"
        );
        assert!(list(&install)[1].current, "list reflects the switch");

        let err = switch(&install, "v9.9.9").unwrap_err();
        assert!(
            matches!(err, VersionsError::NotInstalled(ref v) if v == "v9.9.9"),
            "{err}"
        );
        let err = switch(&install, "latest").unwrap_err();
        assert!(matches!(err, VersionsError::Invalid(_)), "{err}");
        // A failed switch leaves current alone.
        assert_eq!(
            current_target(&install.root),
            install.root.join("versions/v0.1.0")
        );
    }

    #[test]
    fn remove_refuses_current_and_running_with_reasons_and_removes_the_rest() {
        let temp = tempfile::tempdir().unwrap();
        let install = fake(
            temp.path(),
            &["v0.1.0", "v0.2.0", "v0.3.0", "v0.4.0"],
            "v0.3.0",
            "v0.4.0",
        );

        let report = remove(
            &install,
            &[
                "v0.3.0".into(),
                "0.4.0".into(),
                "v0.1.0".into(),
                "v0.1.0".into(),
                "v0.9.0".into(),
            ],
        )
        .unwrap();
        assert_eq!(report.removed, vec!["v0.1.0"]);
        assert_eq!(
            report.kept,
            vec![
                Kept {
                    version: "v0.3.0".into(),
                    reason: REASON_CURRENT.into()
                },
                Kept {
                    version: "v0.4.0".into(),
                    reason: REASON_RUNNING.into()
                },
            ]
        );
        assert_eq!(report.missing, vec!["v0.9.0"]);
        assert!(!install.versions_dir.join("v0.1.0").exists());
        assert!(install.versions_dir.join("v0.2.0").exists());
        assert!(install.versions_dir.join("v0.3.0").exists());
        assert!(install.versions_dir.join("v0.4.0").exists());
    }

    #[test]
    fn prune_keeps_the_newest_n_plus_current_and_running() {
        let temp = tempfile::tempdir().unwrap();
        // current is an old version chosen with `use`; running is a different old version.
        let install = fake(
            temp.path(),
            &["v0.1.0", "v0.2.0", "v0.3.0", "v0.4.0", "v0.5.0"],
            "v0.1.0",
            "v0.2.0",
        );

        let report = prune(&install, 1).unwrap();
        assert_eq!(report.removed, vec!["v0.4.0", "v0.3.0"]);
        assert_eq!(
            report.kept,
            vec![
                Kept {
                    version: "v0.2.0".into(),
                    reason: REASON_RUNNING.into()
                },
                Kept {
                    version: "v0.1.0".into(),
                    reason: REASON_CURRENT.into()
                },
            ]
        );
        assert!(report.missing.is_empty());
        assert_eq!(
            installed_versions(&install.versions_dir),
            vec!["v0.5.0", "v0.2.0", "v0.1.0"]
        );

        // keep=0 behaves as keep=1; nothing left that is removable.
        let report = prune(&install, 0).unwrap();
        assert!(report.removed.is_empty());
        assert_eq!(report.kept.len(), 2);
    }

    #[test]
    fn now_using_line_mentions_the_previous_version_only_when_it_changed() {
        assert_eq!(
            now_using_line("v0.2.0", Some("v0.1.0")),
            "now using v0.2.0 (was v0.1.0); takes effect on your next merge-pipeline command"
        );
        assert_eq!(
            now_using_line("v0.2.0", Some("v0.2.0")),
            "now using v0.2.0; takes effect on your next merge-pipeline command"
        );
        assert_eq!(
            now_using_line("v0.2.0", None),
            "now using v0.2.0; takes effect on your next merge-pipeline command"
        );
    }
}
