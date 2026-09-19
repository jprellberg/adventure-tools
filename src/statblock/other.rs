//! Layouts for every kind that shares the minimal name/source/page/entries
//! shape (`model::Other`): each contributes an italic subtitle and a few
//! `Label value` lines built from its kind-specific fields, then the shared
//! entries body, then any named sections (a vehicle's weapons, a
//! psionic's modes, ...).

use dioxus::prelude::*;
use serde_json::{json, Map, Value};

use super::{class, divider, prop_line, subtitle};
use crate::model::Other;
use crate::render::{render_entries, render_values, RenderCtx};
use crate::routes::{EntityKind, Route};
use crate::statblock::format::*;
use crate::statblock::meta::{describe, prerequisite_text};

/// The inline tag that links to an entity of `kind` (for reprint links).
pub fn tag_for(kind: EntityKind) -> &'static str {
    match kind {
        EntityKind::Subclasses => "subclass",
        EntityKind::Deities => "deity",
        EntityKind::Vehicles => "vehicle",
        EntityKind::VariantRules => "variantrule",
        EntityKind::OptionalFeatures => "optfeature",
        EntityKind::Rewards => "reward",
        EntityKind::Psionics => "psionic",
        EntityKind::Recipes => "recipe",
        EntityKind::CultsBoons => "cult",
        EntityKind::Languages => "language",
        EntityKind::CharOptions => "charoption",
        EntityKind::Facilities => "facility",
        EntityKind::ItemMasteries => "itemMastery",
        EntityKind::Skills => "skill",
        EntityKind::Senses => "sense",
        EntityKind::Tables => "table",
        EntityKind::Decks => "deck",
        EntityKind::Cards => "card",
        EntityKind::LegendaryGroups => "legroup",
        EntityKind::CrochetPatterns => "crochet",
        EntityKind::Rules => "rule",
        EntityKind::Books => "book",
        _ => "variantrule",
    }
}

/// What a kind adds around the shared entries body.
#[derive(Default)]
struct Layout {
    subtitle: String,
    lines: Vec<(&'static str, String)>,
    /// Entries shown right after the stat lines, before the body.
    lead: Vec<Value>,
    /// Named sections shown after the body.
    sections: Vec<(String, Vec<Value>)>,
}

impl Layout {
    fn line(&mut self, label: &'static str, text: impl Into<String>) {
        self.lines.push((label, text.into()));
    }

