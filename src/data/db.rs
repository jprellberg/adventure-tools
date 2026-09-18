//! Thin async key-value wrapper around IndexedDB (via `rexie`), where all
//! persistent data lives. Values are JSON strings under a
//! plain out-of-line string key (callers own their `serde_json`
//! (de)serialization), except the library snapshot, which is raw bytes. This
//! module only knows about the object stores that exist, not what's inside
//! them.

use js_sys::Uint8Array;
use rexie::{ObjectStore, Rexie, TransactionMode};
use wasm_bindgen::JsValue;

const DB_NAME: &str = "adventure-tools";
const DB_VERSION: u32 = 3;

/// Raw fetched `data/` files from the 5etools-src sync (see
/// `super::github`/`super::sync`), keyed by their path relative to `data/`.
pub const STORE_FILES: &str = "files";
/// Small sync bookkeeping: the last-synced commit sha under `"version"`, the
/// per-file blob shas under `"file_shas"`, and the key of the cached library
/// under `"snapshot_key"`.
pub const STORE_META: &str = "meta";
/// The serialized parsed library (see `super::snapshot`), under key `"library"`.
pub const STORE_SNAPSHOT: &str = "snapshot";

/// Opens (creating/upgrading as needed) the app's single IndexedDB database.
/// Called fresh per operation rather than cached: `indexedDB.open` is cheap
/// and this keeps every call here self-contained, with no shared state to
/// manage across the wasm module. Every caller must `.close()` the returned
/// handle once done with it - `Rexie` doesn't close on drop, and a leaked
/// open connection blocks any future version upgrade from ever completing.
async fn open() -> Result<Rexie, String> {
    Rexie::builder(DB_NAME)
        .version(DB_VERSION)
        .add_object_store(ObjectStore::new(STORE_FILES))
        .add_object_store(ObjectStore::new(STORE_META))
        .add_object_store(ObjectStore::new(STORE_SNAPSHOT))
        .build()
        .await
        .map_err(|e| format!("opening IndexedDB: {e}"))
}

async fn get_raw(store: &str, key: &str) -> Result<Option<JsValue>, String> {
    let db = open().await?;
    let tx = db
        .transaction(&[store], TransactionMode::ReadOnly)
        .map_err(|e| e.to_string())?;
    let s = tx.store(store).map_err(|e| e.to_string())?;
    let value = s.get(JsValue::from_str(key)).await.map_err(|e| e.to_string())?;
    tx.done().await.map_err(|e| e.to_string())?;
    db.close();
    Ok(value)
}

async fn put_raw(store: &str, key: &str, value: &JsValue) -> Result<(), String> {
    let db = open().await?;
    let tx = db
        .transaction(&[store], TransactionMode::ReadWrite)
        .map_err(|e| e.to_string())?;
    let s = tx.store(store).map_err(|e| e.to_string())?;
    s.put(value, Some(&JsValue::from_str(key)))
        .await
        .map_err(|e| e.to_string())?;
    tx.done().await.map_err(|e| e.to_string())?;
    db.close();
    Ok(())
}

pub async fn get(store: &str, key: &str) -> Result<Option<String>, String> {
    Ok(get_raw(store, key).await?.and_then(|v| v.as_string()))
}

pub async fn put(store: &str, key: &str, value: &str) -> Result<(), String> {
    put_raw(store, key, &JsValue::from_str(value)).await
}

pub async fn get_bytes(store: &str, key: &str) -> Result<Option<Vec<u8>>, String> {
    Ok(get_raw(store, key).await?.map(|v| Uint8Array::new(&v).to_vec()))
}

pub async fn put_bytes(store: &str, key: &str, value: &[u8]) -> Result<(), String> {
    put_raw(store, key, &Uint8Array::from(value)).await
}

/// Every string entry of `store`, read in one transaction.
pub async fn get_all(store: &str) -> Result<Vec<(String, String)>, String> {
    let db = open().await?;
    let tx = db
        .transaction(&[store], TransactionMode::ReadOnly)
        .map_err(|e| e.to_string())?;
    let s = tx.store(store).map_err(|e| e.to_string())?;
    let pairs = s.scan(None, None, None, None).await.map_err(|e| e.to_string())?;
    tx.done().await.map_err(|e| e.to_string())?;
    db.close();
    Ok(pairs
        .into_iter()
        .filter_map(|(k, v)| Some((k.as_string()?, v.as_string()?)))
        .collect())
}

/// Writes and deletes entries of `store` in one transaction - used by the
/// sync to apply a `data/` update atomically.
pub async fn write_many(store: &str, put: Vec<(String, String)>, delete: Vec<String>) -> Result<(), String> {
    let db = open().await?;
    let tx = db
        .transaction(&[store], TransactionMode::ReadWrite)
        .map_err(|e| e.to_string())?;
    let s = tx.store(store).map_err(|e| e.to_string())?;
    for (key, value) in &put {
        s.put(&JsValue::from_str(value), Some(&JsValue::from_str(key)))
            .await
            .map_err(|e| e.to_string())?;
    }
    for key in delete {
        s.delete(JsValue::from_str(&key)).await.map_err(|e| e.to_string())?;
    }
    tx.done().await.map_err(|e| e.to_string())?;
    db.close();
    Ok(())
}

pub async fn count(store: &str) -> Result<usize, String> {
    let db = open().await?;
    let tx = db
        .transaction(&[store], TransactionMode::ReadOnly)
        .map_err(|e| e.to_string())?;
    let s = tx.store(store).map_err(|e| e.to_string())?;
    let keys = s.get_all_keys(None, None).await.map_err(|e| e.to_string())?;
    tx.done().await.map_err(|e| e.to_string())?;
    db.close();
    Ok(keys.len())
}
