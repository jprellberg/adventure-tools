//! The monster statblock, laid out in the 2024 style: AC and
//! initiative share a line, then HP, speed, and a compact three-column
//! ability table showing score, modifier and saving throw together.

use dioxus::prelude::*;
use serde_json::{Map, Value};

use crate::data::EntityRef;
use crate::model::bestiary::{AbilityScore, Hp, Monster, NamedEntries};
use crate::model::Entry;
use crate::render::{
    render_entries, render_inline, render_lead_in_block, render_spellcasting, render_values, RenderCtx,
};
use crate::routes::EntityKind;
use crate::statblock::format::*;
use crate::tags::strip_tags;

pub fn render(m: &Monster, header: Element, ctx: RenderCtx) -> Element {
    rsx! {
        div { class: "text-sm",
            {header}
            {render_monster_body(m, ctx)}
        }
    }
}

/// A `Label value` line of the statblock's stats area.
fn stat_line(label: &str, value: Element) -> Element {
    rsx! {
        p { class: "leading-snug",
            strong { class: "font-semibold", "{label} " }
            {value}
        }
    }
}

fn section_heading(title: &str, note: Option<&str>, ctx: RenderCtx) -> Element {
    rsx! {
        h4 { class: "mb-1 mt-2 border-b border-gray-300 text-sm font-bold",
            "{title}"
            if let Some(note) = note {
                " "
                span { class: "text-xs font-normal", "(" {render_inline(note, ctx)} ")" }
            }
        }
    }
}

