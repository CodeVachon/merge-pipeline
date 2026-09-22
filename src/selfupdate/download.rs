//! Fetch a release asset, verify it against the release's checksums, and install it.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::layout::{HostTarget, download_base, normalize_version};

/// Generous cap on a downloaded asset; the real binaries are a few megabytes.
const MAX_ASSET_BYTES: u64 = 256 * 1024 * 1024;

/// The download could not be completed or trusted.
#[derive(Debug, Error)]
pub enum DownloadError {
    #[error("could not fetch {url}: {reason}")]
    Fetch { url: String, reason: String },
    #[error("could not download {url} (HTTP {status})")]
    Status { url: String, status: u16 },
    #[error("{asset} is not listed in checksums.txt for {version}")]
    NotListed { asset: String, version: String },
    #[error("checksum mismatch for {asset}\n  expected {expected}\n  actual   {actual}")]
    Mismatch {
        asset: String,
        expected: String,
        actual: String,
    },
    #[error("could not decompress {asset}: {reason}")]
    Decompress { asset: String, reason: String },
    #[error("could not write the installation: {0}")]
    Io(#[from] std::io::Error),
}

fn agent() -> ureq::Agent {
    ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(300)))
            .user_agent("merge-pipeline")
            .build(),
    )
}

fn get(agent: &ureq::Agent, url: &str) -> Result<Vec<u8>, DownloadError> {
    let response = agent.get(url).call().map_err(|e| DownloadError::Fetch {
        url: url.to_owned(),
        reason: e.to_string(),
    })?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(DownloadError::Status {
            url: url.to_owned(),
            status,
        });
    }
    response
        .into_body()
        .with_config()
        .limit(MAX_ASSET_BYTES)
        .read_to_vec()
        .map_err(|e| DownloadError::Fetch {
            url: url.to_owned(),
            reason: e.to_string(),
        })
}

/// The sha256 hex digest listed for `asset` in a `checksums.txt` body, if any.
pub fn expected_checksum(checksums: &str, asset: &str) -> Option<String> {
    checksums
        .lines()
        .map(str::trim)
        .find(|line| line.ends_with(&format!(" {asset}")))
        .and_then(|line| line.split_whitespace().next())
        .map(|hex| hex.to_ascii_lowercase())
}

/// Lowercase hex sha256 of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// Verify `archive` against `checksums` and decompress it.
///
/// The checksum covers the compressed asset, which is what crossed the network; gzip's own CRC
/// catches a corrupt decompression separately.
pub fn verify_and_decompress(
    archive: &[u8],
    checksums: &str,
    asset: &str,
    version: &str,
) -> Result<Vec<u8>, DownloadError> {
    let expected = expected_checksum(checksums, asset).ok_or_else(|| DownloadError::NotListed {
        asset: asset.to_owned(),
        version: normalize_version(version),
    })?;
    let actual = sha256_hex(archive);
    if expected != actual {
        return Err(DownloadError::Mismatch {
            asset: asset.to_owned(),
            expected,
            actual,
        });
    }

    let mut binary = Vec::new();
    GzDecoder::new(archive)
        .read_to_end(&mut binary)
        .map_err(|e| DownloadError::Decompress {
            asset: asset.to_owned(),
            reason: e.to_string(),
        })?;
    Ok(binary)
}

/// Fetch the release asset for `target` and verify it against that release's `checksums.txt`.
///
/// Nothing is written anywhere until verification passes.
pub fn fetch_verified_binary(version: &str, target: &HostTarget) -> Result<Vec<u8>, DownloadError> {
    fetch_verified_binary_from(&download_base(version), version, target)
}

/// [`fetch_verified_binary`] from an explicit base URL.
pub fn fetch_verified_binary_from(
    base: &str,
    version: &str,
    target: &HostTarget,
) -> Result<Vec<u8>, DownloadError> {
    let agent = agent();
    let base = base.trim_end_matches('/');
    let archive = get(&agent, &format!("{base}/{}", target.asset_name))?;
    let checksums = get(&agent, &format!("{base}/checksums.txt"))?;
    let checksums = String::from_utf8_lossy(&checksums);
    verify_and_decompress(&archive, &checksums, &target.asset_name, version)
}

/// Write a verified binary into its version directory and repoint `current`.
///
/// The PATH symlink already points at `current`, so repointing `current` is what makes a new
/// version take effect, and what makes rollback a symlink change rather than a reinstall.
pub fn install_binary(
    root: &Path,
    version: &str,
    binary: &[u8],
    windows: bool,
) -> Result<PathBuf, DownloadError> {
    let normalized = normalize_version(version);
    let version_dir = root.join("versions").join(&normalized);
    let bin_dir = version_dir.join("bin");
    let exe = bin_dir.join(if windows {
        "merge-pipeline.exe"
    } else {
        "merge-pipeline"
    });

    fs::create_dir_all(&bin_dir)?;
    // Write beside and rename, so a crash mid-write never leaves a truncated executable in place.
    let staging = bin_dir.join(".merge-pipeline.partial");
    fs::write(&staging, binary)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&staging, fs::Permissions::from_mode(0o755))?;
    }
    fs::rename(&staging, &exe)?;

    point_current_at(root, &version_dir)?;
    Ok(exe)
}

