//! The on-disk contract shared with `install.sh` and `install.ps1`.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use semver::Version;
use thiserror::Error;

/// GitHub repository releases are fetched from.
pub const REPO: &str = "CodeVachon/merge-pipeline";
/// Name of the executable (without `.exe`).
pub const BIN_NAME: &str = "merge-pipeline";
/// Directory under `$HOME` that holds the managed installation.
pub const ROOT_DIR_NAME: &str = ".merge-pipeline";

/// Operating systems the release workflow builds for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Darwin,
    Linux,
    Windows,
}

impl Platform {
    /// The name used in release asset filenames.
    pub fn as_str(self) -> &'static str {
        match self {
            Platform::Darwin => "darwin",
            Platform::Linux => "linux",
            Platform::Windows => "windows",
        }
    }
}

/// CPU architectures the release workflow builds for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    X64,
    Arm64,
}

impl Arch {
    /// The name used in release asset filenames.
    pub fn as_str(self) -> &'static str {
        match self {
            Arch::X64 => "x64",
            Arch::Arm64 => "arm64",
        }
    }
}

/// The host we are running on, and the release asset that matches it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostTarget {
    pub platform: Platform,
    pub arch: Arch,
    /// Release asset name without the `.gz`, e.g. `merge-pipeline-darwin-arm64`.
    pub binary_name: String,
    /// Release asset name as published, e.g. `merge-pipeline-darwin-arm64.gz`.
    pub asset_name: String,
}

impl HostTarget {
    /// Whether the binary carries a `.exe` suffix.
    pub fn is_windows(&self) -> bool {
        self.platform == Platform::Windows
    }

    /// Filename of the installed executable.
    pub fn exe_name(&self) -> &'static str {
        if self.is_windows() {
            "merge-pipeline.exe"
        } else {
            BIN_NAME
        }
    }
}

/// The host is not one of the five combinations the release workflow builds.
#[derive(Debug, Error, PartialEq, Eq)]
#[error("{0}")]
pub struct UnsupportedHostError(String);

/// The five combinations the release workflow builds.
const BUILT_TARGETS: &[(Platform, Arch)] = &[
    (Platform::Darwin, Arch::Arm64),
    (Platform::Darwin, Arch::X64),
    (Platform::Linux, Arch::X64),
    (Platform::Linux, Arch::Arm64),
    (Platform::Windows, Arch::X64),
];

/// The target of the running binary.
pub fn host_target() -> Result<HostTarget, UnsupportedHostError> {
    host_target_for(env::consts::OS, env::consts::ARCH)
}

/// Map `std::env::consts::{OS, ARCH}` values to a release asset.
pub fn host_target_for(os: &str, arch: &str) -> Result<HostTarget, UnsupportedHostError> {
    let platform = match os {
        "macos" => Platform::Darwin,
        "linux" => Platform::Linux,
        "windows" => Platform::Windows,
        other => {
            return Err(UnsupportedHostError(format!(
                "unsupported platform: {other}"
            )));
        }
    };
    let cpu = match arch {
        "x86_64" => Arch::X64,
        "aarch64" => Arch::Arm64,
        other => {
            return Err(UnsupportedHostError(format!(
                "unsupported architecture: {other}"
            )));
        }
    };

    if !BUILT_TARGETS.contains(&(platform, cpu)) {
        return Err(UnsupportedHostError(format!(
            "no build is published for {}-{}",
            platform.as_str(),
            cpu.as_str()
        )));
    }

    let suffix = if platform == Platform::Windows {
        ".exe"
    } else {
        ""
    };
    let binary_name = format!("{BIN_NAME}-{}-{}{suffix}", platform.as_str(), cpu.as_str());
    Ok(HostTarget {
        platform,
        arch: cpu,
        asset_name: format!("{binary_name}.gz"),
        binary_name,
    })
}

/// The installation root: `$MERGE_PIPELINE_INSTALL_DIR`, else `~/.merge-pipeline`.
pub fn install_root() -> Option<PathBuf> {
    if let Some(dir) = env::var_os("MERGE_PIPELINE_INSTALL_DIR").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    dirs::home_dir().map(|home| home.join(ROOT_DIR_NAME))
}

/// Directories that may hold the `merge-pipeline` symlink on PATH.
pub fn candidate_bin_dirs() -> Vec<PathBuf> {
    let mut dirs_out = Vec::new();
    if let Some(dir) = env::var_os("MERGE_PIPELINE_BIN_DIR").filter(|v| !v.is_empty()) {
        dirs_out.push(PathBuf::from(dir));
    }
    if let Some(home) = dirs::home_dir() {
        dirs_out.push(home.join(".local").join("bin"));
        dirs_out.push(home.join("bin"));
    }
    dirs_out
}

/// The managed installation the running binary belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedInstall {
    /// `~/.merge-pipeline` by default.
    pub root: PathBuf,
    /// The version directory the running binary lives in, e.g. `v0.1.0`.
    pub version: String,
    /// Absolute path to the running binary.
    pub binary: PathBuf,
    pub versions_dir: PathBuf,
    pub current_link: PathBuf,
}

