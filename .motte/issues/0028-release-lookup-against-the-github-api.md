---
id: 28
title: Release lookup against the GitHub API
state: Done
parent: 5
labels: [self-update]
blockedBy: [27]
created: 2026-09-17T17:13:55Z
updated: 2026-09-21T20:39:36Z
---

## Description

Port install/releases.ts with ureq: GET /repos/CodeVachon/merge-pipeline/releases/latest (Accept: application/vnd.github+json, User-Agent: merge-pipeline); on 200 use tag_name; on 403/429 with "rate limit" in the body raise a clear error suggesting `upgrade <version>`; otherwise GET /releases and take the newest non-draft. Accept a base-URL override so tests hit a local server (or fake the transport behind a trait).

## Notes

### 2026-09-21T20:39:10Z — claude-code (agent)

Done in src/selfupdate/releases.rs with ureq 3 (Agent with http_status_as_error(false) so 404/403 bodies are readable). resolve_latest_version_at(api_base, timeout) is the testable entry; MERGE_PIPELINE_API_BASE overrides the host. Behaviour matches motte's releases.ts: /releases/latest on 200 → tag; 403/429 with "rate limit" in body → ReleaseError::RateLimited; else /releases newest non-draft; non-200 → Status(code); empty → NoRelease. Integration tests in tests/selfupdate.rs run against a std::net::TcpListener fake server (no network). Observation for #33: ureq's `rustls` feature pulls the `ring` provider (Cargo.lock confirms), not aws-lc-rs, so the musl builds should not need a C toolchain beyond musl-tools.
