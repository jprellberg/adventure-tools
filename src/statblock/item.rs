use dioxus::prelude::*;
use serde_json::{Map, Value};

use super::{divider, prop_line, subtitle};
use crate::data::{EntityRef, Library};
use crate::model::Item;
use crate::render::{render_entries, RenderCtx};
use crate::routes::EntityKind;
use crate::statblock::format::*;
use crate::statblock::meta::humanize_key;

pub fn render(item: &Item, header: Element, ctx: RenderCtx) -> Element {
    let lib = ctx.library;
    let extra = &item.extra;
    let mut stats = Vec::new();
    if let Some(v) = item.value {
        stats.push(format_coins(v));
    }
    if let Some(w) = item.weight {
        let note = str_at(extra, "weightNote").map(|n| format!(" {n}")).unwrap_or_default();
        stats.push(format!("{}{note}", format_weight(w)));
    }
    let combat = combat_text(item, lib);
    let mastery: Vec<String> = item.mastery.iter().map(|m| format!("{{@itemMastery {m}}}")).collect();
    let see_also: Vec<String> = [("seeAlsoDeck", "deck"), ("seeAlsoVehicle", "vehicle")]
        .into_iter()
        .flat_map(|(key, tag)| {
            strings_at(extra, key)
                .into_iter()
                .map(move |n| format!("{{@{tag} {n}}}"))
        })
        .collect();
    let spells: Vec<String> = strings_at(extra, "attachedSpells")
        .into_iter()
        .map(|s| format!("{{@spell {s}}}"))
        .collect();
    let tables: Vec<String> = {
        let mut t: Vec<&str> = strings_at(extra, "lootTables");
        t.sort_by_key(|s| s.to_lowercase());
        t.into_iter().map(|t| format!("{{@table {t}}}")).collect()
    };
    let contents = pack_contents(extra);
    let includes = group_items(extra);
    let applies_to = variant_requirements(item, lib);
    let modifies = variant_modifiers(extra);
    let charges = charges_text(extra);
    let strength = armor_requirements(item);

    rsx! {
        div { class: "text-sm",
            {header}
            {subtitle(&type_rarity_line(item, lib), ctx)}
            if !stats.is_empty() {
                p { class: "leading-snug", "{capitalize(&stats.join(\", \"))}" }
            }
            {prop_line("", &combat, ctx)}
            {prop_line("Mastery:", &mastery.join(", "), ctx)}
            {prop_line("", &strength, ctx)}
            {prop_line("Charges:", &charges, ctx)}
            {divider()}
            {prop_line("Includes:", &includes, ctx)}
            {prop_line("Applies to:", &applies_to, ctx)}
            {prop_line("Changes the base item:", &modifies, ctx)}
            {render_entries(&item.entries, ctx)}
            if let Some(Value::Array(more)) = extra.get("additionalEntries") {
                {crate::render::render_values(more, ctx)}
            }
            {prop_line("Contents:", &contents, ctx)}
            {prop_line("Spells:", &spells.join(", "), ctx)}
            {prop_line("See also:", &see_also.join(", "), ctx)}
            {prop_line("Found on:", &tables.join(", "), ctx)}
        }
    }
}

/// An item `type`/`typeAlt`/`bardingType` code, optionally with a source
/// ("M|SRC"), spelled out through the corpus' item type table.
fn item_type_name(lib: &Library, code: &str) -> String {
    let (abbr, source) = match code.split_once('|') {
        Some((a, s)) => (a, Some(s)),
        None => (code, None),
    };
    lib.item_types
        .iter()
        .filter(|t| t.abbreviation == abbr)
        .find(|t| source.is_none_or(|s| t.source.eq_ignore_ascii_case(s)))
        .or_else(|| lib.item_types.iter().find(|t| t.abbreviation == abbr))
        .map(|t| t.name.to_lowercase())
        .unwrap_or_else(|| abbr.to_lowercase())
}

