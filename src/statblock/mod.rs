mod action;
mod background;
mod class;
mod feat;
pub mod format;
mod hazard;
mod item;
pub mod meta;
mod monster;
mod object;
mod other;
mod race;
mod spell;

use dioxus::prelude::*;

use crate::data::EntityRef;
use crate::render::{render_entries, render_inline, RenderCtx};
use crate::routes::EntityKind;
use crate::statblock::format::{i64_at, str_at};

pub use monster::render_monster_body;

/// A `Label text` line whose text may contain `{@tag}` markup; renders
/// nothing for an empty text.
pub fn prop_line(label: &str, text: &str, ctx: RenderCtx) -> Element {
    if text.is_empty() {
        return rsx! {};
    }
    rsx! {
        p { class: "leading-snug",
            strong { class: "font-semibold", "{label} " }
            {render_inline(text, ctx)}
        }
    }
}

/// A thin rule between a card's stat lines and its body text.
pub fn divider() -> Element {
    rsx! { div { class: "my-1 border-t border-gray-300" } }
}

/// The italic subtitle under a card's name.
pub fn subtitle(text: &str, ctx: RenderCtx) -> Element {
    if text.is_empty() {
        return rsx! {};
    }
    rsx! { p { class: "mb-1 italic", {render_inline(text, ctx)} } }
}

/// Dispatches to each kind's dedicated layout, threading through a shared
/// name header so every kind renders it inside the same single box as the
/// rest of its content rather than as a separate box
/// above it. The source is shown as a tag on the card itself (see
/// `crate::card`), not repeated here. Clicking the name opens this entity
/// in the detail view.
pub fn render(entity: &EntityRef, kind: EntityKind, name: &str, ctx: RenderCtx) -> Element {
    let header = rsx! {
        h3 { class: "mb-1 text-base font-bold leading-tight",
            {crate::card::title_link(kind, entity.source(), name, ctx)}
            {crate::card::kind_source_tags(kind, entity.source(), ctx.library)}
        }
    };
    let body = match entity {
        EntityRef::Monster(m) => monster::render(m, header, ctx),
        EntityRef::Spell(s) => spell::render(s, header, ctx),
        EntityRef::Item(i) => item::render(i, header, ctx),
        EntityRef::Class(c) => class::render(c, header, ctx),
        EntityRef::Race(r) => race::render(r, header, ctx),
        EntityRef::Background(b) => background::render(b, header, ctx),
        EntityRef::Feat(f) => feat::render(f, header, ctx),
        EntityRef::Condition(c) => rsx! {
            div { class: "text-sm",
                {header}
                {render_entries(&c.entries, ctx)}
            }
        },
        EntityRef::Object(o) => object::render(o, header, ctx),
        EntityRef::Action(a) => action::render(a, header, ctx),
        EntityRef::Hazard(h) => hazard::render(h, header, ctx),
        EntityRef::Other(kind, o) => other::render(*kind, o, header, ctx),
    };
    rsx! {
        {body}
        {source_footer(entity, ctx)}
    }
}

/// Where else the entity appears: its page, other printings, and what it
/// was reprinted as (the newer edition), the way 5etools' page line does.
fn source_footer(entity: &EntityRef, ctx: RenderCtx) -> Element {
    let extra = entity.extra();
    let kind_source = ctx.library.source_full_name(entity.source()).unwrap_or(entity.source());
    let page = entity_page(entity).map(|p| format!(", page {p}")).unwrap_or_default();
    let others: Vec<String> = extra
        .get("otherSources")
        .or_else(|| extra.get("additionalSources"))
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|s| {
                    let source = str_at(s.as_object()?, "source")?;
                    let name = ctx.library.source_full_name(source).unwrap_or(source);
                    Some(match i64_at(s.as_object()?, "page") {
                        Some(p) => format!("{name}, page {p}"),
                        None => name.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let reprinted: Vec<String> = crate::statblock::format::strings_at(extra, "reprintedAs")
        .into_iter()
        .map(|uid| {
            let mut parts = uid.split('|');
            let name = parts.next().unwrap_or("");
            let source = parts.next().unwrap_or("");
            format!("{{@{} {name}|{source}|{name} ({source})}}", entity_tag(entity))
        })
        .collect();
    rsx! {
        div { class: "mt-2 border-t border-gray-100 pt-1 text-[11px] leading-snug text-gray-500",
            p { "{kind_source}{page}" }
            if !others.is_empty() {
                p { "Also in: {others.join(\"; \")}" }
            }
            if !reprinted.is_empty() {
                p {
                    "Reprinted as: "
                    {render_inline(&reprinted.join(", "), ctx)}
                }
            }
        }
    }
}

fn entity_page(entity: &EntityRef) -> Option<u32> {
    match entity {
        EntityRef::Monster(e) => e.page,
        EntityRef::Spell(e) => e.page,
        EntityRef::Item(e) => e.page,
        EntityRef::Class(e) => e.page,
        EntityRef::Race(e) => e.page,
        EntityRef::Background(e) => e.page,
        EntityRef::Feat(e) => e.page,
        EntityRef::Condition(e) => e.page,
        EntityRef::Object(e) => e.page,
        EntityRef::Action(e) => e.page,
        EntityRef::Hazard(e) => e.page,
        EntityRef::Other(_, e) => e.page,
    }
}

/// The inline tag that links to an entity of the same kind as `entity`.
fn entity_tag(entity: &EntityRef) -> &'static str {
    match entity {
        EntityRef::Monster(_) => "creature",
        EntityRef::Spell(_) => "spell",
        EntityRef::Item(_) => "item",
        EntityRef::Class(_) => "class",
        EntityRef::Race(_) => "race",
        EntityRef::Background(_) => "background",
        EntityRef::Feat(_) => "feat",
        EntityRef::Condition(_) => "condition",
        EntityRef::Object(_) => "object",
        EntityRef::Action(_) => "action",
        EntityRef::Hazard(_) => "hazard",
        EntityRef::Other(kind, _) => other::tag_for(*kind),
    }
}
