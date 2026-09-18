//! Formatting helpers shared across statblock layouts: turns the compact
//! codes and polymorphic JSON shapes of the 5etools data (`"M"`, `"L"`,
//! `{ "walk": 30, "fly": 60 }`, `["C", "E"]`, ...) into display text. Most
//! return strings that may still contain `{@tag}` markup, which callers
//! run through `render_inline`.

use serde_json::{Map, Value};

pub fn str_at<'a>(map: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    map.get(key).and_then(Value::as_str)
}

pub fn i64_at(map: &Map<String, Value>, key: &str) -> Option<i64> {
    map.get(key).and_then(Value::as_i64)
}

pub fn bool_at(map: &Map<String, Value>, key: &str) -> bool {
    map.get(key).and_then(Value::as_bool).unwrap_or(false)
}

/// The strings of an array value, skipping anything else.
pub fn strings(v: &Value) -> Vec<&str> {
    v.as_array()
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

pub fn strings_at<'a>(map: &'a Map<String, Value>, key: &str) -> Vec<&'a str> {
    map.get(key).map(strings).unwrap_or_default()
}

pub fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

const SMALL_WORDS: &[&str] = &[
    "a", "an", "the", "and", "as", "at", "but", "by", "for", "from", "in", "nor", "of", "on", "or", "to", "with",
];