    fn section(&mut self, title: &str, entries: &Value) {
        if let Some(list) = entries.as_array().filter(|l| !l.is_empty()) {
            self.sections.push((title.to_string(), list.clone()));
        }
    }
}

pub fn render(kind: EntityKind, o: &Other, header: Element, ctx: RenderCtx) -> Element {
    if kind == EntityKind::Subclasses {
        return class::render_subclass(o, header, ctx);
    }
    let layout = layout_for(kind, o);
    let subtitle = book_path(kind, o, &layout.subtitle, ctx).unwrap_or_else(|| subtitle(&layout.subtitle, ctx));
    rsx! {
        div { class: "text-sm",
            {header}
            {subtitle}
            for (label , text) in layout.lines {
                {prop_line(label, &text, ctx)}
            }
            {render_values(&layout.lead, ctx)}
            {divider()}
            {render_entries(&o.entries, ctx)}
            if kind == EntityKind::Books {
                Link {
                    class: "text-blue-700 underline decoration-dotted underline-offset-2 hover:decoration-solid",
                    to: Route::Book {
                        source: o.source.clone(),
                        section: Vec::new(),
                    },
                    "Read the book →"
                }
            }
            for (title , entries) in layout.sections {
                div { key: "{title}",
                    h4 { class: "mb-1 mt-2 border-b border-gray-300 text-sm font-bold", "{title}" }
                    {render_values(&entries, ctx.with_depth(2))}
                }
            }
        }
    }
}

/// The subtitle of a section or table taken from a book - "Book title ›
/// Chapter › ..." - with the title of the book linking to its page and the
/// rest to the chapter or section the entity sits in.
fn book_path(kind: EntityKind, o: &Other, path: &str, ctx: RenderCtx) -> Option<Element> {
    if !matches!(kind, EntityKind::Rules | EntityKind::Tables) {
        return None;
    }
    let book = ctx.library.book(&o.source)?;
    let rest = path.strip_prefix(book.name.as_str())?.strip_prefix(" › ");
    let index = |key| i64_at(&o.extra, key).and_then(|i| usize::try_from(i).ok());
    let place: Vec<String> = index("chapter")
        .into_iter()
        .chain(index("section"))
        .map(|i| i.to_string())
        .collect();
    let class = "text-blue-700 underline decoration-dotted underline-offset-2 hover:decoration-solid";
    let book_route = Route::Book {
        source: o.source.clone(),
        section: Vec::new(),
    };
    let place_route = Route::Book {
        source: o.source.clone(),
        section: place,
    };
    Some(rsx! {
        p { class: "mb-1 italic",
            Link { class, to: book_route, "{book.name}" }
            if let Some(rest) = rest {
                " › "
                Link { class, to: place_route, "{rest}" }
            }
        }
    })
}

fn join(list: Vec<&str>) -> String {
    list.join(", ")
}

/// "cost: { min, max }" -> "2 psi" / "2-4 psi".
fn psi_cost(c: &Map<String, Value>) -> String {
    let (min, max) = (i64_at(c, "min").unwrap_or(0), i64_at(c, "max").unwrap_or(0));
    if min == max {
        format!("{min} psi")
    } else {
        format!("{min}-{max} psi")
    }
}

fn layout_for(kind: EntityKind, o: &Other) -> Layout {
    let x = &o.extra;
    let mut l = Layout::default();
    match kind {
        EntityKind::Deities => {
            l.subtitle = str_at(x, "title").map(title_case).unwrap_or_default();
            l.line("Also known as:", join(strings_at(x, "altNames")));
            l.line("Pantheon:", str_at(x, "pantheon").unwrap_or_default());
            l.line("Alignment:", x.get("alignment").map(alignment_full).unwrap_or_default());
            l.line("Category:", str_at(x, "category").unwrap_or_default());
            l.line("Domains:", join(strings_at(x, "domains")));
            l.line("Province:", str_at(x, "province").unwrap_or_default());
            l.line("Symbol:", str_at(x, "symbol").unwrap_or_default());
            l.line("Plane:", str_at(x, "plane").unwrap_or_default());
            l.line("Worshipers:", str_at(x, "worshipers").unwrap_or_default());
            l.line("Piety:", x.get("piety").map(describe).unwrap_or_default());
            if let Some(img) = x.get("symbolImg") {
                l.lead.push(img.clone());
            }
        }
        EntityKind::Vehicles => vehicle_layout(x, &mut l),
        EntityKind::VariantRules => {
            l.subtitle = str_at(x, "ruleType").map(rule_type).unwrap_or_default().to_string();
        }
        EntityKind::OptionalFeatures => {
            let types: Vec<String> = strings_at(x, "featureType")
                .into_iter()
                .map(|t| opt_feature_type(t).to_string())
                .collect();
            l.subtitle = types.join(", ");
            l.line(
                "Prerequisite:",
                x.get("prerequisite").map(prerequisite_text).unwrap_or_default(),
            );
            if let Some(Value::Object(c)) = x.get("consumes") {
                let amount = i64_at(c, "amount").unwrap_or(1);
                l.line("Consumes:", format!("{amount} {}", str_at(c, "name").unwrap_or("")));
            }
        }
        EntityKind::Rewards => {
            l.subtitle = [str_at(x, "type"), str_at(x, "rarity")]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(", ");
            let facilities: Vec<String> = strings_at(x, "seeAlsoFacility")
                .into_iter()
                .map(|f| format!("{{@facility {f}}}"))
                .collect();
            l.line("See also:", facilities.join(", "));
        }
        EntityKind::Psionics => {
            let ty = match str_at(x, "type") {
                Some("T") => "Talent",
                Some("D") => "Discipline",
                Some(other) => other,
                None => "",
            };
            l.subtitle = match str_at(x, "order") {
                Some(order) if ty == "Discipline" => format!("{ty}, {order}"),
                _ => ty.to_string(),
            };
            l.line("Psychic Focus:", str_at(x, "focus").unwrap_or_default());
            if let Some(Value::Array(modes)) = x.get("modes") {
                let entries: Vec<Value> = modes
                    .iter()
                    .filter_map(Value::as_object)
                    .flat_map(psionic_mode)
                    .collect();
                l.sections.push(("Modes".to_string(), entries));
            }
        }
        EntityKind::Recipes => recipe_layout(x, &mut l),
        EntityKind::CultsBoons => {
            l.subtitle = str_at(x, "type").unwrap_or_default().to_string();
            for (label, key) in [
                ("Cultists:", "cultists"),
                ("Goals:", "goal"),
                ("Ability Score Adjustment:", "ability"),
                ("Signature Spells:", "signatureSpells"),
            ] {
                let text = match x.get(key) {
                    Some(Value::Object(o)) => str_at(o, "entry").unwrap_or_default().to_string(),
                    Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", "),
                    Some(Value::String(s)) => s.clone(),
                    _ => String::new(),
                };
                l.line(label, text);
            }
        }
        EntityKind::Languages => {
            l.subtitle = str_at(x, "type").map(capitalize).unwrap_or_default();
            l.line("Script:", str_at(x, "script").unwrap_or_default());
            l.line("Typical Speakers:", join(strings_at(x, "typicalSpeakers")));
            l.line("Origin:", str_at(x, "origin").unwrap_or_default());
            l.line("Dialects:", join(strings_at(x, "dialects")));
        }
        EntityKind::CharOptions => {
            let types: Vec<String> = strings_at(x, "optionType")
                .into_iter()
                .map(|t| char_option_type(t).to_string())
                .collect();
            l.subtitle = types.join(", ");
            l.line(
                "Prerequisite:",
                x.get("prerequisite").map(prerequisite_text).unwrap_or_default(),
            );
        }
        EntityKind::Facilities => {
            let kind = str_at(x, "facilityType").map(capitalize).unwrap_or_default();
            l.subtitle = match i64_at(x, "level") {
                Some(level) => format!("Level {level} {kind} Facility"),
                None => format!("{kind} Facility"),
            };
            l.line(
                "Prerequisite:",
                x.get("prerequisite").map(prerequisite_text).unwrap_or_default(),
            );
            l.line("Space:", join(strings_at(x, "space").into_iter().collect()));
            l.line("Hirelings:", x.get("hirelings").map(hirelings_text).unwrap_or_default());
            l.line("Orders:", join(strings_at(x, "orders").into_iter().collect()));
        }
        EntityKind::ItemProperties => {
            l.subtitle = format!("Item property ({})", str_at(x, "abbreviation").unwrap_or(""));
        }
        EntityKind::ItemMasteries => l.subtitle = "Weapon mastery property".to_string(),
        EntityKind::Skills => {
            l.line(
                "Ability:",
                str_at(x, "ability")
                    .map(|a| ability_full(a).to_string())
                    .unwrap_or_default(),
            );
        }
        EntityKind::Decks => {
            let cards: Vec<String> = x
                .get("cards")
                .and_then(Value::as_array)
                .map(|list| {
                    list.iter()
                        .filter_map(|c| {
                            let (uid, count) = match c {
                                Value::String(s) => (s.as_str(), 1),
                                Value::Object(o) => (str_at(o, "uid")?, i64_at(o, "count").unwrap_or(1)),
                                _ => return None,
                            };
                            let name = uid.split('|').next().unwrap_or(uid);
                            let prefix = if count > 1 {
                                format!("{count}× ")
                            } else {
                                String::new()
                            };
                            Some(format!("{prefix}{{@card {uid}|{name}}}"))
                        })
                        .collect()
                })
                .unwrap_or_default();
            l.line("Cards:", cards.join(", "));
            if let Some(back) = x.get("back") {
                l.lead.push(back.clone());
            }
        }
        EntityKind::Cards => {
            l.subtitle = [
                str_at(x, "suit").map(capitalize),
                str_at(x, "valueName").map(String::from),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(", ");
            if let Some(set) = str_at(x, "set") {
                l.line("Deck:", format!("{{@deck {set}|{}}}", o.source));
            }
            for key in ["face", "back"] {
                if let Some(img) = x.get(key) {
                    l.lead.push(img.clone());
                }
            }
        }
        EntityKind::Encounters => {
            let tables = x.get("tables").and_then(Value::as_array).cloned().unwrap_or_default();
            for t in tables.iter().filter_map(Value::as_object) {
                l.lead.push(encounter_table(t));
            }
        }
        EntityKind::LegendaryGroups => {
            for (title, key) in [
                ("Lair Actions", "lairActions"),
                ("Regional Effects", "regionalEffects"),
                ("As a Mythic Encounter", "mythicEncounter"),
            ] {
                if let Some(v) = x.get(key) {
                    l.section(title, v);
                }
            }
        }
        EntityKind::CrochetPatterns => crochet_layout(x, &mut l),
        EntityKind::Rules | EntityKind::Tables => l.subtitle = str_at(x, "path").unwrap_or_default().to_string(),
        EntityKind::Books => {
            l.subtitle = str_at(x, "group").map(book_group).unwrap_or_default().to_string();
            l.line("Author:", str_at(x, "author").unwrap_or_default());
            l.line("Published:", str_at(x, "published").unwrap_or_default());
            l.line("Storyline:", str_at(x, "storyline").unwrap_or_default());
            if let Some(level) = x.get("level") {
                let bound = |key| level.get(key).and_then(Value::as_i64);
                if let (Some(start), Some(end)) = (bound("start"), bound("end")) {
                    l.line("Levels:", format!("{start}-{end}"));
                }
            }
        }
        _ => {}
    }
    l
}

fn book_group(code: &str) -> &str {
    match code {
        "core" => "Core rulebook",
        "screen" => "Screen",
        "setting" => "Setting book",
        "setting-alt" => "Setting book (alternative)",
        "supplement" => "Supplement",
        "supplement-alt" => "Supplement (alternative)",
        "organized-play" => "Organized play",
        "recipe" => "Recipe book",
        "homecraft" => "Homecraft book",
        "other" => "Other",
        other => other,
    }
}

fn rule_type(code: &str) -> &'static str {
    match code {
        "C" => "Core Rule",
        "O" => "Optional Rule",
        "P" => "Prerelease Rule",
        "V" => "Variant Rule",
        "VO" => "Variant Optional Rule",
        "VV" => "Variant Variant Rule",
        _ => "",
    }
}

