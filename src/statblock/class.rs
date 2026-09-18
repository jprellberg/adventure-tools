use dioxus::prelude::*;
use serde_json::{Map, Value};

use super::{divider, prop_line, subtitle};
use crate::model::{Class, ClassFeature, Entry, Other};
use crate::render::{render_cell, render_entries, render_inline, RenderCtx};
use crate::routes::EntityKind;
use crate::statblock::format::*;
use crate::statblock::meta::{describe, humanize_key};

pub fn render(class: &Class, header: Element, ctx: RenderCtx) -> Element {
    let extra = &class.extra;
    let hit_die = class.hd.as_ref().map(hit_die_text).unwrap_or_default();
    let primary = extra
        .get("primaryAbility")
        .map(primary_ability_text)
        .unwrap_or_default();
    let saves: Vec<String> = strings_at(extra, "proficiency")
        .into_iter()
        .map(|a| ability_full(a).to_string())
        .collect();
    let starting = class.starting_proficiencies.as_ref().and_then(Value::as_object);
    let features = class_feature_refs(extra);
    let subclasses = ctx
        .library
        .entities_of(EntityKind::Subclasses)
        .filter(|s| {
            str_at(s.extra(), "className").is_some_and(|c| c.eq_ignore_ascii_case(&class.name))
                && str_at(s.extra(), "classSource").is_some_and(|c| c.eq_ignore_ascii_case(&class.source))
        })
        .map(|s| {
            let sub_source = s.source();
            format!(
                "{{@subclass {}|{}|{}|{sub_source}}}",
                s.name(),
                class.name,
                class.source
            )
        })
        .collect::<Vec<_>>();
    let subclass_title = str_at(extra, "subclassTitle").unwrap_or("Subclasses");

    rsx! {
        div { class: "text-sm",
            {header}
            {prop_line("Hit Die:", &hit_die, ctx)}
            {prop_line("Primary Ability:", &primary, ctx)}
            {prop_line("Saving Throw Proficiencies:", &saves.join(", "), ctx)}
            if let Some(p) = starting {
                {starting_proficiencies(p, ctx)}
            }
            {prop_line("Spellcasting Ability:", &str_at(extra, "spellcastingAbility").map(|a| ability_full(a).to_string()).unwrap_or_default(), ctx)}
            {prop_line("Caster Progression:", &caster_progression(extra), ctx)}
            {prop_line("Multiclassing:", &extra.get("multiclassing").map(multiclassing_text).unwrap_or_default(), ctx)}
            {progressions(extra, ctx)}
            {divider()}
            if let Some(Value::Object(eq)) = extra.get("startingEquipment") {
                {starting_equipment(eq, ctx)}
            }
            {class_table(extra.get("classTableGroups"), &features, ctx)}
            {feature_list(&class.name, &class.source, &features, ctx)}
            if !subclasses.is_empty() {
                {prop_line(&format!("{subclass_title}:"), &subclasses.join(", "), ctx)}
            }
        }
    }
}

/// A subclass: its features by level, plus the spell-list and progression
/// tables it adds.
pub fn render_subclass(sub: &Other, header: Element, ctx: RenderCtx) -> Element {
    let extra = &sub.extra;
    let class_name = str_at(extra, "className").unwrap_or("");
    let class_source = str_at(extra, "classSource").unwrap_or("");
    let short_name = str_at(extra, "shortName").unwrap_or(&sub.name);
    // Features a subclass lists are its top-level ones; the rest are
    // spliced into those through references.
    let listed: Vec<(String, u8)> = strings_at(extra, "subclassFeatures")
        .into_iter()
        .filter_map(|uid| {
            let parts: Vec<&str> = uid.split('|').collect();
            Some((parts.first()?.to_lowercase(), parts.get(5)?.parse().ok()?))
        })
        .collect();
    let features: Vec<&ClassFeature> = {
        let mut list: Vec<&ClassFeature> = ctx
            .library
            .subclass_features
            .iter()
            .filter(|f| {
                f.class_name.eq_ignore_ascii_case(class_name)
                    && f.class_source.eq_ignore_ascii_case(class_source)
                    && f.subclass_short_name
                        .as_deref()
                        .is_some_and(|s| s.eq_ignore_ascii_case(short_name))
                    && f.subclass_source
                        .as_deref()
                        .is_some_and(|s| s.eq_ignore_ascii_case(&sub.source))
                    && (listed.is_empty() || listed.contains(&(f.name.to_lowercase(), f.level)))
            })
            .collect();
        list.sort_by_key(|f| f.level);
        list
    };
    let subtitle_text = format!("{{@class {class_name}|{class_source}}} subclass");
    rsx! {
        div { class: "text-sm",
            {header}
            {subtitle(&subtitle_text, ctx)}
            {prop_line("Spellcasting Ability:", &str_at(extra, "spellcastingAbility").map(|a| ability_full(a).to_string()).unwrap_or_default(), ctx)}
            {prop_line("Caster Progression:", &caster_progression(extra), ctx)}
            {progressions(extra, ctx)}
            {divider()}
            {class_table(extra.get("subclassTableGroups"), &[], ctx)}
            for feature in features {
                {render_feature(feature, ctx)}
            }
            if let Some(spells) = extra.get("additionalSpells") {
                {additional_spells(spells, ctx)}
            }
        }
    }
}

