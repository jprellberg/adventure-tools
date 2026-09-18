use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::entry::Entry;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Feat {
    pub name: String,
    pub source: String,
    pub page: Option<u32>,
    pub prerequisite: Option<Value>,
    pub ability: Option<Value>,
    #[serde(default, deserialize_with = "crate::model::serde_util::null_as_default")]
    pub entries: Vec<Entry>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}
