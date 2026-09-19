//! Shared rendering for the 5etools "entries" content tree - used by every
//! entity card. Inline `{@tag ...}` markup lives in [`inline`].
//!
//! Nesting depth decides how a named block presents itself, as in 5etools:
//! a `section` is depth -1, its children depth 0, and every nested
//! `entries` one deeper. Named blocks shallower than depth 2 are headings;
//! from depth 2 on the name is a bold lead-in running into the first
//! paragraph ("**Cleave.** If you hit a creature..."), which keeps a deep
//! rules outline like the weapon Mastery Properties compact.

mod inline;

use dioxus::prelude::*;
use serde_json::{Map, Value};

use crate::data::Library;
use crate::model::entry::{AttackBlock, EntriesBlock, ListBlock, TableBlock};
use crate::model::Entry;
use crate::routes::EntityKind;
use crate::statblock::format::{ability_full, capitalize, join_conjunct, str_at, strings_at};
use crate::{HoverSignal, Use2024Rules};

pub use inline::render_inline;

/// Everything the shared renderers need to turn an `entries` tree into
/// `Element`s: the library (for entity lookups), the identity of the
/// entity whose card is currently being rendered (so a link back to that
/// same entity can skip its own hover popup), the app-level hover-popup signal
/// entity links set, and the nesting depth of the entries being rendered.
/// Threaded explicitly through the render call graph, rather than pulled
/// from context, since most of these are plain functions - not components -
/// called a data-dependent number of times per render, which `use_context`
/// (a real hook) can't safely support there.
#[derive(Clone, Copy)]
pub struct RenderCtx<'a> {
    pub library: &'a Library,
    pub current: Option<(EntityKind, &'a str, &'a str)>,
    pub hover: HoverSignal,
    pub depth: i32,
    /// The 2014/2024 toggle: which edition an unsourced link resolves to.
    pub use_2024: bool,
}

impl<'a> RenderCtx<'a> {
    /// Reads the ruleset toggle from context, so a component building its
    /// context re-renders when the toggle flips.
    pub fn new(library: &'a Library, hover: HoverSignal) -> Self {
        RenderCtx {
            library,
            current: None,
            hover,
            depth: 1,
            use_2024: consume_context::<Signal<Use2024Rules>>().read().0,
        }
    }

    pub fn with_depth(self, depth: i32) -> Self {
        RenderCtx { depth, ..self }
    }
}

/// A run of sibling entries, each rendered at the context's depth.
pub fn render_entries(entries: &[Entry], ctx: RenderCtx) -> Element {
    rsx! {
        for (i , entry) in entries.iter().enumerate() {
            div { key: "{i}", {render_entry(entry, ctx)} }
        }
    }
}

/// Raw JSON entries (an entity's `lairActions`, a spellcasting header...),
/// for fields the typed model keeps as `Value`.
pub fn render_values(values: &[Value], ctx: RenderCtx) -> Element {
    let entries: Vec<Entry> = values.iter().cloned().map(Entry::from_value).collect();
    render_entries(&entries, ctx)
}

pub fn render_entry(entry: &Entry, ctx: RenderCtx) -> Element {
    match entry {
        Entry::String(s) => rsx! {
            p { class: "mb-1.5 leading-snug", {render_inline(s, ctx)} }
        },
        Entry::Block(b) => render_block(b, ctx),
        Entry::List(l) => render_list(l, ctx),
        Entry::Table(t) => render_table(t, ctx),
        Entry::Quote(q) => rsx! {
            blockquote { class: "mb-1.5 border-l-2 border-gray-300 pl-2 italic",
                {render_entries(&q.entries, ctx)}
                if q.by.is_some() || q.from.is_some() {
                    footer { class: "not-italic text-xs text-gray-500",
                        "— "
                        if let Some(by) = &q.by {
                            {render_inline(&quote_by(by), ctx)}
                        }
                        if let (Some(_), Some(from)) = (&q.by, &q.from) {
                            ", "
                            {render_inline(from, ctx)}
                        } else if let Some(from) = &q.from {
                            {render_inline(from, ctx)}
                        }
                    }
                }
            }
        },
        Entry::Attack(a) => render_attack(a, ctx),
        Entry::Other(v) => render_other(v, ctx),
    }
}

