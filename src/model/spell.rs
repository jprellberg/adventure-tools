use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::entry::Entry;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Spell {
    pub name: String,
    pub source: String,
    pub page: Option<u32>,
    pub level: Option<u8>,
    pub school: Option<String>,
    pub time: Option<Value>,
    pub range: Option<Value>,
    pub components: Option<Value>,
    pub duration: Option<Value>,
    #[serde(default, deserialize_with = "crate::model::serde_util::null_as_default")]
    pub entries: Vec<Entry>,
    #[serde(
        default,
        rename = "entriesHigherLevel",
        deserialize_with = "crate::model::serde_util::null_as_default"
    )]
    pub entries_higher_level: Vec<Entry>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}
