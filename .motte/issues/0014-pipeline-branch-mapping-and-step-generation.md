---
id: 14
title: Pipeline → branch mapping and step generation
state: Done
parent: 2
assignee: claude-code
labels: [core]
blockedBy: [12]
created: 2026-09-17T17:13:05Z
updated: 2026-09-21T20:26:03Z
---

## Description

Port mapPipelineValueToBranch.ts + makeWorkflowSteps.ts. `map_pipeline(pipeline, branches, prompter) -> Result<Vec<String>>`: for each pattern compile `regex::Regex::new(&format!("(?i){pattern}"))` (search, not anchored — baseline used RegExp.test); filter branches not already mapped; 0 → `PipelineError::NoMatch(pattern)`; 1 → take; many → prompter.select with key `branch:<pattern>`. Also expose `map_pipeline_report(pipeline, branches) -> Vec<Resolution>` where Resolution is Resolved(branch) | Ambiguous(pattern, candidates) | NoMatch(pattern) so the MCP plan_workflow tool can report without prompting. `make_steps(branches) -> Vec<Step{source,target}>` pairs consecutive entries.

## Plan

Tests: makeWorkflowSteps parity (4 → 3 steps); regex is case-insensitive; already-mapped branch excluded (the canary config lists ^Release-* twice on purpose and expects two different Release branches); invalid regex surfaces as an error naming the pattern.

## Notes

### 2026-09-21T20:25:51Z — claude-code (agent)

src/pipeline.rs done. Step{source,target} + Step::question_key() = "step:<src>>><tgt>"; make_steps via windows(2). map_pipeline(pipeline, branches, &mut dyn Prompter): compiles `(?i)<pattern>` (unanchored is_match, same as JS RegExp.test), excludes already-mapped, 0 → PipelineError::NoMatch with the baseline message, 1 → take, many → select with key branch:<pattern> and message `Which branch should we use for "<pattern>"` (uncoloured in core; CLI colours it). map_pipeline_report(pipeline, branches, selections) returns Vec<Resolution{Resolved|Ambiguous{candidates}|NoMatch}> for the MCP plan tool; resolved_branches() collapses when complete. Invalid regex → InvalidPattern naming the pattern. Ported makeWorkflowSteps.test.ts; added case-insensitivity, canary double ^Release-* picks two distinct releases (second one auto-resolves once one candidate remains), unanswered prompt surfaces as PromptError::Unanswered with candidate list, report honours selections.
