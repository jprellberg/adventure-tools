use std::collections::BTreeMap;

use dioxus::prelude::*;
use serde_json::{Map, Value};

use super::{divider, prop_line, subtitle};
use crate::model::Spell;
use crate::render::{render_entries, RenderCtx};
use crate::statblock::format::*;

pub fn render(s: &Spell, header: Element, ctx: RenderCtx) -> Element {
    let extra = &s.extra;
    let time = s.time.as_ref().map(format_time).unwrap_or_default();
    let range = s.range.as_ref().map(format_range).unwrap_or_default();
    let components = s.components.as_ref().map(format_components).unwrap_or_default();
    let duration = s.duration.as_ref().map(format_duration).unwrap_or_default();
    let sources = spell_sources(s, ctx);
    rsx! {
        div { class: "text-sm",
            {header}
            {subtitle(&format_level_school(s), ctx)}
            {prop_line("Casting Time:", &time, ctx)}
            {prop_line("Range:", &range, ctx)}
            {prop_line("Components:", &components, ctx)}
            {prop_line("Duration:", &duration, ctx)}
            {divider()}
            {render_entries(&s.entries, ctx)}
            if !s.entries_higher_level.is_empty() {
                {render_entries(&s.entries_higher_level, ctx.with_depth(2))}
            }
            {divider()}
            for (label , text) in sources {
                {prop_line(&label, &text, ctx)}
            }
            {prop_line("Also known as:", &strings_at(extra, "alias").join(", "), ctx)}
        }
    }
}

fn is_2024(source: &str) -> bool {
    source.eq_ignore_ascii_case("XPHB")
}

