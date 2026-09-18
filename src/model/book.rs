use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// One row from `books.json` or `adventures.json` - used only to resolve a
/// source code to its full title, for the source tag's tooltip
/// (`Library::source_full_name`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BookMeta {
    pub name: String,
    pub source: String,
    pub id: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}
