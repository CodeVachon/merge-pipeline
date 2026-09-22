//! Resolve the version to install from the GitHub releases API.

use std::env;
use std::time::Duration;

use serde::Deserialize;
use thiserror::Error;

use super::layout::REPO;

/// Default GitHub API host; `MERGE_PIPELINE_API_BASE` overrides it for tests and mirrors.
pub const DEFAULT_API_BASE: &str = "https://api.github.com";

/// The lookup could not produce a version.
#[derive(Debug, Error)]
pub enum ReleaseError {
    #[error("could not reach the GitHub API: {0}")]
    Network(String),
    #[error(
        "GitHub API rate limit reached. Unauthenticated requests are capped at 60 per hour per IP. \
         Retry later, or pass a version explicitly: merge-pipeline upgrade 0.1.0"
    )]
    RateLimited,
    #[error(
        "the GitHub API returned {0} when listing releases. Pass a version explicitly to skip the \
         lookup: merge-pipeline upgrade 0.1.0"
    )]
    Status(u16),
    #[error("the GitHub API returned something that was not JSON")]
    NotJson,
    #[error(
        "no published release was found for {REPO}. Pass a version explicitly to skip the lookup: \
         merge-pipeline upgrade 0.1.0"
    )]
    NoRelease,
}

#[derive(Debug, Deserialize)]
struct ApiRelease {
    tag_name: Option<String>,
    #[serde(default)]
    draft: bool,
}

/// `$MERGE_PIPELINE_API_BASE` or the real GitHub API.
pub fn api_base() -> String {
    env::var("MERGE_PIPELINE_API_BASE")
        .ok()
        .filter(|v| !v.is_empty())
        .map(|v| v.trim_end_matches('/').to_owned())
        .unwrap_or_else(|| DEFAULT_API_BASE.to_owned())
}

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(timeout))
            .user_agent("merge-pipeline")
            .build(),
    )
}

struct ApiResponse {
    status: u16,
    body: String,
}

fn api_get(agent: &ureq::Agent, url: &str) -> Result<ApiResponse, ReleaseError> {
    let response = agent
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| ReleaseError::Network(e.to_string()))?;
    let status = response.status().as_u16();
    let body = response
        .into_body()
        .read_to_string()
        .map_err(|e| ReleaseError::Network(e.to_string()))?;
    Ok(ApiResponse { status, body })
}

fn is_rate_limited(response: &ApiResponse) -> bool {
    (response.status == 403 || response.status == 429)
        && response.body.to_lowercase().contains("rate limit")
}

/// The newest release tag, preferring a stable release.
///
/// Mirrors `install.sh`: `/releases/latest` excludes prereleases, so while every release is a
/// 0.x prerelease it 404s and the fallback to `/releases` is the only path that finds anything.
pub fn resolve_latest_version() -> Result<String, ReleaseError> {
    resolve_latest_version_at(&api_base(), Duration::from_secs(20))
}

/// [`resolve_latest_version`] against an explicit API base URL.
pub fn resolve_latest_version_at(
    api_base: &str,
    timeout: Duration,
) -> Result<String, ReleaseError> {
    let agent = agent(timeout);
    let base = api_base.trim_end_matches('/');

    let stable = api_get(&agent, &format!("{base}/repos/{REPO}/releases/latest"))?;
    if stable.status == 200
        && let Ok(release) = serde_json::from_str::<ApiRelease>(&stable.body)
        && let Some(tag) = release.tag_name.filter(|t| !t.is_empty())
    {
        return Ok(tag);
    }
    if is_rate_limited(&stable) {
        return Err(ReleaseError::RateLimited);
    }

    let all = api_get(&agent, &format!("{base}/repos/{REPO}/releases"))?;
    if is_rate_limited(&all) {
        return Err(ReleaseError::RateLimited);
    }
    if all.status != 200 {
        return Err(ReleaseError::Status(all.status));
    }

    let releases: Vec<ApiRelease> =
        serde_json::from_str(&all.body).map_err(|_| ReleaseError::NotJson)?;

    // Drafts have no downloadable assets, so skipping them avoids a confusing 404 later.
    releases
        .into_iter()
        .find(|r| !r.draft && r.tag_name.as_deref().is_some_and(|t| !t.is_empty()))
        .and_then(|r| r.tag_name)
        .ok_or(ReleaseError::NoRelease)
}
