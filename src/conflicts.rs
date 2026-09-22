//! Merge-conflict helpers, starting with package.json version-conflict resolution.
//!
//! Ported from `actions/resolvePackageJsonVersionConflicts.ts`. When a merge stops on conflicts
//! and some of them are `"version"` lines in package.json files, offer to pick one version per
//! distinct version pair, rewrite the files, stage the ones with no markers left, and commit if
//! nothing remains conflicted.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::git::{Git, GitError};
use crate::prompt::{Choice, PromptError, Prompter, Question};
use crate::text::plural;

static PACKAGE_JSON_PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(^|/)package\.json$").expect("valid regex"));
static REMAINING_MARKER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^(<<<<<<<|=======|>>>>>>>)").expect("valid regex"));
static VERSION_CONFLICT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?m)^<<<<<<<[^\n\r]*\r?\n",
        r#"(?P<ours_line>[ \t]*"version":\s*"(?P<ours_version>[^"\n\r]+)"\s*,?[ \t]*\r?\n)"#,
        r"=======\r?\n",
        r#"(?P<theirs_line>[ \t]*"version":\s*"(?P<theirs_version>[^"\n\r]+)"\s*,?[ \t]*\r?\n)"#,
        r">>>>>>>[^\n\r]*(?:\r?\n|$)"
    ))
    .expect("valid regex")
});

/// Prompt key asking whether to attempt auto-resolution at all.
pub const AUTO_RESOLVE_KEY: &str = "conflict:auto_resolve";

/// Prompt key for choosing between two versions (lower first, then higher).
pub fn version_question_key(a: &str, b: &str) -> String {
    let (lo, hi) = if compare_versions(a, b) == Ordering::Greater {
        (b, a)
    } else {
        (a, b)
    };
    format!("conflict:version:{lo}|{hi}")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct VersionChoice {
    line: String,
    version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct VersionConflict {
    start: usize,
    end: usize,
    ours: VersionChoice,
    theirs: VersionChoice,
}

#[derive(Debug)]
struct Candidate {
    contents: String,
    full_path: PathBuf,
    path: String,
    conflicts: Vec<VersionConflict>,
}

#[derive(Debug, thiserror::Error)]
pub enum ConflictError {
    #[error("could not read or write {}: {reason}", path.display())]
    Io { path: PathBuf, reason: String },
    #[error(transparent)]
    Git(#[from] GitError),
    #[error(transparent)]
    Prompt(#[from] PromptError),
}

/// What happened.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResolveOutcome {
    /// False when there was nothing to try or the user declined; `remaining` is then the input.
    pub attempted: bool,
    /// Conflicted paths still unresolved afterwards.
    pub remaining: Vec<String>,
    /// package.json files rewritten.
    pub rewritten: Vec<String>,
    /// Whether the merge commit was created.
    pub committed: bool,
}

// --- version ordering (baseline semantics, more lenient than strict semver) -----------------

fn is_numeric(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit())
}

fn compare_prerelease_identifiers(left: &str, right: &str) -> Ordering {
    match (is_numeric(left), is_numeric(right)) {
        (true, true) => left
            .parse::<u128>()
            .unwrap_or(0)
            .cmp(&right.parse::<u128>().unwrap_or(0)),
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        (false, false) => left.cmp(right),
    }
}

fn compare_prerelease(left: &str, right: &str) -> Ordering {
    let l: Vec<&str> = left.split('.').collect();
    let r: Vec<&str> = right.split('.').collect();
    for index in 0..l.len().max(r.len()) {
        match (l.get(index), r.get(index)) {
            (None, _) => return Ordering::Less,
            (_, None) => return Ordering::Greater,
            (Some(a), Some(b)) => {
                let result = compare_prerelease_identifiers(a, b);
                if result != Ordering::Equal {
                    return result;
                }
            }
        }
    }
    Ordering::Equal
}

fn split_version(version: &str) -> (Vec<u64>, Option<&str>) {
    let without_build = version.split('+').next().unwrap_or("");
    let (core, pre) = match without_build.split_once('-') {
        Some((core, pre)) => (core, Some(pre)),
        None => (without_build, None),
    };
    let segments = core
        .split('.')
        .map(|segment| segment.parse::<u64>().unwrap_or(0))
        .collect();
    (segments, pre)
}

/// Compare two version strings the way the baseline did: numeric core segments (missing = 0),
/// build metadata ignored, no-prerelease outranks prerelease, then prerelease identifiers.
pub fn compare_versions(left: &str, right: &str) -> Ordering {
    let (l_core, l_pre) = split_version(left);
    let (r_core, r_pre) = split_version(right);
    for index in 0..l_core.len().max(r_core.len()) {
        let l = l_core.get(index).copied().unwrap_or(0);
        let r = r_core.get(index).copied().unwrap_or(0);
        if l != r {
            return l.cmp(&r);
        }
    }
    match (l_pre, r_pre) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(l), Some(r)) => compare_prerelease(l, r),
    }
}

