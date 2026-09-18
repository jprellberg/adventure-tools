//! Formatters for the structured metadata fields of races, backgrounds,
//! feats, classes and optional features: proficiency grants, ability score
//! bonuses and prerequisites. Like `format`, they return text that may
//! contain `{@tag}` markup.

use serde_json::Value;

use super::format::*;

/// "toolProficiencies" -> "Tool Proficiencies".
pub fn humanize_key(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i == 0 {
            out.extend(c.to_uppercase());
        } else if c.is_uppercase() {
            out.push(' ');
            out.push(c);
        } else if c == '_' || c == '-' {
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out
}

/// A readable rendering of any JSON metadata blob: lists become comma
/// lists, objects `Key value` pairs (a `true` value shows just the key).
/// The fallback for structured fields without a dedicated formatter.
pub fn describe(v: &Value) -> String {
    match v {
        Value::Null | Value::Bool(false) => String::new(),
        Value::Bool(true) => "yes".to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        Value::Array(items) => items
            .iter()
            .map(describe)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(", "),
        Value::Object(map) => map
            .iter()
            .filter_map(|(k, val)| {
                let key = humanize_key(k);
                match val {
                    Value::Bool(true) => Some(key),
                    Value::Bool(false) | Value::Null => None,
                    _ => {
                        let text = describe(val);
                        (!text.is_empty()).then(|| format!("{key} {text}"))
                    }
                }
            })
            .collect::<Vec<_>>()
            .join(", "),
    }
}

/// `[{ insight: true, religion: true }, { choose: { from: [...], count: 2 } }]`
/// (skill/tool/language/weapon/armor proficiency grants) -> "Insight,
/// Religion; choose 2 from ...".
pub fn proficiency_text(v: &Value) -> String {
    let Value::Array(options) = v else { return describe(v) };
    options
        .iter()
        .filter_map(Value::as_object)
        .map(|o| {
            let mut parts: Vec<String> = Vec::new();
            for (key, val) in o {
                match (key.as_str(), val) {
                    ("choose", Value::Object(c)) => {
                        let count = i64_at(c, "count").unwrap_or(1);
                        let from: Vec<String> = strings_at(c, "from").into_iter().map(title_case).collect();
                        parts.push(format!("choose {count} from {}", join_conjunct(&from, "or")));
                    }
                    ("any", Value::Number(n)) => parts.push(format!("any {n}")),
                    ("anyStandard" | "anyExotic" | "anyRare", Value::Number(n)) => {
                        parts.push(format!("{n} {} language", key.trim_start_matches("any").to_lowercase()));
                    }
                    (_, Value::Bool(true)) => parts.push(title_case(&key.replace('|', " "))),
                    (_, Value::Number(n)) => parts.push(format!("{} {n}", title_case(key))),
                    _ => {}
                }
            }
            parts.join(", ")
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("; ")
}

/// Ability score increases: `[{ str: 2, dex: 1 }]` or
/// `[{ choose: { from: [...], count: 2, amount: 1 } }]`.
pub fn ability_bonus_text(v: &Value) -> String {
    let Value::Array(options) = v else { return describe(v) };
    options
        .iter()
        .filter_map(Value::as_object)
        .map(|o| {
            let mut parts: Vec<String> = ABILITIES
                .iter()
                .filter_map(|a| Some(format!("{} {}", ability_full(a), signed(o.get(*a)?.as_i64()?))))
                .collect();
            if let Some(Value::Object(c)) = o.get("choose") {
                let full = |list: Vec<&str>| {
                    join_conjunct(
                        &list
                            .into_iter()
                            .map(|a| ability_full(a).to_string())
                            .collect::<Vec<_>>(),
                        "or",
                    )
                };
                if let Some(Value::Object(w)) = c.get("weighted") {
                    let weights: Vec<String> = w
                        .get("weights")
                        .and_then(Value::as_array)
                        .map(|a| a.iter().filter_map(Value::as_i64).map(signed).collect())
                        .unwrap_or_default();
                    parts.push(format!(
                        "choose from {} ({})",
                        full(strings_at(w, "from")),
                        weights.join(", ")
                    ));
                } else {
                    let count = i64_at(c, "count").unwrap_or(1);
                    let amount = signed(i64_at(c, "amount").unwrap_or(1));
                    parts.push(format!(
                        "choose {count} from {} ({amount} each)",
                        full(strings_at(c, "from"))
                    ));
                }
            }
            parts.join(", ")
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("; or ")
}

fn tagged_list(v: &Value, tag: &str) -> String {
    strings(v)
        .iter()
        .map(|f| format!("{{@{tag} {f}}}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// A feat/optional feature/facility prerequisite list. Each entry of the
/// array is an alternative; the fields inside one are all required.
pub fn prerequisite_text(v: &Value) -> String {
    let Value::Array(options) = v else { return describe(v) };
    options
        .iter()
        .filter_map(Value::as_object)
        .map(|o| {
            let parts: Vec<String> = o
                .iter()
                .map(|(key, val)| match (key.as_str(), val) {
                    ("level", Value::Number(n)) => format!("Level {n}"),
                    ("level", Value::Object(l)) => {
                        let class = l
                            .get("class")
                            .and_then(|c| str_at(c.as_object()?, "name"))
                            .map(|c| format!(" {{@class {c}}}"))
                            .unwrap_or_default();
                        format!("Level {}{class}", i64_at(l, "level").unwrap_or(1))
                    }
                    ("race", Value::Array(races)) => join_conjunct(
                        &races
                            .iter()
                            .filter_map(Value::as_object)
                            .map(|r| {
                                let name = str_at(r, "name").unwrap_or("");
                                match (str_at(r, "displayEntry"), str_at(r, "subrace")) {
                                    (Some(display), _) => display.to_string(),
                                    (None, Some(sub)) => format!("{sub} {name}"),
                                    (None, None) => format!("{{@race {name}}}"),
                                }
                            })
                            .collect::<Vec<_>>(),
                        "or",
                    ),
                    ("ability", Value::Array(alts)) => alts
                        .iter()
                        .filter_map(Value::as_object)
                        .map(|a| {
                            ABILITIES
                                .iter()
                                .filter_map(|ab| Some(format!("{} {}", ability_full(ab), a.get(*ab)?.as_i64()?)))
                                .collect::<Vec<_>>()
                                .join(" and ")
                        })
                        .collect::<Vec<_>>()
                        .join(" or "),
                    ("spellcasting", Value::Bool(true)) => "The ability to cast at least one spell".to_string(),
                    ("spellcasting2020", Value::Bool(true)) => "Spellcasting or Pact Magic feature".to_string(),
                    ("spellcastingFeature", Value::Bool(true)) => "Spellcasting feature".to_string(),
                    ("spellcastingPrepared", Value::Bool(true)) => {
                        "Spellcasting feature that prepares spells".to_string()
                    }
                    ("psionics", Value::Bool(true)) => "Psionic Talent feature".to_string(),
                    ("proficiency", Value::Array(p)) => {
                        format!(
                            "Proficiency with {}",
                            p.iter().map(proficiency_requirement).collect::<Vec<_>>().join(" or ")
                        )
                    }
                    ("feat", f) => tagged_list(f, "feat"),
                    ("optionalfeature", f) => tagged_list(f, "optfeature"),
                    ("item", f) => tagged_list(f, "item"),
                    ("spell", Value::Array(sp)) => sp
                        .iter()
                        .filter_map(Value::as_str)
                        .map(|s| format!("{{@spell {}}}", s.split('#').next().unwrap_or(s)))
                        .collect::<Vec<_>>()
                        .join(", "),
                    ("background", Value::Array(b)) => b
                        .iter()
                        .filter_map(|b| str_at(b.as_object()?, "name"))
                        .map(|n| format!("{{@background {n}}}"))
                        .collect::<Vec<_>>()
                        .join(" or "),
                    ("alignment", a @ Value::Array(_)) => alignment_full(a),
                    ("other", Value::String(s)) => s.clone(),
                    ("otherSummary", Value::Object(s)) => str_at(s, "entrySummary").unwrap_or_default().to_string(),
                    ("campaign", c) => strings(c).join(", "),
                    ("pact", Value::String(p)) => format!("Pact of the {p}"),
                    ("patron", Value::String(p)) => format!("{p} patron"),
                    _ => format!("{}: {}", humanize_key(key), describe(val)),
                })
                .filter(|t| !t.is_empty())
                .collect();
            parts.join(", ")
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("; or ")
}

fn proficiency_requirement(v: &Value) -> String {
    let Some(o) = v.as_object() else { return describe(v) };
    o.iter()
        .map(|(k, val)| match val {
            Value::Object(kinds) => format!("{} {k}", kinds.keys().cloned().collect::<Vec<_>>().join("/")),
            Value::String(s) => format!("{k}: {s}"),
            _ => k.clone(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}
