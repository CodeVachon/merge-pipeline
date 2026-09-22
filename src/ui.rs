//! Terminal presentation: colors, title box, error display, event rendering.
//!
//! Ported from the baseline's `utl/colors.ts`, `utl/title.ts`, `utl/displayError.ts`,
//! `utl/log.ts` and the console output in `actions/shared.ts`. Everything renders to a `String`
//! first so tests can assert on it with the ANSI stripped; the `print_*` helpers write it out.

use std::io::{IsTerminal, Write};

use owo_colors::{OwoColorize, Style};

use crate::runner::{Event, EventSink, MergeError, RunError};

/// Orange `#ff6700`, cyan `#00d5ff`, bright red — or nothing, when colors are off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    enabled: bool,
}

impl Palette {
    /// Colors on when stdout is a terminal, `NO_COLOR` is unset and `TERM` is not `dumb`.
    pub fn detect() -> Self {
        let enabled = std::io::stdout().is_terminal()
            && std::env::var_os("NO_COLOR").is_none_or(|value| value.is_empty())
            && std::env::var("TERM")
                .map(|term| term != "dumb")
                .unwrap_or(true);
        Self { enabled }
    }

    pub fn colored() -> Self {
        Self { enabled: true }
    }

    pub fn plain() -> Self {
        Self { enabled: false }
    }

    pub fn is_enabled(self) -> bool {
        self.enabled
    }

    fn apply(self, text: &str, style: Style) -> String {
        if self.enabled {
            text.style(style).to_string()
        } else {
            text.to_string()
        }
    }

    pub fn orange(self, text: &str) -> String {
        self.apply(text, Style::new().truecolor(0xff, 0x67, 0x00))
    }

    pub fn cyan(self, text: &str) -> String {
        self.apply(text, Style::new().truecolor(0x00, 0xd5, 0xff))
    }

    pub fn red(self, text: &str) -> String {
        self.apply(text, Style::new().bright_red())
    }

    pub fn dim(self, text: &str) -> String {
        self.apply(text, Style::new().dimmed())
    }
}

/// Remove ANSI escape sequences (for tests and for logs that are not terminals).
pub fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// The boxed title from `utl/title.ts`: blank, blank, box, blank.
pub fn title(palette: Palette, text: &str) -> String {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    let width = lines
        .iter()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0);

    let mut out = String::from("\n\n");
    if width > 0 {
        let border = format!("+{}+", "-".repeat(width + 2));
        out.push_str(&palette.orange(&border));
        out.push('\n');
        for line in &lines {
            let padded = format!("| {:<width$} |", line, width = width);
            out.push_str(&palette.orange(&padded));
            out.push('\n');
        }
        out.push_str(&palette.orange(&border));
        out.push('\n');
    }
    out.push('\n');
    out
}

pub fn print_title(palette: Palette, text: &str) {
    print!("{}", title(palette, text));
}

/// `Pipeline: a > b > c` — patterns orange, separators cyan.
pub fn format_pipeline(palette: Palette, patterns: &[String]) -> String {
    let joined = patterns
        .iter()
        .map(|pattern| palette.orange(pattern))
        .collect::<Vec<_>>()
        .join(&palette.cyan(" > "));
    format!("Pipeline: {joined}")
}

/// `Step N: Merge <source> into <target>` lines, surrounded by blank lines.
pub fn format_steps(palette: Palette, steps: &[crate::pipeline::Step]) -> String {
    let mut out = String::from("\n");
    for (index, step) in steps.iter().enumerate() {
        out.push_str(&format!(
            "Step {}: Merge {} into {}\n",
            index + 1,
            palette.cyan(&step.source),
            palette.orange(&step.target)
        ));
    }
    out.push('\n');
    out
}

/// `$ git ...` — the verbose command echo from `utl/log.ts`.
pub fn format_command(palette: Palette, rendered: &str) -> String {
    format!("{} {rendered}", palette.cyan("$"))
}

/// The red error block from `utl/displayError.ts`.
///
/// For a merge that stopped on conflicts, the conflicted paths are listed under "Error
/// Details" so the user knows what to resolve by hand. Every other error prints its message and
/// the chain of causes.
pub fn format_error(palette: Palette, error: &anyhow::Error) -> String {
    let rule = "=".repeat(20);
    let mut out = String::from("\n");
    out.push_str(&palette.red("Error"));
    out.push('\n');
    out.push_str(&palette.red(&rule));
    out.push('\n');
    out.push_str(&format!("{error}\n"));
    out.push('\n');
    out.push_str("Error Details\n");
    out.push_str(&rule);
    out.push('\n');

    if let Some(merge) = merge_error(error) {
        for path in &merge.files {
            out.push_str(path);
            out.push('\n');
        }
        if let Some(cause) = &merge.cause {
            out.push_str(cause);
            out.push('\n');
        }
    } else {
        for cause in error.chain().skip(1) {
            out.push_str(&format!("caused by: {cause}\n"));
        }
    }
    out.push('\n');
    out
}

