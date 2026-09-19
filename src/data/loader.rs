//! Parses the cached `data/` tree (populated by `super::sync`) into typed
//! in-memory collections. Every source present in the corpus is loaded, not
//! a curated subset - this is a reference library, so completeness matters
//! more than trimming it down.

use std::collections::{BTreeMap, HashMap, HashSet};

use dioxus::prelude::{Signal, WritableExt};
use serde::de::DeserializeOwned;
use serde_json::{json, Map, Value};

use crate::model::copy::{class_feature_key, resolve_copies, resolve_copies_by, subclass_feature_key, subclass_key};
use crate::model::{
    Action, Background, BookMeta, Class, ClassFeature, Condition, Feat, Hazard, Item, Monster, Object, Other, Race,
    Spell,
};
use crate::routes::EntityKind;
use crate::search::SearchIndex;

use super::books;
use super::props::{apply_all_properties, apply_own_properties, expand_item_entries};
use super::{db, ItemType, Library, SpellLookup};

/// Where the parser reads `data/`-relative JSON files from: the IndexedDB
/// cache in the app, the checked-out corpus on disk in the tests.
#[allow(async_fn_in_trait)]
pub trait DataSource {
    async fn value(&self, path: &str) -> Result<Value, String>;
}

/// The IndexedDB cache that `super::sync::sync_files` populates.
struct CacheSource {
    files: HashMap<String, String>,
}

impl CacheSource {
    /// Reads every cached file in one transaction; opening the database and
    /// a transaction per file would dominate the parse otherwise.
    async fn open() -> Result<Self, String> {
        Ok(CacheSource {
            files: db::get_all(db::STORE_FILES).await?.into_iter().collect(),
        })
    }
}

impl DataSource for CacheSource {
    async fn value(&self, path: &str) -> Result<Value, String> {
        let raw = self
            .files
            .get(path)
            .ok_or_else(|| format!("{path} missing from the local data cache"))?;
        serde_json::from_str(raw).map_err(|e| format!("parsing {path}: {e}"))
    }
}

/// Takes the array at `file[key]` out of the file, or empty if the file or
/// key is missing. Moving it out avoids copying every entity.
pub(super) fn take_array(file: &mut Value, key: &str) -> Vec<Value> {
    match file.get_mut(key).map(Value::take) {
        Some(Value::Array(items)) => items,
        _ => Vec::new(),
    }
}

/// Deserializes every resolved entity into `T`, skipping (rather than
/// failing the whole kind on) any single entry a `_copy`/`_mod` merge left
/// in a shape that doesn't fit the typed struct.
fn from_values<T: DeserializeOwned>(values: Vec<Value>) -> Vec<T> {
    values
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect()
}

/// `bestiary/index.json`, `spells/index.json`, `class/index.json` all share
/// this shape: a flat map from source/class code to the relative filename
/// holding that source's content. Loading through it (rather than a
/// hardcoded source list) means every source the corpus ships gets picked
/// up automatically.
async fn fetch_index(src: &impl DataSource, path: &str) -> Result<BTreeMap<String, String>, String> {
    let value = src.value(path).await?;
    serde_json::from_value(value).map_err(|e| format!("parsing {path}: {e}"))
}

/// Every array named in `keys`, concatenated across all of `dir`'s
/// index-listed files.
async fn indexed_arrays(src: &impl DataSource, dir: &str, keys: &[&str]) -> Result<Vec<Vec<Value>>, String> {
    let index = fetch_index(src, &format!("{dir}/index.json")).await?;
    let mut out = vec![Vec::new(); keys.len()];
    for filename in index.values() {
        let mut file = src.value(&format!("{dir}/{filename}")).await?;
        for (bucket, key) in out.iter_mut().zip(keys) {
            bucket.extend(take_array(&mut file, key));
        }
    }
    Ok(out)
}

async fn load_monsters(src: &impl DataSource) -> Result<Vec<Monster>, String> {
    let raw = indexed_arrays(src, "bestiary", &["monster"]).await?.remove(0);
    Ok(from_values(resolve_copies(raw)))
}