/// "Weapon (Longsword), Martial Weapon" / "Wondrous Item, Rare (Requires
/// Attunement)" - the italic line under an item's name.
fn type_rarity_line(item: &Item, lib: &Library) -> String {
    let extra = &item.extra;
    let flag = |k: &str| bool_at(extra, k);
    let mut kinds: Vec<String> = Vec::new();
    if flag("wondrous") {
        kinds.push(if flag("tattoo") {
            "wondrous item (tattoo)".into()
        } else {
            "wondrous item".into()
        });
    }
    if flag("staff") {
        kinds.push("staff".into());
    }
    if flag("ammo") {
        kinds.push("ammunition".into());
    }
    if item.weapon_category.is_some() {
        let base = str_at(extra, "baseItem")
            .map(|b| format!(" ({{@item {b}}})"))
            .unwrap_or_default();
        kinds.push(format!("weapon{base}"));
    }
    for code in [
        item.item_type.as_deref(),
        str_at(extra, "typeAlt"),
        str_at(extra, "bardingType"),
    ]
    .into_iter()
    .flatten()
    {
        let name = item_type_name(lib, code);
        let with_base = match str_at(extra, "baseItem") {
            Some(base) if item.weapon_category.is_none() => format!("{name} ({{@item {base}}})"),
            _ => name,
        };
        if !kinds.contains(&with_base) {
            kinds.push(with_base);
        }
    }
    if flag("firearm") {
        kinds.push("firearm".into());
    }
    if flag("poison") {
        let types = strings_at(extra, "poisonTypes");
        kinds.push(if types.is_empty() {
            "poison".into()
        } else {
            format!("poison ({})", types.join(" or "))
        });
    }
    if let Some(cat) = &item.weapon_category {
        kinds.push(format!("{cat} weapon"));
    }
    if let Some(age) = str_at(extra, "age") {
        kinds.push(age.to_string());
    }
    let mut line = kinds.iter().map(|k| title_case(k)).collect::<Vec<_>>().join(", ");
    if let Some(rarity) = item.rarity.as_deref().filter(|r| !matches!(*r, "none")) {
        if !line.is_empty() {
            line.push_str(", ");
        }
        line.push_str(&title_case(rarity));
    }
    if let Some(tier) = str_at(extra, "tier") {
        line.push_str(&format!(", {} Tier", capitalize(tier)));
    }
    if let Some(attune) = item.req_attune.as_ref().and_then(attunement_text) {
        line.push(' ');
        line.push_str(&attune);
    }
    if line.is_empty() {
        "Item".into()
    } else {
        line
    }
}

fn attunement_text(v: &Value) -> Option<String> {
    match v {
        Value::Bool(true) => Some("(Requires Attunement)".into()),
        Value::String(s) if s == "optional" => Some("(Attunement Optional)".into()),
        Value::String(s) if s.to_lowercase().starts_with("by") => {
            Some(format!("(Requires Attunement {})", title_case(s)))
        }
        Value::String(s) => Some(format!("(Requires Attunement {})", title_case(s))),
        _ => None,
    }
}

/// Armor class, damage and properties, as one line: "AC 14 + Dex (max 2),
/// Light, finesse". Vehicles, mounts and bars add their own stats.
fn combat_text(item: &Item, lib: &Library) -> String {
    let extra = &item.extra;
    let mut parts: Vec<String> = Vec::new();
    let type_abbr = item.item_type.as_deref().map(|t| t.split('|').next().unwrap_or(t));
    if let Some(ac) = item.ac {
        let dex_max = i64_at(extra, "dexterityMax").or(if type_abbr == Some("MA") { Some(2) } else { None });
        let adds_dex = extra.contains_key("dexterityMax") || !matches!(type_abbr, Some("HA" | "S"));
        let prefix = if type_abbr == Some("S") { "+" } else { "" };
        let suffix = if adds_dex {
            format!(" + Dex{}", dex_max.map(|m| format!(" (max {m})")).unwrap_or_default())
        } else {
            String::new()
        };
        parts.push(format!("AC {prefix}{ac}{suffix}"));
    }
    if let Some(special) = str_at(extra, "acSpecial") {
        parts.push(if item.ac.is_some() {
            special.to_string()
        } else {
            format!("AC {special}")
        });
    }
    if let Some(dmg) = &item.dmg1 {
        let ty = item.dmg_type.as_deref().map(dmg_type_full).unwrap_or("");
        parts.push(format!("{{@damage {dmg}}} {ty}").trim().to_string());
    }
    if let Some(speed) = extra.get("speed").filter(|_| item.dmg1.is_none()) {
        parts.push(format!("Speed: {}", scalar_text(speed)));
    }
    if let Some(n) = i64_at(extra, "carryingCapacity") {
        parts.push(format!("Carrying Capacity: {n} lb."));
    }
    if let Some(bar) = extra.get("barDimensions").and_then(Value::as_object) {
        let dims: Vec<String> = [("l", "long"), ("w", "wide"), ("h", "thick")]
            .iter()
            .filter_map(|(k, w)| Some(format!("{} in. {w}", scalar_text(bar.get(*k)?))))
            .collect();
        parts.push(dims.join(" × "));
    }
    let vehicle = vehicle_text(extra);
    if !vehicle.is_empty() {
        parts.push(vehicle);
    }
    let mut text = parts.join(", ");
    let props = properties_text(item, lib);
    if !props.is_empty() {
        if !text.is_empty() {
            text.push_str(" | ");
        }
        text.push_str(&props);
    }
    text
}

