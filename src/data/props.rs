//! Data-level text substitution the 5etools format defines: `{=prop/mods}`
//! property injectors (a recipe ingredient's amount, a magic variant's
//! bonus) and `{#itemEntry Name|Source}` templates spliced into an item's
//! entries. Both are resolved once at load, so renderers only see final text.

use serde_json::{Map, Value};

use crate::statblock::format::{dmg_type_full, format_imm_res, join_conjunct, ImmResMode};

const NUMBER_WORDS: [&str; 13] = [
    "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven", "twelve",
];

/// A number as a vulgar fraction: 1.5 -> "1½".
fn vulgar(n: f64) -> String {
    let frac = match ((n - n.trunc()) * 100.0).round() as i64 {
        0 => "",
        12 | 13 => "⅛",
        17 => "⅙",
        20 => "⅕",
        25 => "¼",
        33 => "⅓",
        38 => "⅜",
        40 => "⅖",
        50 => "½",
        60 => "⅗",
        62 | 63 => "⅝",
        67 => "⅔",
        75 => "¾",
        87 | 88 => "⅞",
        _ => return n.to_string(),
    };
    match (n.trunc() as i64, frac) {
        (whole, "") => whole.to_string(),
        (0, f) => f.to_string(),
        (whole, f) => format!("{whole}{f}"),
    }
}

fn title_case_words(s: &str) -> String {
    crate::statblock::format::title_case(s)
}

/// Applies a property injector's modifiers (`v` vulgar, `x` as text, `r`/`f`/
/// `c` round/floor/ceil, `l`/`t`/`u` case, `a` "a"/"an") to a value.
fn apply_modifiers(value: &Value, modifiers: &str) -> String {
    let mut number = value.as_f64();
    let mut text = match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    let ordered = |m: char| "rfcvxltua".find(m).unwrap_or(usize::MAX);
    let mut mods: Vec<char> = modifiers.chars().collect();
    mods.sort_by_key(|m| ordered(*m));
    for m in mods {
        match (m, number) {
            ('r', Some(n)) => number = Some(n.round()),
            ('f', Some(n)) => number = Some(n.floor()),
            ('c', Some(n)) => number = Some(n.ceil()),
            ('v', Some(n)) => text = vulgar(n),
            ('x', Some(n)) => {
                text = NUMBER_WORDS
                    .get(n as usize)
                    .map(|w| w.to_string())
                    .unwrap_or_else(|| n.to_string());
            }
            ('l', _) => text = text.to_lowercase(),
            ('t', _) => text = title_case_words(&text),
            ('u', _) => text = text.to_uppercase(),
            ('a', _) => {
                text = if text.starts_with(['a', 'e', 'i', 'o', 'u', 'A', 'E', 'I', 'O', 'U']) {
                    "an".into()
                } else {
                    "a".into()
                };
            }
            _ => {}
        }
        if let Some(n) = number {
            if !matches!(m, 'v' | 'x' | 'l' | 't' | 'u' | 'a') {
                text = if n.fract() == 0.0 {
                    (n as i64).to_string()
                } else {
                    n.to_string()
                };
            }
        }
    }
    text
}