async fn load_spells(src: &impl DataSource) -> Result<Vec<Spell>, String> {
    let raw = indexed_arrays(src, "spells", &["spell"]).await?.remove(0);
    Ok(from_values(resolve_copies(raw)))
}

/// Magic-item variants (`magicvariants.json`) define an item template plus
/// the base items it applies to; their `inherits` block holds the fields a
/// concrete item would carry (source, rarity, entries, ...). Lifted to the
/// top level so a variant reads like any other item, with `{=field}`
/// placeholders in its text filled from the merged fields.
fn variant_as_item(mut variant: Map<String, Value>) -> Value {
    if let Some(Value::Object(inherits)) = variant.remove("inherits") {
        for (key, value) in inherits {
            if !matches!(key.as_str(), "namePrefix" | "nameSuffix" | "nameRemove") {
                variant.entry(key).or_insert(value);
            }
        }
    }
    let fields = variant.clone();
    let mut item = Value::Object(variant);
    apply_all_properties(&mut item, &fields);
    item
}

async fn load_items(src: &impl DataSource) -> Result<(Vec<Item>, Vec<ItemType>), String> {
    let mut items = src.value("items.json").await?;
    let mut base = src.value("items-base.json").await?;
    let mut variants = src.value("magicvariants.json").await?;
    let templates = take_array(&mut base, "itemEntry");
    let text = |v: &Value, key: &str| v.get(key).and_then(Value::as_str).map(str::to_string);
    let types = take_array(&mut base, "itemType")
        .iter()
        .filter_map(|t| {
            Some(ItemType {
                abbreviation: text(t, "abbreviation")?,
                source: text(t, "source")?,
                name: text(t, "name")?,
            })
        })
        .collect();
    let mut raw = take_array(&mut items, "item");
    raw.extend(take_array(&mut items, "itemGroup"));
    raw.extend(take_array(&mut base, "baseitem"));
    let mut all = resolve_copies(raw);
    for item in all.iter_mut().filter_map(Value::as_object_mut) {
        expand_item_entries(item, &templates);
    }
    all.extend(
        take_array(&mut variants, "magicvariant")
            .into_iter()
            .filter_map(|v| v.as_object().cloned())
            .map(variant_as_item),
    );
    Ok((from_values(all), types))
}

/// Classes and their features, plus subclasses and their features. A
/// subclass is unique only together with its class (the same subclass is
/// reprinted under several class sources), so the copy keys and the
/// de-duplication below include it.
async fn load_classes(
    src: &impl DataSource,
) -> Result<(Vec<Class>, Vec<ClassFeature>, Vec<Other>, Vec<ClassFeature>), String> {
    let [classes, class_features, subclasses, subclass_features] = <[Vec<Value>; 4]>::try_from(
        indexed_arrays(src, "class", &["class", "classFeature", "subclass", "subclassFeature"]).await?,
    )
    .map_err(|_| "class index shape".to_string())?;
    let subclasses: Vec<Other> = from_values(resolve_copies_by(subclasses, subclass_key));
    Ok((
        from_values(resolve_copies(classes)),
        from_values(resolve_copies_by(class_features, class_feature_key)),
        subclasses,
        from_values(resolve_copies_by(subclass_features, subclass_feature_key)),
    ))
}

/// A subrace has no `name` of its own worth searching by; like 5etools it is
/// presented as "Race (Subrace)". Subraces without a name are abstract
/// templates for color variants and aren't shown.
fn subrace_as_race(mut subrace: Map<String, Value>) -> Option<Value> {
    let race = subrace.get("raceName")?.as_str()?.to_string();
    let name = subrace.get("name")?.as_str()?;
    let full = format!("{race} ({name})");
    subrace.insert("name".into(), Value::String(full));
    Some(Value::Object(subrace))
}

fn subrace_key(v: &Value) -> Option<String> {
    crate::model::copy::key_with_fields(v, ["raceName", "raceSource"])
}