fn scalar_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

fn vehicle_text(extra: &Map<String, Value>) -> String {
    let mut parts = Vec::new();
    if let Some(speed) = extra.get("vehSpeed") {
        parts.push(format!("Speed: {} mph", scalar_text(speed)));
    }
    let mut cap = Vec::new();
    if let Some(n) = extra.get("capCargo").and_then(Value::as_f64) {
        cap.push(format!("{n} {} cargo", plural(n as i64, "ton")));
    }
    if let Some(n) = i64_at(extra, "capPassenger") {
        cap.push(format!("{n} {}", plural(n, "passenger")));
    }
    if !cap.is_empty() {
        parts.push(format!("Carrying Capacity: {}", cap.join(", ")));
    }
    for (key, label) in [
        ("travelCost", "Personal Travel Cost"),
        ("shippingCost", "Shipping Cost"),
    ] {
        if let Some(n) = i64_at(extra, key) {
            parts.push(format!("{label}: {}", format_coins(n as f64)));
        }
    }
    if let Some(crew) = i64_at(extra, "crew") {
        parts.push(format!("Crew {crew}"));
    } else if let (Some(min), Some(max)) = (i64_at(extra, "crewMin"), i64_at(extra, "crewMax")) {
        parts.push(format!("Crew {min}-{max}"));
    }
    if let Some(ac) = i64_at(extra, "vehAc") {
        parts.push(format!("AC {ac}"));
    }
    if let Some(hp) = i64_at(extra, "vehHp") {
        let threshold = i64_at(extra, "vehDmgThresh")
            .map(|t| format!(", Damage Threshold {t}"))
            .unwrap_or_default();
        parts.push(format!("HP {hp}{threshold}"));
    }
    parts.join(", ")
}

/// A property's text from its template, e.g. "versatile (1d10)".
fn properties_text(item: &Item, lib: &Library) -> String {
    let mut used_dmg2 = false;
    let mut used_range = false;
    let mut names: Vec<(String, String)> = item
        .property
        .iter()
        .filter_map(|p| {
            let (uid, note) = match p {
                Value::String(s) => (s.as_str(), None),
                Value::Object(o) => (str_at(o, "uid")?, str_at(o, "note")),
                _ => return None,
            };
            let (abbr, source) = uid.split_once('|').map(|(a, s)| (a, Some(s))).unwrap_or((uid, None));
            let candidates: Vec<EntityRef> = lib
                .entities_of(EntityKind::ItemProperties)
                .filter(|e| e.extra().get("abbreviation").and_then(Value::as_str) == Some(abbr))
                .collect();
            let prop = candidates
                .iter()
                .position(|e| source.is_some_and(|s| e.source().eq_ignore_ascii_case(s)))
                .or_else(|| {
                    candidates
                        .iter()
                        .position(|e| e.source().eq_ignore_ascii_case(&item.source))
                })
                .or(if candidates.is_empty() { None } else { Some(0) })?;
            let prop = candidates.into_iter().nth(prop)?;
            let EntityRef::Other(_, other) = prop else { return None };
            let base = other.name.replace('-', "\u{2011}");
            let template = str_at(&other.extra, "template").unwrap_or("{{prop_name}}");
            if template.contains("item.dmg2") {
                used_dmg2 = true;
            }
            if template.contains("item.range") {
                used_range = true;
            }
            let text = template
                .replace("{{prop_name_lower}}", &base.to_lowercase())
                .replace("{{prop_name}}", &base)
                .replace(
                    "{{item.dmg1}}",
                    &item
                        .dmg1
                        .as_deref()
                        .map(|d| format!("{{@damage {d}}}"))
                        .unwrap_or_default(),
                )
                .replace(
                    "{{item.dmg2}}",
                    &item
                        .dmg2
                        .as_deref()
                        .map(|d| format!("{{@damage {d}}}"))
                        .unwrap_or_default(),
                )
                .replace("{{item.range}}", item.range.as_deref().unwrap_or(""))
                .replace(
                    "{{item.ammoType}}",
                    &str_at(&item.extra, "ammoType")
                        .map(|a| format!("{{@item {}}}", title_case(a)))
                        .unwrap_or_default(),
                );
            let note = note.map(|n| format!(" ({n})")).unwrap_or_default();
            Some((other.name.clone(), format!("{text}{note}")))
        })
        .collect();
    names.sort();
    let mut texts: Vec<String> = names.into_iter().map(|(_, t)| t).collect();
    if !used_dmg2 {
        if let Some(d) = &item.dmg2 {
            texts.insert(0, format!("alt. {{@damage {d}}}"));
        }
    }
    if !used_range {
        if let Some(r) = &item.range {
            texts.push(format!("range {r} ft."));
        }
    }
    texts.join(", ")
}

