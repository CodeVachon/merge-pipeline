# merge-pipeline installer for Windows — https://github.com/CodeVachon/merge-pipeline
#
#   irm https://raw.githubusercontent.com/CodeVachon/merge-pipeline/main/install.ps1 | iex
#
# Installs a single self-contained binary. The only runtime prerequisite is `git` on PATH.
#
#   $env:USERPROFILE\.merge-pipeline\versions\v<X.Y.Z>\bin\merge-pipeline.exe   the binary
#   $env:USERPROFILE\.merge-pipeline\current\bin\merge-pipeline.exe             the active version
#
# `current` is a directory junction when the shell may create one, and a plain copy otherwise,
# because symlinks need Developer Mode or elevation on Windows. The bin directory is added to the
# user PATH unless MERGE_PIPELINE_NO_MODIFY_PATH is set.
#
# Environment:
#   MERGE_PIPELINE_VERSION         install a specific version (default: latest release)
#   MERGE_PIPELINE_INSTALL_DIR     root instead of $env:USERPROFILE\.merge-pipeline
#   MERGE_PIPELINE_NO_MODIFY_PATH  set to skip editing the user PATH
#   MERGE_PIPELINE_DOWNLOAD_BASE   where to fetch assets from (must contain <asset>.gz and checksums.txt)

$ErrorActionPreference = 'Stop'

$Repo = 'CodeVachon/merge-pipeline'
$InstallDir = if ($env:MERGE_PIPELINE_INSTALL_DIR) { $env:MERGE_PIPELINE_INSTALL_DIR } else { Join-Path $env:USERPROFILE '.merge-pipeline' }

function Write-Ok($message) { Write-Host "✓ $message" -ForegroundColor Green }
function Write-Info($message) { Write-Host $message -ForegroundColor DarkGray }
function Fail($message) { Write-Host "✗ $message" -ForegroundColor Red; exit 1 }

# --- what are we running on? -------------------------------------------------

$arch = if ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture -eq 'X64') { 'x64' } else { $null }
if (-not $arch) { Fail "no build available for Windows $([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture)" }
$target = "merge-pipeline-windows-$arch.exe"
$asset = "$target.gz"

# --- version resolution ------------------------------------------------------

function Get-LatestVersion {
    $headers = @{ Accept = 'application/vnd.github+json'; 'User-Agent' = 'merge-pipeline-installer' }
    try {
        $latest = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -Headers $headers
        if ($latest.tag_name) { return $latest.tag_name }
    } catch {
        if ("$_" -match 'rate limit') { Fail "GitHub API rate limit reached. Retry later, or set MERGE_PIPELINE_VERSION to skip the lookup." }
    }
    $all = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases" -Headers $headers
    $usable = $all | Where-Object { -not $_.draft -and $_.tag_name } | Select-Object -First 1
    if (-not $usable) { Fail "could not find a release for $Repo. Set MERGE_PIPELINE_VERSION to install a specific version." }
    return $usable.tag_name
}

$version = $env:MERGE_PIPELINE_VERSION
if (-not $version) {
    Write-Info 'finding the latest release...'
    $version = Get-LatestVersion
}
if (-not $version.StartsWith('v')) { $version = "v$version" }

$base = if ($env:MERGE_PIPELINE_DOWNLOAD_BASE) { $env:MERGE_PIPELINE_DOWNLOAD_BASE.TrimEnd('/') } else { "https://github.com/$Repo/releases/download/$version" }
$versionDir = Join-Path $InstallDir "versions\$version"
$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ([System.IO.Path]::GetRandomFileName())
New-Item -ItemType Directory -Path $tmp -Force | Out-Null

try {
    Write-Info "downloading merge-pipeline $version for windows-$arch..."
    Invoke-WebRequest -Uri "$base/$asset" -OutFile (Join-Path $tmp $asset) -UseBasicParsing
    Invoke-WebRequest -Uri "$base/checksums.txt" -OutFile (Join-Path $tmp 'checksums.txt') -UseBasicParsing

    # --- checksum verification ----------------------------------------------
    $line = Get-Content (Join-Path $tmp 'checksums.txt') | Where-Object { $_.Trim().EndsWith(" $asset") } | Select-Object -First 1
    if (-not $line) { Fail "$asset is not listed in checksums.txt" }
    $expected = ($line -split '\s+')[0].ToLower()
    $actual = (Get-FileHash -Algorithm SHA256 -Path (Join-Path $tmp $asset)).Hash.ToLower()
    if ($expected -ne $actual) { Fail "checksum mismatch for $asset`n  expected $expected`n  actual   $actual" }
    Write-Ok 'checksum verified'

    # --- install --------------------------------------------------------------
    $binDir = Join-Path $versionDir 'bin'
    New-Item -ItemType Directory -Path $binDir -Force | Out-Null
    $exe = Join-Path $binDir 'merge-pipeline.exe'
    $in = [System.IO.File]::OpenRead((Join-Path $tmp $asset))
    $gz = New-Object System.IO.Compression.GZipStream($in, [System.IO.Compression.CompressionMode]::Decompress)
    $out = [System.IO.File]::Create($exe)
    try { $gz.CopyTo($out) } finally { $out.Dispose(); $gz.Dispose(); $in.Dispose() }

    $current = Join-Path $InstallDir 'current'
    if (Test-Path $current) { Remove-Item $current -Recurse -Force }
    try {
        New-Item -ItemType Junction -Path $current -Target $versionDir | Out-Null
    } catch {
        # No junction rights: fall back to a copy so current\bin\merge-pipeline.exe still exists.
        New-Item -ItemType Directory -Path (Join-Path $current 'bin') -Force | Out-Null
        Copy-Item $exe (Join-Path $current 'bin\merge-pipeline.exe') -Force
    }

    $installed = & $exe --version
    if ($LASTEXECUTIONCODE -ne 0 -and -not $installed) { Fail 'the downloaded binary did not run' }
    Write-Ok "$installed installed to $versionDir"

    $currentBin = Join-Path $current 'bin'
    if (-not $env:MERGE_PIPELINE_NO_MODIFY_PATH) {
        $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
        if (($userPath -split ';') -notcontains $currentBin) {
            [Environment]::SetEnvironmentVariable('Path', "$currentBin;$userPath", 'User')
            Write-Ok "added $currentBin to your user PATH (open a new terminal)"
        }
    } else {
        Write-Info "add $currentBin to your PATH to use merge-pipeline"
    }
} finally {
    Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
}