/// Locate the installation the running binary belongs to, or `None` when it is not one.
///
/// Symlinks are resolved, so invoking through `~/.local/bin/merge-pipeline` still reports the
/// real path under `versions/`. Running from `cargo run` reports `target/debug/...`, which does
/// not match the shape, and that is how a dev build is detected.
pub fn locate_install() -> Option<ManagedInstall> {
    let exe = env::current_exe().ok()?;
    locate_install_from(&exe)
}

/// [`locate_install`] for an explicit executable path.
pub fn locate_install_from(exe: &Path) -> Option<ManagedInstall> {
    let binary = fs::canonicalize(exe).ok()?;
    let bin_dir = binary.parent()?;
    if bin_dir.file_name()? != "bin" {
        return None;
    }
    let version_dir = bin_dir.parent()?;
    let version = version_dir.file_name()?.to_str()?.to_owned();
    parse_version(&version)?;
    let versions_dir = version_dir.parent()?;
    if versions_dir.file_name()? != "versions" {
        return None;
    }
    let root = versions_dir.parent()?.to_path_buf();

    Some(ManagedInstall {
        current_link: root.join("current"),
        versions_dir: versions_dir.to_path_buf(),
        root,
        version,
        binary,
    })
}

/// Installed version directory names, newest first.
pub fn installed_versions(versions_dir: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(versions_dir) else {
        return Vec::new();
    };
    let mut versions: Vec<String> = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| parse_version(name).is_some())
        .collect();
    versions.sort_by(|a, b| compare_versions_descending(a, b));
    versions
}

/// Parse `v1.2.3` or `1.2.3` (with optional prerelease) into a semver version.
pub fn parse_version(raw: &str) -> Option<Version> {
    let trimmed = raw.trim();
    let bare = trimmed.strip_prefix('v').unwrap_or(trimmed);
    Version::parse(bare).ok()
}

/// Descending semantic ordering; unparseable strings sort as 0.0.0.
pub fn compare_versions_descending(a: &str, b: &str) -> std::cmp::Ordering {
    let zero = Version::new(0, 0, 0);
    let left = parse_version(a).unwrap_or_else(|| zero.clone());
    let right = parse_version(b).unwrap_or(zero);
    right.cmp(&left)
}

/// Normalise `0.1.0` and `v0.1.0` to `v0.1.0`.
pub fn normalize_version(version: &str) -> String {
    let trimmed = version.trim();
    if trimmed.starts_with('v') {
        trimmed.to_owned()
    } else {
        format!("v{trimmed}")
    }
}

