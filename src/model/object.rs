use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::entry::Entry;

/// An interactable/animated object (`objects.json`). Shares a lot of shape
/// with [`super::bestiary::Monster`] (ac/hp/speed/abilities can each be a
/// plain number or a more detailed object) but is siginificantly simpler -
/// no legendary actions, saves, or skills.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Object {
    pub name: String,
    pub source: String,
    pub page: Option<u32>,
    #[serde(default, deserialize_with = "crate::model::serde_util::null_as_default")]
    pub size: Vec<String>,
    #[serde(rename = "objectType")]
    pub object_type: Option<String>,
    pub ac: Option<Value>,
    pub hp: Option<Value>,
    pub speed: Option<Value>,
    pub str: Option<i32>,
    pub dex: Option<i32>,
    pub con: Option<i32>,
    pub int: Option<i32>,
    pub wis: Option<i32>,
    pub cha: Option<i32>,
    #[serde(default, deserialize_with = "crate::model::serde_util::null_as_default")]
    pub senses: Vec<String>,
    pub immune: Option<Value>,
    #[serde(
        rename = "actionEntries",
        default,
        deserialize_with = "crate::model::serde_util::null_as_default"
    )]
    pub action_entries: Vec<Entry>,
    #[serde(default, deserialize_with = "crate::model::serde_util::null_as_default")]
    pub entries: Vec<Entry>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}