fn opt_feature_type(code: &str) -> &str {
    match code {
        "AI" => "Artificer Infusion",
        "ED" => "Elemental Discipline",
        "EI" => "Eldritch Invocation",
        "MM" => "Metamagic",
        "MV" => "Maneuver",
        "MV:B" => "Maneuver, Battle Master",
        "MV:C2-UA" => "Maneuver, Cavalier V2 (UA)",
        "AS:V1-UA" => "Arcane Shot, V1 (UA)",
        "AS:V2-UA" => "Arcane Shot, V2 (UA)",
        "AS" => "Arcane Shot",
        "OTH" => "Other",
        "FS:F" => "Fighting Style; Fighter",
        "FS:B" => "Fighting Style; Bard",
        "FS:P" => "Fighting Style; Paladin",
        "FS:R" => "Fighting Style; Ranger",
        "PB" => "Pact Boon",
        "OR" => "Onomancy Resonant",
        "RN" => "Rune Knight Rune",
        "AF" => "Alchemical Formula",
        "TT" => "Traveler's Trick",
        "RP" => "Renown Perk",
        other => other,
    }
}

fn char_option_type(code: &str) -> &str {
    match code {
        "SG" => "Supernatural Gift",
        "OF" => "Optional Feature",
        "DG" => "Dark Gift",
        "RF:B" => "Replacement Feature: Background",
        "CS" => "Character Secret",
        "PTH" => "Path",
        other => other,
    }
}

