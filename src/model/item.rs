use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::entry::Entry;

/// Backs `items.json`'s `item` and `itemGroup` arrays (magic items, gear),
/// `items-base.json`'s `baseitem` array (mundane equipment) and
/// `magicvariants.json` - they share enough shape (name/source/rarity/
/// entries) to use one struct for all of them. Every other field of the
/// source data (bonuses, charges, vehicle stats, ...) is read from `extra`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Item {
    pub name: String,
    pub source: String,
    pub page: Option<u32>,
    #[serde(rename = "type")]
    pub item_type: Option<String>,
    pub rarity: Option<String>,
    #[serde(rename = "reqAttune")]
    pub req_attune: Option<Value>,
    pub weight: Option<f64>,
    pub value: Option<f64>,
    #[serde(rename = "weaponCategory")]
    pub weapon_category: Option<String>,
    pub dmg1: Option<String>,
    pub dmg2: Option<String>,
    #[serde(rename = "dmgType")]
    pub dmg_type: Option<String>,
    pub range: Option<String>,
    #[serde(default, deserialize_with = "crate::model::serde_util::null_as_default")]
    pub property: Vec<Value>,
    #[serde(default, deserialize_with = "crate::model::serde_util::null_as_default")]
    pub mastery: Vec<String>,
    pub ac: Option<i64>,
    #[serde(default, deserialize_with = "crate::model::serde_util::null_as_default")]
    pub entries: Vec<Entry>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}