pub fn render_monster_body(m: &Monster, ctx: RenderCtx) -> Element {
    let extra = &m.extra;
    let type_line = monster_type_line(m);
    let group = m.legendary_group.as_ref().and_then(|g| {
        ctx.library
            .find_ci(EntityKind::LegendaryGroups, Some(&g.source), &g.name)
    });
    let group_extra = group.as_ref().map(EntityRef::extra);
    let is_mythic = !m.mythic.is_empty() || spellcasting_for(m, "mythic").next().is_some();

    let traits = ordered_traits(m);
    let actions = ordered_actions(m, "action", &m.action);
    let bonus = ordered_actions(m, "bonus", &m.bonus);
    let reactions = ordered_actions(m, "reaction", &m.reaction);
    let legendary = ordered_actions(m, "legendary", &m.legendary);
    let mythic = ordered_actions(m, "mythic", &m.mythic);

    rsx! {
        p { class: "mb-1 italic", "{type_line}" }
        div { class: "flex justify-between gap-2 leading-snug",
            p {
                strong { class: "font-semibold", title: "Armor Class", "AC " }
                {render_inline(&m.ac.as_ref().map(format_ac).unwrap_or_else(|| "—".into()), ctx)}
            }
            p {
                strong { class: "font-semibold", "Initiative " }
                "{initiative_text(m)}"
            }
        }
        {stat_line("HP", rsx! { "{hp_text(m.hp.as_ref())}" })}
        {stat_line("Speed", rsx! { {render_inline(&m.speed.as_ref().map(format_speed).unwrap_or_else(|| "—".into()), ctx)} })}
        {ability_table(m)}
        if let Some(special) = m.save.as_ref().and_then(|s| str_at(s, "special")) {
            {stat_line("Saving Throws", rsx! { {render_inline(special, ctx)} })}
        }
        if let Some(skill) = &m.skill {
            {stat_line("Skills", rsx! { {render_inline(&skills_text(skill), ctx)} })}
        }
        if let Some(Value::Object(tool)) = extra.get("tool") {
            {stat_line("Tools", rsx! { {render_inline(&skills_text(tool), ctx)} })}
        }
        if let Some(v) = &m.vulnerable {
            {stat_line("Vulnerabilities", rsx! { {render_inline(&format_imm_res(v, ImmResMode::Damage), ctx)} })}
        }
        if let Some(v) = &m.resist {
            {stat_line("Resistances", rsx! { {render_inline(&format_imm_res(v, ImmResMode::Damage), ctx)} })}
        }
        if m.immune.is_some() || m.condition_immune.is_some() {
            {stat_line("Immunities", rsx! { {render_inline(&immunities_text(m), ctx)} })}
        }
        if !m.gear.is_empty() || !m.attached_items.is_empty() {
            {stat_line("Gear", rsx! { {render_inline(&gear_text(m), ctx)} })}
        }
        {stat_line("Senses", rsx! { {render_inline(&senses_text(m), ctx)} })}
        {stat_line("Languages", rsx! { {render_inline(&languages_text(m), ctx)} })}
        {stat_line("CR", rsx! { "{m.cr.as_ref().map(|cr| format_cr(cr, m.pb_note.as_deref(), is_mythic)).unwrap_or_else(|| \"—\".into())}" })}

        {ability_section("Traits", None, &traits, extra.get("traitHeader"), ctx)}
        {ability_section("Actions", note(extra, "actionNote"), &actions, extra.get("actionHeader"), ctx)}
        {ability_section("Bonus Actions", note(extra, "bonusNote"), &bonus, extra.get("bonusHeader"), ctx)}
        {ability_section("Reactions", note(extra, "reactionNote"), &reactions, extra.get("reactionHeader"), ctx)}
        if !legendary.is_empty() {
            {section_heading("Legendary Actions", note(extra, "legendaryNote"), ctx)}
            {render_legendary_intro(m, ctx)}
            {render_abilities(&legendary, ctx)}
        }
        if !mythic.is_empty() {
            {ability_section("Mythic Actions", note(extra, "mythicNote"), &mythic, extra.get("mythicHeader"), ctx)}
        }
        if let Some(lair) = group_extra.and_then(|g| g.get("lairActions")).and_then(Value::as_array) {
            {section_heading("Lair Actions", None, ctx)}
            {render_values(lair, ctx.with_depth(1))}
        }
        if let Some(regional) = group_extra.and_then(|g| g.get("regionalEffects")).and_then(Value::as_array) {
            {section_heading("Regional Effects", None, ctx)}
            {render_values(regional, ctx.with_depth(1))}
        }
        if !m.variant.is_empty() {
            div { class: "mt-2", {render_entries(&m.variant, ctx.with_depth(1))} }
        }
        if !m.footer.is_empty() {
            div { class: "mt-2", {render_entries(&m.footer, ctx)} }
        }
        if let Some(spell) = &m.summoned_by_spell {
            p { class: "mt-1 leading-snug",
                strong { class: "font-semibold", "Summoned By: " }
                {render_inline(&format!("{{@spell {spell}}}"), ctx)}
            }
        }
        if !m.environment.is_empty() {
            p { class: "leading-snug",
                strong { class: "font-semibold", "Habitat: " }
                "{environments_text(&m.environment)}"
            }
        }
        if !m.treasure.is_empty() {
            p { class: "leading-snug",
                strong { class: "font-semibold", "Treasure: " }
                {render_inline(&treasure_text(&m.treasure), ctx)}
            }
        }
    }
}

fn note<'a>(extra: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    str_at(extra, key)
}

