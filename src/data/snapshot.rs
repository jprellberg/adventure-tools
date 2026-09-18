//! The prepared library as a cached snapshot: parsing and resolving the
//! downloaded files into a [`Library`] is the expensive part of startup, so
//! its result is serialized once (MessagePack, which - unlike bincode or
//! postcard - handles the self-describing `serde_json::Value`s the model
//! keeps) and every later start just deserializes it.
//!
//! A snapshot is valid for one data commit *and* one build of the code: the
//! key combines the synced commit sha with a fingerprint of the sources
//! (`build.rs`), so changing either discards it and the library is
//! prepared again from the cached files.

use super::{db, Library};

const KEY_LIBRARY: &str = "library";
const KEY_SNAPSHOT: &str = "snapshot_key";

fn snapshot_key(data_sha: &str) -> String {
    format!("{}:{data_sha}", env!("SOURCE_FINGERPRINT"))
}

pub fn encode(library: &Library) -> Result<Vec<u8>, String> {
    rmp_serde::to_vec_named(library).map_err(|e| format!("serializing the library: {e}"))
}

pub fn decode(bytes: &[u8]) -> Result<Library, String> {
    let library: Library = rmp_serde::from_slice(bytes).map_err(|e| format!("reading the cached library: {e}"))?;
    Ok(library.indexed())
}

/// The cached library for `data_sha`, if one was saved for this build.
pub async fn load(data_sha: &str) -> Result<Option<Library>, String> {
    if db::get(db::STORE_META, KEY_SNAPSHOT).await?.as_deref() != Some(&snapshot_key(data_sha)) {
        return Ok(None);
    }
    let Some(bytes) = db::get_bytes(db::STORE_SNAPSHOT, KEY_LIBRARY).await? else {
        return Ok(None);
    };
    // An unreadable snapshot is just a cache miss.
    Ok(decode(&bytes).ok())
}

pub async fn save(library: &Library, data_sha: &str) -> Result<(), String> {
    // Invalidate first so a failure midway can't leave a key that vouches
    // for bytes that were never written.
    db::put(db::STORE_META, KEY_SNAPSHOT, "").await?;
    db::put_bytes(db::STORE_SNAPSHOT, KEY_LIBRARY, &encode(library)?).await?;
    db::put(db::STORE_META, KEY_SNAPSHOT, &snapshot_key(data_sha)).await
}
