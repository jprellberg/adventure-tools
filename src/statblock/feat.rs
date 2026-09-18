use dioxus::prelude::*;

use super::{divider, prop_line, subtitle};
use crate::model::Feat;
use crate::render::{render_entries, RenderCtx};
use crate::statblock::format::*;
use crate::statblock::meta::{ability_bonus_text, prerequisite_text};

fn category_label(code: &str) -> &str {
    match code {
        "G" => "General Feat",
        "O" => "Origin Feat",
        "EB" => "Epic Boon",
        "FS" => "Fighting Style Feat",
        "FS:P" => "Fighting Style Feat (Paladin)",
        "FS:R" => "Fighting Style Feat (Ranger)",
        "D" => "Dragonmark Feat",
        "DG" => "Dark Gift",
        other => other,
    }
}

pub fn render(feat: &Feat, header: Element, ctx: RenderCtx) -> Element {
    let extra = &feat.extra;
    let category = str_at(extra, "category").map(category_label).unwrap_or("");
    let repeatable = if bool_at(extra, "repeatable") { "Repeatable" } else { "" };
    let type_line = [category, repeatable]
        .iter()
        .filter(|s| !s.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    let prerequisite = feat.prerequisite.as_ref().map(prerequisite_text).unwrap_or_default();
    let ability = feat.ability.as_ref().map(ability_bonus_text).unwrap_or_default();
    rsx! {
        div { class: "text-sm",
            {header}
            {subtitle(&type_line, ctx)}
            {prop_line("Prerequisite:", &prerequisite, ctx)}
            {prop_line("Ability Score Increase:", &ability, ctx)}
            {divider()}
            {render_entries(&feat.entries, ctx)}
        }
    }
}
