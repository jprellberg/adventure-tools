use dioxus::prelude::*;

use super::{divider, prop_line};
use crate::model::Background;
use crate::render::{render_entries, RenderCtx};
use crate::statblock::meta::prerequisite_text;

/// A background's own `entries` already spell out its ability scores, feat,
/// proficiencies and equipment (as a hanging list), so the structured copies
/// of those fields are not repeated; only a prerequisite is added.
pub fn render(bg: &Background, header: Element, ctx: RenderCtx) -> Element {
    let prerequisite = bg.extra.get("prerequisite").map(prerequisite_text).unwrap_or_default();
    rsx! {
        div { class: "text-sm",
            {header}
            {prop_line("Prerequisite:", &prerequisite, ctx)}
            {divider()}
            {render_entries(&bg.entries, ctx)}
        }
    }
}
