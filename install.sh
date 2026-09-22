#!/bin/sh
# merge-pipeline installer — https://github.com/CodeVachon/merge-pipeline
#
#   curl -fsSL https://raw.githubusercontent.com/CodeVachon/merge-pipeline/main/install.sh | sh
#
# Installs a single self-contained binary. The only runtime prerequisite is `git` on PATH.
#
#   ~/.merge-pipeline/versions/v<X.Y.Z>/bin/merge-pipeline   the binary
#   ~/.merge-pipeline/current -> versions/v<X.Y.Z>            the active version
#   ~/.local/bin/merge-pipeline -> current/bin/merge-pipeline what lands on PATH
#   ~/.merge-pipeline/completions/                            generated completion scripts
#
# Environment:
#   MERGE_PIPELINE_VERSION         install a specific version (default: latest release)
#   MERGE_PIPELINE_INSTALL_DIR     root instead of ~/.merge-pipeline
#   MERGE_PIPELINE_BIN_DIR         symlink directory instead of ~/.local/bin
#   MERGE_PIPELINE_NO_MODIFY_PATH  set to skip the PATH advice
#   MERGE_PIPELINE_DOWNLOAD_BASE   where to fetch assets from, for a mirror or for testing this
#                                  script against a local build. Must contain <asset>.gz and
#                                  checksums.txt.
#
# The same layout is implemented by `merge-pipeline upgrade`; the Rust test suite reads this file
# and asserts the two agree. POSIX sh on purpose: this has to run under dash and busybox.

set -eu

REPO="CodeVachon/merge-pipeline"
INSTALL_DIR="${MERGE_PIPELINE_INSTALL_DIR:-$HOME/.merge-pipeline}"
BIN_DIR="${MERGE_PIPELINE_BIN_DIR:-$HOME/.local/bin}"

RED=''
GREEN=''
DIM=''
BOLD=''
RESET=''
if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
    RED='\033[31m'
    GREEN='\033[32m'
    DIM='\033[2m'
    BOLD='\033[1m'
    RESET='\033[0m'
fi

say() { printf '%b\n' "$1"; }
ok() { say "${GREEN}✓${RESET} $1"; }
info() { say "${DIM}$1${RESET}"; }

die() {
    say "${RED}✗${RESET} $1" >&2
    exit 1
}

need() {
    command -v "$1" >/dev/null 2>&1 || die "$1 is required but was not found on PATH"
}

# --- what are we running on? -------------------------------------------------

detect_target() {
    os="$(uname -s)"
    arch="$(uname -m)"

    case "$os" in
        Darwin) os_name="darwin" ;;
        Linux) os_name="linux" ;;
        MINGW* | MSYS* | CYGWIN*)
            die "on Windows use PowerShell instead:\n  irm https://raw.githubusercontent.com/$REPO/main/install.ps1 | iex"
            ;;
        *) die "unsupported operating system: $os" ;;
    esac

    case "$arch" in
        x86_64 | amd64) arch_name="x64" ;;
        arm64 | aarch64) arch_name="arm64" ;;
        *) die "unsupported architecture: $arch" ;;
    esac

    # Only the combinations the release workflow actually builds.
    case "$os_name-$arch_name" in
        darwin-arm64 | darwin-x64 | linux-x64 | linux-arm64) ;;
        *) die "no build available for $os_name-$arch_name" ;;
    esac

    printf 'merge-pipeline-%s-%s' "$os_name" "$arch_name"
}

# --- version resolution ------------------------------------------------------

github_api() {
    # No -f: the body is wanted even on an error status, so rate limiting can be reported as
    # rate limiting rather than as a generic network failure.
    if command -v curl >/dev/null 2>&1; then
        curl -sSL -H "Accept: application/vnd.github+json" "$1" 2>/dev/null
    else
        wget -qO- --header="Accept: application/vnd.github+json" "$1" 2>/dev/null
    fi
}

first_tag() {
    # Splitting on commas puts each JSON field on its own line, which is enough to pick the first
    # tag_name without requiring jq. GitHub returns releases newest first.
    printf '%s' "$1" | tr ',' '\n' | grep '"tag_name"' | head -1 |
        sed 's/.*"tag_name": *"\([^"]*\)".*/\1/'
}

latest_version() {
    # Prefer a stable release. /releases/latest excludes prereleases, so while every release is
    # a 0.x prerelease it 404s and the fallback below is the only path that finds anything.
    tag="$(first_tag "$(github_api "https://api.github.com/repos/$REPO/releases/latest")")"
    if [ -n "$tag" ]; then
        printf '%s' "$tag"
        return
    fi

    body="$(github_api "https://api.github.com/repos/$REPO/releases")"

    case "$body" in
        *"rate limit"*)
            die "GitHub API rate limit reached. Retry later, or set MERGE_PIPELINE_VERSION to skip the lookup:\n  MERGE_PIPELINE_VERSION=v0.1.0 sh install.sh"
            ;;
    esac

    tag="$(first_tag "$body")"
    [ -n "$tag" ] ||
        die "could not find a release for $REPO. Set MERGE_PIPELINE_VERSION to install a specific version."

    printf '%s' "$tag"
}

fetch() {
    # fetch <url> <destination>
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$1" -o "$2"
    else
        wget -qO "$2" "$1"
    fi
}

