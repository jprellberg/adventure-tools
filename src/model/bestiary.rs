use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::entry::Entry;

/// Fields with a simple, consistent shape are typed directly. A handful
/// (ac/speed/cr/alignment/type/initiative/immunities) genuinely vary in
/// shape across the corpus (e.g. a monster can have one AC or several from
/// different forms) even in 5etools' own renderer, so those stay `Value`
/// and get formatted by helpers at render time rather than forcing a rigid
/// enum on data that is legitimately polymorphic.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Monster {
    pub name: String,
    pub source: String,
    pub page: Option<u32>,
    #[serde(default, deserialize_with = "super::serde_util::null_as_default")]
    pub size: Vec<String>,
    #[serde(rename = "sizeNote")]
    pub size_note: Option<String>,
    #[serde(rename = "type")]
    pub creature_type: Option<Value>,
    pub alignment: Option<Value>,
    #[serde(rename = "alignmentPrefix")]
    pub alignment_prefix: Option<String>,
    /// Sidekick level, e.g. "1st-level".
    pub level: Option<u32>,
    pub ac: Option<Value>,
    pub hp: Option<Hp>,
    pub speed: Option<Value>,
    pub initiative: Option<Value>,
    pub str: Option<AbilityScore>,
    pub dex: Option<AbilityScore>,
    pub con: Option<AbilityScore>,
    pub int: Option<AbilityScore>,
    pub wis: Option<AbilityScore>,
    pub cha: Option<AbilityScore>,
    pub save: Option<Map<String, Value>>,
    pub skill: Option<Map<String, Value>>,
    pub vulnerable: Option<Value>,
    pub resist: Option<Value>,
    pub immune: Option<Value>,
    #[serde(rename = "conditionImmune")]
    pub condition_immune: Option<Value>,
    #[serde(default, deserialize_with = "super::serde_util::null_as_default")]
    pub gear: Vec<Value>,
    #[serde(
        rename = "attachedItems",
        default,
        deserialize_with = "super::serde_util::null_as_default"
    )]
    pub attached_items: Vec<String>,
    #[serde(default, deserialize_with = "super::serde_util::null_as_default")]
    pub senses: Vec<String>,
    pub passive: Option<Value>,
    #[serde(default, deserialize_with = "super::serde_util::null_as_default")]
    pub languages: Vec<String>,
    pub cr: Option<Value>,
    #[serde(rename = "pbNote")]
    pub pb_note: Option<String>,
    #[serde(default, deserialize_with = "super::serde_util::null_as_default")]
    pub spellcasting: Vec<Value>,
    #[serde(default, rename = "trait", deserialize_with = "super::serde_util::null_as_default")]
    pub traits: Vec<NamedEntries>,
    #[serde(default, deserialize_with = "super::serde_util::null_as_default")]
    pub action: Vec<NamedEntries>,
    #[serde(default, deserialize_with = "super::serde_util::null_as_default")]
    pub bonus: Vec<NamedEntries>,
    #[serde(default, deserialize_with = "super::serde_util::null_as_default")]
    pub reaction: Vec<NamedEntries>,
    #[serde(default, deserialize_with = "super::serde_util::null_as_default")]
    pub legendary: Vec<NamedEntries>,
    #[serde(default, deserialize_with = "super::serde_util::null_as_default")]
    pub mythic: Vec<NamedEntries>,
    #[serde(rename = "legendaryActions")]
    pub legendary_actions: Option<u32>,
    #[serde(rename = "legendaryActionsLair")]
    pub legendary_actions_lair: Option<u32>,
    #[serde(rename = "legendaryGroup")]
    pub legendary_group: Option<SourceRef>,
    #[serde(default, deserialize_with = "super::serde_util::null_as_default")]
    pub variant: Vec<Entry>,
    #[serde(default, deserialize_with = "super::serde_util::null_as_default")]
    pub footer: Vec<Entry>,
    #[serde(default, deserialize_with = "super::serde_util::null_as_default")]
    pub environment: Vec<String>,
    #[serde(default, deserialize_with = "super::serde_util::null_as_default")]
    pub treasure: Vec<String>,
    #[serde(rename = "summonedBySpell")]
    pub summoned_by_spell: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// An ability score: a plain number, or `{ special: "..." }` for creatures
/// whose score is described in prose (e.g. "Behemoth").
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AbilityScore {
    Score(i32),
    Special { special: String },
}

/// A `{ name, source }` pointer to another entity.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceRef {
    pub name: String,
    pub source: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Hp {
    pub average: Option<i64>,
    pub formula: Option<String>,
    pub special: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NamedEntries {
    pub name: Option<String>,
    #[serde(default, deserialize_with = "crate::model::serde_util::null_as_default")]
    pub entries: Vec<Entry>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}