// --- detection ------------------------------------------------------------------------------

pub fn is_package_json_path(path: &str) -> bool {
    PACKAGE_JSON_PATH.is_match(path)
}

fn has_conflict_markers(contents: &str) -> bool {
    REMAINING_MARKER.is_match(contents)
}

fn find_version_conflicts(contents: &str) -> Vec<VersionConflict> {
    VERSION_CONFLICT
        .captures_iter(contents)
        .filter_map(|captures| {
            let whole = captures.get(0)?;
            Some(VersionConflict {
                start: whole.start(),
                end: whole.end(),
                ours: VersionChoice {
                    line: captures.name("ours_line")?.as_str().to_string(),
                    version: captures.name("ours_version")?.as_str().to_string(),
                },
                theirs: VersionChoice {
                    line: captures.name("theirs_line")?.as_str().to_string(),
                    version: captures.name("theirs_version")?.as_str().to_string(),
                },
            })
        })
        .collect()
}

fn prompt_message(paths: &[String]) -> String {
    if paths.len() == 1 {
        format!("Which version should be used for {}?", paths[0])
    } else {
        format!(
            "Which version should be used for {} package.json {}?",
            paths.len(),
            plural("conflict", paths.len())
        )
    }
}

#[derive(Debug, Default)]
struct Group {
    paths: Vec<String>,
    versions: Vec<String>,
}

fn collect_groups(candidates: &[Candidate]) -> BTreeMap<String, Group> {
    let mut groups: BTreeMap<String, Group> = BTreeMap::new();
    for candidate in candidates {
        for conflict in &candidate.conflicts {
            let key = version_question_key(&conflict.ours.version, &conflict.theirs.version);
            let group = groups.entry(key).or_default();
            if !group.paths.contains(&candidate.path) {
                group.paths.push(candidate.path.clone());
            }
            for version in [&conflict.ours.version, &conflict.theirs.version] {
                if !group.versions.contains(version) {
                    group.versions.push(version.clone());
                }
            }
        }
    }
    groups
}

fn select_versions(
    candidates: &[Candidate],
    prompter: &mut dyn Prompter,
) -> Result<BTreeMap<String, String>, PromptError> {
    let mut selected = BTreeMap::new();
    for (key, group) in collect_groups(candidates) {
        let mut versions = group.versions.clone();
        // Highest first, so it is the default.
        versions.sort_by(|a, b| compare_versions(b, a));

        if versions.len() == 1 {
            selected.insert(key, versions[0].clone());
            continue;
        }

        let choices: Vec<Choice> = versions
            .iter()
            .enumerate()
            .map(|(index, version)| {
                if index == 0 {
                    Choice::new(format!("{version} (higher)"), version)
                } else {
                    Choice::plain(version)
                }
            })
            .collect();
        let question = Question::new(key.clone(), prompt_message(&group.paths));
        let index = prompter.select(&question, &choices, Some(0))?;
        selected.insert(key, versions[index].clone());
    }
    Ok(selected)
}

fn apply_selected(
    contents: &str,
    conflicts: &[VersionConflict],
    selected: &BTreeMap<String, String>,
) -> String {
    let mut ordered: Vec<&VersionConflict> = conflicts.iter().collect();
    ordered.sort_by_key(|conflict| std::cmp::Reverse(conflict.start));

    let mut current = contents.to_string();
    for conflict in ordered {
        let key = version_question_key(&conflict.ours.version, &conflict.theirs.version);
        let replacement = match selected.get(&key) {
            Some(version) if *version == conflict.theirs.version => &conflict.theirs.line,
            _ => &conflict.ours.line,
        };
        current.replace_range(conflict.start..conflict.end, replacement);
    }
    current
}

