---
id: 57
title: "CI: book job on PRs, pages.yml deploy-and-verify on main"
state: Done
parent: 55
assignee: claude-code
labels: [docs]
blockedBy: [56]
created: 2026-09-23T18:33:37Z
updated: 2026-09-23T18:43:17Z
---

## Description

Add a `book` job to .github/workflows/ci.yml, paths-filtered to `docs/**` and workflow changes, or just always-on if that is simpler and the cost is acceptable (an mdbook build is seconds): install a pinned mdbook version (curl the release tarball the way install.sh fetches assets, or a maintained setup action — pick one and note the choice), `mdbook build docs`, fail on any warning, and a link-check step (peaceiris/actions-mdbook has a linkcheck backend, or a small script using `mdbook-linkcheck` if trivial to add, or grep for `href="http` 404s with a lightweight local check — pick the cheapest option that actually catches a broken internal link and say why). Add .github/workflows/pages.yml: on push to main (and workflow_dispatch), build the book, actions/configure-pages, actions/upload-pages-artifact with docs/book (mdbook's output dir), actions/deploy-pages, then curl the deployed URL and grep for the book title so a deploy that served nothing is caught — same pattern release.yml already uses for install.sh/install.ps1 and the pattern motte's own pages.yml uses for its schema.

## Plan

Validate both workflow YAMLs with ruby -ryaml. This job cannot be exercised end-to-end until the branch reaches main (pages.yml only triggers on push to main), so the PR description must say so plainly — the CI `book` job on the PR is what can be verified now.

## Notes

### 2026-09-23T18:43:16Z — claude-code (agent)

Added a `book` job to ci.yml (always-on, no path filter, matching the existing `schema` job's convention — mdbook build is seconds so the cost is negligible) and a new pages.yml.

Decisions, with reasons:
- mdBook install: pinned release tarball (v0.5.4, the same version verified locally) downloaded via curl and verified against a hardcoded sha256 before extracting — not peaceiris/actions-mdbook (third-party trust surface for no real benefit) and not `cargo install mdbook` (multi-minute compile every run). Matches how this project already treats every other downloaded binary (install.sh, selfupdate). Verified the exact download+checksum+extract commands locally against the real GitHub release before writing them into YAML.
- Link check: a 60-line checked-in script (docs/check-links.py) that extracts every local href from the built HTML and confirms the target file exists in the output — not mdbook-linkcheck (unmaintained, no prebuilt binary, needs a slow cargo install for one job). Proven to actually catch a break: I reintroduced the real dead-link bug from #56's build (rewrote a MIGRATING.html href to a nonsense target), ran the script, got exit 1 with a clear message; restored, got exit 0.
- `mdbook build` exits 0 even when it printed a WARN line (confirmed: the unclosed-`<branch>`-tag warning from #56 did not fail the build). So the book job explicitly greps the build log for " WARN " and fails if found — a bare `mdbook build docs` in CI would NOT have caught that bug.
- pages.yml paths-filtered to `docs/**` plus itself, so an unrelated push to main does not redeploy the site. Deploy job's final step curls the live URL and greps for the literal opening sentence of introduction.md ("Merge git branches, one into the next"), the same "verify what's actually live" pattern release.yml uses for install.sh and motte's own pages.yml uses for its schema — a green upload does not by itself prove the site serves anything.

Cannot be exercised end-to-end pre-merge: pages.yml only triggers on push to main, so the actual Pages deployment and its live-content check have not run yet. What WAS verified locally, exactly as CI will run it: the mdbook download+checksum+extract sequence against the real GitHub release, `mdbook build docs` producing zero WARN lines, and check-links.py against the real build output (14 pages, 0 broken links, and proven to catch the one real bug found during #56).