fn armor_requirements(item: &Item) -> String {
    let extra = &item.extra;
    let mut parts = Vec::new();
    if let Some(s) = extra.get("strength") {
        parts.push(format!("Strength {} required", scalar_text(s)));
    }
    if bool_at(extra, "stealth") {
        parts.push("Stealth Disadvantage".into());
    }
    parts.join("; ")
}

fn charges_text(extra: &Map<String, Value>) -> String {
    let Some(charges) = i64_at(extra, "charges") else {
        return String::new();
    };
    let mut text = charges.to_string();
    if let Some(recharge) = str_at(extra, "recharge") {
        let when = match recharge {
            "restShort" => "after a short rest",
            "restLong" => "after a long rest",
            "dawn" => "at dawn",
            "dusk" => "at dusk",
            "midnight" => "at midnight",
            "round" => "each round",
            "turn" => "each turn",
            "week" => "weekly",
            "month" => "monthly",
            "year" => "yearly",
            "decade" => "each decade",
            other => other,
        };
        let amount = str_at(extra, "rechargeAmount")
            .map(|a| {
                if a.contains("{@") {
                    format!("{a} charges ")
                } else {
                    format!("{{@dice {a}}} charges ")
                }
            })
            .unwrap_or_default();
        text.push_str(&format!("; regains {amount}{when}"));
    }
    text
}

/// A pack's contents: item references with optional quantities.
fn pack_contents(extra: &Map<String, Value>) -> String {
    let list = extra
        .get("packContents")
        .or_else(|| extra.get("atomicPackContents"))
        .and_then(Value::as_array);
    list.map(|items| {
        items
            .iter()
            .filter_map(|c| match c {
                Value::String(s) => Some(format!("{{@item {s}}}")),
                Value::Object(o) => {
                    if let Some(special) = str_at(o, "special") {
                        return Some(special.to_string());
                    }
                    let item = str_at(o, "item")?;
                    Some(match i64_at(o, "quantity") {
                        Some(q) if q != 1 => format!("{q} {{@item {item}}}"),
                        _ => format!("{{@item {item}}}"),
                    })
                }
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(", ")
    })
    .unwrap_or_default()
}

/// The members of an item group.
fn group_items(extra: &Map<String, Value>) -> String {
    strings_at(extra, "items")
        .into_iter()
        .map(|i| format!("{{@item {i}}}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// A magic variant applies to every base item matching one of its
/// `requires` clauses (and none of its `excludes`).
fn variant_requirements(item: &Item, lib: &Library) -> String {
    let clause = |c: &Value| -> Option<String> {
        let o = c.as_object()?;
        let mut bits = Vec::new();
        for (k, v) in o {
            match (k.as_str(), v) {
                ("type", Value::String(t)) => bits.push(item_type_name(lib, t)),
                ("property", Value::String(p)) => bits.push(format!("{} property", p.to_lowercase())),
                ("name", Value::String(n)) => bits.push(format!("{{@item {n}}}")),
                (_, Value::Bool(true)) => bits.push(humanize_key(k)),
                (_, Value::String(s)) => bits.push(s.clone()),
                (_, Value::Array(a)) => bits.push(a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join("/")),
                _ => {}
            }
        }
        Some(bits.join(" "))
    };
    let list = |key: &str| -> Vec<String> {
        item.extra
            .get(key)
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(clause).collect())
            .unwrap_or_default()
    };
    let requires = list("requires");
    if requires.is_empty() {
        return String::new();
    }
    let excludes = list("excludes");
    let mut text = requires.join("; ");
    if !excludes.is_empty() {
        text.push_str(&format!(" (not {})", excludes.join("; ")));
    }
    text
}

/// What a magic variant changes about the base item's own stats: its price
/// and weight, and any properties it adds.
fn variant_modifiers(extra: &Map<String, Value>) -> String {
    let mut parts = Vec::new();
    for (mult, expression, label) in [
        ("valueMult", "valueExpression", "value"),
        ("weightMult", "weightExpression", "weight"),
    ] {
        if let Some(m) = extra.get(mult) {
            parts.push(format!("{label} ×{}", scalar_text(m)));
        }
        if let Some(e) = str_at(extra, expression) {
            parts.push(format!("{label} {e}"));
        }
    }
    let added: Vec<&str> = strings_at(extra, "propertyAdd");
    if !added.is_empty() {
        parts.push(format!("adds {}", added.join(", ")));
    }
    parts.join("; ")
}