fn quote_by(by: &Value) -> String {
    match by {
        Value::Array(a) => a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", "),
        other => other.as_str().unwrap_or_default().to_string(),
    }
}

/// Block names that already end in punctuation don't get a period added.
fn lead_in(name: &str) -> String {
    if name.ends_with(['.', ',', '!', '?', ';', ':', '"']) {
        name.to_string()
    } else {
        format!("{name}.")
    }
}

/// A named block: a heading above its content for shallow depths, an inline
/// bold lead-in for deep ones (see the module docs). `item` blocks (named
/// list items) always use the lead-in form.
fn render_block(b: &EntriesBlock, ctx: RenderCtx) -> Element {
    let name = b.name.as_deref().filter(|n| !n.trim().is_empty());
    match b.kind.as_str() {
        "inset" | "insetReadaloud" | "variant" => return render_inset(b, ctx),
        "item" | "itemSub" | "actions" => return render_list_item(b, ctx),
        _ => {}
    }
    let depth = if b.kind == "section" { -1 } else { ctx.depth };
    let inner = ctx.with_depth(depth + 1);
    if depth >= 2 {
        return render_lead_in_block(name, &b.entries, inner);
    }
    let heading = match (name, depth) {
        (None, _) => rsx! {},
        (Some(name), d) if d < 0 => rsx! {
            h3 { class: "mb-1 mt-3 border-b border-gray-300 text-base font-bold leading-tight", {render_inline(name, ctx)} }
        },
        (Some(name), 0) => rsx! {
            h4 { class: "mb-1 mt-2 border-b border-gray-200 text-sm font-bold leading-tight", {render_inline(name, ctx)} }
        },
        (Some(name), _) => rsx! {
            h5 { class: "mb-0.5 mt-1.5 text-sm font-semibold leading-tight", {render_inline(name, ctx)} }
        },
    };
    rsx! {
        div {
            {heading}
            {render_entries(&b.entries, inner)}
        }
    }
}

/// "**Name.** first paragraph" followed by any remaining entries. Only
/// possible when the first entry is plain text; anything else (a list, a
/// table) can't join a `<p>` and sits below the name on its own.
pub fn render_lead_in_block(name: Option<&str>, entries: &[Entry], ctx: RenderCtx) -> Element {
    match (name, entries.split_first()) {
        (Some(name), Some((Entry::String(s), rest))) => rsx! {
            div { class: "mb-1.5",
                p { class: "leading-snug",
                    strong { class: "font-semibold", {render_inline(&lead_in(name), ctx)} }
                    " "
                    {render_inline(s, ctx)}
                }
                {render_entries(rest, ctx)}
            }
        },
        (Some(name), _) => rsx! {
            div { class: "mb-1.5",
                p { class: "leading-snug",
                    strong { class: "font-semibold", {render_inline(&lead_in(name), ctx)} }
                }
                {render_entries(entries, ctx)}
            }
        },
        (None, _) => rsx! {
            div { class: "mb-1.5", {render_entries(entries, ctx)} }
        },
    }
}

/// `item` / `itemSub` / `actions`: a named paragraph, with the name bold
/// (or italic for `itemSub`) and the text in `entry` or `entries`.
fn render_list_item(b: &EntriesBlock, ctx: RenderCtx) -> Element {
    let name_class = if b.kind == "itemSub" { "italic" } else { "font-semibold" };
    let ctx = ctx.with_depth(2);
    let (first, rest): (Option<&Entry>, &[Entry]) = match (&b.entry, b.entries.split_first()) {
        (Some(entry), _) => (Some(entry), &[]),
        (None, Some((first, rest))) => (Some(first), rest),
        (None, None) => (None, &[]),
    };
    let lead = b.name.as_deref().map(|n| {
        let text = if b.extra.get("nameDot").and_then(Value::as_bool) == Some(false) {
            n.to_string()
        } else {
            lead_in(n)
        };
        rsx! {
            span { class: "{name_class}", {render_inline(&text, ctx)} }
            " "
        }
    });
    let first_inline = first.map(|e| render_entry_inline(e, ctx));
    rsx! {
        div { class: "mb-1.5",
            p { class: "leading-snug",
                {lead}
                {first_inline}
            }
            {render_entries(rest, ctx)}
        }
    }
}