async fn load_races(src: &impl DataSource) -> Result<Vec<Race>, String> {
    let mut file = src.value("races.json").await?;
    let mut all = resolve_copies(take_array(&mut file, "race"));
    let subraces = resolve_copies_by(take_array(&mut file, "subrace"), subrace_key);
    all.extend(
        subraces
            .into_iter()
            .filter_map(|v| v.as_object().cloned())
            .filter_map(subrace_as_race),
    );
    Ok(from_values(all))
}

async fn load_backgrounds(src: &impl DataSource) -> Result<Vec<Background>, String> {
    let mut file = src.value("backgrounds.json").await?;
    Ok(from_values(resolve_copies(take_array(&mut file, "background"))))
}

async fn load_feats(src: &impl DataSource) -> Result<Vec<Feat>, String> {
    let mut file = src.value("feats.json").await?;
    Ok(from_values(resolve_copies(take_array(&mut file, "feat"))))
}

async fn load_conditions(src: &impl DataSource) -> Result<Vec<Condition>, String> {
    let mut file = src.value("conditionsdiseases.json").await?;
    let mut raw = take_array(&mut file, "condition");
    raw.extend(take_array(&mut file, "disease"));
    raw.extend(take_array(&mut file, "status"));
    Ok(from_values(resolve_copies(raw)))
}

async fn load_objects(src: &impl DataSource) -> Result<Vec<Object>, String> {
    let mut file = src.value("objects.json").await?;
    Ok(from_values(resolve_copies(take_array(&mut file, "object"))))
}

async fn load_actions(src: &impl DataSource) -> Result<Vec<Action>, String> {
    let mut file = src.value("actions.json").await?;
    Ok(from_values(resolve_copies(take_array(&mut file, "action"))))
}

async fn load_hazards(src: &impl DataSource) -> Result<Vec<Hazard>, String> {
    let mut file = src.value("trapshazards.json").await?;
    let mut raw = take_array(&mut file, "trap");
    raw.extend(take_array(&mut file, "hazard"));
    Ok(from_values(resolve_copies(raw)))
}

/// Where each kind that shares the minimal (name, source, page, entries)
/// shape lives: its data file and the arrays within it. Bestiary-adjacent
/// and item-adjacent kinds (legendary groups, properties, masteries) are
/// listed here too - they're entities of their own, not just lookup data.
const OTHER_SOURCES: &[(EntityKind, &str, &[&str])] = &[
    (EntityKind::Deities, "deities.json", &["deity"]),
    (EntityKind::Vehicles, "vehicles.json", &["vehicle", "vehicleUpgrade"]),
    (EntityKind::VariantRules, "variantrules.json", &["variantrule"]),
    (
        EntityKind::OptionalFeatures,
        "optionalfeatures.json",
        &["optionalfeature"],
    ),
    (EntityKind::Rewards, "rewards.json", &["reward"]),
    (EntityKind::Psionics, "psionics.json", &["psionic"]),
    (EntityKind::Recipes, "recipes.json", &["recipe"]),
    (EntityKind::CultsBoons, "cultsboons.json", &["cult", "boon"]),
    (EntityKind::Languages, "languages.json", &["language"]),
    (EntityKind::CharOptions, "charcreationoptions.json", &["charoption"]),
    (EntityKind::Facilities, "bastions.json", &["facility"]),
    (EntityKind::ItemProperties, "items-base.json", &["itemProperty"]),
    (EntityKind::ItemMasteries, "items-base.json", &["itemMastery"]),
    (EntityKind::Skills, "skills.json", &["skill"]),
    (EntityKind::Senses, "senses.json", &["sense"]),
    (EntityKind::Tables, "tables.json", &["table"]),
    (EntityKind::Decks, "decks.json", &["deck"]),
    (EntityKind::Cards, "decks.json", &["card"]),
    (EntityKind::Encounters, "encounters.json", &["encounter"]),
    (
        EntityKind::LegendaryGroups,
        "bestiary/legendarygroups.json",
        &["legendaryGroup"],
    ),
    (EntityKind::CrochetPatterns, "homecrafts.json", &["crochetPattern"]),
    (EntityKind::Books, "books.json", &["book"]),
];