fn hirelings_text(v: &Value) -> String {
    v.as_array()
        .map(|list| {
            list.iter()
                .filter_map(Value::as_object)
                .map(|h| {
                    let count = match (i64_at(h, "exact"), i64_at(h, "min")) {
                        (Some(n), _) => n.to_string(),
                        (None, Some(n)) => format!("{n}+"),
                        _ => String::new(),
                    };
                    match str_at(h, "space") {
                        Some(space) => format!("{count} ({space})"),
                        None => count,
                    }
                })
                .collect::<Vec<_>>()
                .join("; ")
        })
        .unwrap_or_default()
}

/// A psionic mode as headed entries: "Name (2 psi; conc., 1 hour)".
fn psionic_mode(mode: &Map<String, Value>) -> Vec<Value> {
    let mut bracket = Vec::new();
    if let Some(Value::Object(c)) = mode.get("cost") {
        bracket.push(psi_cost(c));
    }
    if let Some(Value::Object(c)) = mode.get("concentration") {
        bracket.push(format!(
            "conc., {} {}",
            i64_at(c, "duration").unwrap_or(0),
            str_at(c, "unit").unwrap_or("")
        ));
    }
    let name = match (str_at(mode, "name"), bracket.is_empty()) {
        (Some(n), true) => n.to_string(),
        (Some(n), false) => format!("{n} ({})", bracket.join("; ")),
        (None, _) => String::new(),
    };
    let mut entries = mode.get("entries").cloned().unwrap_or(json!([]));
    if let Some(Value::Array(subs)) = mode.get("submodes") {
        let subs: Vec<Value> = subs
            .iter()
            .filter_map(Value::as_object)
            .flat_map(psionic_mode)
            .collect();
        if let Value::Array(list) = &mut entries {
            list.extend(subs);
        }
    }
    vec![json!({ "type": "entries", "name": name, "entries": entries })]
}