/// An entry that can sit inside a paragraph: strings and simple inline
/// values flow inline; blocks fall back to their normal rendering.
pub fn render_entry_inline(entry: &Entry, ctx: RenderCtx) -> Element {
    match entry {
        Entry::String(s) => render_inline(s, ctx),
        other => render_entry(other, ctx),
    }
}

fn render_inset(b: &EntriesBlock, ctx: RenderCtx) -> Element {
    let name = match (b.kind.as_str(), b.name.as_deref()) {
        ("variant", Some(n)) => Some(format!("Variant: {n}")),
        ("variant", None) => Some("Variant".to_string()),
        (_, n) => n.map(String::from),
    };
    let tone = if b.kind == "insetReadaloud" {
        "border-amber-300 bg-amber-50"
    } else {
        "border-gray-300 bg-gray-50"
    };
    let inner = ctx.with_depth(2);
    rsx! {
        div { class: "mb-1.5 rounded border-l-4 p-2 {tone}",
            if let Some(name) = name {
                h5 { class: "mb-0.5 text-sm font-bold leading-tight", {render_inline(&name, ctx)} }
            }
            {render_entries(&b.entries, inner)}
        }
    }
}

fn render_list(l: &ListBlock, ctx: RenderCtx) -> Element {
    let style = l.style.as_deref().unwrap_or("");
    let has = |s: &str| style.split(' ').any(|p| p == s);
    let marker = if has("list-decimal") {
        "list-decimal ml-5"
    } else if has("list-no-bullets") || has("list-hang-notitle") || has("list-hang") {
        "list-none"
    } else {
        "list-disc ml-4"
    };
    let columns = l.columns.map(|c| format!("column-count: {c};")).unwrap_or_default();
    let name = l.extra.get("name").and_then(Value::as_str);
    let items = rsx! {
        for (i , item) in l.items.iter().enumerate() {
            {render_list_entry(i, item, ctx)}
        }
    };
    let content = if has("list-decimal") {
        rsx! { ol { class: "mb-1.5 {marker}", style: "{columns}", {items} } }
    } else {
        rsx! { ul { class: "mb-1.5 {marker}", style: "{columns}", {items} } }
    };
    rsx! {
        if let Some(name) = name {
            p { class: "font-semibold", {render_inline(name, ctx)} }
        }
        {content}
    }
}

fn render_list_entry(i: usize, item: &Entry, ctx: RenderCtx) -> Element {
    match item {
        // A nested list carries its own markers; wrapping it in an `li`
        // would double them.
        Entry::List(_) => rsx! { {render_entry(item, ctx)} },
        Entry::String(s) => rsx! {
            li { key: "{i}", class: "leading-snug", {render_inline(s, ctx)} }
        },
        other => rsx! {
            li { key: "{i}", {render_entry(other, ctx)} }
        },
    }
}

fn cell_alignment(t: &TableBlock, col: usize) -> &'static str {
    let style = t.col_styles.get(col).map(String::as_str).unwrap_or("");
    if style.split(' ').any(|s| s == "text-center") {
        "text-center"
    } else if style.split(' ').any(|s| s == "text-right") {
        "text-right"
    } else {
        "text-left"
    }
}

