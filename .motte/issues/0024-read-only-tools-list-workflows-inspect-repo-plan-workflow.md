---
id: 24
title: "Read-only tools: list_workflows, inspect_repo, plan_workflow"
state: Done
parent: 4
assignee: claude-code
labels: [mcp]
blockedBy: [13, 14, 15, 19, 23]
created: 2026-09-17T17:13:33Z
updated: 2026-09-21T20:53:37Z
---

## Description

list_workflows → workflows with source paths + load errors + discovery root. inspect_repo → {clean, current_branch, branches:[{name, has_upstream}]}, `fetch` bool arg. plan_workflow {workflow, selections?: {pattern: branch}} → {steps:[{source,target}]} or {ambiguous:[{pattern, candidates}], no_match:[pattern]} using map_pipeline_report; never prompts.

## Notes

### 2026-09-21T20:53:20Z — claude-code (agent)

list_workflows → {root, workflows[{name,description,order,disabled,pipeline,source}], enabled[names], errors[{path,reason}]} — disabled workflows and unparseable files are reported, not hidden. inspect_repo → {cwd, clean, current_branch, fetched, branches[{name, has_upstream}]}; has_upstream is computed with `git rev-parse --abbrev-ref --symbolic-full-name <branch>@{u}` so it never checks branches out (Git::branch_has_upstream would, as the baseline did). fetch defaults to false here (read-only, no network unless asked). plan_workflow {workflow, selections?, fetch=true} → {workflow, pipeline, resolutions[...], complete, and either branches+steps or ambiguous[{pattern,candidates}]+no_match[]}; ambiguity is a normal result, unknown workflow is an isError carrying the baseline's "Could not find Workflow named" message with the valid names. Note fetch defaults to true for plan (matches the CLI's prepare) but false for inspect.