/// Title case that leaves short connecting words lowercase: "ring of spell storing".
pub fn title_case(s: &str) -> String {
    s.split(' ')
        .enumerate()
        .map(|(i, w)| {
            if i > 0 && SMALL_WORDS.contains(&w) {
                w.to_string()
            } else {
                capitalize(w)
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// "a, or b" / "a, b, or c": 5etools' comma-before-or list style.
fn join_with_or(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [init @ .., last] => format!("{}, or {last}", init.join(", ")),
    }
}

/// "a, b, and c" / "a or b" style lists.
pub fn join_conjunct(items: &[String], conjunction: &str) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [a, b] => format!("{a} {conjunction} {b}"),
        [init @ .., last] => format!("{}, {conjunction} {last}", init.join(", ")),
    }
}

pub fn ordinal(n: i64) -> String {
    let suffix = match (n % 100, n % 10) {
        (11..=13, _) => "th",
        (_, 1) => "st",
        (_, 2) => "nd",
        (_, 3) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

/// "+3" / "-1".
pub fn signed(n: i64) -> String {
    if n >= 0 {
        format!("+{n}")
    } else {
        n.to_string()
    }
}

pub fn plural(n: i64, singular: &str) -> String {
    if n == 1 {
        singular.to_string()
    } else {
        format!("{singular}s")
    }
}

/// A coin amount given in copper pieces, in the largest denomination that
/// divides it evenly ("1,500 gp", "5 sp", "3 cp").
pub fn format_coins(copper: f64) -> String {
    if copper >= 100.0 && copper % 100.0 == 0.0 {
        format!("{} gp", thousands((copper / 100.0) as i64))
    } else if copper >= 10.0 && copper % 10.0 == 0.0 {
        format!("{} sp", copper / 10.0)
    } else {
        format!("{copper} cp")
    }
}

/// Weight in pounds, with tons for very heavy things.
pub fn format_weight(lbs: f64) -> String {
    let tons = (lbs / 2000.0).floor();
    let rest = lbs - tons * 2000.0;
    let mut parts = Vec::new();
    if tons > 0.0 {
        parts.push(format!("{tons} {}", plural(tons as i64, "ton")));
    }
    if rest > 0.0 || parts.is_empty() {
        parts.push(format!("{rest} lb."));
    }
    parts.join(", ")
}

pub const ABILITIES: [&str; 6] = ["str", "dex", "con", "int", "wis", "cha"];

pub fn ability_full(abv: &str) -> &str {
    match abv {
        "str" => "Strength",
        "dex" => "Dexterity",
        "con" => "Constitution",
        "int" => "Intelligence",
        "wis" => "Wisdom",
        "cha" => "Charisma",
        "spellcasting" => "Spellcasting",
        other => other,
    }
}

pub fn ability_mod(score: i32) -> i64 {
    (score as i64 - 10).div_euclid(2)
}

pub fn size_label(code: &str) -> &'static str {
    match code {
        "T" => "Tiny",
        "S" => "Small",
        "M" => "Medium",
        "L" => "Large",
        "H" => "Huge",
        "G" => "Gargantuan",
        "F" => "Fine",
        "D" => "Diminutive",
        "C" => "Colossal",
        "V" => "Varies",
        _ => "Unknown size",
    }
}

pub fn format_size(sizes: &[String]) -> String {
    let labels: Vec<String> = sizes.iter().map(|s| size_label(s).to_string()).collect();
    join_conjunct(&labels, "or")
}

fn monster_type_plural(t: &str) -> String {
    match t {
        "dragon" | "beast" | "monstrosity" | "giant" | "fey" | "plant" | "ooze" | "fiend" | "elemental"
        | "construct" | "celestial" | "aberration" | "undead" | "humanoid" => format!("{t}s"),
        other => other.to_string(),
    }
}

/// A monster's `type`: a plain string, or `{ type, tags, swarmSize, note,
/// sidekickType, ... }` (5etools' `Parser.monTypeToFullObj`), e.g.
/// "Dragon (Chromatic)" or "swarm of Tiny Beasts".
pub fn monster_type(v: &Value) -> String {
    let Value::Object(map) = v else {
        return v.as_str().map(title_case).unwrap_or_default();
    };
    let types: Vec<String> = match map.get("type") {
        Some(Value::Object(choose)) => strings_at(choose, "choose").into_iter().map(title_case).collect(),
        Some(Value::String(t)) => vec![title_case(t)],
        _ => Vec::new(),
    };
    let mut text = match str_at(map, "swarmSize") {
        Some(size) => format!(
            "swarm of {} {}",
            size_label(size),
            join_conjunct(
                &types
                    .iter()
                    .map(|t| title_case(&monster_type_plural(&t.to_lowercase())))
                    .collect::<Vec<_>>(),
                "or"
            )
        ),
        None => join_conjunct(&types, "or"),
    };
    let tags = type_tags(map.get("tags"));
    if !tags.is_empty() {
        text.push_str(&format!(" ({})", tags.join(", ")));
    }
    if let Some(note) = str_at(map, "note") {
        text.push(' ');
        text.push_str(note);
    }
    text
}

/// A monster type's tags, each a plain string or `{ tag, prefix, prefixHidden }`.
fn type_tags(tags: Option<&Value>) -> Vec<String> {
    tags.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|t| match t {
                    Value::String(s) => Some(title_case(s)),
                    Value::Object(o) => {
                        let tag = str_at(o, "tag")?;
                        Some(match (str_at(o, "prefix"), bool_at(o, "prefixHidden")) {
                            (Some(prefix), false) => title_case(&format!("{prefix} {tag}")),
                            _ => title_case(tag),
                        })
                    }
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The sidekick type text, when the type has one that isn't hidden.
pub fn sidekick_type(v: &Value) -> Option<String> {
    let map = v.as_object()?;
    if bool_at(map, "sidekickHidden") {
        return None;
    }
    let base = str_at(map, "sidekickType")?.to_string();
    let tags = type_tags(map.get("sidekickTags"));
    Some(if tags.is_empty() {
        base
    } else {
        format!("{base} ({})", tags.join(", "))
    })
}

fn alignment_code(code: &str) -> &str {
    match code {
        "L" => "lawful",
        "N" => "neutral",
        "NX" => "neutral (law/chaos axis)",
        "NY" => "neutral (good/evil axis)",
        "C" => "chaotic",
        "G" => "good",
        "E" => "evil",
        "U" => "unaligned",
        "A" => "any alignment",
        other => other,
    }
}

/// `["C", "E"]` -> "chaotic evil", `["L", "NX", "C", "NY", "E"]`-style
/// sets -> "any non-good alignment", and the object forms with notes,
/// chances and specials (`Parser.alignmentListToFull`).
pub fn alignment_full(v: &Value) -> String {
    let Value::Array(list) = v else {
        return String::new();
    };
    if list.iter().any(Value::is_object) {
        return list
            .iter()
            .filter_map(Value::as_object)
            .filter(|o| o.get("alignment") != Some(&Value::Null))
            .map(|o| {
                if let Some(special) = str_at(o, "special") {
                    return special.to_string();
                }
                let base = o.get("alignment").map(alignment_full).unwrap_or_default();
                let chance = i64_at(o, "chance").map(|c| format!("{c}% ")).unwrap_or_default();
                let note = str_at(o, "note").map(|n| format!(" ({n})")).unwrap_or_default();
                format!("{chance}{base}{note}")
            })
            .collect::<Vec<_>>()
            .join(" or ");
    }
    let codes: Vec<&str> = list.iter().filter_map(Value::as_str).collect();
    let has = |c: &str| codes.contains(&c);
    match codes.len() {
        1 => alignment_code(codes[0]).to_string(),
        2 => codes.iter().map(|c| alignment_code(c)).collect::<Vec<_>>().join(" "),
        3 if has("NX") && has("NY") && has("N") => "any neutral alignment".to_string(),
        5 if !has("G") => "any non-good alignment".to_string(),
        5 if !has("E") => "any non-evil alignment".to_string(),
        5 if !has("L") => "any non-lawful alignment".to_string(),
        5 if !has("C") => "any non-chaotic alignment".to_string(),
        4 if !has("L") && !has("NX") => "any chaotic alignment".to_string(),
        4 if !has("G") && !has("NY") => "any evil alignment".to_string(),
        4 if !has("C") && !has("NX") => "any lawful alignment".to_string(),
        4 if !has("E") && !has("NY") => "any good alignment".to_string(),
        _ => codes.iter().map(|c| alignment_code(c)).collect::<Vec<_>>().join(" "),
    }
}

const XP_BY_CR: [(&str, i64); 34] = [
    ("0", 10),
    ("1/8", 25),
    ("1/4", 50),
    ("1/2", 100),
    ("1", 200),
    ("2", 450),
    ("3", 700),
    ("4", 1100),
    ("5", 1800),
    ("6", 2300),
    ("7", 2900),
    ("8", 3900),
    ("9", 5000),
    ("10", 5900),
    ("11", 7200),
    ("12", 8400),
    ("13", 10000),
    ("14", 11500),
    ("15", 13000),
    ("16", 15000),
    ("17", 18000),
    ("18", 20000),
    ("19", 22000),
    ("20", 25000),
    ("21", 33000),
    ("22", 41000),
    ("23", 50000),
    ("24", 62000),
    ("25", 75000),
    ("26", 90000),
    ("27", 105000),
    ("28", 120000),
    ("29", 135000),
    ("30", 155000),
];

pub fn cr_to_xp(cr: &str) -> Option<i64> {
    XP_BY_CR.iter().find(|(c, _)| *c == cr).map(|(_, xp)| *xp)
}

fn cr_to_number(cr: &str) -> Option<f64> {
    match cr.split_once('/') {
        Some((n, d)) => Some(n.parse::<f64>().ok()? / d.parse::<f64>().ok()?),
        None => cr.parse().ok(),
    }
}

/// Proficiency bonus for a challenge rating.
pub fn cr_to_pb(cr: &str) -> Option<i64> {
    let n = cr_to_number(cr)?;
    Some(if n < 5.0 { 2 } else { (n / 4.0).ceil() as i64 + 1 })
}

/// A number with thousands separators.
pub fn thousands(n: i64) -> String {
    let digits = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 {
        format!("-{out}")
    } else {
        out
    }
}

/// 2024-style challenge line: "17 (XP 18,000, or 20,000 in lair; PB +6)".
pub fn format_cr(cr: &Value, pb_note: Option<&str>, is_mythic: bool) -> String {
    let (base, xp_override, lair, xp_lair, coven, xp_coven) = match cr {
        Value::Object(o) => (
            str_at(o, "cr").unwrap_or("").to_string(),
            i64_at(o, "xp"),
            str_at(o, "lair").map(String::from),
            i64_at(o, "xpLair"),
            str_at(o, "coven").map(String::from),
            i64_at(o, "xpCoven"),
        ),
        Value::String(s) => (s.clone(), None, None, None, None, None),
        Value::Number(n) => (n.to_string(), None, None, None, None, None),
        _ => return "—".to_string(),
    };
    let xp_base = xp_override.or_else(|| cr_to_xp(&base));
    let mut xps = Vec::new();
    if let Some(xp) = xp_base {
        xps.push(thousands(xp));
        if is_mythic {
            xps.push(format!("{} as a mythic encounter", thousands(xp * 2)));
        }
    }
    if lair.is_some() || xp_lair.is_some() {
        if let Some(xp) = xp_lair.or_else(|| lair.as_deref().and_then(cr_to_xp)) {
            xps.push(format!("{} in lair", thousands(xp)));
        }
    }
    if coven.is_some() || xp_coven.is_some() {
        if let Some(xp) = xp_coven.or_else(|| coven.as_deref().and_then(cr_to_xp)) {
            xps.push(format!("{} when part of a coven", thousands(xp)));
        }
    }
    let pb = pb_note.map(String::from).or_else(|| cr_to_pb(&base).map(signed));
    let mut parens = Vec::new();
    if !xps.is_empty() {
        parens.push(format!("XP {}", join_with_or(&xps)));
    }
    if let Some(pb) = pb {
        parens.push(format!("PB {pb}"));
    }
    let shown = if base.is_empty() { "None" } else { &base };
    if parens.is_empty() {
        shown.to_string()
    } else {
        format!("{shown} ({})", parens.join("; "))
    }
}

/// `[12]`, `[{ ac: 17, from: ["natural armor"] }, { ac: 19, condition: "..." }]`
/// or `{ special: "..." }` entries -> "12", "17 (natural armor), 19 ..."
/// (5etools' `Parser.acToFull`, including its bracing of grouped entries).
pub fn format_ac(v: &Value) -> String {
    let Value::Array(items) = v else {
        return match v {
            Value::Number(n) => n.to_string(),
            Value::String(s) => s.clone(),
            _ => String::new(),
        };
    };
    let braces_of = |v: Option<&Value>| {
        v.and_then(|v| v.get("braces"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
    };
    let mut out = String::new();
    let mut in_braces = false;
    for (i, cur) in items.iter().enumerate() {
        let next = items.get(i + 1);
        match cur {
            Value::Object(o) if o.contains_key("special") => {
                in_braces = false;
                out.push_str(str_at(o, "special").unwrap_or_default());
            }
            Value::Object(o) if o.contains_key("ac") => {
                let braces = bool_at(o, "braces");
                let next_braces = braces_of(next);
                if !in_braces && braces {
                    out.push('(');
                    in_braces = true;
                }
                out.push_str(&i64_at(o, "ac").unwrap_or(0).to_string());
                let from = strings_at(o, "from");
                if !from.is_empty() {
                    out.push_str(if braces || !in_braces { " (" } else { "; " });
                    in_braces = true;
                    out.push_str(&from.join(", "));
                    if braces {
                        out.push(')');
                    } else if !next_braces {
                        out.push(')');
                        in_braces = false;
                    }
                }
                if let Some(cond) = str_at(o, "condition") {
                    out.push(' ');
                    out.push_str(cond);
                }
                if in_braces && !next_braces {
                    out.push(')');
                    in_braces = false;
                }
            }
            Value::Number(n) => out.push_str(&n.to_string()),
            Value::String(s) => out.push_str(s),
            _ => {}
        }
        if next.is_some() {
            if braces_of(next) {
                out.push_str(if in_braces { "; " } else { " (" });
                in_braces = true;
            } else {
                out.push_str(", ");
            }
        }
    }
    if in_braces {
        out.push(')');
    }
    out.trim().to_string()
}

const SPEED_MODES: [&str; 5] = ["walk", "burrow", "climb", "fly", "swim"];

/// A speed object (`{ walk: 30, fly: { number: 60, condition: "(hover)" } }`)
/// or a bare number -> "30 ft., Fly 60 ft. (hover)".
pub fn format_speed(v: &Value) -> String {
    let Value::Object(map) = v else {
        return match v {
            Value::Number(n) => format!("{n} ft."),
            Value::String(s) => s.clone(),
            _ => "—".to_string(),
        };
    };
    let hidden = strings_at(map, "hidden");
    let mut parts = Vec::new();
    for mode in SPEED_MODES.into_iter().filter(|m| !hidden.contains(m)) {
        let speed = map.get(mode);
        if speed.is_some() || mode == "walk" {
            parts.push(speed_part(mode, speed.unwrap_or(&Value::Null)));
        }
        if let Some(Value::Array(alt)) = map.get("alternate").and_then(|a| a.get(mode)) {
            parts.extend(alt.iter().map(|s| speed_part(mode, s)));
        }
    }
    let mut joiner = ", ";
    if let Some(Value::Object(choose)) = map.get("choose") {
        joiner = "; ";
        let from: Vec<String> = strings_at(choose, "from").into_iter().map(capitalize).collect();
        let amount = i64_at(choose, "amount").unwrap_or(0);
        let note = str_at(choose, "note").map(|n| format!(" {n}")).unwrap_or_default();
        parts.push(format!("{} {amount} ft.{note}", join_conjunct(&from, "or")));
    }
    let mut text = parts.join(joiner);
    if let Some(note) = str_at(map, "note") {
        text.push(' ');
        text.push_str(note);
    }
    text
}

fn speed_part(mode: &str, speed: &Value) -> String {
    let name = if mode == "walk" {
        String::new()
    } else {
        format!("{} ", capitalize(mode))
    };
    let (value, condition) = match speed {
        Value::Bool(true) => (
            if mode == "walk" {
                "0 ft.".to_string()
            } else {
                "equal to your walking speed".to_string()
            },
            None,
        ),
        Value::Number(n) => (format!("{n} ft."), None),
        Value::Object(o) => (
            format!("{} ft.", i64_at(o, "number").unwrap_or(0)),
            str_at(o, "condition"),
        ),
        _ => ("0 ft.".to_string(), None),
    };
    match condition {
        Some(c) => format!("{name}{value} {c}"),
        None => format!("{name}{value}"),
    }
}

pub fn dmg_type_full(code: &str) -> &str {
    match code {
        "A" => "acid",
        "B" => "bludgeoning",
        "C" => "cold",
        "F" => "fire",
        "O" => "force",
        "L" => "lightning",
        "N" => "necrotic",
        "P" => "piercing",
        "I" => "poison",
        "Y" => "psychic",
        "R" => "radiant",
        "S" => "slashing",
        "T" => "thunder",
        other => other,
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum ImmResMode {
    Damage,
    Condition,
}

/// Immunity / resistance / vulnerability lists (`Parser.getFullImmRes`):
/// plain strings, `{ special }`, and grouped `{ immune: [...], note,
/// preNote }` objects. Title-cased like the 2024 statblocks; condition
/// lists come back with `{@condition}` links.
pub fn format_imm_res(values: &Value, mode: ImmResMode) -> String {
    let Value::Array(list) = values else {
        return String::new();
    };
    imm_res_list(list, mode, false)
}

fn imm_res_is_simple(v: &Value) -> bool {
    match v {
        Value::String(_) => true,
        Value::Object(o) => o.contains_key("special") || imm_res_group(o).is_none(),
        _ => true,
    }
}

fn imm_res_group(o: &Map<String, Value>) -> Option<&Value> {
    ["immune", "resist", "vulnerable", "conditionImmune"]
        .iter()
        .find_map(|k| o.get(*k))
}

fn imm_res_list(list: &[Value], mode: ImmResMode, is_group: bool) -> String {
    let mut out = String::new();
    for (i, val) in list.iter().enumerate() {
        let simple = imm_res_is_simple(val);
        let text = match val {
            Value::String(s) => {
                let t = title_case(s);
                if mode == ImmResMode::Condition {
                    format!("{{@condition {t}}}")
                } else {
                    t
                }
            }
            Value::Object(o) if simple => str_at(o, "special").unwrap_or_default().to_string(),
            Value::Object(o) => {
                let mut parts = Vec::new();
                if let Some(pre) = str_at(o, "preNote") {
                    parts.push(pre.to_string());
                }
                if let Some(Value::Array(inner)) = imm_res_group(o) {
                    parts.push(imm_res_list(inner, mode, true));
                }
                if let Some(note) = str_at(o, "note") {
                    parts.push(note.to_string());
                }
                parts.join(" ")
            }
            _ => String::new(),
        };
        out.push_str(&text);
        if i + 1 == list.len() {
            break;
        }
        let next_simple = imm_res_is_simple(&list[i + 1]);
        if !simple || !next_simple {
            out.push_str("; ");
        } else if !is_group || i + 2 != list.len() || list.len() < 2 {
            out.push_str(", ");
        } else if list.len() == 2 {
            out.push_str(" and ");
        } else {
            out.push_str(", and ");
        }
    }
    out
}