fn io_error(path: &Path, error: std::io::Error) -> ConflictError {
    ConflictError::Io {
        path: path.to_path_buf(),
        reason: error.to_string(),
    }
}

/// Try to resolve `"version"` conflicts in any package.json among `conflicted_paths`.
///
/// Reads files relative to `cwd`, asks `prompter` (`conflict:auto_resolve`, then one
/// `conflict:version:<lo>|<hi>` per distinct version pair), rewrites, stages files with no markers
/// left, and commits with `--no-edit --no-verify` when git reports no remaining conflicts.
pub fn try_resolve_package_json_versions(
    cwd: &Path,
    conflicted_paths: &[String],
    git: &mut Git,
    prompter: &mut dyn Prompter,
) -> Result<ResolveOutcome, ConflictError> {
    let untouched = || ResolveOutcome {
        attempted: false,
        remaining: conflicted_paths.to_vec(),
        ..Default::default()
    };

    let package_json_paths: Vec<&String> = conflicted_paths
        .iter()
        .filter(|path| is_package_json_path(path))
        .collect();
    if package_json_paths.is_empty() {
        return Ok(untouched());
    }

    let mut candidates = Vec::new();
    for path in package_json_paths {
        let full_path = cwd.join(path);
        let contents = fs::read_to_string(&full_path).map_err(|e| io_error(&full_path, e))?;
        let conflicts = find_version_conflicts(&contents);
        if conflicts.is_empty() {
            continue;
        }
        candidates.push(Candidate {
            contents,
            full_path,
            path: path.clone(),
            conflicts,
        });
    }

    let conflict_count: usize = candidates.iter().map(|c| c.conflicts.len()).sum();
    if conflict_count == 0 {
        return Ok(untouched());
    }

    let should_resolve = prompter.confirm(
        &Question::new(
            AUTO_RESOLVE_KEY,
            format!(
                "Auto-resolve {conflict_count} package.json version {} across {} {}?",
                plural("conflict", conflict_count),
                candidates.len(),
                plural("file", candidates.len())
            ),
        ),
        true,
    )?;
    if !should_resolve {
        return Ok(untouched());
    }

    let selected = select_versions(&candidates, prompter)?;

    let mut rewritten = Vec::new();
    for candidate in &candidates {
        let resolved = apply_selected(&candidate.contents, &candidate.conflicts, &selected);
        if resolved != candidate.contents {
            fs::write(&candidate.full_path, &resolved)
                .map_err(|e| io_error(&candidate.full_path, e))?;
            rewritten.push(candidate.path.clone());
        }
        if !has_conflict_markers(&resolved) {
            git.call(&["add", candidate.path.as_str()])?;
        }
    }

    let remaining = git.conflicted_paths()?;
    let committed = remaining.is_empty();
    if committed {
        git.call(&["commit", "--no-edit", "--no-verify"])?;
    }

    Ok(ResolveOutcome {
        attempted: true,
        remaining,
        rewritten,
        committed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::fakes::ScriptedRunner;
    use crate::prompt::{PromptKind, RecordingPrompter, ScriptedPrompter};

    fn version_conflict_contents(ours: &str, theirs: &str) -> String {
        format!(
            "{{\n  \"name\": \"example\",\n<<<<<<< HEAD\n  \"version\": \"{ours}\",\n=======\n  \"version\": \"{theirs}\",\n>>>>>>> feature/release\n  \"private\": true\n}}\n"
        )
    }

    fn unresolved_contents(ours: &str, theirs: &str) -> String {
        format!(
            "{{\n  \"name\": \"example\",\n<<<<<<< HEAD\n  \"version\": \"{ours}\",\n=======\n  \"version\": \"{theirs}\",\n>>>>>>> feature/release\n<<<<<<< HEAD\n  \"private\": true\n=======\n  \"private\": false\n>>>>>>> feature/release\n}}\n"
        )
    }

    fn write(dir: &Path, relative: &str, contents: &str) {
        let path = dir.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn strs(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| v.to_string()).collect()
    }

    // Ported from resolvePackageJsonVersionConflicts.test.ts —
    // "resolves grouped monorepo package.json version conflicts and commits the merge"
    #[test]
    fn resolves_grouped_monorepo_version_conflicts_and_commits_the_merge() {
        let temp = tempfile::tempdir().unwrap();
        write(
            temp.path(),
            "package.json",
            &version_conflict_contents("1.9.0", "1.10.0"),
        );
        write(
            temp.path(),
            "apps/frontend/package.json",
            &version_conflict_contents("1.9.0", "1.10.0"),
        );

        let runner = ScriptedRunner::new().stdout("ls-files", "");
        let calls = runner.calls_handle();
        let mut git = runner.into_git();

        // Confirm yes; pick the first (higher) choice for every version question.
        let scripted = ScriptedPrompter::new()
            .answer(AUTO_RESOLVE_KEY, true)
            .answer_prefix("conflict:version:", crate::prompt::Answer::Index(0));
        let mut prompter = RecordingPrompter::new(scripted);

        let outcome = try_resolve_package_json_versions(
            temp.path(),
            &strs(&["package.json", "apps/frontend/package.json"]),
            &mut git,
            &mut prompter,
        )
        .unwrap();

        assert_eq!(
            outcome,
            ResolveOutcome {
                attempted: true,
                remaining: vec![],
                rewritten: strs(&["package.json", "apps/frontend/package.json"]),
                committed: true,
            }
        );

        // Two prompts: the confirm, then ONE grouped version question.
        assert_eq!(prompter.recorded.len(), 2);
        assert_eq!(
            prompter.recorded[1].question.message,
            "Which version should be used for 2 package.json conflicts?"
        );
        assert_eq!(
            prompter.recorded[1].question.key,
            "conflict:version:1.9.0|1.10.0"
        );
        match &prompter.recorded[1].kind {
            PromptKind::Select { choices, default } => {
                assert_eq!(*default, Some(0));
                assert_eq!(
                    choices.iter().map(|c| c.value.as_str()).collect::<Vec<_>>(),
                    vec!["1.10.0", "1.9.0"]
                );
                assert_eq!(choices[0].label, "1.10.0 (higher)");
            }
            other => panic!("expected a select, got {other:?}"),
        }

        // git: add, add, (ls-files to re-check), commit — baseline asserted add/add/commit.
        let calls = calls.borrow();
        assert_eq!(calls[0], vec!["add", "package.json"]);
        assert_eq!(calls[1], vec!["add", "apps/frontend/package.json"]);
        assert_eq!(calls[2], vec!["ls-files", "--unmerged"]);
        assert_eq!(calls[3], vec!["commit", "--no-edit", "--no-verify"]);
        assert_eq!(calls.len(), 4);

        let root = fs::read_to_string(temp.path().join("package.json")).unwrap();
        assert!(root.contains("\"version\": \"1.10.0\""));
        assert!(!root.contains("<<<<<<<"));
        let frontend = fs::read_to_string(temp.path().join("apps/frontend/package.json")).unwrap();
        assert!(frontend.contains("\"version\": \"1.10.0\""));
    }

    // Ported — "does not stage or commit when a package.json still has remaining conflicts"
    #[test]
    fn does_not_stage_or_commit_when_a_package_json_still_has_remaining_conflicts() {
        let temp = tempfile::tempdir().unwrap();
        write(
            temp.path(),
            "package.json",
            &unresolved_contents("1.32.5", "1.33.0"),
        );

        let runner = ScriptedRunner::new().stdout("ls-files", "100644 aaa 1\tpackage.json");
        let calls = runner.calls_handle();
        let mut git = runner.into_git();
        let mut prompter = ScriptedPrompter::new()
            .answer(AUTO_RESOLVE_KEY, true)
            .answer_prefix("conflict:version:", crate::prompt::Answer::Index(0));

        let outcome = try_resolve_package_json_versions(
            temp.path(),
            &strs(&["package.json"]),
            &mut git,
            &mut prompter,
        )
        .unwrap();

        assert!(outcome.attempted);
        assert_eq!(outcome.remaining, strs(&["package.json"]));
        assert!(!outcome.committed);

        // No `add`, no `commit` — only the re-check of conflicted paths.
        let calls = calls.borrow();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0], vec!["ls-files", "--unmerged"]);

        let contents = fs::read_to_string(temp.path().join("package.json")).unwrap();
        assert!(contents.contains("\"version\": \"1.33.0\""));
        assert!(contents.contains("<<<<<<<"));
    }

    #[test]
    fn non_package_json_conflicts_pass_through_untouched() {
        let temp = tempfile::tempdir().unwrap();
        let mut git = ScriptedRunner::new().into_git();
        let mut prompter = RecordingPrompter::new(ScriptedPrompter::new());
        let outcome = try_resolve_package_json_versions(
            temp.path(),
            &strs(&["src/main.rs", "README.md"]),
            &mut git,
            &mut prompter,
        )
        .unwrap();
        assert_eq!(
            outcome,
            ResolveOutcome {
                attempted: false,
                remaining: strs(&["src/main.rs", "README.md"]),
                ..Default::default()
            }
        );
        assert!(prompter.recorded.is_empty());
    }

    #[test]
    fn declining_auto_resolve_leaves_everything_alone() {
        let temp = tempfile::tempdir().unwrap();
        write(
            temp.path(),
            "package.json",
            &version_conflict_contents("1.0.0", "1.1.0"),
        );
        let runner = ScriptedRunner::new();
        let calls = runner.calls_handle();
        let mut git = runner.into_git();
        let mut prompter = ScriptedPrompter::new().answer(AUTO_RESOLVE_KEY, false);
        let outcome = try_resolve_package_json_versions(
            temp.path(),
            &strs(&["package.json"]),
            &mut git,
            &mut prompter,
        )
        .unwrap();
        assert!(!outcome.attempted);
        assert!(calls.borrow().is_empty());
        assert!(
            fs::read_to_string(temp.path().join("package.json"))
                .unwrap()
                .contains("<<<<<<<")
        );
    }

    #[test]
    fn choosing_ours_keeps_the_ours_line_and_crlf_files_are_handled() {
        let temp = tempfile::tempdir().unwrap();
        let crlf = version_conflict_contents("2.0.0", "2.1.0").replace('\n', "\r\n");
        write(temp.path(), "package.json", &crlf);
        let mut git = ScriptedRunner::new().stdout("ls-files", "").into_git();
        let mut prompter = ScriptedPrompter::new()
            .answer(AUTO_RESOLVE_KEY, true)
            .answer("conflict:version:2.0.0|2.1.0", "2.0.0");
        let outcome = try_resolve_package_json_versions(
            temp.path(),
            &strs(&["package.json"]),
            &mut git,
            &mut prompter,
        )
        .unwrap();
        assert!(outcome.committed);
        let contents = fs::read_to_string(temp.path().join("package.json")).unwrap();
        assert!(contents.contains("\"version\": \"2.0.0\",\r\n"));
        assert!(!contents.contains("2.1.0"));
        assert!(!contents.contains("<<<<<<<"));
    }

    #[test]
    fn single_path_prompt_names_the_file() {
        assert_eq!(
            prompt_message(&strs(&["apps/web/package.json"])),
            "Which version should be used for apps/web/package.json?"
        );
    }

    #[test]
    fn version_ordering_follows_the_baseline_rules() {
        use Ordering::*;
        let table = [
            ("1.9.0", "1.10.0", Less),
            ("1.10.0", "1.9.0", Greater),
            ("1.2", "1.2.0", Equal),
            ("1.0.0+build.5", "1.0.0", Equal),
            ("1.0.0", "1.0.0-rc.1", Greater),
            ("1.0.0-rc.1", "1.0.0-rc.2", Less),
            ("1.0.0-rc.2", "1.0.0-rc.10", Less),
            ("1.0.0-alpha", "1.0.0-1", Greater),
            ("1.0.0-alpha", "1.0.0-alpha.1", Less),
            ("1.0.0-alpha", "1.0.0-beta", Less),
        ];
        for (left, right, expected) in table {
            assert_eq!(compare_versions(left, right), expected, "{left} vs {right}");
        }
        assert_eq!(
            version_question_key("1.10.0", "1.9.0"),
            "conflict:version:1.9.0|1.10.0"
        );
    }

    #[test]
    fn package_json_path_detection() {
        assert!(is_package_json_path("package.json"));
        assert!(is_package_json_path("apps/frontend/package.json"));
        assert!(!is_package_json_path("package.json.bak"));
        assert!(!is_package_json_path("mypackage.json"));
    }
}
