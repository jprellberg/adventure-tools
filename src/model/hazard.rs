use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::entry::Entry;

/// Backs both `trap` and `hazard` entries from trapshazards.json - they
/// share the same shape and are presented as one "Hazards" kind.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Hazard {
    pub name: String,
    pub source: String,
    pub page: Option<u32>,
    #[serde(rename = "trapHazType")]
    pub trap_haz_type: Option<String>,
    #[serde(default, deserialize_with = "crate::model::serde_util::null_as_default")]
    pub rating: Vec<Value>,
    #[serde(default, deserialize_with = "crate::model::serde_util::null_as_default")]
    pub entries: Vec<Entry>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}
