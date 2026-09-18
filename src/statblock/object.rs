use dioxus::prelude::*;
use serde_json::Value;

use super::{divider, prop_line, subtitle};
use crate::model::object::Object;
use crate::render::{render_entries, RenderCtx};
use crate::statblock::format::*;

fn object_type_label(code: &str) -> &str {
    match code {
        "SW" => "Siege Weapon",
        "GEN" => "Generic Object",
        "U" => "Unknown",
        other => other,
    }
}

pub fn render(o: &Object, header: Element, ctx: RenderCtx) -> Element {
    let size = format_size(&o.size);
    let subtitle_text = match o.object_type.as_deref() {
        Some(t) => format!("{size} Object ({})", object_type_label(t)),
        None => format!("{size} Object"),
    };
    let ac = o.ac.as_ref().map(format_ac).unwrap_or_default();
    let hp = o.hp.as_ref().map(format_hp).unwrap_or_default();
    let speed = o
        .speed
        .as_ref()
        .map(format_speed)
        .filter(|s| s != "0 ft.")
        .unwrap_or_default();
    let extra = &o.extra;
    let imm = |key: &str, mode: ImmResMode| extra.get(key).map(|v| format_imm_res(v, mode)).unwrap_or_default();
    let immune = o
        .immune
        .as_ref()
        .map(|v| format_imm_res(v, ImmResMode::Damage))
        .unwrap_or_default();
    let abilities = [o.str, o.dex, o.con, o.int, o.wis, o.cha];
    let has_abilities = abilities.iter().any(Option::is_some);
    let cells: Vec<String> = abilities
        .iter()
        .map(|s| {
            s.map(|s| format!("{s} ({})", signed(ability_mod(s))))
                .unwrap_or_else(|| "—".into())
        })
        .collect();
    rsx! {
        div { class: "text-sm",
            {header}
            {subtitle(&subtitle_text, ctx)}
            {prop_line("AC", &ac, ctx)}
            {prop_line("HP", &hp, ctx)}
            {prop_line("Speed", &speed, ctx)}
            if has_abilities {
                table { class: "my-1 w-full text-center text-xs tabular-nums",
                    thead {
                        tr {
                            for abv in ABILITIES {
                                th { key: "{abv}", class: "uppercase", "{abv}" }
                            }
                        }
                    }
                    tbody {
                        tr {
                            for (abv , cell) in ABILITIES.iter().zip(cells) {
                                td { key: "{abv}", "{cell}" }
                            }
                        }
                    }
                }
            }
            {prop_line("Vulnerabilities", &imm("vulnerable", ImmResMode::Damage), ctx)}
            {prop_line("Resistances", &imm("resist", ImmResMode::Damage), ctx)}
            {prop_line("Immunities", &[immune, imm("conditionImmune", ImmResMode::Condition)].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join("; "), ctx)}
            {prop_line("Senses", &capitalize(&o.senses.join(", ")), ctx)}
            {divider()}
            {render_entries(&o.entries, ctx)}
            if !o.action_entries.is_empty() {
                h4 { class: "mb-1 mt-2 border-b border-gray-300 text-sm font-bold", "Actions" }
                {render_entries(&o.action_entries, ctx.with_depth(2))}
            }
        }
    }
}

fn format_hp(v: &Value) -> String {
    match v {
        Value::Number(n) => n.to_string(),
        Value::Object(map) => {
            let avg = i64_at(map, "average");
            let formula = str_at(map, "formula");
            match (avg, formula) {
                (Some(a), Some(f)) => format!("{a} ({f})"),
                (Some(a), None) => a.to_string(),
                (None, Some(f)) => f.to_string(),
                (None, None) => crate::statblock::meta::describe(v),
            }
        }
        Value::String(s) => s.clone(),
        _ => String::new(),
    }
}
