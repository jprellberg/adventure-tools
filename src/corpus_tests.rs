//! Native tests over the full 5etools-src corpus (checked out as a sibling
//! directory): the real loader runs against the files on
//! disk (no gloo-net, which needs a browser). They catch schema drift with a
//! normal native stack/backtrace - a real deserialization bug surfaces on
//! wasm as an opaque "memory access out of bounds" trap - and check that
//! every field of every entity is either shown by its statblock or
//! deliberately not. The app itself fetches its data live from GitHub at
//! runtime (see `src/data/sync.rs`).
#![cfg(test)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::sync::OnceLock;

use serde_json::Value;

use crate::data::loader::{parse_library_from, DataSource};
use crate::data::{EntityRef, Library};
use crate::model::copy::resolve_copies;
use crate::routes::EntityKind;

struct DiskSource;

impl DataSource for DiskSource {
    async fn value(&self, path: &str) -> Result<Value, String> {
        let full = format!("{}/../5etools-src/data/{path}", env!("CARGO_MANIFEST_DIR"));
        let text = fs::read_to_string(&full).map_err(|e| format!("read {full}: {e}"))?;
        serde_json::from_str(&text).map_err(|e| format!("parse {full}: {e}"))
    }
}

fn library() -> &'static Library {
    static LIBRARY: OnceLock<Library> = OnceLock::new();
    LIBRARY.get_or_init(|| {
        futures::executor::block_on(parse_library_from(&DiskSource, |_| {})).expect("corpus should parse")
    })
}

fn find(kind: EntityKind, source: &str, name: &str) -> EntityRef<'static> {
    library()
        .find_ci(kind, Some(source), name)
        .unwrap_or_else(|| panic!("{name} ({source}) should exist as {kind}"))
}

/// Every kind must load a plausible number of entities; a kind that comes
/// back empty means its file, key or shape drifted.
#[test]
fn every_kind_loads() {
    let lib = library();
    let minimums = [
        (EntityKind::Bestiary, 4000),
        (EntityKind::Spells, 900),
        (EntityKind::Items, 2500),
        (EntityKind::Classes, 30),
        (EntityKind::Subclasses, 200),
        (EntityKind::Races, 200),
        (EntityKind::Backgrounds, 150),
        (EntityKind::Feats, 300),
        (EntityKind::Conditions, 60),
        (EntityKind::Objects, 30),
        (EntityKind::Actions, 40),
        (EntityKind::Hazards, 100),
        (EntityKind::Deities, 500),
        (EntityKind::Vehicles, 60),
        (EntityKind::VariantRules, 200),
        (EntityKind::OptionalFeatures, 200),
        (EntityKind::Rewards, 250),
        (EntityKind::Psionics, 50),
        (EntityKind::Recipes, 200),
        (EntityKind::CultsBoons, 40),
        (EntityKind::Languages, 190),
        (EntityKind::CharOptions, 40),
        (EntityKind::Facilities, 60),
        (EntityKind::ItemProperties, 25),
        (EntityKind::ItemMasteries, 8),
        (EntityKind::Skills, 30),
        (EntityKind::Senses, 8),
        (EntityKind::Tables, 1000),
        (EntityKind::Rules, 1000),
        (EntityKind::Decks, 30),
        (EntityKind::Cards, 700),
        (EntityKind::Encounters, 40),
        (EntityKind::LegendaryGroups, 150),
        (EntityKind::CrochetPatterns, 15),
    ];
    let too_few: Vec<String> = minimums
        .iter()
        .map(|(kind, min)| (kind, min, lib.entities_of(*kind).count()))
        .filter(|(_, min, count)| count < min)
        .map(|(kind, min, count)| format!("{kind}: expected at least {min}, loaded {count}"))
        .collect();
    assert!(too_few.is_empty(), "{too_few:#?}");
}

