use dioxus::prelude::*;
use serde_json::Value;

use super::{divider, prop_line};
use crate::model::Action;
use crate::render::{render_entries, RenderCtx};
use crate::statblock::format::*;

pub fn render(a: &Action, header: Element, ctx: RenderCtx) -> Element {
    let time = a.time.as_ref().map(format_time).unwrap_or_default();
    let see_also: Vec<String> = strings_at(&a.extra, "seeAlsoAction")
        .into_iter()
        .map(|n| format!("{{@action {n}}}"))
        .collect();
    let variant = str_at(&a.extra, "fromVariant")
        .map(|v| format!("{{@variantrule {v}}}"))
        .unwrap_or_default();
    rsx! {
        div { class: "text-sm",
            {header}
            {prop_line("Time:", &time, ctx)}
            {prop_line("Optional Rule:", &variant, ctx)}
            {divider()}
            {render_entries(&a.entries, ctx)}
            {prop_line("See also:", &see_also.join(", "), ctx)}
        }
    }
}

/// `[{ number: 1, unit: "action" }, "Free"]` -> "1 Action or Free".
fn format_time(v: &Value) -> String {
    let Some(items) = v.as_array() else {
        return String::new();
    };
    items
        .iter()
        .map(|t| match t {
            Value::String(s) => s.clone(),
            Value::Object(o) => {
                let number = i64_at(o, "number").unwrap_or(1);
                let unit = str_at(o, "unit").unwrap_or("");
                match unit {
                    "bonus" => format!("{number} Bonus Action"),
                    "action" | "reaction" => format!("{number} {}", plural(number, &capitalize(unit))),
                    other => format!("{number} {}", plural(number, other)),
                }
            }
            _ => String::new(),
        })
        .collect::<Vec<_>>()
        .join(" or ")
}
