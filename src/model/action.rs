use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::entry::Entry;

/// A combat action (`actions.json`, e.g. "Dash", "Ready") - little more than
/// a name, an optional time cost, and its rules text.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Action {
    pub name: String,
    pub source: String,
    pub page: Option<u32>,
    pub time: Option<Value>,
    #[serde(default, deserialize_with = "crate::model::serde_util::null_as_default")]
    pub entries: Vec<Entry>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}