fn hit_die_text(hd: &Value) -> String {
    let faces = hd.get("faces").and_then(Value::as_i64).unwrap_or(0);
    let number = hd.get("number").and_then(Value::as_i64).unwrap_or(1);
    format!("{number}d{faces}")
}

fn primary_ability_text(v: &Value) -> String {
    let options: Vec<String> = v
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(Value::as_object)
                .map(|o| {
                    let names: Vec<String> = o.keys().map(|k| ability_full(k).to_string()).collect();
                    names.join(" and ")
                })
                .collect()
        })
        .unwrap_or_default();
    join_conjunct(&options, "or")
}

fn caster_progression(extra: &Map<String, Value>) -> String {
    match str_at(extra, "casterProgression") {
        Some("full") => "Full caster".into(),
        Some("1/2") => "Half caster".into(),
        Some("1/3") => "Third caster".into(),
        Some("pact") => "Pact magic".into(),
        Some("artificer") => "Artificer".into(),
        Some(other) => other.into(),
        None => String::new(),
    }
}

/// Armor, weapon, tool and skill grants of a class's `startingProficiencies`
/// / `multiclassing.proficienciesGained`.
fn proficiency_lines(p: &Map<String, Value>) -> Vec<(&'static str, String)> {
    let armor: Vec<String> = match p.get("armor").and_then(Value::as_array) {
        Some(list) => list
            .iter()
            .filter_map(|a| match a {
                Value::String(s) if s == "shield" => Some("shields".to_string()),
                Value::String(s) => Some(format!("{s} armor")),
                Value::Object(o) => str_at(o, "full").map(String::from),
                _ => None,
            })
            .collect(),
        None => Vec::new(),
    };
    let weapons: Vec<String> = p
        .get("weapons")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|w| match w {
                    Value::String(s) if s.contains('|') => Some(format!("{{@item {s}}}")),
                    Value::String(s) if matches!(s.as_str(), "simple" | "martial") => Some(format!("{s} weapons")),
                    Value::String(s) => Some(s.clone()),
                    Value::Object(o) => str_at(o, "proficiency").map(String::from),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    let tools = p
        .get("toolProficiencies")
        .map(crate::statblock::meta::proficiency_text)
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| strings_at(p, "tools").join(", "));
    let skills = p.get("skills").map(skill_grants).unwrap_or_default();
    vec![
        ("Armor:", capitalize(&armor.join(", "))),
        ("Weapons:", capitalize(&weapons.join(", "))),
        ("Tools:", tools),
        ("Skills:", skills),
    ]
}

fn skill_grants(v: &Value) -> String {
    v.as_array()
        .map(|options| {
            options
                .iter()
                .filter_map(Value::as_object)
                .map(|o| {
                    if let Some(Value::Object(c)) = o.get("choose") {
                        let count = i64_at(c, "count").unwrap_or(1);
                        let from: Vec<String> = strings_at(c, "from").into_iter().map(title_case).collect();
                        format!("Choose {count}: {}", from.join(", "))
                    } else if let Some(n) = i64_at(o, "any") {
                        format!("Any {n}")
                    } else {
                        o.keys().map(|k| title_case(k)).collect::<Vec<_>>().join(", ")
                    }
                })
                .collect::<Vec<_>>()
                .join("; ")
        })
        .unwrap_or_default()
}

fn starting_proficiencies(p: &Map<String, Value>, ctx: RenderCtx) -> Element {
    let lines = proficiency_lines(p);
    rsx! {
        for (label , text) in lines {
            {prop_line(&format!("Starting {label}"), &text, ctx)}
        }
    }
}

fn multiclassing_text(v: &Value) -> String {
    let Some(m) = v.as_object() else { return String::new() };
    let mut parts = Vec::new();
    if let Some(Value::Object(req)) = m.get("requirements") {
        let text = |o: &Map<String, Value>| {
            o.iter()
                .filter_map(|(k, v)| Some(format!("{} {}", ability_full(k), v.as_i64()?)))
                .collect::<Vec<_>>()
                .join(" and ")
        };
        let alternatives = req.get("or").and_then(Value::as_array).map(|a| {
            a.iter()
                .filter_map(Value::as_object)
                .map(text)
                .collect::<Vec<_>>()
                .join(" or ")
        });
        parts.push(format!("requires {}", alternatives.unwrap_or_else(|| text(req))));
    }
    if let Some(Value::Object(gained)) = m.get("proficienciesGained") {
        let gains: Vec<String> = proficiency_lines(gained)
            .into_iter()
            .filter(|(_, t)| !t.is_empty())
            .map(|(label, t)| format!("{} {t}", label.trim_end_matches(':').to_lowercase()))
            .collect();
        if !gains.is_empty() {
            parts.push(format!("gains {}", gains.join("; ")));
        }
    }
    parts.join("; ")
}

