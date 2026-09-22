---
id: 12
title: Prompter trait and answer model
state: Done
parent: 2
assignee: claude-code
labels: [core]
blockedBy: [9]
created: 2026-09-17T17:13:05Z
updated: 2026-09-21T20:19:33Z
---

## Description

`trait Prompter { fn confirm(&mut self, q: &Question, default: bool) -> Result<bool>; fn select(&mut self, q: &Question, choices: &[Choice]) -> Result<usize>; fn text(&mut self, q: &Question, default: Option<&str>) -> Result<String>; }` where Question carries a stable `key` (e.g. "cwd", "workflow", "action", "auto_push", "branch:<pattern>", "step:<src>>>tgt", "push:<branch>", "conflict:auto_resolve", "conflict:version:<a>|<b>") and a human message. Keys are what the MCP surface uses to pre-answer. Provide `ScriptedPrompter` (map key→answer, errors with `UnansweredQuestion{key, message, choices}`) for tests and MCP, and a `RecordingPrompter` wrapper for assertions.

## Plan

Write the trait, the two test-oriented impls, and unit tests. The inquire-backed impl lives in the CLI epic.

## Notes

### 2026-09-21T20:19:32Z — claude-code (agent)

Implemented src/prompt.rs: Question{key,message}, Choice{label,value}, PromptError{Unanswered{key,message,choices}, InvalidAnswer, Interrupted}, trait Prompter{confirm, select, text}. Deviation from the sketch: `select` takes `default: Option<usize>` — the package.json resolver needs "higher version pre-selected" (baseline passed default: 0) and the CLI needs to show it. Answer enum: Bool | Text (matches select by value then label) | Index | Default. ScriptedPrompter supports exact keys plus prefix rules (`answer_prefix("step:", true)`) so the MCP run_workflow tool can say yes to every step in one rule; exact beats prefix. RecordingPrompter wraps any Prompter and records Question + PromptKind (with choices/defaults) for assertions. Also added src/text.rs (plural, capitalize, action_to_string, array_to_string_list) with the baseline table tests from plural/capitalize/convertActionToString/arrayToStringList ported; capitalize's "throws on non-string" case is a compile-time type error in Rust and has no runtime equivalent. 13 unit tests green.