fn recipe_layout(x: &Map<String, Value>, l: &mut Layout) {
    let diet = match str_at(x, "diet") {
        Some("V") => "Vegan",
        Some("C") => "Vegetarian",
        Some("X") => "Omni",
        Some(other) => other,
        None => "",
    };
    let mut parts: Vec<String> = [str_at(x, "type").map(String::from)].into_iter().flatten().collect();
    parts.extend(strings_at(x, "dishTypes").into_iter().map(capitalize));
    if !diet.is_empty() {
        parts.push(diet.to_string());
    }
    l.subtitle = parts.join(", ");
    let amount = |v: &Value| -> String {
        let Value::Object(o) = v else { return String::new() };
        let count = match (i64_at(o, "exact"), i64_at(o, "min"), i64_at(o, "max")) {
            (Some(n), _, _) => n.to_string(),
            (None, Some(a), Some(b)) => format!("{a}-{b}"),
            _ => String::new(),
        };
        match str_at(o, "note") {
            Some(note) => format!("{count} ({note})"),
            None => count,
        }
    };
    l.line("Makes:", x.get("makes").map(amount).unwrap_or_default());
    l.line("Serves:", x.get("serves").map(amount).unwrap_or_default());
    l.line("Allergens:", join(strings_at(x, "allergenGroups")));
    l.line("Equipment:", join(strings_at(x, "equipment")));
    if let Some(Value::Array(ingredients)) = x.get("ingredients") {
        l.sections.push((
            "Ingredients".to_string(),
            vec![json!({ "type": "list", "style": "list-no-bullets", "items": ingredients })],
        ));
    }
    if let Some(Value::Array(steps)) = x.get("instructions") {
        l.sections.push((
            "Instructions".to_string(),
            vec![json!({ "type": "list", "style": "list-decimal", "items": steps })],
        ));
    }
    if let Some(note) = x.get("noteCook") {
        let entries = match note {
            Value::Array(a) => a.clone(),
            other => vec![other.clone()],
        };
        l.sections.push(("Cook's Notes".to_string(), entries));
    }
}

