use dioxus::prelude::*;
use serde_json::Value;

use super::{divider, prop_line, subtitle};
use crate::model::{Entry, Hazard};
use crate::render::{render_entries, render_lead_in_block, RenderCtx};
use crate::statblock::format::*;
use crate::statblock::meta::describe;

fn type_label(code: &str) -> &'static str {
    match code {
        "MECH" => "Mechanical Trap",
        "MAG" => "Magical Trap",
        "SMPL" => "Simple Trap",
        "CMPX" => "Complex Trap",
        "TRP" => "Trap",
        "HAUNT" => "Haunt",
        "WLD" => "Wilderness Hazard",
        "ENV" => "Environmental Hazard",
        "WTH" => "Weather Hazard",
        "EST" => "Eldritch Storm",
        "GEN" => "Generic Hazard",
        _ => "Trap",
    }
}

pub fn render(h: &Hazard, header: Element, ctx: RenderCtx) -> Element {
    let extra = &h.extra;
    let kind = type_label(h.trap_haz_type.as_deref().unwrap_or(""));
    let type_line = format!("{kind}{}", format_rating(&h.rating));
    let initiative = i64_at(extra, "initiative").map(|n| {
        let note = str_at(extra, "initiativeNote")
            .map(|n| format!(" ({n})"))
            .unwrap_or_default();
        format!("{n}{note}")
    });
    let duration = extra.get("duration").map(describe).unwrap_or_default();
    let haunt = extra
        .get("hauntBonus")
        .map(|b| format!("{} to attack rolls, saving throws and checks", describe(b)))
        .unwrap_or_default();
    let sections: Vec<(&str, &str)> = vec![
        ("Trigger", "trigger"),
        ("Effect", "effect"),
        ("Active Elements", "eActive"),
        ("Dynamic Elements", "eDynamic"),
        ("Constant Elements", "eConstant"),
        ("Countermeasures", "countermeasures"),
    ];
    rsx! {
        div { class: "text-sm",
            {header}
            {subtitle(&type_line, ctx)}
            {prop_line("Initiative Count:", &initiative.unwrap_or_default(), ctx)}
            {prop_line("Duration:", &duration, ctx)}
            {prop_line("Haunt Bonus:", &haunt, ctx)}
            {divider()}
            {render_entries(&h.entries, ctx)}
            for (title , key) in sections {
                if let Some(Value::Array(entries)) = extra.get(key) {
                    div { key: "{key}",
                        {render_lead_in_block(Some(title), &to_entries(entries), ctx.with_depth(2))}
                    }
                }
            }
        }
    }
}

fn to_entries(values: &[Value]) -> Vec<Entry> {
    values.iter().cloned().map(Entry::from_value).collect()
}

fn format_rating(rating: &[Value]) -> String {
    let parts: Vec<String> = rating
        .iter()
        .filter_map(|r| {
            let tier = r.get("tier").and_then(Value::as_i64)?;
            let threat = r.get("threat").and_then(Value::as_str).unwrap_or("");
            Some(format!("tier {tier}, {threat} threat"))
        })
        .collect();
    if parts.is_empty() {
        String::new()
    } else {
        format!(" ({})", parts.join("; "))
    }
}