# --- checksum verification ---------------------------------------------------

sha256_of() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d' ' -f1
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$1" | cut -d' ' -f1
    else
        # Refusing is better than installing an unverified binary from the internet.
        die "neither sha256sum nor shasum was found, so the download cannot be verified"
    fi
}

verify() {
    # verify <file> <asset-name> <checksums-file>
    expected="$(grep " $2\$" "$3" | head -1 | cut -d' ' -f1)"
    [ -n "$expected" ] || die "$2 is not listed in checksums.txt"

    actual="$(sha256_of "$1")"
    [ "$expected" = "$actual" ] || die "checksum mismatch for $2\n  expected $expected\n  actual   $actual"
}

# --- shell completion --------------------------------------------------------

# Generated with the binary that was just installed rather than shipped as a static file, so the
# completions always describe the version present. Only fish is switched on automatically because
# it autoloads from a known directory; bash and zsh need a profile line, and this installer does
# not touch your shell. A binary without a `completion` subcommand must not fail the install.
install_completions() {
    bin="$1"
    comp_dir="$INSTALL_DIR/completions"

    "$bin" completion bash >/dev/null 2>&1 || return 0
    mkdir -p "$comp_dir" || return 0

    for shell in bash zsh fish; do
        "$bin" completion "$shell" > "$comp_dir/merge-pipeline.$shell" 2>/dev/null || {
            rm -f "$comp_dir/merge-pipeline.$shell"
            continue
        }
    done

    fish_dir="${XDG_CONFIG_HOME:-$HOME/.config}/fish/completions"
    if [ -d "$fish_dir" ] && [ -f "$comp_dir/merge-pipeline.fish" ]; then
        if cp "$comp_dir/merge-pipeline.fish" "$fish_dir/merge-pipeline.fish" 2>/dev/null; then
            ok "fish completion installed to $fish_dir/merge-pipeline.fish"
        fi
    fi

    if [ -f "$comp_dir/merge-pipeline.bash" ]; then
        say ""
        say "${BOLD}Tab completion${RESET}:"
        say "  bash:  echo 'source \"$comp_dir/merge-pipeline.bash\"' >> ~/.bashrc"
        say "  zsh:   echo 'source \"$comp_dir/merge-pipeline.zsh\"' >> ~/.zshrc"
    fi
}

# --- install -----------------------------------------------------------------

main() {
    need uname
    need mkdir
    need ln
    command -v curl >/dev/null 2>&1 || command -v wget >/dev/null 2>&1 ||
        die "either curl or wget is required"
    command -v gunzip >/dev/null 2>&1 || die "gunzip is required"

    target="$(detect_target)"

    version="${MERGE_PIPELINE_VERSION:-}"
    if [ -z "$version" ]; then
        info "finding the latest release..."
        version="$(latest_version)"
        [ -n "$version" ] || die "could not determine the latest version"
    fi
    # Accept "0.1.0" as well as "v0.1.0".
    case "$version" in v*) ;; *) version="v$version" ;; esac

    asset="$target.gz"
    base="${MERGE_PIPELINE_DOWNLOAD_BASE:-https://github.com/$REPO/releases/download/$version}"

    version_dir="$INSTALL_DIR/versions/$version"
    tmp="$(mktemp -d)"
    # shellcheck disable=SC2064
    trap "rm -rf '$tmp'" EXIT INT TERM

    info "downloading merge-pipeline $version for ${target#merge-pipeline-}..."
    fetch "$base/$asset" "$tmp/$asset" || die "could not download $base/$asset"
    fetch "$base/checksums.txt" "$tmp/checksums.txt" ||
        die "could not download checksums.txt for $version"

    verify "$tmp/$asset" "$asset" "$tmp/checksums.txt"
    ok "checksum verified"

    mkdir -p "$version_dir/bin"
    gunzip -c "$tmp/$asset" > "$version_dir/bin/merge-pipeline" || die "could not decompress $asset"
    chmod +x "$version_dir/bin/merge-pipeline"

    # Repoint both symlinks. `ln -sfn` rather than `-sf` so an existing symlink to a directory is
    # replaced rather than followed into.
    ln -sfn "$version_dir" "$INSTALL_DIR/current"
    mkdir -p "$BIN_DIR"
    ln -sfn "$INSTALL_DIR/current/bin/merge-pipeline" "$BIN_DIR/merge-pipeline"

    installed="$("$version_dir/bin/merge-pipeline" --version 2>/dev/null)" ||
        die "the downloaded binary did not run — this build may not match your platform"

    ok "$installed installed to $version_dir"
    ok "linked $BIN_DIR/merge-pipeline"

    install_completions "$version_dir/bin/merge-pipeline"

    if [ -z "${MERGE_PIPELINE_NO_MODIFY_PATH:-}" ]; then
        case ":$PATH:" in
            *":$BIN_DIR:"*) ;;
            *)
                say ""
                say "${BOLD}$BIN_DIR is not on your PATH.${RESET} Add this to your shell profile:"
                say "  export PATH=\"$BIN_DIR:\$PATH\""
                ;;
        esac
    fi

    say ""
    info "This does not change your current shell — open a new terminal, then:"
    info "  merge-pipeline --help     everything"
    info "  merge-pipeline upgrade    update in place later"
}

main "$@"