fn vehicle_type(code: &str) -> &str {
    match code {
        "SHIP" => "Ship",
        "SPELLJAMMER" => "Spelljammer Ship",
        "ELEMENTAL_AIRSHIP" => "Elemental Airship",
        "INFWAR" => "Infernal War Machine",
        "CREATURE" => "Creature",
        "OBJECT" => "Object",
        other => other,
    }
}

fn vehicle_upgrade_type(code: &str) -> &str {
    match code {
        "SHP:H" => "Ship Upgrade, Hull",
        "SHP:M" => "Ship Upgrade, Movement",
        "SHP:W" => "Ship Upgrade, Weapon",
        "SHP:F" => "Ship Upgrade, Figurehead",
        "SHP:O" => "Ship Upgrade, Miscellaneous",
        "IWM:W" => "Infernal War Machine Variant, Weapon",
        "IWM:A" => "Infernal War Machine Upgrade, Armor",
        "IWM:G" => "Infernal War Machine Upgrade, Gadget",
        other => other,
    }
}

/// A `speed` / `pace` value: a number of feet or miles per hour, or an
/// object of movement modes.
fn scalar_or_speed(v: &Value) -> String {
    match v {
        Value::Object(_) => format_speed(v),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        _ => String::new(),
    }
}

fn vehicle_layout(x: &Map<String, Value>, l: &mut Layout) {
    if let Some(types) = x.get("upgradeType") {
        l.subtitle = strings(types)
            .into_iter()
            .map(|t| vehicle_upgrade_type(t).to_string())
            .collect::<Vec<_>>()
            .join(", ");
        return;
    }
    let size = match x.get("size") {
        Some(Value::String(s)) => size_label(s).to_string(),
        Some(Value::Array(a)) => format_size(&a.iter().filter_map(Value::as_str).map(String::from).collect::<Vec<_>>()),
        _ => String::new(),
    };
    let dims = strings_at(x, "dimensions");
    let dims = if dims.is_empty() {
        String::new()
    } else {
        format!(" ({})", dims.join(" by "))
    };
    l.subtitle = format!(
        "{size} {}{dims}",
        str_at(x, "vehicleType").map(vehicle_type).unwrap_or("vehicle")
    )
    .trim()
    .to_string();
    l.line(
        "Terrain:",
        join(strings_at(x, "terrain").into_iter().map(|t| t as &str).collect()),
    );
    let capacity = |key: &str, label: &str| i64_at(x, key).map(|n| format!("{n} {label}"));
    let caps: Vec<String> = [
        capacity("capCrew", "crew"),
        capacity("capPassenger", "passengers"),
        capacity("capCreature", "creatures"),
        match x.get("capCargo") {
            Some(Value::Number(n)) => Some(format!(
                "{n} {} cargo",
                plural(n.as_f64().map_or(0, |f| f as i64), "ton")
            )),
            Some(Value::String(s)) => Some(format!("{s} cargo")),
            _ => None,
        },
    ]
    .into_iter()
    .flatten()
    .collect();
    l.line(
        "Capacity:",
        format!(
            "{}{}",
            caps.join(", "),
            str_at(x, "capCrewNote").map(|n| format!(" ({n})")).unwrap_or_default()
        ),
    );
    l.line(
        "Weight:",
        x.get("weight")
            .and_then(Value::as_f64)
            .map(format_weight)
            .unwrap_or_default(),
    );
    l.line(
        "Cost:",
        x.get("cost")
            .and_then(Value::as_f64)
            .map(format_coins)
            .unwrap_or_default(),
    );
    l.line(
        "Pace:",
        x.get("pace")
            .map(scalar_or_speed)
            .map(|p| if p.contains("ft") { p } else { format!("{p} mph") })
            .unwrap_or_default(),
    );
    l.line(
        "Speed:",
        x.get("speed")
            .map(scalar_or_speed)
            .map(|s| {
                if s.chars().all(|c| c.is_ascii_digit()) {
                    format!("{s} ft.")
                } else {
                    s
                }
            })
            .unwrap_or_default(),
    );
    l.line(
        "AC:",
        x.get("ac")
            .map(|a| format_ac(&Value::Array(vec![a.clone()])))
            .unwrap_or_default(),
    );
    let hp = match x.get("hp") {
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Object(o)) => {
            let mut parts = vec![i64_at(o, "hp").map(|h| h.to_string()).unwrap_or_default()];
            if let Some(dt) = i64_at(o, "dt") {
                parts.push(format!("damage threshold {dt}"));
            }
            if let Some(mt) = i64_at(o, "mt") {
                parts.push(format!("mishap threshold {mt}"));
            }
            parts
                .into_iter()
                .filter(|p| !p.is_empty())
                .collect::<Vec<_>>()
                .join(", ")
        }
        _ => String::new(),
    };
    l.line("HP:", hp);
    if let Some(Value::Object(hull)) = x.get("hull") {
        let ac = i64_at(hull, "ac").map(|a| {
            format!("AC {a}{}", {
                let from = strings_at(hull, "acFrom");
                if from.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", from.join(", "))
                }
            })
        });
        let rest = [
            i64_at(hull, "hp").map(|h| format!("HP {h}")),
            i64_at(hull, "dt").map(|d| format!("damage threshold {d}")),
        ];
        l.line(
            "Hull:",
            [ac, rest[0].clone(), rest[1].clone()]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    let abilities: Vec<String> = ABILITIES
        .iter()
        .filter_map(|a| {
            Some(format!(
                "{} {} ({})",
                capitalize(a),
                x.get(*a)?.as_i64()?,
                signed(ability_mod(x.get(*a)?.as_i64()? as i32))
            ))
        })
        .collect();
    l.line("Abilities:", abilities.join(", "));
    l.line(
        "Damage Immunities:",
        x.get("immune")
            .map(|v| format_imm_res(v, ImmResMode::Damage))
            .unwrap_or_default(),
    );
    l.line(
        "Condition Immunities:",
        x.get("conditionImmune")
            .map(|v| format_imm_res(v, ImmResMode::Condition))
            .unwrap_or_default(),
    );
    if let Some(Value::Object(t)) = x.get("actionThresholds") {
        let text = t
            .iter()
            .map(|(hp, n)| format!("{hp} HP: {}", describe(n)))
            .collect::<Vec<_>>()
            .join("; ");
        l.line("Action Thresholds:", text);
    }
    for (title, key) in [
        ("Control", "control"),
        ("Movement", "movement"),
        ("Stations", "station"),
        ("Action Stations", "actionStation"),
        ("Traits", "trait"),
        ("Actions", "action"),
        ("Reactions", "reaction"),
        ("Weapons", "weapon"),
    ] {
        if let Some(v) = x.get(key) {
            l.section(title, &vehicle_section(v));
        }
    }
}