fn render_table(t: &TableBlock, ctx: RenderCtx) -> Element {
    let ctx = ctx.with_depth(2);
    rsx! {
        div { class: "mb-1.5",
            {render_entries(&t.intro, ctx)}
            table { class: "w-full border-collapse text-xs",
                if let Some(caption) = &t.caption {
                    caption { class: "text-left font-semibold", {render_inline(caption, ctx)} }
                }
                if !t.col_labels.is_empty() {
                    thead {
                        tr {
                            for (i , label) in t.col_labels.iter().enumerate() {
                                th { key: "{i}", class: "border-b border-gray-300 px-1 py-0.5 {cell_alignment(t, i)}", {render_cell(label, ctx)} }
                            }
                        }
                    }
                }
                tbody {
                    for (i , row) in t.rows.iter().enumerate() {
                        tr { key: "{i}", class: if i % 2 == 1 { "bg-gray-50" } else { "" },
                            for (j , cell) in row.cells.iter().enumerate() {
                                td { key: "{j}", class: "border-b border-gray-100 px-1 py-0.5 align-top {cell_alignment(t, j)}", {render_cell(cell, ctx)} }
                            }
                        }
                    }
                }
                if !t.footnotes.is_empty() {
                    tfoot {
                        for (i , note) in t.footnotes.iter().enumerate() {
                            tr { key: "{i}", td { colspan: "99", class: "pt-0.5 text-[11px] text-gray-500", {render_cell(note, ctx)} } }
                        }
                    }
                }
            }
            {render_entries(&t.outro, ctx)}
        }
    }
}

/// A table cell: an ordinary entry, or `{ type: "cell", roll, entry }`
/// where `roll` gives the dice range the row covers ("1-3", "20").
pub fn render_cell(cell: &Entry, ctx: RenderCtx) -> Element {
    if let Entry::Other(Value::Object(o)) = cell {
        if str_at(o, "type") == Some("cell") {
            if let Some(entry) = o.get("entry") {
                return render_cell(&Entry::from_value(entry.clone()), ctx);
            }
            if let Some(Value::Object(roll)) = o.get("roll") {
                return rsx! { "{roll_text(roll)}" };
            }
        }
    }
    render_entry_inline(cell, ctx)
}

fn roll_text(roll: &Map<String, Value>) -> String {
    let pad = roll.get("pad").and_then(Value::as_bool).unwrap_or(false);
    let num = |key: &str| {
        roll.get(key)
            .and_then(Value::as_i64)
            .map(|n| if pad { format!("{n:02}") } else { n.to_string() })
    };
    if let Some(exact) = num("exact") {
        return exact;
    }
    match (num("min"), num("max")) {
        (Some(min), Some(max)) if min == max => min,
        (Some(min), Some(max)) => format!("{min}–{max}"),
        (Some(min), None) => format!("{min}+"),
        _ => String::new(),
    }
}

fn attack_type_label(code: &str) -> &'static str {
    match code {
        "MW" => "Melee Weapon Attack",
        "RW" => "Ranged Weapon Attack",
        "MS" => "Melee Spell Attack",
        "RS" => "Ranged Spell Attack",
        _ => "Attack",
    }
}

fn render_attack(a: &AttackBlock, ctx: RenderCtx) -> Element {
    rsx! {
        p { class: "mb-1.5 leading-snug",
            em { "{attack_type_label(a.attack_type.as_deref().unwrap_or(\"\"))}: " }
            for (i , e) in a.attack_entries.iter().enumerate() {
                span { key: "{i}", {render_entry_inline(e, ctx)} }
            }
            em { " Hit: " }
            for (i , e) in a.hit_entries.iter().enumerate() {
                span { key: "{i}", {render_entry_inline(e, ctx)} }
            }
        }
    }
}

/// Where 5etools' image repository is served from; entries reference
/// images by path relative to its root.
const IMAGE_BASE: &str = "https://raw.githubusercontent.com/5etools-mirror-3/5etools-img/main/";

fn image_src(href: &Value) -> Option<String> {
    let href = href.as_object()?;
    match str_at(href, "type")? {
        "internal" => Some(format!("{IMAGE_BASE}{}", str_at(href, "path")?.replace(' ', "%20"))),
        _ => str_at(href, "url").map(String::from),
    }
}

fn render_image(o: &Map<String, Value>, ctx: RenderCtx) -> Element {
    let Some(src) = o.get("href").and_then(image_src) else {
        return rsx! {};
    };
    let title = str_at(o, "title").unwrap_or("").to_string();
    let credit = str_at(o, "credit").map(String::from);
    rsx! {
        figure { class: "mb-1.5",
            img { class: "mx-auto max-h-64 max-w-full rounded", src: "{src}", alt: "{title}", loading: "lazy" }
            if !title.is_empty() || credit.is_some() {
                figcaption { class: "text-center text-[11px] text-gray-500",
                    {render_inline(&title, ctx)}
                    if let Some(credit) = credit {
                        " (Art: {credit})"
                    }
                }
            }
        }
    }
}

