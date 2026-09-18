use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::entry::Entry;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Background {
    pub name: String,
    pub source: String,
    pub page: Option<u32>,
    #[serde(rename = "skillProficiencies")]
    pub skill_proficiencies: Option<Value>,
    #[serde(rename = "languageProficiencies")]
    pub language_proficiencies: Option<Value>,
    #[serde(default, deserialize_with = "crate::model::serde_util::null_as_default")]
    pub entries: Vec<Entry>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}