/// Feat, optional-feature and spell progressions: "Fighting Style: level 1".
fn progressions(extra: &Map<String, Value>, ctx: RenderCtx) -> Element {
    let mut lines: Vec<(String, String)> = Vec::new();
    for key in ["featProgression", "optionalfeatureProgression"] {
        for p in extra
            .get(key)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_object)
        {
            let name = str_at(p, "name").unwrap_or("").to_string();
            let by_level = match p.get("progression") {
                Some(Value::Object(levels)) => levels
                    .iter()
                    .map(|(l, n)| format!("{} at level {l}", n.as_i64().map(|n| n.to_string()).unwrap_or_default()))
                    .collect::<Vec<_>>()
                    .join(", "),
                Some(Value::Array(levels)) => format!(
                    "{} total by level {}",
                    levels.iter().rev().find_map(Value::as_i64).unwrap_or(0),
                    levels.len()
                ),
                _ => String::new(),
            };
            lines.push((format!("{name}:"), by_level));
        }
    }
    for (key, label) in [
        ("cantripProgression", "Cantrips known"),
        ("spellsKnownProgression", "Spells known"),
        ("preparedSpellsProgression", "Spells prepared"),
    ] {
        if let Some(Value::Array(levels)) = extra.get(key) {
            let list: Vec<String> = levels.iter().map(describe).collect();
            lines.push((format!("{label}:"), format!("{} (by level)", list.join("/"))));
        }
    }
    for (key, label) in [
        ("preparedSpells", "Prepared spells"),
        ("preparedSpellsChange", "Prepared spells change"),
    ] {
        if let Some(v) = extra.get(key) {
            lines.push((format!("{label}:"), describe(v)));
        }
    }
    rsx! {
        for (label , text) in lines {
            {prop_line(&label, &text, ctx)}
        }
    }
}

fn starting_equipment(eq: &Map<String, Value>, ctx: RenderCtx) -> Element {
    let entries: Vec<Entry> = eq
        .get("entries")
        .and_then(Value::as_array)
        .map(|a| a.iter().cloned().map(Entry::from_value).collect())
        .unwrap_or_else(|| {
            strings_at(eq, "default")
                .into_iter()
                .map(|s| Entry::String(s.to_string()))
                .collect()
        });
    let extra_note = if bool_at(eq, "additionalFromBackground") {
        "Plus any equipment from your background."
    } else {
        ""
    };
    rsx! {
        h4 { class: "mb-0.5 mt-1 text-sm font-bold", "Starting Equipment" }
        {render_entries(&entries, ctx.with_depth(2))}
        if !extra_note.is_empty() {
            p { class: "italic", "{extra_note}" }
        }
    }
}

/// The parsed pieces of a `classFeatures` reference such as
/// `"Rage|Barbarian|SRC|1"` or `{ classFeature: "...", gainSubclassFeature }`.
struct FeatureRef {
    name: String,
    level: u8,
}