/// "Strength or Dexterity modifier" from an `attributes` list.
fn attribute_text(o: &Map<String, Value>) -> String {
    let attrs: Vec<String> = strings_at(o, "attributes")
        .into_iter()
        .map(|a| ability_full(a).to_string())
        .collect();
    let joined = join_conjunct(&attrs, "or");
    if attrs.len() == 1 && attrs[0] == "Spellcasting" {
        "Spellcasting ability modifier".to_string()
    } else {
        format!("{joined} modifier")
    }
}

/// A `spellcasting` block's list of spells, grouped by usage frequency
/// (`will`, `daily`, `spells` by level, ...) the way statblocks print it.
pub fn render_spellcasting(o: &Map<String, Value>, ctx: RenderCtx) -> Element {
    let ctx = ctx.with_depth(2);
    let hidden = strings_at(o, "hidden");
    let show = |k: &str| !hidden.contains(&k);
    let spell_list = |v: &Value| -> String {
        let items: Vec<String> = match v {
            Value::Array(a) => a.iter().map(spell_entry_text).collect(),
            other => vec![spell_entry_text(other)],
        };
        items.join(", ")
    };
    let mut rows: Vec<(String, String)> = Vec::new();
    for (key, label) in [("constant", "Constant"), ("will", "At Will")] {
        if let (Some(v), true) = (o.get(key), show(key)) {
            rows.push((format!("{label}:"), spell_list(v)));
        }
    }
    for (key, per) in [
        ("recharge", ""),
        ("legendary", " legendary action"),
        ("charges", " charge"),
        ("rest", "/rest"),
        ("restLong", "/long rest"),
        ("daily", "/day"),
        ("weekly", "/week"),
        ("monthly", "/month"),
        ("yearly", "/year"),
    ] {
        let (Some(Value::Object(map)), true) = (o.get(key), show(key)) else {
            continue;
        };
        let mut keys: Vec<&String> = map.keys().collect();
        keys.sort_by(|a, b| b.cmp(a));
        for k in keys {
            let each = k.ends_with('e');
            let n: i64 = k.trim_end_matches('e').parse().unwrap_or(0);
            let label = match key {
                "recharge" => format!("{{@recharge {n}|m}}"),
                "legendary" | "charges" => format!("{n}{per}{}", if n == 1 { "" } else { "s" }),
                _ => format!("{n}{per}"),
            };
            let suffix = if each { " each" } else { "" };
            rows.push((format!("{label}{suffix}:"), spell_list(&map[k])));
        }
    }
    if let (Some(v), true) = (o.get("ritual"), show("ritual")) {
        rows.push(("Rituals:".to_string(), spell_list(v)));
    }
    if let (Some(Value::Object(spells)), true) = (o.get("spells"), show("spells")) {
        let mut levels: Vec<(i64, &Value)> = spells.iter().filter_map(|(k, v)| Some((k.parse().ok()?, v))).collect();
        levels.sort_by_key(|(l, _)| *l);
        for (level, group) in levels {
            let Some(group) = group.as_object() else { continue };
            let level_name = |l: i64| {
                if l == 0 {
                    "Cantrips".to_string()
                } else {
                    format!("{} level", crate::statblock::format::ordinal(l))
                }
            };
            let slots = group.get("slots").and_then(Value::as_i64);
            let mut label = level_name(level);
            let mut slot_text = match slots {
                Some(0) => String::new(),
                Some(n) => format!(" ({n} slot{})", if n > 1 { "s" } else { "" }),
                None => " (at will)".to_string(),
            };
            if let Some(lower) = group.get("lower").and_then(Value::as_i64).filter(|l| *l != level) {
                label = format!("{}-{label}", crate::statblock::format::ordinal(lower));
                if let Some(n) = slots.filter(|n| *n > 0) {
                    slot_text = format!(
                        " ({n} {}-level slot{})",
                        crate::statblock::format::ordinal(level),
                        if n > 1 { "s" } else { "" }
                    );
                }
            }
            rows.push((
                format!("{label}{slot_text}:"),
                group.get("spells").map(spell_list).unwrap_or_default(),
            ));
        }
    }
    let header: Vec<Entry> = o
        .get("headerEntries")
        .and_then(Value::as_array)
        .map(|a| a.iter().cloned().map(Entry::from_value).collect())
        .unwrap_or_default();
    let footer: Vec<Entry> = o
        .get("footerEntries")
        .and_then(Value::as_array)
        .map(|a| a.iter().cloned().map(Entry::from_value).collect())
        .unwrap_or_default();
    rsx! {
        {render_lead_in_block(str_at(o, "name"), &header, ctx)}
        for (i , (label , spells)) in rows.into_iter().enumerate() {
            p { key: "{i}", class: "leading-snug",
                em { {render_inline(&label, ctx)} }
                " "
                {render_inline(&spells, ctx)}
            }
        }
        {render_entries(&footer, ctx)}
    }
}