/// Reshapes the few kinds whose source data isn't already name/source/
/// entries: a standalone table *is* the table block; an item property is
/// named by the heading of its only entry.
fn normalize(kind: EntityKind, mut entity: Map<String, Value>) -> Map<String, Value> {
    match kind {
        EntityKind::Tables => {
            let mut table = entity.clone();
            for key in ["source", "page", "otherSources", "srd", "basicRules"] {
                table.remove(key);
            }
            table.insert("type".into(), json!("table"));
            entity.retain(|k, _| {
                matches!(
                    k.as_str(),
                    "name" | "source" | "page" | "otherSources" | "srd" | "basicRules"
                )
            });
            entity.insert("entries".into(), Value::Array(vec![Value::Object(table)]));
        }
        EntityKind::ItemProperties => {
            let inner = entity
                .get("entries")
                .and_then(Value::as_array)
                .and_then(|e| e.first())
                .cloned();
            if let Some(Value::Object(mut first)) = inner {
                if let Some(name) = first.get("name").cloned() {
                    entity.entry("name").or_insert(name);
                }
                if let Some(entries) = first.remove("entries") {
                    entity.insert("entries".into(), entries);
                }
            }
        }
        EntityKind::Recipes => {
            let mut recipe = Value::Object(entity);
            apply_own_properties(&mut recipe, &Map::new());
            entity = recipe.as_object().cloned().unwrap_or_default();
        }
        _ => {}
    }
    entity
}

/// The field that tells apart same-named entities of a kind within one
/// source (a card in two sets, a deity in two pantheons).
fn disambiguator(kind: EntityKind) -> Option<&'static str> {
    match kind {
        EntityKind::Cards => Some("set"),
        EntityKind::Rules | EntityKind::Tables => Some("path"),
        EntityKind::Deities => Some("pantheon"),
        _ => None,
    }
}

/// Entities are looked up and keyed by name and source, so a repeat of the
/// same pair is either renamed with its [`disambiguator`] or dropped.
fn dedupe(kind: EntityKind, items: Vec<Other>) -> Vec<Other> {
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(items.len());
    for mut item in items {
        let key = |i: &Other| (i.name.to_lowercase(), i.source.to_lowercase());
        if !seen.insert(key(&item)) {
            let Some(Value::String(extra)) = disambiguator(kind).and_then(|f| item.extra.get(f)).cloned() else {
                continue;
            };
            item.name = format!("{} ({extra})", item.name);
            if !seen.insert(key(&item)) {
                continue;
            }
        }
        out.push(item);
    }
    out
}

async fn load_other(src: &impl DataSource, subclasses: Vec<Other>) -> Result<HashMap<EntityKind, Vec<Other>>, String> {
    let mut by_kind: HashMap<EntityKind, Vec<Other>> = HashMap::new();
    for (kind, path, keys) in OTHER_SOURCES {
        let mut file = src.value(path).await?;
        let mut raw = Vec::new();
        for key in *keys {
            raw.extend(take_array(&mut file, key));
        }
        let raw = resolve_copies(raw)
            .into_iter()
            .filter_map(|v| v.as_object().cloned())
            .map(|m| Value::Object(normalize(*kind, m)))
            .collect();
        by_kind.entry(*kind).or_default().extend(from_values::<Other>(raw));
    }
    by_kind.entry(EntityKind::Subclasses).or_default().extend(subclasses);
    let (rules, tables) = load_book_content(src).await?;
    by_kind.entry(EntityKind::Rules).or_default().extend(rules);
    // Standalone tables are the curated copies of tables that also sit in books.
    let known: HashSet<(String, String)> = by_kind
        .get(&EntityKind::Tables)
        .into_iter()
        .flatten()
        .map(|t| (t.name.to_lowercase(), t.source.to_lowercase()))
        .collect();
    by_kind.entry(EntityKind::Tables).or_default().extend(
        tables
            .into_iter()
            .filter(|t| !known.contains(&(t.name.to_lowercase(), t.source.to_lowercase()))),
    );
    for (kind, items) in by_kind.iter_mut() {
        *items = dedupe(*kind, std::mem::take(items));
    }
    Ok(by_kind)
}