#[test]
fn known_entities_resolve() {
    // Monsters get lair actions and regional effects through their legendary group.
    for (source, name, key) in [
        ("XMM", "Adult Red Dragon", "regionalEffects"),
        ("MM", "Aboleth", "lairActions"),
    ] {
        let EntityRef::Monster(monster) = find(EntityKind::Bestiary, source, name) else {
            panic!()
        };
        let group = monster.legendary_group.as_ref().expect("legendary group reference");
        let EntityRef::Other(_, group) = find(EntityKind::LegendaryGroups, &group.source, &group.name) else {
            panic!()
        };
        assert!(group.extra.contains_key(key), "{name}'s group should have {key}");
    }

    find(EntityKind::Subclasses, "XPHB", "Champion");
    find(EntityKind::ItemMasteries, "XPHB", "Cleave");
    find(EntityKind::ItemProperties, "XPHB", "Two-Handed");
    find(EntityKind::Items, "DMG", "+1 Weapon");
    find(EntityKind::Races, "PHB", "Elf (High)");
    let EntityRef::Other(_, rule) = find(EntityKind::Rules, "XPHB", "Mastery Properties") else {
        panic!()
    };
    let book = library().source_full_name("XPHB").expect("the book should be listed");
    assert!(rule.extra["path"].as_str().unwrap().starts_with(&format!("{book} › ")));
}

/// "Undead Soldier" (DSotDQ) is a plain `_copy` of "Wight" with a
/// "*"/replaceTxt _mod - it should inherit Wight's AC/HP/actions rather
/// than rendering as an empty statblock.
#[test]
fn bestiary_copies_resolve() {
    let EntityRef::Monster(soldier) = find(EntityKind::Bestiary, "DSotDQ", "Undead Soldier") else {
        panic!()
    };
    assert_eq!(soldier.hp.as_ref().and_then(|h| h.average), Some(45));
    assert!(
        !soldier.action.is_empty(),
        "actions should be inherited from the Wight base"
    );
}

#[test]
fn copies_dont_lose_entities() {
    let raw: Vec<Value> = futures::executor::block_on(async {
        let index: BTreeMap<String, String> =
            serde_json::from_value(DiskSource.value("bestiary/index.json").await.unwrap()).unwrap();
        let mut all = Vec::new();
        for file in index.values() {
            let v = DiskSource.value(&format!("bestiary/{file}")).await.unwrap();
            all.extend(v["monster"].as_array().cloned().unwrap_or_default());
        }
        all
    });
    let total = raw.len();
    let with_copy = raw.iter().filter(|m| m.get("_copy").is_some()).count();
    assert!(with_copy > 1000, "expected >1000 _copy monsters, got {with_copy}");
    assert_eq!(resolve_copies(raw).len(), total);
    // A merge/_mod bug must not silently drop entries at deserialization.
    assert!(
        library().monsters.len() as f64 > total as f64 * 0.99,
        "{} of {total} monsters loaded",
        library().monsters.len()
    );
}

fn entity_json(entity: &EntityRef) -> Value {
    match entity {
        EntityRef::Monster(e) => serde_json::to_value(e),
        EntityRef::Spell(e) => serde_json::to_value(e),
        EntityRef::Item(e) => serde_json::to_value(e),
        EntityRef::Class(e) => serde_json::to_value(e),
        EntityRef::Race(e) => serde_json::to_value(e),
        EntityRef::Background(e) => serde_json::to_value(e),
        EntityRef::Feat(e) => serde_json::to_value(e),
        EntityRef::Condition(e) => serde_json::to_value(e),
        EntityRef::Object(e) => serde_json::to_value(e),
        EntityRef::Action(e) => serde_json::to_value(e),
        EntityRef::Hazard(e) => serde_json::to_value(e),
        EntityRef::Other(_, e) => serde_json::to_value(e),
    }
    .expect("entities serialize")
}

/// Fields every statblock handles the same way: identity, the page line and
/// the source footer, plus catalog metadata that describes the data rather
/// than the entity (which books reprint it, filter tags, art and token
/// assets, SRD flags, data migration markers).
const COMMON: &[&str] = &[
    "name",
    "source",
    "page",
    "entries",
    "reprintedAs",
    "otherSources",
    "additionalSources",
    "alias",
    "srd",
    "srd52",
    "basicRules",
    "basicRules2024",
    "edition",
    "referenceSources",
    "migrationVersion",
    "hasFluff",
    "hasFluffImages",
    "hasToken",
    "tokenCredit",
    "tokenCustom",
    "altArt",
    "soundClip",
    "isReprinted",
    "foundryTokenScale",
    "prototypeToken",
    "effects",
    "activities",
    "system",
    "_versions",
    "_copy",
];