fn monster_type_line(m: &Monster) -> String {
    let ty = m.creature_type.as_ref();
    let mut parts = Vec::new();
    if let Some(level) = m.level {
        parts.push(format!("{}-level", ordinal(level as i64)));
    }
    if let Some(sidekick) = ty.and_then(sidekick_type) {
        parts.push(format!("{sidekick};"));
    }
    parts.push(format_size(&m.size));
    if let Some(note) = &m.size_note {
        parts.push(note.clone());
    }
    let type_text = ty.map(monster_type).unwrap_or_default();
    let alignment = m
        .alignment
        .as_ref()
        .map(|a| {
            format!(
                "{}{}",
                m.alignment_prefix.as_deref().unwrap_or(""),
                title_case(&alignment_full(a))
            )
        })
        .filter(|a| !a.is_empty());
    let joiner = if type_text.contains(',') { "; " } else { ", " };
    parts.push(
        [Some(type_text), alignment]
            .into_iter()
            .flatten()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(joiner),
    );
    parts
        .into_iter()
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn hp_text(hp: Option<&Hp>) -> String {
    let Some(hp) = hp else { return "—".into() };
    if let Some(special) = &hp.special {
        return special.clone();
    }
    match (hp.average, &hp.formula) {
        (Some(avg), Some(formula)) if formula.ends_with("d1") => avg.to_string(),
        (Some(avg), Some(formula)) => format!("{avg} ({formula})"),
        (Some(avg), None) => avg.to_string(),
        (None, Some(formula)) => formula.clone(),
        (None, None) => "—".into(),
    }
}

fn score_of(s: &Option<AbilityScore>) -> Option<i32> {
    match s {
        Some(AbilityScore::Score(n)) => Some(*n),
        _ => None,
    }
}

fn initiative_text(m: &Monster) -> String {
    let dex_mod = score_of(&m.dex).map(ability_mod);
    let (bonus, advantage) = match &m.initiative {
        None => (dex_mod, None),
        Some(Value::Number(n)) => (n.as_i64(), None),
        Some(Value::Object(o)) => {
            let pb = m.cr.as_ref().and_then(cr_string).and_then(|cr| cr_to_pb(&cr));
            let bonus = i64_at(o, "initiative").or_else(|| {
                let proficiency = i64_at(o, "proficiency").unwrap_or(0);
                Some(dex_mod? + proficiency * pb.unwrap_or(0))
            });
            (bonus, str_at(o, "advantageMode"))
        }
        _ => (None, None),
    };
    let Some(bonus) = bonus else { return "—".into() };
    let passive = 10
        + bonus
        + match advantage {
            Some("adv") => 5,
            Some("dis") => -5,
            _ => 0,
        };
    format!("{} ({passive})", signed(bonus))
}

fn cr_string(cr: &Value) -> Option<String> {
    match cr {
        Value::String(s) => Some(s.clone()),
        Value::Object(o) => str_at(o, "cr").map(String::from),
        _ => None,
    }
}

/// The 2024 ability table: per ability, score / modifier / saving throw
/// (the save is the modifier unless the creature is proficient), physical
/// abilities on the first row and mental ones on the second. Abilities
/// given as prose (`{ special }`) are listed above it instead.
fn ability_table(m: &Monster) -> Element {
    let scores = [&m.str, &m.dex, &m.con, &m.int, &m.wis, &m.cha];
    let special: Vec<(&str, &str)> = ABILITIES
        .iter()
        .zip(scores)
        .filter_map(|(abv, s)| match s {
            Some(AbilityScore::Special { special }) => Some((*abv, special.as_str())),
            _ => None,
        })
        .collect();
    let has_numeric = scores.iter().any(|s| score_of(s).is_some());
    let cell = |i: usize| -> Element {
        let abv = ABILITIES[i];
        let score = score_of(scores[i]);
        let modifier = score.map(|s| signed(ability_mod(s)));
        let save = m
            .save
            .as_ref()
            .and_then(|s| str_at(s, abv))
            .map(String::from)
            .or_else(|| modifier.clone());
        let tint = if i < 3 { "bg-gray-100" } else { "bg-gray-50" };
        let dash = || "–".to_string();
        let score_text = score.map(|s| s.to_string()).unwrap_or_else(dash);
        let mod_text = modifier.unwrap_or_else(dash);
        let save_text = save.unwrap_or_else(dash);
        rsx! {
            th { class: "pr-1 text-right text-[10px] font-bold uppercase", "{abv}" }
            td { class: "text-center {tint}", "{score_text}" }
            td { class: "text-center {tint}", "{mod_text}" }
            td { class: "text-center {tint}", "{save_text}" }
        }
    };
    rsx! {
        for (abv , text) in special {
            p { key: "{abv}", class: "leading-snug",
                strong { class: "font-semibold", "{capitalize(abv)}. " }
                "{text}"
            }
        }
        if has_numeric {
            table { class: "my-1 w-full table-fixed border-collapse text-xs tabular-nums",
                thead {
                    tr { class: "text-[9px] uppercase text-gray-500",
                        for group in 0..3 {
                            th { key: "{group}", class: "w-[27px]" }
                            th { class: "w-[20px]" }
                            th { class: "w-[23px]", "mod" }
                            th { class: "w-[23px]", "save" }
                        }
                    }
                }
                tbody {
                    tr {
                        {cell(0)} {cell(1)} {cell(2)}
                    }
                    tr {
                        {cell(3)} {cell(4)} {cell(5)}
                    }
                }
            }
        }
    }
}

/// `{ perception: "+5" }` -> "Perception +5", with `other`/`oneOf` extras.
fn skills_text(skills: &Map<String, Value>) -> String {
    let mut names: Vec<&String> = skills
        .keys()
        .filter(|k| !matches!(k.as_str(), "other" | "special"))
        .collect();
    names.sort();
    let one = |name: &str, bonus: &str| format!("{{@skill {}}} {bonus}", title_case(name));
    let mut parts: Vec<String> = names
        .iter()
        .map(|k| one(k, skills[*k].as_str().unwrap_or("")))
        .collect();
    if let Some(Value::Array(other)) = skills.get("other") {
        for o in other.iter().filter_map(|o| o.get("oneOf")).filter_map(Value::as_object) {
            let mut keys: Vec<&String> = o.keys().collect();
            keys.sort();
            let choices: Vec<String> = keys.iter().map(|k| one(k, o[*k].as_str().unwrap_or(""))).collect();
            parts.push(format!("plus one of the following: {}", join_conjunct(&choices, "or")));
        }
    }
    if let Some(special) = str_at(skills, "special") {
        parts.push(special.to_string());
    }
    parts.join(", ")
}

fn immunities_text(m: &Monster) -> String {
    let damage = m
        .immune
        .as_ref()
        .map(|v| format_imm_res(v, ImmResMode::Damage))
        .unwrap_or_default();
    let conditions = m
        .condition_immune
        .as_ref()
        .map(|v| format_imm_res(v, ImmResMode::Condition))
        .unwrap_or_default();
    [damage, conditions]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("; ")
}

fn gear_text(m: &Monster) -> String {
    let refs: Vec<(String, i64, Option<String>)> = if m.gear.is_empty() {
        m.attached_items.iter().map(|uid| (uid.clone(), 1, None)).collect()
    } else {
        m.gear
            .iter()
            .filter_map(|g| match g {
                Value::String(uid) => Some((uid.clone(), 1, None)),
                Value::Object(o) => Some((
                    str_at(o, "item")?.to_string(),
                    i64_at(o, "quantity").unwrap_or(1),
                    str_at(o, "displayName").map(String::from),
                )),
                _ => None,
            })
            .collect()
    };
    refs.into_iter()
        .map(|(uid, quantity, display)| {
            let tag = match display {
                Some(d) => format!("{{@item {uid}|{d}}}"),
                None => format!("{{@item {uid}}}"),
            };
            if quantity == 1 {
                tag
            } else {
                format!("{quantity} {tag}")
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn senses_text(m: &Monster) -> String {
    let passive = match &m.passive {
        Some(Value::Number(n)) => Some(n.to_string()),
        Some(Value::String(s)) => Some(s.clone()),
        _ => score_of(&m.wis).map(|w| (10 + ability_mod(w)).to_string()),
    };
    let mut parts: Vec<String> = m.senses.iter().map(|s| capitalize(s)).collect();
    match passive {
        Some(p) => parts.push(format!("Passive Perception {p}")),
        None if !m.senses.is_empty() => parts.push("—".into()),
        None => {}
    }
    parts.join(", ")
}

fn languages_text(m: &Monster) -> String {
    if m.languages.is_empty() {
        "—".into()
    } else {
        capitalize(&m.languages.join(", "))
    }
}

fn environments_text(envs: &[String]) -> String {
    let mut names: Vec<String> = envs
        .iter()
        .map(|e| match e.strip_prefix("planar-") {
            Some(rest) => format!("Planar ({})", title_case(rest)),
            None => title_case(e),
        })
        .collect();
    names.sort();
    names.join(", ")
}

fn treasure_text(treasure: &[String]) -> String {
    let mut names: Vec<String> = treasure
        .iter()
        .map(|t| match t.as_str() {
            "arcana" | "armaments" | "implements" | "relics" => {
                let name = title_case(t);
                format!("{{@table Random Magic Items - {name}|XDMG|{name}}}")
            }
            other => title_case(other),
        })
        .collect();
    names.sort();
    names.join(", ")
}

/// One entry of a traits/actions list: a named block from the data, or a
/// `spellcasting` block shown in that section via its `displayAs`.
enum Ability<'a> {
    Named(&'a NamedEntries),
    Spellcasting(&'a Map<String, Value>),
}

impl Ability<'_> {
    fn name(&self) -> Option<&str> {
        match self {
            Ability::Named(n) => n.name.as_deref(),
            Ability::Spellcasting(s) => str_at(s, "name"),
        }
    }
}

fn spellcasting_for<'a>(m: &'a Monster, section: &'a str) -> impl Iterator<Item = &'a Map<String, Value>> {
    m.spellcasting
        .iter()
        .filter_map(Value::as_object)
        .filter(move |s| str_at(s, "displayAs").unwrap_or("trait") == section)
}

/// 5etools' trait order: an explicit `sort` first, "(... Only)" traits last,
/// a few conventional openers (temporary statblock, special equipment,
/// shapechanger), then alphabetical.
fn ordered_traits(m: &Monster) -> Vec<Ability<'_>> {
    const OPENERS: [&str; 3] = ["temporary statblock", "special equipment", "shapechanger"];
    let mut all: Vec<Ability> = m.traits.iter().map(Ability::Named).collect();
    all.extend(spellcasting_for(m, "trait").map(Ability::Spellcasting));
    let sort_field = |a: &Ability| match a {
        Ability::Named(n) => n.extra.get("sort").and_then(Value::as_i64),
        Ability::Spellcasting(s) => i64_at(s, "sort"),
    };
    all.sort_by(|a, b| {
        use std::cmp::Ordering;
        match (sort_field(a), sort_field(b)) {
            (Some(x), Some(y)) => return x.cmp(&y),
            (Some(_), None) => return Ordering::Less,
            (None, Some(_)) => return Ordering::Greater,
            _ => {}
        }
        let (an, bn) = (a.name().unwrap_or(""), b.name().unwrap_or(""));
        let (only_a, only_b) = (an.ends_with(" Only)"), bn.ends_with(" Only)"));
        if only_a != only_b {
            return only_a.cmp(&only_b);
        }
        let (ac, bc) = (strip_tags(an).to_lowercase(), strip_tags(bn).to_lowercase());
        match (
            OPENERS.iter().position(|o| *o == ac),
            OPENERS.iter().position(|o| *o == bc),
        ) {
            (Some(x), Some(y)) => x.cmp(&y),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            _ => ac.cmp(&bc),
        }
    });
    all
}

/// Actions keep their printed order ("Multiattack", then attacks, then the
/// rest); spellcasting entries slot into the non-attack tail alphabetically.
fn ordered_actions<'a>(m: &'a Monster, section: &'a str, list: &'a [NamedEntries]) -> Vec<Ability<'a>> {
    let mut actions: Vec<Ability> = list.iter().map(Ability::Named).collect();
    let spells: Vec<&Map<String, Value>> = spellcasting_for(m, section).collect();
    if spells.is_empty() {
        return actions;
    }
    let is_attack = |a: &Ability| match a {
        Ability::Named(n) => matches!(n.entries.first(), Some(Entry::String(s)) if s.contains("{@atk")),
        Ability::Spellcasting(_) => false,
    };
    let split = actions.iter().rposition(is_attack).unwrap_or(0);
    let mut tail = actions.split_off(split);
    for spell in spells {
        let name = str_at(spell, "name").unwrap_or("").to_lowercase();
        let at = tail
            .iter()
            .position(|a| a.name().is_some_and(|n| strip_tags(n).to_lowercase() >= name))
            .unwrap_or(tail.len());
        tail.insert(at, Ability::Spellcasting(spell));
    }
    actions.extend(tail);
    actions
}

fn ability_section(
    title: &str,
    note: Option<&str>,
    items: &[Ability],
    header: Option<&Value>,
    ctx: RenderCtx,
) -> Element {
    if items.is_empty() {
        return rsx! {};
    }
    let header: Vec<Value> = header.and_then(Value::as_array).cloned().unwrap_or_default();
    rsx! {
        {section_heading(title, note, ctx)}
        {render_values(&header, ctx)}
        {render_abilities(items, ctx)}
    }
}

fn render_abilities(items: &[Ability], ctx: RenderCtx) -> Element {
    let ctx = ctx.with_depth(2);
    rsx! {
        for (i , item) in items.iter().enumerate() {
            div { key: "{i}",
                match item {
                    Ability::Named(n) => rsx! { {render_named(n, ctx)} },
                    Ability::Spellcasting(s) => rsx! { {render_spellcasting(s, ctx)} },
                }
            }
        }
    }
}

/// A trait or action: "**Name.** text" running inline, like a printed
/// statblock. Entries that aren't plain text (lists, tables) sit below.
fn render_named(item: &NamedEntries, ctx: RenderCtx) -> Element {
    render_lead_in_block(item.name.as_deref(), &item.entries, ctx)
}

/// The 2024 legendary-action preamble, unless the creature supplies its own.
fn render_legendary_intro(m: &Monster, ctx: RenderCtx) -> Element {
    if let Some(Value::Array(header)) = m.extra.get("legendaryHeader") {
        return render_values(header, ctx);
    }
    let uses = m.legendary_actions.unwrap_or(3);
    let lair = m.legendary_actions_lair.unwrap_or(uses);
    let lair_note = if lair != uses {
        format!(" ({lair} in Lair)")
    } else {
        String::new()
    };
    let named = m.extra.get("isNamedCreature").and_then(Value::as_bool).unwrap_or(false);
    let subject = short_name(m, false);
    let subject_title = short_name(m, true);
    let possessive = if named { "their" } else { "its" };
    let text = format!(
        "{{@note Legendary Action Uses: {uses}{lair_note}. Immediately after another creature's turn, {subject} can expend a use to take one of the following actions. {subject_title} regains all expended uses at the start of each of {possessive} turns.}}"
    );
    rsx! { p { class: "mb-1 leading-snug", {render_inline(&text, ctx)} } }
}

/// "the dragon" / "The dragon" for a creature, or its first name if named.
fn short_name(m: &Monster, title: bool) -> String {
    let named = m.extra.get("isNamedCreature").and_then(Value::as_bool).unwrap_or(false);
    let prefix = if named {
        ""
    } else if title {
        "The "
    } else {
        "the "
    };
    let base = match m.extra.get("shortName") {
        Some(Value::String(s)) => s.to_lowercase(),
        Some(Value::Bool(true)) => m.name.clone(),
        _ => {
            let first = m.name.split(',').next().unwrap_or(&m.name);
            let lower = first.to_lowercase();
            let mut words = lower.split(' ').collect::<Vec<_>>();
            if words.len() == 4
                && matches!(words[0], "adult" | "ancient" | "young")
                && matches!(words[3], "dragon" | "dracolich")
            {
                words = vec![words[3]];
            } else if words.len() == 3
                && matches!(words[0], "adult" | "ancient" | "young")
                && matches!(words[2], "dragon" | "dracolich")
            {
                words = vec![words[2]];
            }
            if named {
                first.split(' ').next().unwrap_or(first).to_string()
            } else {
                words.join(" ")
            }
        }
    };
    format!("{prefix}{base}")
}
