use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::entry::Entry;

/// Backs `condition`, `disease`, and `status` entries from
/// conditionsdiseases.json - they share the same shape and are presented as
/// one "Conditions & Diseases" kind.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Condition {
    pub name: String,
    pub source: String,
    pub page: Option<u32>,
    #[serde(default, deserialize_with = "crate::model::serde_util::null_as_default")]
    pub entries: Vec<Entry>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}
