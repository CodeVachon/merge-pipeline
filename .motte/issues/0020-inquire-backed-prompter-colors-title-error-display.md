---
id: 20
title: inquire-backed Prompter, colors, title, error display
state: Done
parent: 3
assignee: claude-code
labels: [cli]
blockedBy: [12]
created: 2026-09-17T17:13:18Z
updated: 2026-09-21T20:40:38Z
---

## Description

Implement `Prompter` with inquire: confirm → Confirm, select → Select (fuzzy filter on, replaces inquirer-search-list; when >1 choice), text → Text with default. Colors via owo-colors: orange #ff6700, cyan #00d5ff, red bright; respect NO_COLOR and non-tty. `title()` renders the +---+ box from utl/title.ts. `display_error()` renders the red "Error" block, the message, "Error Details" and, for MergeError, one conflicted path per line. Event renderer prints `Pipeline: a > b > c`, `Step N: Merge <src> into <tgt>`, and `$ git …` in verbose mode.

## Plan

Snapshot the title box and error block for a MergeError with two files (strip ANSI in tests).

## Notes

### 2026-09-21T20:40:37Z — claude-code (agent)

Done. src/ui.rs: Palette (detect: stdout is a tty, NO_COLOR unset, TERM != dumb; plain/colored for tests) with orange #ff6700 / cyan #00d5ff / bright red via owo_colors::Style; strip_ansi; title() reproduces utl/title.ts byte for byte ("\n\n+----+\n| .. |\n+----+\n\n"); format_pipeline/format_steps/format_command reproduce shared.ts + log.ts; format_error reproduces displayError.ts, listing MergeError.files under "Error Details" (downcasts anyhow → RunError::Merge or MergeError) and otherwise the anyhow cause chain in place of the raw object dump. StdoutRenderer implements runner::EventSink and prints only Pipeline, StepsPlanned and a non-empty ConflictsResolved line, matching how little the baseline printed between git echoes. src/cli/prompter.rs: InteractivePrompter over inquire 0.9 (Confirm/Select::raw_prompt for the index with starting cursor as default/Text with default); Esc, Ctrl-C and non-tty map to PromptError::Interrupted. 7 unit tests; interactive prompts themselves are not unit-tested (no tty), the flow is tested non-interactively in #21.