/// Where release assets for `version` are downloaded from.
///
/// `MERGE_PIPELINE_DOWNLOAD_BASE` overrides the GitHub URL so CI can verify an installer
/// against locally served assets before they are published.
pub fn download_base(version: &str) -> String {
    if let Ok(base) = env::var("MERGE_PIPELINE_DOWNLOAD_BASE")
        && !base.is_empty()
    {
        return base.trim_end_matches('/').to_owned();
    }
    format!(
        "https://github.com/{REPO}/releases/download/{}",
        normalize_version(version)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_targets_name_assets_like_the_release_workflow() {
        let t = host_target_for("macos", "aarch64").unwrap();
        assert_eq!(t.asset_name, "merge-pipeline-darwin-arm64.gz");
        assert_eq!(t.exe_name(), "merge-pipeline");
        let t = host_target_for("linux", "x86_64").unwrap();
        assert_eq!(t.asset_name, "merge-pipeline-linux-x64.gz");
        let t = host_target_for("windows", "x86_64").unwrap();
        assert_eq!(t.binary_name, "merge-pipeline-windows-x64.exe");
        assert_eq!(t.asset_name, "merge-pipeline-windows-x64.exe.gz");
        assert_eq!(t.exe_name(), "merge-pipeline.exe");
    }

    #[test]
    fn unsupported_hosts_are_rejected() {
        assert!(host_target_for("freebsd", "x86_64").is_err());
        assert!(host_target_for("linux", "riscv64").is_err());
        assert!(host_target_for("windows", "aarch64").is_err());
    }

    #[test]
    fn normalize_adds_the_v_once() {
        assert_eq!(normalize_version("0.1.0"), "v0.1.0");
        assert_eq!(normalize_version("v0.1.0"), "v0.1.0");
        assert_eq!(normalize_version("  1.2.3 "), "v1.2.3");
    }

    #[test]
    fn versions_sort_newest_first_with_prereleases_below_release() {
        let mut list = vec!["v1.0.0-rc.1", "v0.9.0", "v1.0.0", "v1.2.0", "junk"];
        list.sort_by(|a, b| compare_versions_descending(a, b));
        assert_eq!(
            list,
            vec!["v1.2.0", "v1.0.0", "v1.0.0-rc.1", "v0.9.0", "junk"]
        );
    }

    #[test]
    fn installed_versions_ignores_non_version_entries() {
        let temp = tempfile::tempdir().unwrap();
        for name in ["v0.1.0", "v0.3.0", "v0.2.0", "current", "notes.txt"] {
            fs::create_dir_all(temp.path().join(name)).unwrap();
        }
        assert_eq!(
            installed_versions(temp.path()),
            vec!["v0.3.0", "v0.2.0", "v0.1.0"]
        );
        assert!(installed_versions(&temp.path().join("missing")).is_empty());
    }

    #[test]
    fn locate_install_recognises_the_managed_shape() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("root");
        let bin = root.join("versions").join("v0.4.2").join("bin");
        fs::create_dir_all(&bin).unwrap();
        let exe = bin.join(BIN_NAME);
        fs::write(&exe, b"#!/bin/sh\n").unwrap();

        let install = locate_install_from(&exe).expect("managed install");
        assert_eq!(install.version, "v0.4.2");
        assert_eq!(install.root, fs::canonicalize(&root).unwrap());
        assert_eq!(install.versions_dir, install.root.join("versions"));
        assert_eq!(install.current_link, install.root.join("current"));
    }

    #[cfg(unix)]
    #[test]
    fn locate_install_follows_the_path_symlink() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("root");
        let version_dir = root.join("versions").join("v0.4.2");
        fs::create_dir_all(version_dir.join("bin")).unwrap();
        fs::write(version_dir.join("bin").join(BIN_NAME), b"x").unwrap();
        std::os::unix::fs::symlink(&version_dir, root.join("current")).unwrap();
        let link_dir = temp.path().join("local-bin");
        fs::create_dir_all(&link_dir).unwrap();
        let link = link_dir.join(BIN_NAME);
        std::os::unix::fs::symlink(root.join("current").join("bin").join(BIN_NAME), &link).unwrap();

        let install = locate_install_from(&link).expect("managed install through symlinks");
        assert_eq!(install.version, "v0.4.2");
    }

    #[test]
    fn locate_install_rejects_unmanaged_paths() {
        let temp = tempfile::tempdir().unwrap();
        let dev = temp.path().join("target").join("debug");
        fs::create_dir_all(&dev).unwrap();
        let exe = dev.join(BIN_NAME);
        fs::write(&exe, b"x").unwrap();
        assert!(locate_install_from(&exe).is_none());

        let odd = temp.path().join("versions").join("latest").join("bin");
        fs::create_dir_all(&odd).unwrap();
        fs::write(odd.join(BIN_NAME), b"x").unwrap();
        assert!(locate_install_from(&odd.join(BIN_NAME)).is_none());
    }

    #[test]
    fn download_base_points_at_the_github_release_by_default() {
        // Only meaningful when the override is unset, which is the case in the test process.
        if env::var_os("MERGE_PIPELINE_DOWNLOAD_BASE").is_none() {
            assert_eq!(
                download_base("0.1.0"),
                "https://github.com/CodeVachon/merge-pipeline/releases/download/v0.1.0"
            );
        }
    }

    /// `install.sh` implements this layout independently, in POSIX sh. Read it and make sure the
    /// asset names and directory shape it uses are the ones this module produces.
    #[test]
    fn install_sh_agrees_with_this_layout() {
        let script = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/install.sh"))
            .expect("install.sh at the repository root");

        assert!(
            script.contains(&format!("REPO=\"{REPO}\"")),
            "REPO in install.sh"
        );
        assert!(script.contains(ROOT_DIR_NAME), "install root dir name");
        assert!(script.contains("/versions/"), "versions directory");
        assert!(script.contains("/bin\""), "bin directory under the version");
        assert!(script.contains("current"), "current symlink");
        assert!(script.contains("checksums.txt"), "checksum verification");
        assert!(
            script.contains("MERGE_PIPELINE_DOWNLOAD_BASE"),
            "download base override"
        );
        assert!(
            script.contains("MERGE_PIPELINE_INSTALL_DIR"),
            "install dir override"
        );
        assert!(
            script.contains("MERGE_PIPELINE_BIN_DIR"),
            "bin dir override"
        );
        assert!(
            script.contains("MERGE_PIPELINE_VERSION"),
            "version override"
        );

        // The script builds `merge-pipeline-<os>-<arch>`; each os/arch token must be present.
        for platform in [Platform::Darwin, Platform::Linux] {
            assert!(script.contains(platform.as_str()), "{}", platform.as_str());
        }
        for arch in [Arch::X64, Arch::Arm64] {
            assert!(script.contains(arch.as_str()), "{}", arch.as_str());
        }
        for (platform, arch) in BUILT_TARGETS
            .iter()
            .filter(|(p, _)| *p != Platform::Windows)
        {
            let combo = format!("{}-{}", platform.as_str(), arch.as_str());
            assert!(script.contains(&combo), "install.sh must allow {combo}");
        }
        assert!(
            script.contains(&format!("'{BIN_NAME}-%s-%s'")),
            "asset name template"
        );
    }
}
