use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::entry::Entry;

/// A minimal catch-all for every non-core kind (deities, vehicles, variant
/// rules, optional features, rewards, psionics, recipes, cults & boons,
/// languages, character creation options, bastion facilities, ...). These
/// don't get a dedicated statblock layout yet - just a
/// name/source header plus whatever `entries` prose the source data has.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Other {
    pub name: String,
    pub source: String,
    pub page: Option<u32>,
    #[serde(default, deserialize_with = "crate::model::serde_util::null_as_default")]
    pub entries: Vec<Entry>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}
