//! Talks to GitHub for the 5etools-src `data/` tree: the
//! commit REST API for version checks and the file-tree listing (two calls,
//! subject to GitHub's unauthenticated REST rate limit), then
//! raw.githubusercontent.com - a CDN, not the rate-limited REST API - for
//! the many individual file fetches.

use serde::Deserialize;

const OWNER: &str = "5etools-mirror-3";
const REPO: &str = "5etools-src";
const BRANCH: &str = "main";

#[derive(Deserialize)]
struct CommitResponse {
    sha: String,
}

/// The current commit sha of the tracked branch, used both as the sync's
/// "is our cache stale" check and to pin every file fetch in a sync to one
/// consistent snapshot.
pub async fn fetch_head_sha() -> Result<String, String> {
    let url = format!("https://api.github.com/repos/{OWNER}/{REPO}/commits/{BRANCH}");
    let resp = gloo_net::http::Request::get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("fetching {url}: {e}"))?;
    if !resp.ok() {
        return Err(format!("fetching {url}: HTTP {}", resp.status()));
    }
    let commit: CommitResponse = resp.json().await.map_err(|e| format!("parsing {url}: {e}"))?;
    Ok(commit.sha)
}

#[derive(Deserialize)]
struct TreeResponse {
    tree: Vec<TreeEntry>,
    truncated: bool,
}

#[derive(Deserialize)]
struct TreeEntry {
    path: String,
    /// The git blob sha of the file's content: unchanged content keeps its sha
    /// across commits, which is what lets a sync skip unchanged files.
    sha: String,
    #[serde(rename = "type")]
    kind: String,
}

/// A `data/` file as listed by the repo tree.
pub struct RemoteFile {
    /// Relative to `data/`.
    pub path: String,
    pub blob_sha: String,
}

/// Every JSON file under `data/` at `sha`, with paths relative to `data/`
/// (matching what the loader's data source expects).
pub async fn fetch_data_file_list(sha: &str) -> Result<Vec<RemoteFile>, String> {
    let url = format!("https://api.github.com/repos/{OWNER}/{REPO}/git/trees/{sha}?recursive=1");
    let resp = gloo_net::http::Request::get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("fetching {url}: {e}"))?;
    if !resp.ok() {
        return Err(format!("fetching {url}: HTTP {}", resp.status()));
    }
    let body: TreeResponse = resp.json().await.map_err(|e| format!("parsing {url}: {e}"))?;
    if body.truncated {
        // GitHub caps a single recursive tree response; a truncated result
        // would silently sync an incomplete dataset, which is worse than
        // failing loudly.
        return Err("GitHub truncated the repo tree listing - can't see the full data/ tree".to_string());
    }
    Ok(body
        .tree
        .into_iter()
        .filter(|e| e.kind == "blob" && e.path.starts_with("data/") && e.path.ends_with(".json"))
        .map(|e| RemoteFile {
            path: e.path.trim_start_matches("data/").to_string(),
            blob_sha: e.sha,
        })
        .collect())
}

pub async fn fetch_data_file(sha: &str, relative_path: &str) -> Result<String, String> {
    let url = format!("https://raw.githubusercontent.com/{OWNER}/{REPO}/{sha}/data/{relative_path}");
    let resp = gloo_net::http::Request::get(&url)
        .send()
        .await
        .map_err(|e| format!("fetching {url}: {e}"))?;
    if !resp.ok() {
        return Err(format!("fetching {url}: HTTP {}", resp.status()));
    }
    resp.text().await.map_err(|e| format!("reading {url}: {e}"))
}
