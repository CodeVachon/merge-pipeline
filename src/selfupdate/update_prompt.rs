//! A once-a-day offer to update, made at the start of an interactive run.
//!
//! When a newer release exists, the user is asked whether to install it now. On yes the same
//! code path as `upgrade` runs and the process re-executes itself on the new version with the
//! same arguments, so the run that follows is already on the new version. On no, or on any
//! failure, the run continues on the current version.
//!
//! This replaced the passive after-run nudge line: one question, at most once a day, and only
//! where it can act (a managed install, on a terminal).
//!
//! Opt out with `MERGE_PIPELINE_NO_UPDATE_CHECK=1`. Also off under `CI`, and inside the
//! re-executed child (`MERGE_PIPELINE_REEXEC=1`) so it cannot ask twice.

use std::cmp::Ordering;
use std::env;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::layout::{compare_versions_descending, normalize_version};
use super::releases::resolve_latest_version_at;
use super::upgrade::{
    CheckRecord, UpgradeContext, UpgradeOptions, read_check_record, record_check_at,
    run_upgrade_with,
};
use crate::prompt::{PromptError, Prompter, Question};

/// How long since the last attempt before the release API is asked again.
pub const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
/// How long the lookup may take before it is abandoned for this run.
pub const LOOKUP_TIMEOUT: Duration = Duration::from_millis(1500);
/// Set on the re-executed child so it does not offer again.
pub const REEXEC_ENV: &str = "MERGE_PIPELINE_REEXEC";
/// The prompt's question key, for scripted answers.
pub const QUESTION_KEY: &str = "update:upgrade";

fn env_set(name: &str) -> bool {
    env::var_os(name).is_some_and(|v| !v.is_empty())
}

/// Whether the offer is switched off by the environment.
pub fn disabled() -> bool {
    env_set("MERGE_PIPELINE_NO_UPDATE_CHECK") || env_set("CI") || env_set(REEXEC_ENV)
}

/// Milliseconds since the Unix epoch, or 0 if the clock is before it.
pub fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Whether a lookup is due: no record, or the last attempt is at least [`CHECK_INTERVAL`] old.
///
/// Every attempt counts, including one whose prompt was declined or whose lookup failed, so
/// the user is asked at most once per interval.
pub fn is_due(record: Option<&CheckRecord>, now_millis: u64) -> bool {
    match record {
        None => true,
        Some(record) => {
            now_millis.saturating_sub(record.last_attempt_at) >= CHECK_INTERVAL.as_millis() as u64
        }
    }
}

/// What the offer came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Offer {
    /// A lookup was made within the interval; nothing was asked.
    NotDue,
    /// The lookup failed or found nothing newer; nothing was asked.
    NothingNewer,
    /// The user said no.
    Declined { latest: String },
    /// Installed and made current; `binary` is the new executable to re-exec.
    Upgraded { to: String, binary: PathBuf },
    /// The user said yes but the upgrade did not complete.
    Failed { latest: String, error: String },
}

/// Ask GitHub for the newest release, with the short timeout this prompt allows itself.
pub fn lookup_latest(ctx: &UpgradeContext) -> Option<String> {
    resolve_latest_version_at(&ctx.api_base, LOOKUP_TIMEOUT).ok()
}

/// Make the offer against the real clock and release API.
pub fn offer_update(
    ctx: &UpgradeContext,
    prompter: &mut dyn Prompter,
    out: &mut dyn Write,
) -> Result<Offer, PromptError> {
    offer_update_with(ctx, prompter, out, now_millis(), || lookup_latest(ctx))
}

