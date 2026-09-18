use dioxus::prelude::*;
use serde_json::{Map, Value};

use super::{divider, prop_line, subtitle};
use crate::model::Entry;
use crate::model::Race;
use crate::render::{render_cell, render_entries, RenderCtx};
use crate::statblock::format::*;
use crate::statblock::meta::ability_bonus_text;

pub fn render(race: &Race, header: Element, ctx: RenderCtx) -> Element {
    let extra = &race.extra;
    let ability = race.ability.as_ref().map(ability_bonus_text).unwrap_or_default();
    let creature_type = creature_types_text(extra);
    let size = if race.size.is_empty() {
        String::new()
    } else {
        format_size(&race.size)
    };
    let speed = race.speed.as_ref().map(format_speed).unwrap_or_default();
    let parent = match (str_at(extra, "raceName"), str_at(extra, "raceSource")) {
        (Some(name), Some(source)) => format!("Subrace of {{@race {name}|{source}}}"),
        _ => String::new(),
    };
    let is_npc_race = strings_at(extra, "traitTags").contains(&"NPC Race");
    rsx! {
        div { class: "text-sm",
            {header}
            {subtitle(&parent, ctx)}
            {override_or_line("Ability Scores:", extra.get("abilityEntry"), &ability, ctx)}
            {prop_line("Creature Type:", &creature_type, ctx)}
            {override_or_line("Size:", extra.get("sizeEntry"), &size, ctx)}
            {override_or_line("Speed:", extra.get("speedEntry"), &speed, ctx)}
            {divider()}
            {render_entries(&race.entries, ctx)}
            if is_npc_race {
                p { class: "text-xs italic text-gray-500",
                    "Note: This race is only listed as an option for creating NPCs. It is not designed for use as a playable race."
                }
            }
            if let Some(Value::Object(hw)) = extra.get("heightAndWeight") {
                {height_and_weight(hw, ctx)}
            }
        }
    }
}

/// A stat line, or the data's own prose replacement (`sizeEntry`, ...)
/// where it supplies one.
fn override_or_line(label: &str, entry: Option<&Value>, text: &str, ctx: RenderCtx) -> Element {
    match entry {
        Some(Value::Object(o)) => {
            let body = str_at(o, "entry").unwrap_or_default();
            crate::statblock::prop_line(label, body, ctx)
        }
        Some(Value::String(s)) => crate::statblock::prop_line(label, s, ctx),
        _ => prop_line(label, text, ctx),
    }
}

fn creature_types_text(extra: &Map<String, Value>) -> String {
    let Some(Value::Array(types)) = extra.get("creatureTypes") else {
        return String::new();
    };
    let parts: Vec<String> = types
        .iter()
        .map(|t| match t {
            Value::String(s) => title_case(s),
            Value::Object(o) => {
                let mut choices: Vec<String> = strings_at(o, "choose").into_iter().map(title_case).collect();
                choices.sort();
                join_conjunct(&choices, "or")
            }
            _ => String::new(),
        })
        .collect();
    join_conjunct(&parts, "and")
}

/// Inches as feet and inches: 53 -> 4' 5".
fn feet_inches(inches: i64) -> String {
    format!("{}' {}\"", inches / 12, inches % 12)
}

/// The "Height and Weight" table: base values plus the dice added on top.
fn height_and_weight(hw: &Map<String, Value>, ctx: RenderCtx) -> Element {
    let base_height = i64_at(hw, "baseHeight").map(feet_inches).unwrap_or_default();
    let base_weight = i64_at(hw, "baseWeight").map(|w| format!("{w} lb.")).unwrap_or_default();
    let height_mod = str_at(hw, "heightMod")
        .map(|m| format!("+{{@dice {m}}} in."))
        .unwrap_or_default();
    let weight_mod = str_at(hw, "weightMod")
        .map(|m| format!("× ({{@dice {m}}}) lb."))
        .unwrap_or_default();
    let cells: Vec<Entry> = [base_height, base_weight, height_mod, weight_mod]
        .into_iter()
        .map(Entry::String)
        .collect();
    rsx! {
        table { class: "mt-1 w-full border-collapse text-center text-xs",
            caption { class: "text-left font-semibold", "Height and Weight" }
            thead {
                tr {
                    for label in ["Base Height", "Base Weight", "Height Modifier", "Weight Modifier"] {
                        th { key: "{label}", class: "border-b border-gray-300 px-1", "{label}" }
                    }
                }
            }
            tbody {
                tr {
                    for (i , cell) in cells.iter().enumerate() {
                        td { key: "{i}", class: "px-1", {render_cell(cell, ctx)} }
                    }
                }
            }
        }
    }
}
