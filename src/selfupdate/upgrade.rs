//! `merge-pipeline upgrade` and `merge-pipeline uninstall`.
//!
//! Output lines and `--json` shapes follow motte's `upgrade` command so the two tools read the
//! same on a machine that has both.

use std::cmp::Ordering;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

use super::download::{DownloadError, fetch_verified_binary_from, install_binary, prune_versions};
use super::layout::{
    HostTarget, ManagedInstall, REPO, UnsupportedHostError, candidate_bin_dirs,
    compare_versions_descending, download_base, host_target, installed_versions, locate_install,
    normalize_version,
};
use super::releases::{ReleaseError, api_base, resolve_latest_version_at};

/// Printed when the running binary is not part of a managed installation.
pub const INSTALL_HINT: &str = "merge-pipeline is not running from a managed installation, so there is nothing to upgrade in place.
  This happens when running from source, or when the binary was copied somewhere by hand.
  To install a managed copy:
    curl -fsSL https://raw.githubusercontent.com/CodeVachon/merge-pipeline/main/install.sh | sh";

/// Why an upgrade or uninstall did not complete.
#[derive(Debug, Error)]
pub enum UpgradeError {
    #[error("{INSTALL_HINT}")]
    NotInstalled,
    #[error(transparent)]
    UnsupportedHost(#[from] UnsupportedHostError),
    #[error(transparent)]
    Release(#[from] ReleaseError),
    #[error(transparent)]
    Download(#[from] DownloadError),
    #[error("could not update the installation: {0}")]
    Io(#[from] std::io::Error),
    #[error("could not write output: {0}")]
    Output(std::io::Error),
}

/// Flags for `upgrade`.
#[derive(Debug, Clone, Default)]
pub struct UpgradeOptions {
    /// Install this version instead of the newest.
    pub target: Option<String>,
    /// Report whether an update is available, changing nothing.
    pub check: bool,
    /// How many versions to keep on disk (default 2).
    pub keep: Option<usize>,
    /// Reinstall even if already on the target version.
    pub force: bool,
    /// Machine-readable output.
    pub json: bool,
}

/// Flags for `uninstall`.
#[derive(Debug, Clone, Default)]
pub struct UninstallOptions {
    /// Skip the confirmation.
    pub yes: bool,
    /// Machine-readable output.
    pub json: bool,
}

/// Everything `upgrade` needs from its environment, so tests can supply a fake install,
/// version and servers without touching the real home directory.
#[derive(Debug, Clone)]
pub struct UpgradeContext {
    pub install: ManagedInstall,
    pub current_version: String,
    pub host: HostTarget,
    pub api_base: String,
    /// Base URL assets are downloaded from, or `None` to derive it from the version.
    pub download_base: Option<String>,
    pub api_timeout: Duration,
}

impl UpgradeContext {
    /// The real environment: the running binary's install, version and host.
    pub fn detect() -> Result<Self, UpgradeError> {
        let install = locate_install().ok_or(UpgradeError::NotInstalled)?;
        Ok(Self {
            install,
            current_version: crate::VERSION.to_owned(),
            host: host_target()?,
            api_base: api_base(),
            download_base: None,
            api_timeout: Duration::from_secs(20),
        })
    }
}

/// What `update-check.json` records.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckRecord {
    pub last_attempt_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_success_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest: Option<String>,
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Path of the update-check cache under `root`.
pub fn check_record_path(root: &Path) -> PathBuf {
    root.join("update-check.json")
}

/// Read the cache, tolerating absence and corruption.
pub fn read_check_record(root: &Path) -> Option<CheckRecord> {
    let text = fs::read_to_string(check_record_path(root)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Record a lookup attempt and, when it succeeded, the version it found. Best effort.
pub fn record_check(root: &Path, latest: Option<&str>) {
    let now = now_millis();
    let mut record = read_check_record(root).unwrap_or_default();
    record.last_attempt_at = now;
    if let Some(latest) = latest {
        record.last_success_at = Some(now);
        record.latest = Some(latest.to_owned());
    }
    if let Ok(text) = serde_json::to_string_pretty(&record) {
        let _ = fs::create_dir_all(root);
        let _ = fs::write(check_record_path(root), format!("{text}\n"));
    }
}

fn emit(out: &mut dyn Write, line: impl AsRef<str>) -> Result<(), UpgradeError> {
    writeln!(out, "{}", line.as_ref()).map_err(UpgradeError::Output)
}

fn emit_json(out: &mut dyn Write, value: serde_json::Value) -> Result<(), UpgradeError> {
    emit(
        out,
        serde_json::to_string_pretty(&value).unwrap_or_default(),
    )
}

/// Run `upgrade` against the real environment. Returns the process exit code.
pub fn run_upgrade(options: &UpgradeOptions, out: &mut dyn Write) -> Result<i32, UpgradeError> {
    let ctx = UpgradeContext::detect()?;
    run_upgrade_with(options, &ctx, out)
}

/// Run `upgrade` with an explicit context. Returns the process exit code.
pub fn run_upgrade_with(
    options: &UpgradeOptions,
    ctx: &UpgradeContext,
    out: &mut dyn Write,
) -> Result<i32, UpgradeError> {
    let install = &ctx.install;
    let wanted = match &options.target {
        Some(target) => normalize_version(target),
        None => {
            let latest = resolve_latest_version_at(&ctx.api_base, ctx.api_timeout);
            record_check(&install.root, latest.as_deref().ok());
            normalize_version(&latest?)
        }
    };
    let current = normalize_version(&ctx.current_version);

    // Semantic comparison, so `upgrade 0.0.9` is recognised as a downgrade rather than treated
    // like an upgrade. `compare_versions_descending(a, b)` is Greater when a is OLDER than b.
    let ordering = compare_versions_descending(&wanted, &current);
    let up_to_date = ordering == Ordering::Equal;
    let is_downgrade = ordering == Ordering::Greater;

    if options.check {
        if options.json {
            emit_json(
                out,
                json!({
                    "current": current,
                    "latest": wanted,
                    "upToDate": up_to_date,
                    "isDowngrade": is_downgrade,
                    "installRoot": install.root,
                    "installed": installed_versions(&install.versions_dir),
                }),
            )?;
            return Ok(0);
        }
        if up_to_date {
            emit(
                out,
                format!("✓ merge-pipeline {current} is the newest release"),
            )?;
        } else {
            emit(
                out,
                format!("✓ merge-pipeline {wanted} is available (you have {current})"),
            )?;
            emit(out, "  merge-pipeline upgrade")?;
        }
        return Ok(0);
    }

    if up_to_date && !options.force {
        emit(
            out,
            format!("✓ already on {current} — pass --force to reinstall"),
        )?;
        return Ok(0);
    }

    if is_downgrade {
        emit(
            out,
            format!("! {wanted} is older than the running {current}"),
        )?;
    }

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

    let ordered = installed_versions(&install.versions_dir);
    let pruned = prune_versions(
        &install.versions_dir,
        &ordered,
        options.keep.unwrap_or(2),
        &install.version,
    );

    if !pruned.removed.is_empty() {
        emit(out, format!("pruned {}", pruned.removed.join(", ")))?;
    }
    if let Some(kept) = &pruned.kept_running {
        emit(
            out,
            format!(
                "kept {kept} — it is the binary currently running; the next upgrade will remove it"
            ),
        )?;
    }

    if options.json {
        let mut value = json!({
            "from": current,
            "to": wanted,
            "pruned": pruned.removed,
            "installRoot": install.root,
        });
        if let Some(kept) = &pruned.kept_running {
            value["keptRunning"] = json!(kept);
        }
        emit_json(out, value)?;
        return Ok(0);
    }

    // PATH points at the stable `current` link, so there is no shell to restart.
    emit(
        out,
        super::versions::now_using_line(&wanted, Some(&current)),
    )?;
    Ok(0)
}

/// Symlinks on PATH that resolve into `root`. A `merge-pipeline` from a package manager or a
/// different installation is not ours to delete.
pub fn owned_bin_links(root: &Path) -> Vec<PathBuf> {
    let root = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let mut links = Vec::new();
    for dir in candidate_bin_dirs() {
        for name in ["merge-pipeline", "merge-pipeline.exe"] {
            let link = dir.join(name);
            if fs::symlink_metadata(&link).is_err() {
                continue;
            }
            if let Ok(real) = fs::canonicalize(&link)
                && real.starts_with(&root)
                && !links.contains(&link)
            {
                links.push(link);
            }
        }
    }
    links
}

/// Run `uninstall` against the real environment. Returns the process exit code.
pub fn run_uninstall(options: &UninstallOptions, out: &mut dyn Write) -> Result<i32, UpgradeError> {
    let install = locate_install().ok_or(UpgradeError::NotInstalled)?;
    run_uninstall_with(options, &install, out)
}

/// Run `uninstall` for an explicit installation. Returns the process exit code.
pub fn run_uninstall_with(
    options: &UninstallOptions,
    install: &ManagedInstall,
    out: &mut dyn Write,
) -> Result<i32, UpgradeError> {
    let links = owned_bin_links(&install.root);

    if !options.yes {
        emit(
            out,
            format!("! this will remove {}", install.root.display()),
        )?;
        for link in &links {
            emit(out, format!("  and the symlink {}", link.display()))?;
        }
        emit(out, "")?;
        emit(
            out,
            "Your workflow configs are untouched — only the installation is removed.",
        )?;
        emit(out, "Re-run with --yes to proceed.")?;
        return Ok(1);
    }

    for link in &links {
        let _ = fs::remove_file(link);
    }
    fs::remove_dir_all(&install.root)?;

    let mut removed: Vec<PathBuf> = vec![install.root.clone()];
    removed.extend(links.iter().cloned());

    if options.json {
        emit_json(out, json!({ "removed": removed }))?;
        return Ok(0);
    }

    for path in &removed {
        emit(out, format!("✓ removed {}", path.display()))?;
    }
    emit(out, "")?;
    emit(
        out,
        format!(
            "Reinstall any time: curl -fsSL https://raw.githubusercontent.com/{REPO}/main/install.sh | sh"
        ),
    )?;
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_record_round_trips_and_tolerates_corruption() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("root");
        assert!(read_check_record(&root).is_none());

        record_check(&root, None);
        let first = read_check_record(&root).unwrap();
        assert!(first.last_attempt_at > 0);
        assert!(first.latest.is_none());

        record_check(&root, Some("v1.2.3"));
        let second = read_check_record(&root).unwrap();
        assert_eq!(second.latest.as_deref(), Some("v1.2.3"));
        assert!(second.last_success_at.is_some());

        fs::write(check_record_path(&root), "{ not json").unwrap();
        assert!(read_check_record(&root).is_none());
        record_check(&root, Some("v2.0.0"));
        assert_eq!(
            read_check_record(&root).unwrap().latest.as_deref(),
            Some("v2.0.0")
        );
    }

    #[test]
    fn install_hint_names_the_installer() {
        assert!(INSTALL_HINT.contains("CodeVachon/merge-pipeline/main/install.sh"));
        assert!(
            UpgradeError::NotInstalled
                .to_string()
                .contains("install.sh | sh")
        );
    }
}
