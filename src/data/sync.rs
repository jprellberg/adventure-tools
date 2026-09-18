//! Downloads and caches the 5etools-src `data/` tree.
//! `super::loader` decides *when* to call [`sync_files`] (first run vs. an
//! explicit update) and reads individual files back out of the cache it
//! populates.

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use dioxus::prelude::{Signal, WritableExt};
use futures::stream::{self, StreamExt};

use super::github::RemoteFile;
use super::{db, github};

/// How many files to fetch concurrently during a sync. raw.githubusercontent.com
/// isn't subject to the REST API's strict per-hour rate limit, but browsers
/// still cap concurrent connections per host, so an unbounded `join_all`
/// over a few hundred files wouldn't actually run them all in parallel -
/// it'd just queue them chaotically. A modest, explicit cap is simpler to
/// reason about.
const CONCURRENCY: usize = 8;

/// `STORE_META` key of the last-synced commit sha.
const KEY_VERSION: &str = "version";
/// `STORE_META` key of the JSON map from file path to the blob sha last synced.
const KEY_FILE_SHAS: &str = "file_shas";

pub async fn remote_sha() -> Result<String, String> {
    github::fetch_head_sha().await
}

pub async fn stored_sha() -> Result<Option<String>, String> {
    db::get(db::STORE_META, KEY_VERSION).await
}

pub async fn cached_file_count() -> Result<usize, String> {
    db::count(db::STORE_FILES).await
}

/// Brings the local cache up to date with the tree at `sha`, downloading
/// only files that are new or whose content changed (by git blob sha) and
/// removing files that no longer exist, reporting which file is in flight via
/// `progress`.
pub async fn sync_files(sha: &str, mut progress: Signal<String>) -> Result<(), String> {
    progress.set("Fetching file list...".to_string());
    let remote = github::fetch_data_file_list(sha).await?;
    let known: HashMap<String, String> = match db::get(db::STORE_META, KEY_FILE_SHAS).await? {
        Some(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        None => HashMap::new(),
    };

    let wanted: HashSet<&str> = remote.iter().map(|f| f.path.as_str()).collect();
    let removed: Vec<String> = known.keys().filter(|p| !wanted.contains(p.as_str())).cloned().collect();
    let changed: Vec<&RemoteFile> = remote
        .iter()
        .filter(|f| known.get(&f.path) != Some(&f.blob_sha))
        .collect();
    let total = changed.len();
    let done = Rc::new(Cell::new(0usize));

    let fetches = changed.iter().map(|file| {
        let done = done.clone();
        async move {
            let content = github::fetch_data_file(sha, &file.path).await?;
            done.set(done.get() + 1);
            progress.set(format!("Downloading data ({}/{total}): {}", done.get(), file.path));
            Ok::<_, String>((file.path.clone(), content))
        }
    });
    let results: Vec<Result<(String, String), String>> =
        stream::iter(fetches).buffer_unordered(CONCURRENCY).collect().await;
    let downloaded = results.into_iter().collect::<Result<Vec<_>, _>>()?;

    progress.set("Saving to local cache...".to_string());
    db::write_many(db::STORE_FILES, downloaded, removed).await?;
    let shas: HashMap<&str, &str> = remote.iter().map(|f| (f.path.as_str(), f.blob_sha.as_str())).collect();
    let shas = serde_json::to_string(&shas).map_err(|e| e.to_string())?;
    db::put(db::STORE_META, KEY_FILE_SHAS, &shas).await?;
    db::put(db::STORE_META, KEY_VERSION, sha).await?;
    Ok(())
}