/// The rules and captioned tables inside the text of every book, and the
/// tables of every adventure (see [`books::extract`]).
async fn load_book_content(src: &impl DataSource) -> Result<(Vec<Other>, Vec<Other>), String> {
    let (mut rules, mut tables) = (Vec::new(), Vec::new());
    for (index, dir, with_rules) in [("books.json", "book", true), ("adventures.json", "adventure", false)] {
        let mut listing = src.value(index).await?;
        for meta in take_array(&mut listing, if with_rules { "book" } else { "adventure" }) {
            let (Some(id), Some(source), Some(name)) = (
                meta.get("id").and_then(Value::as_str),
                meta.get("source").and_then(Value::as_str),
                meta.get("name").and_then(Value::as_str),
            ) else {
                continue;
            };
            let mut file = src.value(&format!("{dir}/{dir}-{}.json", id.to_lowercase())).await?;
            let content = books::extract(source, name, &take_array(&mut file, "data"), with_rules);
            rules.extend(from_values::<Other>(content.rules));
            tables.extend(from_values::<Other>(content.tables));
        }
    }
    Ok((rules, tables))
}

/// Metadata for every bundled book/adventure - not the Books section itself
/// (not entities), just used to resolve a source
/// code to its full title for the source tag's tooltip (`Library::source_full_name`).
async fn load_books(src: &impl DataSource) -> Result<Vec<BookMeta>, String> {
    let mut books = src.value("books.json").await?;
    let mut adventures = src.value("adventures.json").await?;
    let mut raw = take_array(&mut books, "book");
    raw.extend(take_array(&mut adventures, "adventure"));
    Ok(from_values(raw))
}

async fn load_spell_lookup(src: &impl DataSource) -> Result<SpellLookup, String> {
    let value = src.value("generated/gendata-spell-source-lookup.json").await?;
    serde_json::from_value(value).map_err(|e| format!("parsing spell source lookup: {e}"))
}

/// Parses every kind out of the local cache into a [`Library`], reporting
/// progress. Assumes the cache is already populated (see
/// `super::sync::sync_files`).
pub async fn parse_library(mut progress: Signal<String>) -> Result<Library, String> {
    let source = CacheSource::open().await?;
    parse_library_from(&source, |message| progress.set(message.to_string())).await
}

/// [`parse_library`] over any [`DataSource`].
pub async fn parse_library_from(src: &impl DataSource, mut progress: impl FnMut(&str)) -> Result<Library, String> {
    progress("Parsing classes...");
    let (classes, class_features, subclasses, subclass_features) = load_classes(src).await?;
    progress("Parsing monsters...");
    let monsters = load_monsters(src).await?;
    progress("Parsing spells...");
    let spells = load_spells(src).await?;
    let spell_lookup = load_spell_lookup(src).await?;
    progress("Parsing items...");
    let (items, item_types) = load_items(src).await?;
    progress("Parsing races...");
    let races = load_races(src).await?;
    progress("Parsing backgrounds...");
    let backgrounds = load_backgrounds(src).await?;
    progress("Parsing feats...");
    let feats = load_feats(src).await?;
    progress("Parsing conditions...");
    let conditions = load_conditions(src).await?;
    progress("Parsing objects...");
    let objects = load_objects(src).await?;
    progress("Parsing actions...");
    let actions = load_actions(src).await?;
    progress("Parsing hazards...");
    let hazards = load_hazards(src).await?;
    progress("Parsing remaining reference data...");
    let other = load_other(src, subclasses).await?;
    progress("Parsing books...");
    let books = load_books(src).await?;

    Ok(Library {
        monsters,
        spells,
        items,
        item_types,
        classes,
        class_features,
        subclass_features,
        races,
        backgrounds,
        feats,
        conditions,
        objects,
        actions,
        hazards,
        other,
        spell_lookup,
        books,
        search_index: SearchIndex::default(),
        pin_index: HashMap::new(),
    }
    .indexed())
}