/// The fields a kind's statblock reads (or, for the listed few, ones that
/// only describe the data - tags for filtering, structured copies of
/// information already spelled out in the entries).
fn known_fields(kind: EntityKind) -> &'static [&'static str] {
    match kind {
        EntityKind::Bestiary => &[
            "size",
            "sizeNote",
            "type",
            "alignment",
            "alignmentPrefix",
            "level",
            "ac",
            "hp",
            "speed",
            "initiative",
            "str",
            "dex",
            "con",
            "int",
            "wis",
            "cha",
            "save",
            "skill",
            "tool",
            "vulnerable",
            "resist",
            "immune",
            "conditionImmune",
            "gear",
            "attachedItems",
            "senses",
            "passive",
            "languages",
            "cr",
            "pbNote",
            "spellcasting",
            "trait",
            "action",
            "bonus",
            "reaction",
            "legendary",
            "mythic",
            "legendaryActions",
            "legendaryActionsLair",
            "legendaryGroup",
            "variant",
            "footer",
            "environment",
            "treasure",
            "summonedBySpell",
            "traitHeader",
            "actionHeader",
            "bonusHeader",
            "reactionHeader",
            "legendaryHeader",
            "mythicHeader",
            "actionNote",
            "bonusNote",
            "reactionNote",
            "legendaryNote",
            "mythicNote",
            "isNamedCreature",
            "shortName",
            // Filter tags and catalog data:
            "damageTags",
            "damageTagsSpell",
            "damageTagsLegendary",
            "miscTags",
            "senseTags",
            "languageTags",
            "actionTags",
            "traitTags",
            "spellcastingTags",
            "savingThrowForced",
            "savingThrowForcedSpell",
            "savingThrowForcedLegendary",
            "conditionInflict",
            "conditionInflictSpell",
            "conditionInflictLegendary",
            "isNpc",
            "group",
            "familiar",
            "dragonAge",
            "dragonCastingColor",
            "summonedBySpellLevel",
            "summonedByClass",
        ],
        EntityKind::Spells => &[
            "level",
            "school",
            "time",
            "range",
            "components",
            "duration",
            "entriesHigherLevel",
            "meta",
            "miscTags",
            "areaTags",
            "savingThrow",
            "damageInflict",
            "conditionInflict",
            "affectsCreatureType",
            "spellAttack",
            "abilityCheck",
            "scalingLevelDice",
            "damageResist",
            "damageImmune",
            "damageVulnerable",
            "conditionImmune",
        ],
        EntityKind::Items => &[
            "type",
            "typeAlt",
            "rarity",
            "reqAttune",
            "reqAttuneAlt",
            "reqAttuneTags",
            "weight",
            "weightNote",
            "value",
            "valueRarity",
            "weaponCategory",
            "dmg1",
            "dmg2",
            "dmgType",
            "range",
            "property",
            "mastery",
            "ac",
            "acSpecial",
            "dexterityMax",
            "strength",
            "stealth",
            "baseItem",
            "wondrous",
            "tattoo",
            "staff",
            "ammo",
            "ammoType",
            "firearm",
            "poison",
            "poisonTypes",
            "age",
            "tier",
            "charges",
            "recharge",
            "rechargeAmount",
            "additionalEntries",
            "packContents",
            "atomicPackContents",
            "attachedSpells",
            "lootTables",
            "seeAlsoDeck",
            "seeAlsoVehicle",
            "items",
            "requires",
            "excludes",
            "speed",
            "carryingCapacity",
            "barDimensions",
            "vehSpeed",
            "capCargo",
            "capPassenger",
            "crew",
            "crewMin",
            "crewMax",
            "vehAc",
            "vehHp",
            "vehDmgThresh",
            "travelCost",
            "shippingCost",
            "bardingType",
            "detail1",
            "detail2",
            "ability",
            "grantsLanguage",
            "grantsProficiency",
            "focus",
            "scfType",
            "sentient",
            "curse",
            "miscTags",
            "hasRefs",
            "itemsHidden",
            "light",
            "modifySpeed",
            "resist",
            "immune",
            "vulnerable",
            "conditionImmune",
            "bonusWeapon",
            "bonusWeaponAttack",
            "bonusWeaponDamage",
            "bonusAc",
            "bonusSpellAttack",
            "bonusSpellSaveDc",
            "bonusSavingThrow",
            "bonusSavingThrowConcentration",
            "bonusAbilityCheck",
            "bonusProficiencyBonus",
            "critThreshold",
            "spellScrollLevel",
            "containerCapacity",
            "optionalfeatures",
            "classFeatures",
            "group",
            "weapon",
            "armor",
            "reload",
            "sword",
            "crossbow",
            "axe",
            "hammer",
            "bolt",
            "arrow",
            "bow",
            "club",
            "dagger",
            "mace",
            "net",
            "polearm",
            "spear",
            "rapier",
            "lance",
            "needleBlowgun",
            "bulletFirearm",
            "bulletSling",
            "cellEnergy",
            "barding",
            "bonusSpellDamage",
            "propertyAdd",
            "valueExpression",
            "valueMult",
            "weightExpression",
            "weightMult",
        ],
        EntityKind::Classes => &[
            "primaryAbility",
            "hd",
            "proficiency",
            "startingProficiencies",
            "startingEquipment",
            "multiclassing",
            "classTableGroups",
            "classFeatures",
            "subclassTitle",
            "casterProgression",
            "spellcastingAbility",
            "cantripProgression",
            "spellsKnownProgression",
            "spellsKnownProgressionFixed",
            "spellsKnownProgressionFixedByLevel",
            "spellsKnownProgressionFixedAllowLowerLevel",
            "preparedSpells",
            "preparedSpellsChange",
            "preparedSpellsProgression",
            "featProgression",
            "optionalfeatureProgression",
            "additionalSpells",
            "advancement",
            "isSidekick",
        ],
        EntityKind::Races => &[
            "size",
            "sizeEntry",
            "speed",
            "speedEntry",
            "ability",
            "abilityEntry",
            "creatureTypes",
            "creatureTypesEntry",
            "heightAndWeight",
            "raceName",
            "raceSource",
            "traitTags",
            "creatureTypeTags",
            // Covered by the racial traits in the entries:
            "age",
            "darkvision",
            "blindsight",
            "languageProficiencies",
            "skillProficiencies",
            "toolProficiencies",
            "weaponProficiencies",
            "armorProficiencies",
            "skillToolLanguageProficiencies",
            "resist",
            "immune",
            "vulnerable",
            "conditionImmune",
            "additionalSpells",
            "lineage",
            "feats",
            "overwrite",
        ],
        EntityKind::Backgrounds => &[
            "prerequisite",
            // Spelled out in the background's own entries:
            "ability",
            "feats",
            "skillProficiencies",
            "toolProficiencies",
            "languageProficiencies",
            "skillToolLanguageProficiencies",
            "startingEquipment",
            "additionalSpells",
            "fromFeature",
        ],
        EntityKind::Feats => &[
            "prerequisite",
            "category",
            "repeatable",
            "repeatableHidden",
            "ability",
            // Covered by the feat's entries:
            "additionalSpells",
            "skillProficiencies",
            "toolProficiencies",
            "languageProficiencies",
            "armorProficiencies",
            "weaponProficiencies",
            "savingThrowProficiencies",
            "skillToolLanguageProficiencies",
            "expertise",
            "resist",
            "immune",
            "conditionImmune",
            "senses",
            "bonusSenses",
            "optionalfeatureProgression",
            "traitTags",
        ],
        EntityKind::Conditions => &["type"],
        EntityKind::Objects => &[
            "size",
            "objectType",
            "ac",
            "hp",
            "speed",
            "str",
            "dex",
            "con",
            "int",
            "wis",
            "cha",
            "senses",
            "immune",
            "resist",
            "vulnerable",
            "conditionImmune",
            "actionEntries",
            "token",
            "isNpc",
        ],
        EntityKind::Actions => &["time", "seeAlsoAction", "fromVariant"],
        EntityKind::Hazards => &[
            "trapHazType",
            "rating",
            "trigger",
            "effect",
            "countermeasures",
            "eActive",
            "eDynamic",
            "eConstant",
            "initiative",
            "initiativeNote",
            "duration",
            "hauntBonus",
        ],
        EntityKind::Subclasses => &[
            "shortName",
            "className",
            "classSource",
            "subclassFeatures",
            "additionalSpells",
            "spellcastingAbility",
            "casterProgression",
            "cantripProgression",
            "spellsKnownProgression",
            "preparedSpellsProgression",
            "preparedSpellsChange",
            "optionalfeatureProgression",
            "featProgression",
            "subclassTableGroups",
            "advancement",
            "fluff",
            "identifier",
        ],
        EntityKind::Deities => &[
            "title",
            "altNames",
            "pantheon",
            "alignment",
            "category",
            "domains",
            "province",
            "symbol",
            "plane",
            "worshipers",
            "piety",
            "symbolImg",
            "reprintAlias",
            "customExtensionOf",
        ],
        EntityKind::Vehicles => &[
            "vehicleType",
            "upgradeType",
            "size",
            "dimensions",
            "terrain",
            "capCrew",
            "capCrewNote",
            "capPassenger",
            "capCreature",
            "capCargo",
            "weight",
            "cost",
            "pace",
            "speed",
            "ac",
            "hp",
            "hull",
            "str",
            "dex",
            "con",
            "int",
            "wis",
            "cha",
            "immune",
            "conditionImmune",
            "actionThresholds",
            "control",
            "movement",
            "station",
            "actionStation",
            "trait",
            "action",
            "reaction",
            "weapon",
            "type",
        ],
        EntityKind::VariantRules => &["ruleType", "type"],
        EntityKind::OptionalFeatures => &[
            "featureType",
            "prerequisite",
            "consumes",
            "isClassFeatureVariant",
            "featProgression",
            "optionalfeatureProgression",
            "additionalSpells",
            "senses",
            "skillProficiencies",
        ],
        EntityKind::Rewards => &["type", "rarity", "seeAlsoFacility", "additionalSpells"],
        EntityKind::Psionics => &["type", "order", "focus", "modes"],
        EntityKind::Recipes => &[
            "type",
            "dishTypes",
            "diet",
            "allergenGroups",
            "serves",
            "makes",
            "ingredients",
            "equipment",
            "instructions",
            "noteCook",
            "miscTags",
        ],
        EntityKind::CultsBoons => &["type", "cultists", "goal", "signatureSpells", "ability"],
        EntityKind::Languages => &["type", "script", "typicalSpeakers", "origin", "dialects", "fonts"],
        EntityKind::CharOptions => &["optionType", "prerequisite"],
        EntityKind::Facilities => &["facilityType", "level", "space", "hirelings", "orders", "prerequisite"],
        EntityKind::ItemProperties => &["abbreviation", "template"],
        EntityKind::ItemMasteries => &[],
        EntityKind::Skills => &["ability"],
        EntityKind::Senses => &[],
        EntityKind::Tables => &["path"],
        EntityKind::Rules => &["id", "path"],
        EntityKind::Decks => &["cards", "back", "spreads", "hasCardArt"],
        EntityKind::Cards => &["set", "face", "back", "suit", "value", "valueName"],
        EntityKind::Encounters => &["tables"],
        EntityKind::LegendaryGroups => &["lairActions", "regionalEffects", "mythicEncounter"],
        EntityKind::CrochetPatterns => &[
            "designers",
            "level",
            "patternType",
            "size",
            "sizeNote",
            "yarn",
            "hooks",
            "notions",
            "gauge",
            "stitches",
            "notes",
            "abbreviations",
            "instructions",
            "finishing",
            "seeAlsoCreature",
            "seeAlsoItem",
        ],
    }
}