/// Replace `<root>/current` with a link to `version_dir`.
fn point_current_at(root: &Path, version_dir: &Path) -> std::io::Result<()> {
    let current = root.join("current");
    match fs::symlink_metadata(&current) {
        Ok(meta) if meta.file_type().is_symlink() => fs::remove_file(&current)?,
        Ok(meta) if meta.is_dir() => fs::remove_dir_all(&current)?,
        Ok(_) => fs::remove_file(&current)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(version_dir, &current)
    }
    #[cfg(windows)]
    {
        // Symlinks need Developer Mode or elevation; fall back to a copy of bin/ so
        // current\bin\merge-pipeline.exe exists either way.
        if std::os::windows::fs::symlink_dir(version_dir, &current).is_ok() {
            return Ok(());
        }
        let bin = current.join("bin");
        fs::create_dir_all(&bin)?;
        for entry in fs::read_dir(version_dir.join("bin"))? {
            let entry = entry?;
            fs::copy(entry.path(), bin.join(entry.file_name()))?;
        }
        Ok(())
    }
}

/// What [`prune_versions`] did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct PruneResult {
    pub removed: Vec<String>,
    /// A version that was eligible but kept because it is the running binary. Reported rather
    /// than skipped silently, so `--keep 1` leaving two versions behind is explained.
    pub kept_running: Option<String>,
}

/// Remove all but the newest `keep` versions, never the one currently executing.
///
/// Deleting a running executable is harmless on Unix and fails on Windows where the file is
/// locked; either way the running version is left for the next upgrade to clean up.
pub fn prune_versions(
    versions_dir: &Path,
    ordered_newest_first: &[String],
    keep: usize,
    active: &str,
) -> PruneResult {
    let mut result = PruneResult::default();
    for version in ordered_newest_first.iter().skip(keep.max(1)) {
        if version == active {
            result.kept_running = Some(version.clone());
            continue;
        }
        if fs::remove_dir_all(versions_dir.join(version)).is_ok() {
            result.removed.push(version.clone());
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::Write;

    fn gz(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap()
    }

    #[test]
    fn checksum_lookup_tolerates_one_or_two_spaces_and_case() {
        let checksums =
            "ABCDEF  merge-pipeline-linux-x64.gz\n012345 merge-pipeline-darwin-arm64.gz\n";
        assert_eq!(
            expected_checksum(checksums, "merge-pipeline-linux-x64.gz").as_deref(),
            Some("abcdef")
        );
        assert_eq!(
            expected_checksum(checksums, "merge-pipeline-darwin-arm64.gz").as_deref(),
            Some("012345")
        );
        assert_eq!(
            expected_checksum(checksums, "merge-pipeline-windows-x64.exe.gz"),
            None
        );
    }

    #[test]
    fn verify_and_decompress_round_trips_a_good_asset() {
        let archive = gz(b"binary bytes");
        let checksums = format!("{}  asset.gz\n", sha256_hex(&archive));
        let out = verify_and_decompress(&archive, &checksums, "asset.gz", "1.0.0").unwrap();
        assert_eq!(out, b"binary bytes");
    }

    #[test]
    fn verify_refuses_a_mismatch_and_an_unlisted_asset() {
        let archive = gz(b"binary bytes");
        let err =
            verify_and_decompress(&archive, "0000  asset.gz\n", "asset.gz", "1.0.0").unwrap_err();
        assert!(matches!(err, DownloadError::Mismatch { .. }), "{err}");
        let err = verify_and_decompress(&archive, "", "asset.gz", "1.0.0").unwrap_err();
        assert!(matches!(err, DownloadError::NotListed { .. }), "{err}");
    }

    #[test]
    fn install_binary_writes_the_version_dir_and_repoints_current() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let exe = install_binary(root, "0.1.0", b"one", false).unwrap();
        assert_eq!(exe, root.join("versions/v0.1.0/bin/merge-pipeline"));
        assert_eq!(fs::read(&exe).unwrap(), b"one");
        assert_eq!(
            fs::read_link(root.join("current")).unwrap(),
            root.join("versions/v0.1.0")
        );

        install_binary(root, "v0.2.0", b"two", false).unwrap();
        assert_eq!(
            fs::read_link(root.join("current")).unwrap(),
            root.join("versions/v0.2.0")
        );
        assert_eq!(
            fs::read(root.join("current/bin/merge-pipeline")).unwrap(),
            b"two"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(root.join("versions/v0.2.0/bin/merge-pipeline"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o755, 0o755);
        }
    }

    #[test]
    fn prune_keeps_the_newest_and_never_the_running_version() {
        let temp = tempfile::tempdir().unwrap();
        let versions = temp.path();
        let ordered: Vec<String> = ["v0.4.0", "v0.3.0", "v0.2.0", "v0.1.0"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        for v in &ordered {
            fs::create_dir_all(versions.join(v).join("bin")).unwrap();
        }

        let result = prune_versions(versions, &ordered, 2, "v0.2.0");
        assert_eq!(result.removed, vec!["v0.1.0"]);
        assert_eq!(result.kept_running.as_deref(), Some("v0.2.0"));
        assert!(versions.join("v0.4.0").exists());
        assert!(versions.join("v0.3.0").exists());
        assert!(versions.join("v0.2.0").exists());
        assert!(!versions.join("v0.1.0").exists());

        // keep=0 is treated as keep=1.
        let result = prune_versions(versions, &ordered[..3], 0, "v0.4.0");
        assert_eq!(result.removed, vec!["v0.3.0", "v0.2.0"]);
        assert_eq!(result.kept_running, None);
    }
}
