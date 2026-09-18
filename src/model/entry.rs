use serde::de::Deserializer;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// A node in 5etools' recursive "entries" content tree. Blocks we render
/// specially are typed structs (with their own `extra` catch-all for
/// forward-compatible fields); anything with an unrecognized `type` - or
/// that isn't an object/string/number/bool at all - is kept as raw `Value`
/// in `Other` so parsing never fails on novel or custom block types.
#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum Entry {
    String(String),
    Block(EntriesBlock),
    List(ListBlock),
    Table(TableBlock),
    Quote(QuoteBlock),
    Attack(AttackBlock),
    Other(Value),
}

/// Covers every entry type whose shape is `{ type, name?, entries, ...}`:
/// "entries", "section", "inset", "insetReadaloud", "item", "itemSub",
/// "options", "actions", "variant", and any future type sharing that shape.
/// `kind` holds the original `type` string so its layout can vary by block
/// kind. `item`-style blocks may carry a single `entry` instead of
/// `entries`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EntriesBlock {
    #[serde(rename = "type", default)]
    pub kind: String,
    pub name: Option<String>,
    #[serde(default, deserialize_with = "crate::model::serde_util::null_as_default")]
    pub entries: Vec<Entry>,
    pub entry: Option<Box<Entry>>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ListBlock {
    #[serde(rename = "type")]
    pub kind: String,
    pub style: Option<String>,
    pub columns: Option<u32>,
    #[serde(default, deserialize_with = "crate::model::serde_util::null_as_default")]
    pub items: Vec<Entry>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TableBlock {
    #[serde(rename = "type")]
    pub kind: String,
    pub caption: Option<String>,
    #[serde(
        rename = "colLabels",
        default,
        deserialize_with = "crate::model::serde_util::null_as_default"
    )]
    pub col_labels: Vec<Entry>,
    #[serde(
        rename = "colStyles",
        default,
        deserialize_with = "crate::model::serde_util::null_as_default"
    )]
    pub col_styles: Vec<String>,
    #[serde(default, deserialize_with = "crate::model::serde_util::null_as_default")]
    pub intro: Vec<Entry>,
    #[serde(default, deserialize_with = "crate::model::serde_util::null_as_default")]
    pub outro: Vec<Entry>,
    #[serde(default, deserialize_with = "crate::model::serde_util::null_as_default")]
    pub footnotes: Vec<Entry>,
    #[serde(default, deserialize_with = "crate::model::serde_util::null_as_default")]
    pub rows: Vec<TableRow>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A table row: either a plain array of cells or `{ type: "row", row: [...] }`.
/// A cell is an ordinary entry, or `{ type: "cell", roll, entry }` for a
/// dice-range label (see `render::render_cell`).
///
/// Serialized as the plain cell array (`transparent`) so a stored table -
/// the cached library snapshot - parses back into a typed [`TableBlock`].
#[derive(Clone, Debug, Serialize)]
#[serde(transparent)]
pub struct TableRow {
    pub cells: Vec<Entry>,
}

impl<'de> Deserialize<'de> for TableRow {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        let cells = match value {
            Value::Object(mut map) => map.remove("row"),
            other => Some(other),
        };
        let Some(Value::Array(cells)) = cells else {
            return Err(serde::de::Error::custom("table row is not an array"));
        };
        Ok(TableRow {
            cells: cells.into_iter().map(Entry::from_value).collect(),
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuoteBlock {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default, deserialize_with = "crate::model::serde_util::null_as_default")]
    pub entries: Vec<Entry>,
    pub by: Option<Value>,
    pub from: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A structured attack block (used by e.g. objects' `actionEntries`):
/// `{ type: "attack", attackEntries, hitEntries, ... }`, distinct from the
/// plain-string `{@atk}...{@h}...` convention monsters use.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AttackBlock {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(rename = "attackType")]
    pub attack_type: Option<String>,
    #[serde(
        rename = "attackEntries",
        default,
        deserialize_with = "crate::model::serde_util::null_as_default"
    )]
    pub attack_entries: Vec<Entry>,
    #[serde(
        rename = "hitEntries",
        default,
        deserialize_with = "crate::model::serde_util::null_as_default"
    )]
    pub hit_entries: Vec<Entry>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl<'de> Deserialize<'de> for Entry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        Ok(Entry::from_value(value))
    }
}

impl Entry {
    pub fn from_value(value: Value) -> Entry {
        match value {
            Value::String(s) => Entry::String(s),
            Value::Number(n) => Entry::String(n.to_string()),
            Value::Bool(b) => Entry::String(b.to_string()),
            Value::Object(_) => Entry::from_object(value),
            other => Entry::Other(other),
        }
    }

    fn from_object(value: Value) -> Entry {
        let kind = value.get("type").and_then(Value::as_str).unwrap_or("");

        /// Falls back to the raw value if the typed shape doesn't fit.
        fn typed<T: serde::de::DeserializeOwned>(value: Value, wrap: fn(T) -> Entry) -> Entry {
            match serde_json::from_value::<T>(value.clone()) {
                Ok(t) => wrap(t),
                Err(_) => Entry::Other(value),
            }
        }

        match kind {
            "entries" | "section" | "inset" | "insetReadaloud" | "item" | "itemSub" | "options"
            | "entriesOtherTile" | "actions" | "variant" | "variantInner" | "variantSub" | "optfeature" | "patron" => {
                typed(value, Entry::Block)
            }
            "list" => typed(value, Entry::List),
            "table" => typed(value, Entry::Table),
            "quote" => typed(value, Entry::Quote),
            "attack" => typed(value, Entry::Attack),
            // A missing `type` means a plain "entries" node (e.g. a class
            // feature's root object).
            "" if value.get("entries").is_some() => typed(value, Entry::Block),
            _ => Entry::Other(value),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The library snapshot stores entries by serializing them and reads
    /// them back through `Entry::from_value`; each typed variant must
    /// survive that trip rather than degrade to `Entry::Other`.
    #[test]
    fn typed_entries_survive_a_serialize_round_trip() {
        let samples = [
            json!({ "type": "entries", "name": "N", "entries": ["a"] }),
            json!({ "type": "list", "items": ["a"] }),
            json!({ "type": "table", "colLabels": ["d"], "rows": [["1"], { "type": "row", "row": ["2"] }] }),
            json!({ "type": "quote", "entries": ["a"], "by": "b" }),
            json!({ "type": "attack", "attackType": "MW", "attackEntries": ["a"], "hitEntries": ["b"] }),
        ];
        for sample in samples {
            let entry = Entry::from_value(sample.clone());
            assert!(!matches!(entry, Entry::Other(_)), "{sample} should parse typed");
            let again = Entry::from_value(serde_json::to_value(&entry).unwrap());
            assert_eq!(
                std::mem::discriminant(&entry),
                std::mem::discriminant(&again),
                "{sample} degraded after a round trip"
            );
        }
    }
}