/// "3rd-level evocation (ritual)" - or the 2024 "Level 3 Evocation" form for
/// the 2024 rules.
fn format_level_school(s: &Spell) -> String {
    let school = s.school.as_deref().map(school_name).unwrap_or("");
    let ritual = s
        .extra
        .get("meta")
        .and_then(|m| m.get("ritual"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let text = match (s.level, is_2024(&s.source)) {
        (Some(0), true) => format!("{school} Cantrip"),
        (Some(n), true) => format!("Level {n} {school}"),
        (Some(0), false) => format!("{school} cantrip"),
        (Some(n), false) => format!("{}-level {}", ordinal(n as i64), school.to_lowercase()),
        (None, _) => school.to_string(),
    };
    if ritual {
        format!("{text} (ritual)")
    } else {
        text
    }
}

fn school_name(code: &str) -> &'static str {
    match code {
        "A" => "Abjuration",
        "C" => "Conjuration",
        "D" => "Divination",
        "E" => "Enchantment",
        "V" => "Evocation",
        "I" => "Illusion",
        "N" => "Necromancy",
        "T" => "Transmutation",
        "P" => "Psionic",
        _ => "",
    }
}

fn format_time(v: &Value) -> String {
    let Some(times) = v.as_array() else {
        return String::new();
    };
    times
        .iter()
        .filter_map(Value::as_object)
        .map(|t| {
            let number = i64_at(t, "number").unwrap_or(1);
            let unit = str_at(t, "unit").unwrap_or("");
            let base = match unit {
                "bonus" => "Bonus Action".to_string(),
                "action" | "reaction" if number == 1 => capitalize(unit),
                "action" | "reaction" => format!("{number} {unit}s"),
                other => format!("{number} {}", plural(number, other)),
            };
            match str_at(t, "condition") {
                Some(c) => format!("{base}, {c}"),
                None => base,
            }
        })
        .collect::<Vec<_>>()
        .join(" or ")
}

fn distance_text(d: &Map<String, Value>) -> String {
    let kind = str_at(d, "type").unwrap_or("");
    match i64_at(d, "amount") {
        Some(amount) => {
            let unit = if amount == 1 { kind.trim_end_matches('s') } else { kind };
            format!("{amount} {unit}")
        }
        None => capitalize(kind),
    }
}

/// "Self (15-foot cone)", "120 feet", "Touch", "Sight".
fn format_range(v: &Value) -> String {
    let Some(range) = v.as_object() else {
        return String::new();
    };
    let kind = str_at(range, "type").unwrap_or("");
    let distance = range.get("distance").and_then(Value::as_object);
    match (kind, distance) {
        ("special", _) => "Special".to_string(),
        ("point", Some(d)) => distance_text(d),
        (shape, Some(d)) => {
            let amount = i64_at(d, "amount").unwrap_or(0);
            let unit = if str_at(d, "type") == Some("miles") {
                "mile"
            } else {
                "foot"
            };
            format!("Self ({amount}-{unit} {shape})")
        }
        (_, None) => capitalize(kind),
    }
}

fn format_components(v: &Value) -> String {
    let Some(c) = v.as_object() else { return String::new() };
    let mut parts = Vec::new();
    if bool_at(c, "v") {
        parts.push("V".to_string());
    }
    if bool_at(c, "s") {
        parts.push("S".to_string());
    }
    match c.get("m") {
        Some(Value::String(desc)) => parts.push(format!("M ({desc})")),
        Some(Value::Object(m)) => {
            let text = str_at(m, "text").unwrap_or("");
            let cost = i64_at(m, "cost")
                .map(|c| format!(", worth {} gp", thousands(c / 100)))
                .unwrap_or_default();
            let consume = match m.get("consume") {
                Some(Value::Bool(true)) => ", which the spell consumes",
                Some(Value::String(_)) => ", which the spell consumes",
                _ => "",
            };
            parts.push(format!("M ({text}{cost}{consume})"));
        }
        Some(Value::Bool(true)) => parts.push("M".to_string()),
        _ => {}
    }
    match c.get("r") {
        Some(Value::Bool(true)) => parts.push("R".to_string()),
        Some(Value::String(r)) => parts.push(format!("R ({r})")),
        _ => {}
    }
    parts.join(", ")
}

fn duration_part(d: &Value) -> String {
    let Some(d) = d.as_object() else { return String::new() };
    let base = match str_at(d, "type") {
        Some("instant") => "Instantaneous".to_string(),
        Some("permanent") => {
            let ends = strings_at(d, "ends");
            if ends.is_empty() {
                "Permanent".to_string()
            } else {
                let names: Vec<String> = ends
                    .iter()
                    .map(|e| match *e {
                        "dispel" => "dispelled".to_string(),
                        "trigger" => "triggered".to_string(),
                        "discharge" => "discharged".to_string(),
                        other => other.to_string(),
                    })
                    .collect();
                format!("Until {}", join_conjunct(&names, "or"))
            }
        }
        Some("timed") => {
            let dur = d.get("duration").and_then(Value::as_object);
            let amount = dur.and_then(|x| i64_at(x, "amount")).unwrap_or(1);
            let unit = dur.and_then(|x| str_at(x, "type")).unwrap_or("");
            let up_to = dur.is_some_and(|x| bool_at(x, "upTo"));
            let text = format!("{amount} {}", plural(amount, unit));
            if up_to {
                format!("up to {text}")
            } else {
                text
            }
        }
        Some("special") => "Special".to_string(),
        _ => String::new(),
    };
    if bool_at(d, "concentration") {
        format!(
            "Concentration, {}",
            if base.starts_with("up to") {
                base
            } else {
                format!("up to {base}")
            }
        )
    } else {
        base
    }
}

fn format_duration(v: &Value) -> String {
    v.as_array()
        .map(|a| a.iter().map(duration_part).collect::<Vec<_>>().join(" or "))
        .unwrap_or_default()
}

/// Who can cast the spell: the "Classes:", "Subclasses:", "Species:" ...
/// lines, from the corpus' spell source lookup.
fn spell_sources(s: &Spell, ctx: RenderCtx) -> Vec<(String, String)> {
    let Some(entry) = ctx
        .library
        .spell_lookup
        .get(&s.source.to_lowercase())
        .and_then(|by_name| by_name.get(&s.name.to_lowercase()))
        .and_then(Value::as_object)
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Some(Value::Object(classes)) = entry.get("class") {
        let mut names: Vec<String> = classes
            .iter()
            .flat_map(|(source, list)| {
                list.as_object()
                    .into_iter()
                    .flatten()
                    .map(move |(class, _)| format!("{{@class {class}|{source}}}"))
            })
            .collect();
        names.sort();
        names.dedup();
        out.push(("Classes:".to_string(), names.join(", ")));
    }
    if let Some(Value::Object(by_source)) = entry.get("subclass") {
        // A subclass reprinted under several class or spell lists is listed once.
        let mut subclasses: BTreeMap<(String, String), String> = BTreeMap::new();
        for classes in by_source.values() {
            for (class, subs) in classes.as_object().into_iter().flatten() {
                for (sub_source, subs) in subs.as_object().into_iter().flatten() {
                    for (short, info) in subs.as_object().into_iter().flatten() {
                        let name = info.get("name").and_then(Value::as_str).unwrap_or(short);
                        subclasses
                            .entry((class.clone(), name.to_string()))
                            .or_insert_with(|| sub_source.clone());
                    }
                }
            }
        }
        let names: Vec<String> = subclasses
            .into_iter()
            .map(|((class, name), source)| format!("{class}: {{@subclass {name}|{class}||{source}|{name}}}"))
            .collect();
        out.push(("Subclasses:".to_string(), names.join(", ")));
    }
    for (key, label, tag) in [
        ("race", "Species:", "race"),
        ("background", "Backgrounds:", "background"),
        ("feat", "Feats:", "feat"),
        ("optionalfeature", "Other options/features:", "optfeature"),
        ("reward", "Rewards:", "reward"),
    ] {
        if let Some(Value::Object(by_source)) = entry.get(key) {
            let mut names: Vec<String> = by_source
                .iter()
                .flat_map(|(source, list)| {
                    list.as_object()
                        .into_iter()
                        .flatten()
                        .map(move |(name, _)| format!("{{@{tag} {name}|{source}}}"))
                })
                .collect();
            names.sort();
            out.push((label.to_string(), names.join(", ")));
        }
    }
    out
}