/// Replaces every `{=path}` / `{=path/mods}` in `text` with the property of
/// that name in `props`; unknown properties are left as written.
pub fn apply_properties(text: &str, props: &Map<String, Value>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("{=") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find('}') else {
            out.push_str(&rest[start..]);
            return out;
        };
        let (path, modifiers) = after[..end].split_once('/').unwrap_or((&after[..end], ""));
        match props.get(path) {
            Some(Value::String(code)) if path == "dmgType" && modifiers.is_empty() => out.push_str(dmg_type_full(code)),
            Some(value) => out.push_str(&apply_modifiers(value, modifiers)),
            None => out.push_str(&rest[start..start + 3 + end]),
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

/// Applies property injectors throughout `value`, all reading from `props`.
pub fn apply_all_properties(value: &mut Value, props: &Map<String, Value>) {
    match value {
        Value::String(s) if s.contains("{=") => *s = apply_properties(s, props),
        Value::Array(items) => items.iter_mut().for_each(|v| apply_all_properties(v, props)),
        Value::Object(map) => map.values_mut().for_each(|v| apply_all_properties(v, props)),
        _ => {}
    }
}

/// Like [`apply_all_properties`], but each string reads the properties of
/// its nearest enclosing object (a recipe ingredient's own `amount1`).
pub fn apply_own_properties(value: &mut Value, inherited: &Map<String, Value>) {
    match value {
        Value::String(s) if s.contains("{=") => *s = apply_properties(s, inherited),
        Value::Array(items) => items.iter_mut().for_each(|v| apply_own_properties(v, inherited)),
        Value::Object(map) => {
            let own = map.clone();
            map.values_mut().for_each(|v| apply_own_properties(v, &own));
        }
        _ => {}
    }
}

/// An item field for a `{{item.field}}` template placeholder.
fn template_value(item: &Map<String, Value>, expr: &str) -> String {
    let (function, path) = match expr.split_once(' ') {
        Some((f, p)) => (Some(f), p),
        None => (None, expr),
    };
    let Some(value) = path.strip_prefix("item.").and_then(|k| item.get(k)) else {
        return String::new();
    };
    match (function, value) {
        (Some("getFullImmRes"), v) => format_imm_res(v, ImmResMode::Damage).to_lowercase(),
        (_, Value::String(s)) => s.clone(),
        (_, Value::Array(a)) => join_conjunct(
            &a.iter().filter_map(Value::as_str).map(String::from).collect::<Vec<_>>(),
            "and",
        ),
        (_, other) => other.to_string(),
    }
}

fn fill_template(value: &mut Value, item: &Map<String, Value>) {
    match value {
        Value::String(s) if s.contains("{{") => {
            let mut out = String::new();
            let mut rest = s.as_str();
            while let Some(start) = rest.find("{{") {
                out.push_str(&rest[..start]);
                let after = &rest[start + 2..];
                let Some(end) = after.find("}}") else { break };
                out.push_str(&template_value(item, after[..end].trim()));
                rest = &after[end + 2..];
            }
            out.push_str(rest);
            *s = out;
        }
        Value::Array(items) => items.iter_mut().for_each(|v| fill_template(v, item)),
        Value::Object(map) => map.values_mut().for_each(|v| fill_template(v, item)),
        _ => {}
    }
}

/// Replaces each `{#itemEntry Name|Source}` entry of an item with the named
/// template's entries, its `{{item.field}}` placeholders filled from the
/// item. `templates` are the `itemEntry` objects of `items-base.json`.
pub fn expand_item_entries(item: &mut Map<String, Value>, templates: &[Value]) {
    let snapshot = item.clone();
    for key in ["entries", "additionalEntries"] {
        let Some(Value::Array(entries)) = item.get_mut(key) else {
            continue;
        };
        let expanded: Vec<Value> = entries
            .drain(..)
            .flat_map(|entry| {
                let reference = entry
                    .as_str()
                    .and_then(|s| s.strip_prefix("{#itemEntry "))
                    .and_then(|s| s.strip_suffix('}'));
                let Some((name, source)) = reference.and_then(|r| r.split_once('|')) else {
                    return vec![entry];
                };
                let template = templates.iter().find(|t| {
                    t.get("name")
                        .and_then(Value::as_str)
                        .is_some_and(|n| n.eq_ignore_ascii_case(name))
                        && t.get("source")
                            .and_then(Value::as_str)
                            .is_some_and(|s| s.eq_ignore_ascii_case(source))
                });
                match template
                    .and_then(|t| t.get("entriesTemplate"))
                    .and_then(Value::as_array)
                {
                    Some(parts) => parts
                        .iter()
                        .cloned()
                        .map(|mut part| {
                            fill_template(&mut part, &snapshot);
                            part
                        })
                        .collect(),
                    None => vec![entry],
                }
            })
            .collect();
        *entries = expanded;
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn props(v: Value) -> Map<String, Value> {
        v.as_object().cloned().unwrap()
    }

    #[test]
    fn injects_plain_and_modified_properties() {
        let p = props(json!({ "amount1": 1.5, "name": "egg", "bonusWeapon": "+1", "dmgType": "S" }));
        assert_eq!(apply_properties("{=amount1/v} cups", &p), "1½ cups");
        assert_eq!(apply_properties("{=amount1/c} {=name/t}", &p), "2 Egg");
        assert_eq!(
            apply_properties("{=bonusWeapon} bonus, {=dmgType} damage", &p),
            "+1 bonus, slashing damage"
        );
        assert_eq!(apply_properties("{=missing} stays", &p), "{=missing} stays");
    }

    #[test]
    fn injects_inside_tags() {
        let p = props(json!({ "amount1": 2 }));
        assert_eq!(
            apply_properties("{@unit {=amount1/c}|egg|eggs}", &p),
            "{@unit 2|egg|eggs}"
        );
    }

    #[test]
    fn expands_item_entry_templates() {
        let templates =
            vec![json!({ "name": "Tattoo", "source": "TCE", "entriesTemplate": ["one color ({{item.detail1}})."] })];
        let mut item = props(json!({ "detail1": "blue", "entries": ["Intro.", "{#itemEntry Tattoo|TCE}"] }));
        expand_item_entries(&mut item, &templates);
        assert_eq!(item["entries"], json!(["Intro.", "one color (blue)."]));
    }
}
