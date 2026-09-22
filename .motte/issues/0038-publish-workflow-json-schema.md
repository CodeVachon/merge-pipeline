---
id: 38
title: Publish workflow JSON schema
state: Done
parent: 7
assignee: claude-code
labels: [docs]
blockedBy: [11]
created: 2026-09-17T17:14:13Z
updated: 2026-09-21T20:19:28Z
---

## Description

schema/config.json copied from baseline (draft-07; fields disabled/name/description/order/pipeline minItems 2; additionalProperties false). Serve via GitHub Pages or reference the raw main URL as `$id`; examples/config/*.json carry `$schema`. Validate examples against it in CI with a tiny Rust test using serde (no external validator needed for this shape) or `check-jsonschema` in the workflow.

## Notes

### 2026-09-21T20:19:23Z — claude-code (agent)

schema/config.json: `$id` set to https://raw.githubusercontent.com/CodeVachon/merge-pipeline/main/schema/config.json; title/description rewritten for a merge-pipeline workflow file (baseline text said "Server Definitions"); removed the meaningless `additionalItems`. Important fix: the baseline schema had additionalProperties:false with no `$schema` property, so any example carrying a `$schema` key would FAIL validation. Added `$schema` as an allowed string property. examples/config/*.json now point at the absolute URL. CI `schema` job added to ci.yml running `pipx run check-jsonschema --schemafile schema/config.json examples/config/*.json`. Local validation: check-jsonschema, pipx and python jsonschema are not installed here, so I ran a hand-rolled python check (required keys, no unexpected properties, types, pipeline minItems 2, each pipeline entry compiles as a regex) — all four examples pass. The real check-jsonschema run happens in CI on the first push. Note the URL only resolves once the repo is pushed to GitHub.