fn class_feature_refs(extra: &Map<String, Value>) -> Vec<FeatureRef> {
    extra
        .get("classFeatures")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|f| {
                    let uid = match f {
                        Value::String(s) => s.as_str(),
                        Value::Object(o) => str_at(o, "classFeature")?,
                        _ => return None,
                    };
                    let mut parts = uid.split('|');
                    let name = parts.next()?.to_string();
                    let level = parts.nth(2)?.parse().ok()?;
                    Some(FeatureRef { name, level })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The progression table: level, proficiency bonus, features, then each
/// column group the class defines (rage damage, spell slots, ...).
fn class_table(groups: Option<&Value>, features: &[FeatureRef], ctx: RenderCtx) -> Element {
    let groups: Vec<&Map<String, Value>> = groups
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
        .collect();
    if groups.is_empty() {
        return rsx! {};
    }
    let labels: Vec<String> = groups
        .iter()
        .flat_map(|g| strings_at(g, "colLabels").into_iter().map(String::from))
        .collect();
    let columns: Vec<Vec<Entry>> = groups
        .iter()
        .flat_map(|g| {
            let rows = g
                .get("rows")
                .or_else(|| g.get("rowsSpellProgression"))
                .and_then(Value::as_array);
            let width = strings_at(g, "colLabels").len();
            (0..width).map(move |c| {
                rows.map(|rows| {
                    rows.iter()
                        .map(|r| {
                            r.get(c)
                                .cloned()
                                .map(Entry::from_value)
                                .unwrap_or(Entry::String(String::new()))
                        })
                        .collect()
                })
                .unwrap_or_default()
            })
        })
        .collect();
    let levels = columns.iter().map(Vec::len).max().unwrap_or(0);
    let show_base = !features.is_empty();
    rsx! {
        div { class: "my-1 overflow-x-auto",
            table { class: "w-full border-collapse text-[11px] tabular-nums",
                thead {
                    tr { class: "border-b border-gray-300 text-left",
                        th { class: "px-1", "Lvl" }
                        if show_base {
                            th { class: "px-1", "PB" }
                            th { class: "px-1", "Features" }
                        }
                        for (i , label) in labels.iter().enumerate() {
                            th { key: "{i}", class: "px-1 text-center", {render_inline(label, ctx)} }
                        }
                    }
                }
                tbody {
                    for level in 0..levels {
                        tr { key: "{level}", class: if level % 2 == 1 { "bg-gray-50" } else { "" },
                            td { class: "px-1", "{level + 1}" }
                            if show_base {
                                td { class: "px-1", "{signed(cr_pb(level as i64 + 1))}" }
                                td { class: "px-1", "{features_at(features, level as u8 + 1)}" }
                            }
                            for (i , column) in columns.iter().enumerate() {
                                td { key: "{i}", class: "px-1 text-center",
                                    if let Some(cell) = column.get(level) {
                                        {render_cell(cell, ctx)}
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Proficiency bonus at a character level.
fn cr_pb(level: i64) -> i64 {
    (level - 1) / 4 + 2
}

fn features_at(features: &[FeatureRef], level: u8) -> String {
    let names: Vec<&str> = features
        .iter()
        .filter(|f| f.level == level)
        .map(|f| f.name.as_str())
        .collect();
    if names.is_empty() {
        "—".to_string()
    } else {
        names.join(", ")
    }
}

fn feature_list(class_name: &str, class_source: &str, refs: &[FeatureRef], ctx: RenderCtx) -> Element {
    let features: Vec<&ClassFeature> = refs
        .iter()
        .filter_map(|r| {
            ctx.library.class_features.iter().find(|f| {
                f.level == r.level
                    && f.name.eq_ignore_ascii_case(&r.name)
                    && f.class_name.eq_ignore_ascii_case(class_name)
                    && f.class_source.eq_ignore_ascii_case(class_source)
            })
        })
        .collect();
    rsx! {
        for feature in features {
            {render_feature(feature, ctx)}
        }
    }
}

fn render_feature(feature: &ClassFeature, ctx: RenderCtx) -> Element {
    rsx! {
        div { key: "{feature.name}|{feature.level}", class: "mb-1.5",
            h5 { class: "mt-1.5 text-sm font-semibold leading-tight",
                {render_inline(&feature.name, ctx)}
                " "
                span { class: "text-xs font-normal text-gray-500", "(Level {feature.level})" }
            }
            {render_entries(&feature.entries, ctx.with_depth(2))}
        }
    }
}

/// A subclass's `additionalSpells`: spells it always has prepared or knows,
/// by the level they're gained at.
fn additional_spells(v: &Value, ctx: RenderCtx) -> Element {
    let mut lines: Vec<String> = Vec::new();
    for block in v.as_array().into_iter().flatten().filter_map(Value::as_object) {
        for (kind, spells) in block {
            if !matches!(kind.as_str(), "innate" | "known" | "prepared" | "expanded") {
                continue;
            }
            spell_lines(&humanize_key(kind), spells, &mut lines);
        }
    }
    rsx! {
        if !lines.is_empty() {
            h5 { class: "mt-1.5 text-sm font-semibold", "Additional Spells" }
            for (i , line) in lines.iter().enumerate() {
                p { key: "{i}", class: "leading-snug", {render_inline(line, ctx)} }
            }
        }
    }
}

fn spell_lines(label: &str, v: &Value, out: &mut Vec<String>) {
    fn names(v: &Value) -> Vec<String> {
        match v {
            Value::String(s) => vec![format!("{{@spell {}}}", s.split('#').next().unwrap_or(s))],
            Value::Array(a) => a.iter().flat_map(names).collect(),
            Value::Object(o) => o.values().flat_map(names).collect(),
            _ => Vec::new(),
        }
    }
    match v {
        Value::Object(levels) => {
            for (level, spells) in levels {
                let list = names(spells);
                if !list.is_empty() {
                    out.push(format!("{label} ({level}): {}", list.join(", ")));
                }
            }
        }
        other => {
            let list = names(other);
            if !list.is_empty() {
                out.push(format!("{label}: {}", list.join(", ")));
            }
        }
    }
}