/// A spell in a spellcasting list: a `{@spell}` string, or
/// `{ entry, hidden }` for spells shown only through their note.
fn spell_entry_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Object(o) => str_at(o, "entry").unwrap_or_default().to_string(),
        _ => String::new(),
    }
}

/// Every entry type without a typed variant of its own.
fn render_other(v: &Value, ctx: RenderCtx) -> Element {
    let Value::Object(o) = v else {
        return rsx! {
            p { class: "mb-1.5 italic text-gray-400", "[unsupported content]" }
        };
    };
    let kind = str_at(o, "type").unwrap_or("");
    let entries_of = |key: &str| -> Vec<Entry> {
        o.get(key)
            .and_then(Value::as_array)
            .map(|a| a.iter().cloned().map(Entry::from_value).collect())
            .unwrap_or_default()
    };
    match kind {
        "image" => render_image(o, ctx),
        "gallery" => {
            let images = entries_of("images");
            rsx! { {render_entries(&images, ctx)} }
        }
        "hr" => rsx! { hr { class: "my-1.5 border-gray-300" } },
        "abilityDc" | "abilityAttackMod" | "abilityGeneric" => {
            let name = str_at(o, "name").unwrap_or("");
            let (bold, rest) = match kind {
                "abilityDc" => (
                    format!("{name} save DC"),
                    format!(" = 8 + {} + Proficiency Bonus", attribute_text(o)),
                ),
                "abilityAttackMod" => (
                    format!("{name} attack modifier"),
                    format!(" = {} + Proficiency Bonus", attribute_text(o)),
                ),
                _ => {
                    let text = str_at(o, "text").unwrap_or("");
                    let attrs = if o.contains_key("attributes") {
                        format!(" {}", attribute_text(o))
                    } else {
                        String::new()
                    };
                    let sep = if name.is_empty() { "" } else { " = " };
                    (name.to_string(), format!("{sep}{text}{attrs}"))
                }
            };
            rsx! {
                p { class: "mb-1.5 text-center leading-snug",
                    strong { {render_inline(&bold, ctx)} }
                    {render_inline(&rest, ctx)}
                }
            }
        }
        "bonus" => {
            let n = o.get("value").and_then(Value::as_i64).unwrap_or(0);
            rsx! { "{crate::statblock::format::signed(n)}" }
        }
        "bonusSpeed" => {
            let n = o.get("value").and_then(Value::as_i64).unwrap_or(0);
            if n == 0 {
                rsx! { "—" }
            } else {
                rsx! { "{crate::statblock::format::signed(n)} ft." }
            }
        }
        "dice" => {
            let roll = match o.get("toRoll") {
                Some(Value::Array(dice)) => dice
                    .iter()
                    .filter_map(|d| Some(format!("{}d{}", d.get("number")?.as_i64()?, d.get("faces")?.as_i64()?)))
                    .collect::<Vec<_>>()
                    .join(" + "),
                Some(Value::String(s)) => s.clone(),
                _ => String::new(),
            };
            let text = str_at(o, "displayText").map(String::from).unwrap_or(roll);
            rsx! { strong { class: "font-semibold", "{text}" } }
        }
        "link" => {
            let text = str_at(o, "text").unwrap_or("");
            let url = o.get("href").and_then(|h| h.get("url")).and_then(Value::as_str);
            match url {
                Some(url) => rsx! {
                    a { class: "text-blue-700 underline", href: "{url}", target: "_blank", rel: "noopener noreferrer", "{text}" }
                },
                None => rsx! { {render_inline(text, ctx)} },
            }
        }
        "inline" | "inlineBlock" => {
            let entries = entries_of("entries");
            rsx! {
                for (i , e) in entries.iter().enumerate() {
                    span { key: "{i}", {render_entry_inline(e, ctx)} }
                }
            }
        }
        "itemSpell" => {
            let name = str_at(o, "name").unwrap_or("");
            let entry = str_at(o, "entry").unwrap_or("");
            rsx! {
                p { class: "leading-snug",
                    em { {render_inline(name, ctx)} }
                    " "
                    {render_inline(entry, ctx)}
                }
            }
        }
        "ingredient" => {
            let entry = str_at(o, "entry").unwrap_or_default();
            rsx! { li { class: "list-none leading-snug", {render_inline(entry, ctx)} } }
        }
        "spellcasting" => render_spellcasting(o, ctx),
        "tableGroup" => {
            let tables = entries_of("tables");
            rsx! { {render_entries(&tables, ctx)} }
        }
        "statblock" => render_statblock_ref(o, ctx),
        "statblockInline" => render_statblock_inline(o, ctx),
        "refClassFeature" | "refSubclassFeature" | "refOptionalfeature" | "refFeat" => render_reference(kind, o, ctx),
        "wrapper" => {
            let wrapped = o.get("wrapped").cloned().map(Entry::from_value);
            rsx! {
                if let Some(wrapped) = wrapped {
                    {render_entry(&wrapped, ctx)}
                }
            }
        }
        _ => {
            let children = entries_of("entries");
            let name = str_at(o, "name");
            if children.is_empty() {
                rsx! {
                    p { class: "mb-1.5 italic text-gray-400", "{name.unwrap_or(\"[unsupported content]\")}" }
                }
            } else {
                render_lead_in_block(name, &children, ctx.with_depth(2))
            }
        }
    }
}