/// Every field found on any loaded entity must be either rendered or
/// explicitly listed above; a new field in the data fails here until a
/// statblock shows it (or it's declared metadata).
#[test]
fn every_field_is_accounted_for() {
    let mut unhandled: BTreeMap<EntityKind, BTreeSet<String>> = BTreeMap::new();
    for kind in EntityKind::all() {
        let known: BTreeSet<&str> = COMMON.iter().chain(known_fields(kind)).copied().collect();
        for entity in library().entities_of(kind) {
            let Value::Object(map) = entity_json(&entity) else {
                continue;
            };
            for key in map.keys().filter(|k| !known.contains(k.as_str())) {
                unhandled.entry(kind).or_default().insert(key.clone());
            }
        }
    }
    assert!(
        unhandled.is_empty(),
        "fields neither rendered nor declared as metadata: {unhandled:#?}"
    );
}

/// The cached snapshot must reproduce the parsed library exactly: every
/// entity of every kind, and the lookups the statblocks rely on.
#[test]
fn snapshot_round_trips_the_library() {
    use crate::data::snapshot;

    let original = library();
    let bytes = snapshot::encode(original).expect("library serializes");
    let restored = snapshot::decode(&bytes).expect("snapshot deserializes");
    println!("snapshot: {} MB", bytes.len() / 1_000_000);

    for kind in EntityKind::all() {
        let before: Vec<Value> = original.entities_of(kind).map(|e| entity_json(&e)).collect();
        let after: Vec<Value> = restored.entities_of(kind).map(|e| entity_json(&e)).collect();
        assert_eq!(before.len(), after.len(), "{kind}: entity count");
        assert!(before == after, "{kind}: entities differ after the round trip");
    }
    assert_eq!(original.class_features.len(), restored.class_features.len());
    assert_eq!(original.subclass_features.len(), restored.subclass_features.len());
    assert_eq!(original.item_types.len(), restored.item_types.len());
    assert_eq!(original.spell_lookup, restored.spell_lookup);
    assert_eq!(original.books.len(), restored.books.len());
    // The search index isn't stored; decoding rebuilds it.
    assert_eq!(
        crate::search::search(original, "ghoul", true).len(),
        crate::search::search(&restored, "ghoul", true).len()
    );
}