/// Make the offer with an explicit clock and lookup, so tests need neither a network nor a
/// real day to pass.
///
/// Records the attempt in `update-check.json` before asking anything, which is what makes a
/// declined prompt count towards the interval.
pub fn offer_update_with<F>(
    ctx: &UpgradeContext,
    prompter: &mut dyn Prompter,
    out: &mut dyn Write,
    now_millis: u64,
    lookup: F,
) -> Result<Offer, PromptError>
where
    F: FnOnce() -> Option<String>,
{
    let root = &ctx.install.root;
    if !is_due(read_check_record(root).as_ref(), now_millis) {
        return Ok(Offer::NotDue);
    }

    let looked_up = lookup();
    record_check_at(root, looked_up.as_deref(), now_millis);
    let Some(latest) = looked_up else {
        return Ok(Offer::NothingNewer);
    };

    let latest = normalize_version(&latest);
    let current = normalize_version(&ctx.current_version);
    // Greater means `current` is OLDER than `latest`.
    if compare_versions_descending(&current, &latest) != Ordering::Greater {
        return Ok(Offer::NothingNewer);
    }

    let question = Question::new(
        QUESTION_KEY,
        format!("merge-pipeline {latest} is available (you have {current}). Update now?"),
    );
    if !prompter.confirm(&question, true)? {
        return Ok(Offer::Declined { latest });
    }

    let options = UpgradeOptions {
        target: Some(latest.clone()),
        ..UpgradeOptions::default()
    };
    match run_upgrade_with(&options, ctx, out) {
        Ok(_) => Ok(Offer::Upgraded {
            binary: installed_binary(ctx, &latest),
            to: latest,
        }),
        Err(error) => Ok(Offer::Failed {
            latest,
            error: error.to_string(),
        }),
    }
}

/// Where `upgrade` put the binary for `version`.
fn installed_binary(ctx: &UpgradeContext, version: &str) -> PathBuf {
    ctx.install
        .versions_dir
        .join(version)
        .join("bin")
        .join(ctx.host.exe_name())
}

