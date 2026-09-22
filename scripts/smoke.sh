#!/bin/sh
# Smoke-test an installed merge-pipeline binary.
#
#   scripts/smoke.sh <binary> <expected-version>
#
# Run by the release workflow's `verify` job against the binary that install.sh just installed from
# the freshly built assets, so a broken build is caught before anything is published. It also runs
# locally against `target/release/merge-pipeline`, where the checks that need a managed install
# (upgrade) are skipped and the MCP check can be downgraded to a warning with SMOKE_ALLOW_MCP_STUB=1.
#
# Environment honoured:
#   MERGE_PIPELINE_API_BASE   when set, `upgrade --check` (latest lookup) is exercised against it
#   SMOKE_ALLOW_MCP_STUB      when set, a failing `mcp` handshake warns instead of failing
#
# POSIX sh on purpose; no jq, no bash-isms.

set -eu

BIN="${1:?usage: smoke.sh <binary> <expected-version>}"
VERSION="${2:?usage: smoke.sh <binary> <expected-version>}"
VERSION="${VERSION#v}"

pass() { printf '\342\234\223 %s\n' "$1"; }
warn() { printf '! %s\n' "$1" >&2; }
fail() { printf '\342\234\227 %s\n' "$1" >&2; exit 1; }

strip_ansi() { sed 's/\x1b\[[0-9;]*m//g'; }

resolve() {
    # readlink -f is GNU coreutils and macOS 12.3+; realpath covers the rest.
    readlink -f "$1" 2>/dev/null || realpath "$1"
}

tmp="$(mktemp -d)"
# shellcheck disable=SC2064
trap "rm -rf '$tmp'" EXIT INT TERM

# Colour off and git fully isolated from the runner's or developer's global config.
export NO_COLOR=1
export CI="${CI:-1}"
export GIT_CONFIG_GLOBAL=/dev/null
export GIT_CONFIG_NOSYSTEM=1
export GIT_AUTHOR_NAME=Smoke GIT_AUTHOR_EMAIL=smoke@example.com
export GIT_COMMITTER_NAME=Smoke GIT_COMMITTER_EMAIL=smoke@example.com
export GIT_TERMINAL_PROMPT=0
export GIT_CONFIG_COUNT=2
export GIT_CONFIG_KEY_0=commit.gpgsign GIT_CONFIG_VALUE_0=false
export GIT_CONFIG_KEY_1=init.defaultBranch GIT_CONFIG_VALUE_1=main

# --- 1. version ---------------------------------------------------------------

actual="$("$BIN" --version)"
[ "$actual" = "merge-pipeline $VERSION" ] ||
    fail "--version printed '$actual', expected 'merge-pipeline $VERSION'"
pass "--version reports $VERSION"

# --- 2. help ------------------------------------------------------------------

"$BIN" --help >/dev/null || fail "--help exited non-zero"
"$BIN" upgrade --help >/dev/null || fail "upgrade --help exited non-zero"
pass "--help exits 0"

# --- 3. MCP handshake ---------------------------------------------------------

init='{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"smoke","version":"0"}}}'
mcp_out="$(printf '%s\n' "$init" | "$BIN" mcp 2>"$tmp/mcp.err" || true)"
case "$mcp_out" in
    *'"serverInfo"'*) pass "mcp answers initialize with serverInfo" ;;
    *)
        if [ -n "${SMOKE_ALLOW_MCP_STUB:-}" ]; then
            warn "mcp did not answer initialize (allowed by SMOKE_ALLOW_MCP_STUB): $(cat "$tmp/mcp.err")"
        else
            fail "mcp did not answer initialize with serverInfo. stdout: '$mcp_out' stderr: $(cat "$tmp/mcp.err")"
        fi
        ;;
esac

# --- 4. a real workflow against a throwaway repo -------------------------------

origin="$tmp/origin.git"
work="$tmp/work"
cfg="$tmp/config"
mkdir -p "$work" "$cfg"

git init --bare -q "$origin"
git -C "$work" init -q --initial-branch=main
git -C "$work" remote add origin "$origin"
printf '# smoke\n' > "$work/README.md"
git -C "$work" add -A && git -C "$work" commit -q -m initial
git -C "$work" push -q -u origin main
git -C "$work" checkout -q -b staging-patch main
git -C "$work" push -q -u origin staging-patch
git -C "$work" checkout -q -b Patch-v0.1.1 main
printf 'fix\n' > "$work/fix.txt"
git -C "$work" add -A && git -C "$work" commit -q -m "patch fix"
git -C "$work" push -q -u origin Patch-v0.1.1
git -C "$work" checkout -q main

cat > "$cfg/patch.json" <<'JSON'
{
  "disabled": false,
  "name": "Patch Release to Staging Pipeline",
  "description": "Deploy a Patch Release Branch to Staging",
  "order": 1,
  "pipeline": ["^Patch-*", "^staging-patch$"]
}
JSON

test_out="$("$BIN" -c "$work" -f "$cfg" -w "Patch Release to Staging Pipeline" -a test --yes | strip_ansi)"
case "$test_out" in
    *"Step 1: Merge Patch-v0.1.1 into staging-patch"*) ;;
    *) fail "test action did not print the expected step. Output:
$test_out" ;;
esac
case "$test_out" in
    *"Task Complete"*) ;;
    *) fail "test action did not finish with Task Complete. Output:
$test_out" ;;
esac
pass "test action resolves ^Patch-* > ^staging-patch$ to the expected step"

before="$(git -C "$origin" rev-parse staging-patch)"
run_out="$("$BIN" -c "$work" -f "$cfg" -w "Patch Release to Staging Pipeline" -a run --auto-push --yes | strip_ansi)"
case "$run_out" in
    *"Task Complete"*) ;;
    *) fail "run action did not finish with Task Complete. Output:
$run_out" ;;
esac
after="$(git -C "$origin" rev-parse staging-patch)"
[ "$before" != "$after" ] || fail "run --auto-push did not push staging-patch to origin"
git -C "$origin" merge-base --is-ancestor "$(git -C "$origin" rev-parse Patch-v0.1.1)" staging-patch ||
    fail "origin/staging-patch does not contain the Patch-v0.1.1 commit after run"
pass "run --auto-push merged Patch-v0.1.1 into staging-patch and pushed it"

# --- 5. upgrade, only meaningful from a managed install ------------------------

case "$(resolve "$BIN")" in
    */versions/*/bin/*)
        check="$("$BIN" upgrade "v$VERSION" --check --json)"
        case "$check" in
            *'"upToDate": true'* | *'"upToDate":true'*) pass "upgrade v$VERSION --check reports up to date" ;;
            *) fail "upgrade --check --json did not report upToDate true: $check" ;;
        esac
        if [ -n "${MERGE_PIPELINE_API_BASE:-}" ]; then
            latest="$("$BIN" upgrade --check --json)"
            case "$latest" in
                *'"upToDate": true'* | *'"upToDate":true'*) pass "upgrade --check (latest lookup via $MERGE_PIPELINE_API_BASE) reports up to date" ;;
                *) fail "upgrade --check via MERGE_PIPELINE_API_BASE did not report upToDate true: $latest" ;;
            esac
        fi
        ;;
    *)
        warn "skipping upgrade checks: $BIN is not inside a managed install (versions/<v>/bin)"
        ;;
esac

pass "smoke test complete"