#[test]
fn search_ranks_and_caps_results() {
    let hits = crate::search::search(library(), "a", true);
    assert_eq!(hits.len(), 60, "a broad query is capped");

    let hits = crate::search::search(library(), "ghoul", true);
    let names: Vec<&str> = hits.iter().map(|h| h.entity.name()).collect();
    assert_eq!(names[0], "Ghoul", "an exact name match ranks first: {names:?}");
    assert!(hits
        .iter()
        .all(|h| !crate::data::excluded_by_ruleset(h.entity.source(), true)));

    // Keywords: a subclass is found through its class name.
    let hits = crate::search::search(library(), "champion fighter", true);
    assert!(hits.iter().any(|h| h.entity.name() == "Champion"));
    assert!(crate::search::search(library(), "   ", true).is_empty());
}

/// Every entity has its own pin id, and the library finds it again by it.
#[test]
fn pin_ids_are_unique() {
    use crate::pins::PinId;

    let lib = library();
    let mut seen = std::collections::HashMap::new();
    for kind in EntityKind::all() {
        for entity in lib.entities_of(kind) {
            let id = PinId::new(kind, entity.source(), entity.name());
            let key = (kind, entity.source().to_string(), entity.name().to_string());
            if let Some(other) = seen.insert(id, key.clone()) {
                panic!("{other:?} and {key:?} share a pin id");
            }
            let (found_kind, found) = lib.pinned(id).expect("a pin id resolves");
            assert_eq!((found_kind, found.name()), (kind, entity.name()));
        }
    }
}

/// Unsourced links (`{@condition charmed}`) follow the ruleset toggle.
#[test]
fn unsourced_links_follow_the_ruleset() {
    let lib = library();
    for (use_2024, expected) in [(true, "XPHB"), (false, "PHB")] {
        let found = lib
            .find_for_ruleset(EntityKind::Conditions, None, "charmed", use_2024)
            .expect("Charmed exists");
        assert_eq!(found.source(), expected);
    }
    // An explicit source always wins.
    let found = lib
        .find_for_ruleset(EntityKind::Conditions, Some("PHB"), "charmed", true)
        .unwrap();
    assert_eq!(found.source(), "PHB");
}