/// `{ type: "statblock", tag, name, source }` embeds another entity; shown
/// as its card body in place, or - for an entity the library doesn't have,
/// or one that embeds itself - as a link to it.
fn render_statblock_ref(o: &Map<String, Value>, ctx: RenderCtx) -> Element {
    let tag = str_at(o, "tag").unwrap_or("creature");
    let name = str_at(o, "name").unwrap_or("");
    let source = str_at(o, "source").unwrap_or("");
    let display = str_at(o, "displayName").unwrap_or(name);
    let embedded = inline::tag_kind(tag)
        .and_then(|kind| {
            let found =
                ctx.library
                    .find_for_ruleset(kind, Some(source).filter(|s| !s.is_empty()), name, ctx.use_2024)?;
            Some((kind, found))
        })
        .filter(|(kind, found)| {
            !ctx.current
                .is_some_and(|(k, s, n)| k == *kind && s == found.source() && n == found.name())
        });
    if let Some((kind, entity)) = embedded {
        let inner = RenderCtx {
            current: Some((kind, entity.source(), entity.name())),
            ..ctx
        };
        return rsx! {
            div { class: "mb-2 rounded border border-gray-50 bg-gray-50 p-2",
                {crate::statblock::render(&entity, kind, entity.name(), inner)}
            }
        };
    }
    rsx! {
        p { class: "mb-1.5 leading-snug", {render_inline(&format!("{{@{tag} {name}|{source}|{display}}}"), ctx)} }
    }
}

