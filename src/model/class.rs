use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::entry::Entry;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Class {
    pub name: String,
    pub source: String,
    pub page: Option<u32>,
    pub hd: Option<Value>,
    #[serde(rename = "startingProficiencies")]
    pub starting_proficiencies: Option<Value>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A single class or subclass feature (the `classFeature` and
/// `subclassFeature` arrays of the class files), kept separate from
/// [`Class`] since features are looked up by (class name, class source,
/// level) rather than nested under the class itself. Subclass features
/// additionally name their subclass.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClassFeature {
    pub name: String,
    pub source: String,
    #[serde(rename = "className")]
    pub class_name: String,
    #[serde(rename = "classSource")]
    pub class_source: String,
    pub level: u8,
    #[serde(rename = "subclassShortName")]
    pub subclass_short_name: Option<String>,
    #[serde(rename = "subclassSource")]
    pub subclass_source: Option<String>,
    #[serde(default, deserialize_with = "crate::model::serde_util::null_as_default")]
    pub entries: Vec<Entry>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}