/// A vehicle's named sections hold headed entries; ship weapons carry a
/// count and their own attack actions, which are folded into the entry.
fn vehicle_section(v: &Value) -> Value {
    let Value::Array(list) = v else { return json!([]) };
    Value::Array(
        list.iter()
            .map(|e| {
                let Value::Object(o) = e else { return e.clone() };
                let mut entries = o.get("entries").and_then(Value::as_array).cloned().unwrap_or_default();
                if let Some(Value::Array(actions)) = o.get("action") {
                    entries.extend(actions.iter().map(|a| {
                        let mut a = a.clone();
                        if let Value::Object(m) = &mut a {
                            m.entry("type").or_insert(json!("entries"));
                        }
                        a
                    }));
                }
                let count = i64_at(o, "count")
                    .filter(|c| *c > 1)
                    .map(|c| format!("{c} "))
                    .unwrap_or_default();
                let mut stats = Vec::new();
                if let Some(crew) = i64_at(o, "crew") {
                    stats.push(format!("crew {crew}"));
                }
                if let Some(ac) = i64_at(o, "ac") {
                    stats.push(format!("AC {ac}"));
                }
                if let Some(hp) = i64_at(o, "hp") {
                    stats.push(format!("HP {hp}"));
                }
                let stats = if stats.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", stats.join(", "))
                };
                let name = str_at(o, "name")
                    .map(|n| format!("{count}{n}{stats}"))
                    .unwrap_or_default();
                json!({ "type": "entries", "name": name, "entries": entries })
            })
            .collect(),
    )
}