/// `{ type: "statblockInline", dataType: "monster", data }` carries a whole
/// creature in place; rendered as its statblock.
fn render_statblock_inline(o: &Map<String, Value>, ctx: RenderCtx) -> Element {
    let monster = (str_at(o, "dataType") == Some("monster"))
        .then(|| o.get("data").cloned())
        .flatten()
        .and_then(|d| serde_json::from_value::<crate::model::Monster>(d).ok());
    match monster {
        Some(m) => rsx! {
            div { class: "mb-1.5 rounded border border-gray-50 bg-gray-50 p-2",
                h5 { class: "font-bold", "{m.name}" }
                {crate::statblock::render_monster_body(&m, ctx.with_depth(2))}
            }
        },
        None => rsx! {},
    }
}

/// `refClassFeature` and friends splice another entity's text in place.
fn render_reference(kind: &str, o: &Map<String, Value>, ctx: RenderCtx) -> Element {
    let key = match kind {
        "refClassFeature" => "classFeature",
        "refSubclassFeature" => "subclassFeature",
        "refOptionalfeature" => "optionalfeature",
        _ => "feat",
    };
    let uid = str_at(o, key).unwrap_or("");
    let parts: Vec<&str> = uid.split('|').collect();
    let part = |i: usize| parts.get(i).copied().filter(|s| !s.is_empty());
    let name = part(0).unwrap_or("");
    let lib = ctx.library;
    let (title, entries): (String, Vec<Entry>) = match kind {
        "refClassFeature" => {
            let level = part(3).and_then(|l| l.parse::<u8>().ok());
            match lib.class_features.iter().find(|f| {
                f.name.eq_ignore_ascii_case(name)
                    && part(1).is_none_or(|c| f.class_name.eq_ignore_ascii_case(c))
                    && part(2).is_none_or(|s| f.class_source.eq_ignore_ascii_case(s))
                    && level.is_none_or(|l| f.level == l)
            }) {
                Some(f) => (f.name.clone(), f.entries.clone()),
                None => (name.to_string(), Vec::new()),
            }
        }
        "refSubclassFeature" => {
            let level = part(5).and_then(|l| l.parse::<u8>().ok());
            match lib.subclass_features.iter().find(|f| {
                f.name.eq_ignore_ascii_case(name)
                    && part(1).is_none_or(|c| f.class_name.eq_ignore_ascii_case(c))
                    && part(2).is_none_or(|s| f.class_source.eq_ignore_ascii_case(s))
                    && part(3).is_none_or(|s| {
                        f.subclass_short_name
                            .as_deref()
                            .is_some_and(|x| x.eq_ignore_ascii_case(s))
                    })
                    && part(4).is_none_or(|s| f.subclass_source.as_deref().is_some_and(|x| x.eq_ignore_ascii_case(s)))
                    && level.is_none_or(|l| f.level == l)
            }) {
                Some(f) => (f.name.clone(), f.entries.clone()),
                None => (name.to_string(), Vec::new()),
            }
        }
        "refOptionalfeature" => match lib.find_ci(EntityKind::OptionalFeatures, part(1), name) {
            Some(crate::data::EntityRef::Other(_, f)) => (f.name.clone(), f.entries.clone()),
            _ => (name.to_string(), Vec::new()),
        },
        _ => match lib.find_ci(EntityKind::Feats, part(1), name) {
            Some(crate::data::EntityRef::Feat(f)) => (f.name.clone(), f.entries.clone()),
            _ => (name.to_string(), Vec::new()),
        },
    };
    render_lead_in_block(Some(&capitalize(&title)), &entries, ctx.with_depth(2))
}

/// An entity's compact preview, shown in its hover popup and as its full
/// detail view: a name/source header plus its dedicated statblock layout.
pub fn render_entity_preview(kind: EntityKind, name: &str, source: &str, ctx: RenderCtx) -> Element {
    let Some(entity) = ctx.library.find_ci(kind, Some(source), name) else {
        return rsx! {
            p { class: "text-gray-400", "{name} ({source}) not found." }
        };
    };
    let inner_ctx = RenderCtx {
        current: Some((kind, entity.source(), entity.name())),
        ..ctx
    };
    crate::statblock::render(&entity, kind, entity.name(), inner_ctx)
}