/// Continue this run on `binary`: same arguments, with [`REEXEC_ENV`] set so the child does
/// not offer again.
///
/// On unix this replaces the process and only returns if that failed. Elsewhere the child is
/// waited on and this process exits with its code.
pub fn reexec(binary: &Path) -> std::io::Error {
    let _ = std::io::stdout().flush();
    let mut command = Command::new(binary);
    command.args(env::args_os().skip(1)).env(REEXEC_ENV, "1");

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.exec()
    }
    #[cfg(not(unix))]
    {
        match command.status() {
            Ok(status) => std::process::exit(status.code().unwrap_or(1)),
            Err(error) => error,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::prompt::{RecordingPrompter, ScriptedPrompter};
    use crate::selfupdate::layout::{host_target, locate_install_from};
    use crate::selfupdate::upgrade::check_record_path;

    const DAY: u64 = 24 * 60 * 60 * 1000;

    fn record(last_attempt_at: u64) -> CheckRecord {
        CheckRecord {
            last_attempt_at,
            last_success_at: None,
            latest: None,
        }
    }

    #[test]
    fn due_when_no_record_or_a_day_has_passed_and_not_before() {
        let now = 10 * DAY;
        assert!(is_due(None, now));
        assert!(is_due(Some(&record(now - DAY)), now));
        assert!(is_due(Some(&record(now - 2 * DAY)), now));
        assert!(!is_due(Some(&record(now - DAY + 1)), now));
        assert!(!is_due(Some(&record(now - 60_000)), now));
        assert!(!is_due(Some(&record(now)), now));
        // A clock that went backwards must not ask forever.
        assert!(!is_due(Some(&record(now + DAY)), now));
    }

    /// A managed layout in a temp dir with the running "binary" at versions/<current>/bin.
    fn context(current: &str) -> (tempfile::TempDir, UpgradeContext) {
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap().join("root");
        let host = host_target().unwrap();
        let bin = root.join("versions").join(current).join("bin");
        fs::create_dir_all(&bin).unwrap();
        let exe = bin.join(host.exe_name());
        fs::write(&exe, "").unwrap();
        let install = locate_install_from(&exe).unwrap();
        let ctx = UpgradeContext {
            install,
            current_version: current.to_owned(),
            host,
            api_base: "http://127.0.0.1:9".to_owned(),
            download_base: Some("http://127.0.0.1:9".to_owned()),
            api_timeout: Duration::from_millis(10),
        };
        (temp, ctx)
    }

    #[test]
    fn declined_offer_continues_and_counts_towards_the_interval() {
        let (_temp, ctx) = context("v0.1.0");
        let mut prompter =
            RecordingPrompter::new(ScriptedPrompter::new().answer(QUESTION_KEY, false));
        let mut out = Vec::new();
        let now = 10 * DAY;

        let offer = offer_update_with(&ctx, &mut prompter, &mut out, now, || Some("v0.2.0".into()))
            .unwrap();
        assert_eq!(
            offer,
            Offer::Declined {
                latest: "v0.2.0".into()
            }
        );
        assert_eq!(prompter.keys(), vec![QUESTION_KEY]);
        assert!(
            out.is_empty(),
            "declining prints nothing from the module itself"
        );

        let saved = read_check_record(&ctx.install.root).unwrap();
        assert_eq!(saved.last_attempt_at, now);
        assert_eq!(saved.latest.as_deref(), Some("v0.2.0"));

        // Same day: not asked again, and the lookup is not even attempted.
        let mut prompter = RecordingPrompter::new(ScriptedPrompter::new());
        let offer = offer_update_with(&ctx, &mut prompter, &mut out, now + DAY / 2, || {
            panic!("lookup must not run within the interval")
        })
        .unwrap();
        assert_eq!(offer, Offer::NotDue);
        assert!(prompter.keys().is_empty());

        // Next day: asked again.
        let mut prompter =
            RecordingPrompter::new(ScriptedPrompter::new().answer(QUESTION_KEY, false));
        let offer = offer_update_with(&ctx, &mut prompter, &mut out, now + DAY, || {
            Some("v0.2.0".into())
        })
        .unwrap();
        assert!(matches!(offer, Offer::Declined { .. }));
        assert_eq!(prompter.keys(), vec![QUESTION_KEY]);
    }

    #[test]
    fn nothing_newer_or_failed_lookup_asks_nothing_but_still_records() {
        let (_temp, ctx) = context("v0.2.0");
        let mut prompter = RecordingPrompter::new(ScriptedPrompter::new());
        let mut out = Vec::new();

        let offer = offer_update_with(&ctx, &mut prompter, &mut out, DAY, || None).unwrap();
        assert_eq!(offer, Offer::NothingNewer);
        let saved = read_check_record(&ctx.install.root).unwrap();
        assert_eq!(saved.last_attempt_at, DAY);
        assert!(saved.latest.is_none());

        fs::remove_file(check_record_path(&ctx.install.root)).unwrap();
        for known in ["v0.2.0", "0.2.0", "v0.1.9"] {
            fs::remove_file(check_record_path(&ctx.install.root)).ok();
            let offer =
                offer_update_with(&ctx, &mut prompter, &mut out, DAY, || Some(known.into()))
                    .unwrap();
            assert_eq!(
                offer,
                Offer::NothingNewer,
                "{known} is not newer than v0.2.0"
            );
        }
        assert!(prompter.keys().is_empty());
    }

    #[test]
    fn accepted_offer_that_cannot_download_reports_failure_and_continues() {
        let (_temp, ctx) = context("v0.1.0");
        let mut prompter = ScriptedPrompter::new().answer(QUESTION_KEY, true);
        let mut out = Vec::new();

        // download_base points at a closed port, so the upgrade fails after the prompt.
        let offer = offer_update_with(&ctx, &mut prompter, &mut out, DAY, || Some("v0.2.0".into()))
            .unwrap();
        match offer {
            Offer::Failed { latest, error } => {
                assert_eq!(latest, "v0.2.0");
                assert!(!error.is_empty());
            }
            other => panic!("expected Failed, got {other:?}"),
        }
        let printed = String::from_utf8(out).unwrap();
        assert!(
            printed.contains("downloading merge-pipeline v0.2.0"),
            "{printed}"
        );
        assert!(!ctx.install.versions_dir.join("v0.2.0").exists());
    }

    #[test]
    fn installed_binary_is_under_the_new_version_dir() {
        let (_temp, ctx) = context("v0.1.0");
        let path = installed_binary(&ctx, "v0.2.0");
        assert_eq!(
            path,
            ctx.install
                .versions_dir
                .join("v0.2.0")
                .join("bin")
                .join(ctx.host.exe_name())
        );
    }
}