fn merge_error(error: &anyhow::Error) -> Option<&MergeError> {
    if let Some(merge) = error.downcast_ref::<MergeError>() {
        return Some(merge);
    }
    match error.downcast_ref::<RunError>() {
        Some(RunError::Merge(merge)) => Some(merge),
        _ => None,
    }
}

pub fn print_error(palette: Palette, error: &anyhow::Error) {
    let text = format_error(palette, error);
    let stderr = std::io::stderr();
    let mut lock = stderr.lock();
    let _ = lock.write_all(text.as_bytes());
    let _ = lock.flush();
}

/// Renders runner events to stdout the way the baseline's actions logged them.
pub struct StdoutRenderer {
    palette: Palette,
}

impl StdoutRenderer {
    pub fn new(palette: Palette) -> Self {
        Self { palette }
    }

    /// What to print for `event`, if anything.
    pub fn render(&self, event: &Event) -> Option<String> {
        match event {
            Event::Pipeline { patterns } => {
                Some(format!("{}\n", format_pipeline(self.palette, patterns)))
            }
            Event::StepsPlanned { steps } => Some(format_steps(self.palette, steps)),
            Event::ConflictsResolved {
                rewritten,
                committed,
                ..
            } if !rewritten.is_empty() => Some(format!(
                "{} {}{}\n",
                self.palette.cyan("Resolved package.json versions in"),
                rewritten.join(", "),
                if *committed { " (committed)" } else { "" }
            )),
            _ => None,
        }
    }
}

impl EventSink for StdoutRenderer {
    fn event(&mut self, event: &Event) {
        if let Some(text) = self.render(event) {
            print!("{text}");
        }
    }
}

/// A [`crate::git::CommandLog`] that echoes `$ git ...` to stdout.
pub fn command_logger(palette: Palette) -> impl FnMut(&str) {
    move |rendered: &str| println!("{}", format_command(palette, rendered))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::Step;

    #[test]
    fn title_draws_the_box_from_the_baseline() {
        let out = title(Palette::plain(), "Git Branch\nWorkflow");
        assert_eq!(
            out,
            "\n\n+------------+\n| Git Branch |\n| Workflow   |\n+------------+\n\n"
        );
    }

    #[test]
    fn title_with_no_lines_prints_only_blank_lines() {
        assert_eq!(title(Palette::plain(), " \n "), "\n\n\n");
    }

    #[test]
    fn colored_output_strips_back_to_plain() {
        let colored = title(Palette::colored(), "Hi");
        assert_ne!(colored, title(Palette::plain(), "Hi"));
        assert_eq!(strip_ansi(&colored), title(Palette::plain(), "Hi"));
        assert!(strip_ansi(&Palette::colored().orange("x")) == "x");
    }

    #[test]
    fn pipeline_and_steps_render_like_shared_ts() {
        let palette = Palette::plain();
        assert_eq!(
            format_pipeline(palette, &["^Patch-*".into(), "^staging-patch$".into()]),
            "Pipeline: ^Patch-* > ^staging-patch$"
        );
        let steps = vec![
            Step {
                source: "a".into(),
                target: "b".into(),
            },
            Step {
                source: "b".into(),
                target: "c".into(),
            },
        ];
        assert_eq!(
            format_steps(palette, &steps),
            "\nStep 1: Merge a into b\nStep 2: Merge b into c\n\n"
        );
        assert_eq!(
            format_command(palette, "git merge \"a b\" --no-verify"),
            "$ git merge \"a b\" --no-verify"
        );
    }

    #[test]
    fn merge_error_lists_conflicted_files_under_details() {
        let error: anyhow::Error = RunError::Merge(MergeError {
            step: Step {
                source: "Patch-v0.1.1".into(),
                target: "staging-patch".into(),
            },
            files: vec!["package.json".into(), "apps/web/package.json".into()],
            cause: None,
        })
        .into();
        let out = format_error(Palette::plain(), &error);
        assert_eq!(
            out,
            "\nError\n====================\n\
             Merge Error: Patch-v0.1.1 into staging-patch. 2 conflicted files\n\n\
             Error Details\n====================\n\
             package.json\napps/web/package.json\n\n"
        );
    }

    #[test]
    fn plain_error_prints_message_and_causes() {
        let error = anyhow::anyhow!("inner problem").context("Git is in a Dirty State");
        let out = format_error(Palette::plain(), &error);
        assert!(out.starts_with("\nError\n====================\nGit is in a Dirty State\n"));
        assert!(out.contains("caused by: inner problem\n"));
    }

    #[test]
    fn renderer_prints_only_pipeline_steps_and_resolutions() {
        let renderer = StdoutRenderer::new(Palette::plain());
        assert!(renderer.render(&Event::Fetched).is_none());
        assert!(
            renderer
                .render(&Event::Pipeline {
                    patterns: vec!["a".into()]
                })
                .is_some()
        );
        assert!(
            renderer
                .render(&Event::ConflictsResolved {
                    step: Step {
                        source: "a".into(),
                        target: "b".into()
                    },
                    rewritten: vec![],
                    committed: false
                })
                .is_none()
        );
    }
}
