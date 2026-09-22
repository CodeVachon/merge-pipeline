//! Managed install layout, release lookup, download/verify, upgrade and uninstall.
//!
//! This mirrors motte's install contract so both tools behave identically on a machine:
//!
//! ```text
//! <root>/versions/v<X.Y.Z>/bin/merge-pipeline   the binary
//! <root>/current -> versions/v<X.Y.Z>           the active version
//! <bin>/merge-pipeline -> <root>/current/bin/merge-pipeline
//! <root>/update-check.json                      when we last looked for a release
//! ```
//!
//! `<root>` is `~/.merge-pipeline` unless `MERGE_PIPELINE_INSTALL_DIR` says otherwise, and `<bin>`
//! is `~/.local/bin` unless `MERGE_PIPELINE_BIN_DIR` says otherwise. `install.sh` and `install.ps1`
//! implement the same layout; `layout::tests` reads `install.sh` to make sure they agree.

pub mod download;
pub mod layout;
pub mod nudge;
pub mod releases;
pub mod upgrade;

use std::io::Write;

pub use download::{
    DownloadError, PruneResult, fetch_verified_binary, install_binary, prune_versions,
};
pub use layout::{
    Arch, BIN_NAME, HostTarget, ManagedInstall, Platform, REPO, UnsupportedHostError,
    download_base, host_target, install_root, installed_versions, locate_install,
    normalize_version,
};
pub use releases::{ReleaseError, resolve_latest_version};
pub use upgrade::{UninstallOptions, UpgradeContext, UpgradeError, UpgradeOptions};

use crate::cli::{UninstallArgs, UpgradeArgs};

impl From<&UpgradeArgs> for UpgradeOptions {
    fn from(args: &UpgradeArgs) -> Self {
        Self {
            target: args.target.clone(),
            check: args.check,
            keep: Some(args.keep),
            force: args.force,
            json: args.json,
        }
    }
}

impl From<&UninstallArgs> for UninstallOptions {
    fn from(args: &UninstallArgs) -> Self {
        Self {
            yes: args.yes,
            json: args.json,
        }
    }
}

/// Exit the process with `code` when it is non-zero, flushing stdout first.
///
/// `uninstall` without `--yes` reports what it would do and exits 1 without that being an
/// error, so it cannot travel back through `anyhow::Result` without being printed as one.
fn finish(code: i32) -> anyhow::Result<()> {
    if code != 0 {
        let _ = std::io::stdout().flush();
        std::process::exit(code);
    }
    Ok(())
}

/// `merge-pipeline upgrade`: entry point used by `main`.
pub fn run_upgrade(args: &UpgradeArgs) -> anyhow::Result<()> {
    let options = UpgradeOptions::from(args);
    let code = upgrade::run_upgrade(&options, &mut std::io::stdout())?;
    finish(code)
}

/// `merge-pipeline uninstall`: entry point used by `main`.
pub fn run_uninstall(args: &UninstallArgs) -> anyhow::Result<()> {
    let options = UninstallOptions::from(args);
    let code = upgrade::run_uninstall(&options, &mut std::io::stdout())?;
    finish(code)
}
