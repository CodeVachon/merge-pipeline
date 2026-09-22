//! A once-a-day, best-effort "an update is available" line for interactive runs.
//!
//! Never blocks for long, never fails the run, and is silent unless a newer release is known.
//! Opt out with `MERGE_PIPELINE_NO_UPDATE_CHECK=1`.

use std::cmp::Ordering;
use std::env;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::layout::{compare_versions_descending, locate_install, normalize_version};
use super::releases::{api_base, resolve_latest_version_at};
use super::upgrade::{read_check_record, record_check};

/// How long a cached lookup is trusted before the API is asked again.
pub const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
/// How long the background lookup may take before it is abandoned for this run.
pub const LOOKUP_TIMEOUT: Duration = Duration::from_millis(1500);

/// Whether the nudge is switched off by the environment.
pub fn disabled() -> bool {
    env::var_os("MERGE_PIPELINE_NO_UPDATE_CHECK").is_some_and(|v| !v.is_empty())
        || env::var_os("CI").is_some_and(|v| !v.is_empty())
}

/// The line to print at the end of an interactive run, if a newer release is known.
///
/// Only a managed installation is nudged: a dev build has nothing to upgrade in place. The cached
/// `update-check.json` is used when fresh; otherwise one quick lookup is attempted and cached.
pub fn maybe_line() -> Option<String> {
    if disabled() {
        return None;
    }
    let install = locate_install()?;
    let current = normalize_version(crate::VERSION);

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let record = read_check_record(&install.root);
    let fresh = record
        .as_ref()
        .is_some_and(|r| now.saturating_sub(r.last_attempt_at) < CHECK_INTERVAL.as_millis() as u64);

    let latest = if fresh {
        record.and_then(|r| r.latest)?
    } else {
        let looked_up = resolve_latest_version_at(&api_base(), LOOKUP_TIMEOUT).ok();
        record_check(&install.root, looked_up.as_deref());
        looked_up?
    };

    let latest = normalize_version(&latest);
    // Greater means `current` is older than `latest`.
    (compare_versions_descending(&current, &latest) == Ordering::Greater).then(|| {
        format!(
            "merge-pipeline {latest} is available (you have {current}) — merge-pipeline upgrade"
        )
    })
}