/// An encounter's dice table as a table entry.
fn encounter_table(t: &Map<String, Value>) -> Value {
    let rows: Vec<Value> = t
        .get("table")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(Value::as_object)
                .map(|r| {
                    let (min, max) = (i64_at(r, "min").unwrap_or(0), i64_at(r, "max").unwrap_or(0));
                    let range = if min == max {
                        min.to_string()
                    } else {
                        format!("{min}–{max}")
                    };
                    json!([range, r.get("result").cloned().unwrap_or(json!(""))])
                })
                .collect()
        })
        .unwrap_or_default();
    let levels = match (i64_at(t, "minlvl"), i64_at(t, "maxlvl")) {
        (Some(a), Some(b)) => format!("Levels {a}-{b}"),
        _ => String::new(),
    };
    let caption = [str_at(t, "caption"), Some(levels.as_str())]
        .into_iter()
        .flatten()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(", ");
    json!({
        "type": "table",
        "caption": caption,
        "colLabels": [str_at(t, "diceExpression").unwrap_or("d"), "Encounter"],
        "colStyles": ["col-2 text-center", "col-10"],
        "rows": rows,
    })
}

fn crochet_layout(x: &Map<String, Value>, l: &mut Layout) {
    let level = match str_at(x, "level") {
        Some("B") => "Beginner",
        Some("E") => "Easy",
        Some("I") => "Intermediate",
        Some("A") => "Advanced",
        Some(other) => other,
        None => "",
    };
    l.subtitle = [Some(level), str_at(x, "patternType")]
        .into_iter()
        .flatten()
        .map(capitalize)
        .collect::<Vec<_>>()
        .join(", ");
    l.line("Designers:", join(strings_at(x, "designers")));
    let sizes: Vec<String> = x
        .get("size")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(Value::as_object)
                .map(|s| {
                    let dim = |k: &str| s.get(k).and_then(|d| str_at(d.as_object()?, "entry")).map(String::from);
                    [
                        dim("height").map(|h| format!("{h} tall")),
                        dim("width").map(|w| format!("{w} wide")),
                    ]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(", ")
                })
                .collect()
        })
        .unwrap_or_default();
    l.line(
        "Size:",
        format!(
            "{}{}",
            sizes.join("; "),
            str_at(x, "sizeNote").map(|n| format!(" ({n})")).unwrap_or_default()
        ),
    );
    l.line("Hooks:", x.get("hooks").map(describe).unwrap_or_default());
    l.line("Stitches:", join(strings_at(x, "stitches")));
    let see_also: Vec<String> = [("seeAlsoCreature", "creature"), ("seeAlsoItem", "item")]
        .into_iter()
        .flat_map(|(key, tag)| strings_at(x, key).into_iter().map(move |n| format!("{{@{tag} {n}}}")))
        .collect();
    l.line("See also:", see_also.join(", "));
    for (title, key) in [
        ("Yarn", "yarn"),
        ("Notions", "notions"),
        ("Gauge", "gauge"),
        ("Notes", "notes"),
        ("Abbreviations", "abbreviations"),
        ("Instructions", "instructions"),
        ("Finishing", "finishing"),
    ] {
        if let Some(v) = x.get(key) {
            l.section(title, v);
        }
    }
}
