//! Inline `{@tag ...}` markup: styling, combat text, dice, and links to
//! other entities. Tag semantics follow 5etools' own renderer.

use dioxus::prelude::*;

use super::RenderCtx;
use crate::routes::EntityKind;
use crate::statblock::format::{ability_full, capitalize, ordinal, signed};
use crate::tags::{parse_inline, InlineNode};
use crate::EntityTarget;

pub fn render_inline(text: &str, ctx: RenderCtx) -> Element {
    let nodes = parse_inline(text);
    rsx! {
        for (i , node) in nodes.iter().enumerate() {
            span { key: "{i}", {render_inline_node(node, ctx)} }
        }
    }
}

fn render_inline_node(node: &InlineNode, ctx: RenderCtx) -> Element {
    match node {
        InlineNode::Text(s) => rsx! { "{s}" },
        InlineNode::Tag { name, args } => render_tag(name, args, ctx),
    }
}

fn arg(args: &[String], i: usize) -> &str {
    args.get(i).map(String::as_str).unwrap_or("")
}

/// The arg at `i` if present and non-empty.
fn opt_arg(args: &[String], i: usize) -> Option<&str> {
    args.get(i).map(String::as_str).filter(|s| !s.is_empty())
}

/// A tag argument that's shown as-is: its markup, if any, rendered.
fn text(args: &[String], i: usize, ctx: RenderCtx) -> Element {
    render_inline(arg(args, i), ctx)
}

fn italic(label: &str) -> Element {
    rsx! { em { "{label}" } }
}

fn render_tag(name: &str, args: &[String], ctx: RenderCtx) -> Element {
    match name {
        "b" | "bold" => rsx! { strong { class: "font-semibold", {text(args, 0, ctx)} } },
        "i" | "italic" => rsx! { em { {text(args, 0, ctx)} } },
        "u" | "underline" | "u2" | "underlineDouble" => rsx! { u { {text(args, 0, ctx)} } },
        "s" | "strike" | "s2" | "strikeDouble" => rsx! { s { {text(args, 0, ctx)} } },
        "sup" => rsx! { sup { {text(args, 0, ctx)} } },
        "sub" => rsx! { sub { {text(args, 0, ctx)} } },
        "kbd" => rsx! { kbd { class: "rounded border border-gray-300 px-1 text-xs", {text(args, 0, ctx)} } },
        "code" => rsx! { code { class: "rounded bg-gray-100 px-1 text-xs", {text(args, 0, ctx)} } },
        "note" => rsx! { span { class: "text-xs italic text-gray-500", {text(args, 0, ctx)} } },
        "highlight" => rsx! { mark { {text(args, 0, ctx)} } },
        "tip" | "help" => rsx! {
            span { class: "cursor-help underline decoration-dotted", title: "{arg(args, 1)}", {text(args, 0, ctx)} }
        },
        "footnote" => rsx! {
            span { class: "cursor-help underline decoration-dotted", title: "{arg(args, 1)}", {text(args, 0, ctx)} }
        },
        "homebrew" => text(args, 0, ctx),
        "color" => match opt_arg(args, 1).filter(|c| c.chars().all(|ch| ch.is_ascii_hexdigit())) {
            Some(hex) => rsx! { span { style: "color: #{hex}", {text(args, 0, ctx)} } },
            None => text(args, 0, ctx),
        },
        "unit" => rsx! { "{unit_text(args)}" },

        "atk" => rsx! { em { "{attack_text(arg(args, 0), false)}" } },
        "atkr" => rsx! { em { "{attack_text(arg(args, 0), true)}" } },
        "h" => rsx! { em { "Hit:" } " " },
        "m" => rsx! { em { "Miss:" } " " },
        "hom" => rsx! { em { "Hit or Miss:" } " " },
        "actSave" => italic(&format!("{} Saving Throw:", ability_full(&arg(args, 0).to_lowercase()))),
        "actSaveSuccess" => italic("Success:"),
        "actSaveFail" => match opt_arg(args, 0) {
            Some(n) => italic(&format!("{} Failure:", capitalize(&number_word(n)))),
            None => italic("Failure:"),
        },
        "actSaveFailBy" => italic(&format!("Failure by {} or More:", arg(args, 0))),
        "actSaveSuccessOrFail" => italic("Failure or Success:"),
        "actTrigger" => italic("Trigger:"),
        "actResponse" => italic(if arg(args, 0).contains('d') {
            "Response—"
        } else {
            "Response:"
        }),
        "hit" | "d20" | "initiative" => {
            let display = opt_arg(args, 1)
                .map(String::from)
                .unwrap_or_else(|| modifier_text(arg(args, 0)));
            rsx! { span { class: "font-semibold", "{display}" } }
        }
        "dc" => rsx! { "DC " span { class: "font-semibold", "{opt_arg(args, 1).unwrap_or(arg(args, 0))}" } },
        "dcYourSpellSave" => rsx! { "{opt_arg(args, 0).unwrap_or(\"your spell save DC\")}" },
        "hitYourSpellAttack" => rsx! { "{opt_arg(args, 0).unwrap_or(\"your spell attack modifier\")}" },
        "recharge" => {
            let n: u32 = arg(args, 0).parse().unwrap_or(6);
            let range = if n < 6 { format!("{n}–6") } else { "6".to_string() };
            if arg(args, 1).contains('m') {
                rsx! { "Recharge {range}" }
            } else {
                rsx! { "(Recharge {range})" }
            }
        }
        "chance" => {
            let display = opt_arg(args, 1)
                .map(String::from)
                .unwrap_or_else(|| format!("{}%", arg(args, 0)));
            rsx! { span { class: "font-semibold", "{display}" } }
        }
        "coinflip" => rsx! { "{opt_arg(args, 0).unwrap_or(\"flip a coin\")}" },
        "dice" | "autodice" | "damage" => {
            let roll = arg(args, 0);
            let display = opt_arg(args, 1)
                .map(String::from)
                .unwrap_or_else(|| roll.replace(';', "/"));
            rsx! { span { class: "font-semibold", "{display}" } }
        }
        "scaledice" | "scaledamage" => {
            let display = opt_arg(args, 3).or_else(|| opt_arg(args, 0)).unwrap_or("");
            rsx! { span { class: "font-semibold", "{display}" } }
        }
        "ability" => {
            let display = opt_arg(args, 1)
                .map(String::from)
                .unwrap_or_else(|| ability_text(arg(args, 0)));
            rsx! { "{display}" }
        }
        "savingThrow" => {
            let display = opt_arg(args, 1).map(String::from).unwrap_or_else(|| {
                let rest = arg(args, 0).split_once(' ').map(|(_, r)| r).unwrap_or("");
                modifier_text(rest)
            });
            rsx! { span { class: "font-semibold", "{display}" } }
        }
        "skillCheck" => {
            let display = opt_arg(args, 1).map(String::from).unwrap_or_else(|| {
                let bonus = arg(args, 0).split_once(' ').map(|(_, r)| r).unwrap_or("");
                modifier_text(bonus)
            });
            rsx! { span { class: "font-semibold", "{display}" } }
        }

        // Links out of the entity graph.
        "link" => {
            let url = opt_arg(args, 1).unwrap_or(arg(args, 0));
            let url = if url.contains("://") || url.starts_with("mailto:") {
                url.to_string()
            } else {
                format!("http://{url}")
            };
            rsx! {
                a { class: "text-blue-700 underline", href: "{url}", target: "_blank", rel: "noopener noreferrer", {text(args, 0, ctx)} }
            }
        }
        "filter" | "5etools" | "5etoolsImg" | "5etoolsAudio" | "book" | "adventure" | "area" | "loader" => {
            text(args, 0, ctx)
        }
        "quickref" => match opt_arg(args, 3) {
            Some(display) => render_inline(display, ctx),
            None => text(args, 0, ctx),
        },
        "classFeature" | "subclassFeature" => match opt_arg(args, 4) {
            Some(display) => render_inline(display, ctx),
            None => text(args, 0, ctx),
        },
        "font" | "style" | "comic" | "comicH1" | "comicH2" | "comicH3" | "comicH4" | "comicNote" => text(args, 0, ctx),

        "subclass" => {
            let display = opt_arg(args, 4).unwrap_or(arg(args, 0));
            let source = opt_arg(args, 3);
            link_to(EntityKind::Subclasses, arg(args, 0), source, display, ctx)
        }
        "deity" => {
            let display = opt_arg(args, 3).unwrap_or(arg(args, 0));
            link_to(EntityKind::Deities, arg(args, 0), opt_arg(args, 2), display, ctx)
        }
        "card" => {
            let display = opt_arg(args, 3).unwrap_or(arg(args, 0));
            link_to(EntityKind::Cards, arg(args, 0), opt_arg(args, 2), display, ctx)
        }
        "itemProperty" => item_property_link(args, ctx),
        _ => match tag_kind(name) {
            Some(kind) => entity_link(kind, args, ctx),
            // Unrecognized tag: best-effort render its first argument as
            // plain text rather than dropping/crashing.
            None => text(args, 0, ctx),
        },
    }
}

/// The entity kind a `{@tag Name|Source|Display}` link points at.
pub(super) fn tag_kind(tag: &str) -> Option<EntityKind> {
    Some(match tag {
        "creature" => EntityKind::Bestiary,
        "spell" => EntityKind::Spells,
        "item" => EntityKind::Items,
        "class" => EntityKind::Classes,
        "race" => EntityKind::Races,
        "background" => EntityKind::Backgrounds,
        "feat" => EntityKind::Feats,
        "condition" | "status" | "disease" => EntityKind::Conditions,
        "object" => EntityKind::Objects,
        "action" => EntityKind::Actions,
        "hazard" | "trap" => EntityKind::Hazards,
        "vehicle" | "vehupgrade" => EntityKind::Vehicles,
        "variantrule" => EntityKind::VariantRules,
        "optfeature" => EntityKind::OptionalFeatures,
        "reward" => EntityKind::Rewards,
        "psionic" => EntityKind::Psionics,
        "recipe" => EntityKind::Recipes,
        "cult" | "boon" => EntityKind::CultsBoons,
        "language" => EntityKind::Languages,
        "charoption" => EntityKind::CharOptions,
        "facility" => EntityKind::Facilities,
        "itemMastery" => EntityKind::ItemMasteries,
        "skill" => EntityKind::Skills,
        "sense" => EntityKind::Senses,
        "table" => EntityKind::Tables,
        "deck" => EntityKind::Decks,
        "legroup" => EntityKind::LegendaryGroups,
        "crochet" => EntityKind::CrochetPatterns,
        "subclass" => EntityKind::Subclasses,
        "deity" => EntityKind::Deities,
        "card" => EntityKind::Cards,
        _ => return None,
    })
}

fn unit_text(args: &[String]) -> String {
    let single = arg(args, 1);
    match arg(args, 0).parse::<f64>() {
        Ok(n) if n > 1.0 => opt_arg(args, 2)
            .map(String::from)
            .unwrap_or_else(|| format!("{single}s")),
        _ => single.to_string(),
    }
}

fn number_word(n: &str) -> String {
    const WORDS: [&str; 10] = [
        "zero", "first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth", "ninth",
    ];
    n.parse::<usize>()
        .ok()
        .and_then(|i| WORDS.get(i).map(|w| w.to_string()))
        .unwrap_or_else(|| match n.parse::<i64>() {
            Ok(i) => ordinal(i),
            Err(_) => n.to_string(),
        })
}

/// `{@atk mw}` -> "Melee Weapon Attack:", `{@atkr m,r}` -> "Melee or Ranged
/// Attack Roll:" (5etools' `attackTagToFull`).
fn attack_text(codes: &str, is_roll: bool) -> String {
    let groups: Vec<Vec<char>> = codes
        .to_lowercase()
        .split(',')
        .map(|g| g.trim().chars().collect::<Vec<_>>())
        .filter(|g| !g.is_empty())
        .collect();
    let mut seen: Vec<char> = groups.last().cloned().unwrap_or_default();
    let mut labels: Vec<String> = Vec::new();
    for (i, group) in groups.iter().enumerate().rev() {
        let kept: Vec<char> = if i + 1 == groups.len() {
            group.clone()
        } else {
            group
                .iter()
                .copied()
                .filter(|c| {
                    !seen.contains(c) && {
                        seen.push(*c);
                        true
                    }
                })
                .collect()
        };
        let has = |c: char| kept.contains(&c);
        let ty = if has('m') {
            "Melee "
        } else if has('r') {
            "Ranged "
        } else if has('g') {
            "Magical "
        } else if has('a') {
            "Area "
        } else {
            ""
        };
        let method = if has('w') {
            "Weapon "
        } else if has('s') {
            "Spell "
        } else if has('p') {
            "Power "
        } else {
            ""
        };
        labels.push(format!("{ty}{method}"));
    }
    labels.reverse();
    format!("{}Attack{}:", labels.join(" or "), if is_roll { " Roll" } else { "" })
}

/// "+4" for `4` and `+4`, "-1" for `-1`; other expressions ("+ PB") pass through.
fn modifier_text(raw: &str) -> String {
    let raw = raw.trim();
    match raw.parse::<i64>() {
        Ok(n) => signed(n),
        Err(_) if raw.starts_with(['+', '-']) => raw.to_string(),
        Err(_) => format!("+{raw}"),
    }
}

/// `{@ability str 20}` -> "20 (+5)".
fn ability_text(raw: &str) -> String {
    let score = raw
        .split_once(' ')
        .and_then(|(_, s)| s.parse::<i32>().ok())
        .unwrap_or(0);
    format!(
        "{score}\u{a0}({})",
        signed(crate::statblock::format::ability_mod(score))
    )
}

/// An entity-link tag: `{@tag Name|Source|DisplayText}`, both `Source` and
/// `DisplayText` optional.
fn entity_link(kind: EntityKind, args: &[String], ctx: RenderCtx) -> Element {
    let name = arg(args, 0);
    let display = opt_arg(args, 2).unwrap_or(name);
    link_to(kind, name, opt_arg(args, 1), display, ctx)
}

/// `{@itemProperty V|SRC}` names a property by its abbreviation.
fn item_property_link(args: &[String], ctx: RenderCtx) -> Element {
    let abbreviation = arg(args, 0);
    let source = opt_arg(args, 1);
    let found = ctx.library.entities_of(EntityKind::ItemProperties).find(|p| {
        p.extra()
            .get("abbreviation")
            .and_then(|a| a.as_str())
            .is_some_and(|a| a.eq_ignore_ascii_case(abbreviation))
            && source.is_none_or(|s| p.source().eq_ignore_ascii_case(s))
    });
    match found {
        Some(p) => link_to(
            EntityKind::ItemProperties,
            p.name(),
            Some(p.source()),
            opt_arg(args, 2).unwrap_or(p.name()),
            ctx,
        ),
        None => rsx! { "{opt_arg(args, 2).unwrap_or(abbreviation)}" },
    }
}

/// Hovering floats a preview above every card (see
/// `layout::HoverPopupOverlay`) unless the link points back at the entity
/// whose card is currently being rendered (`ctx.current`). A plain click opens the
/// target maximized in the modal overlay (`layout::ModalOverlay`) instead
/// of navigating; a modified click (ctrl/cmd/shift/middle-click) - "open in
/// a new tab" - is left alone, since this renders a real link to the
/// `/entities/:kind/:source/:name` permalink and `Link`'s `onclick_only`
/// only suppresses its own default navigation for a plain click, not a
/// modified one.
fn link_to(kind: EntityKind, name: &str, source: Option<&str>, display: &str, ctx: RenderCtx) -> Element {
    if name.is_empty() {
        return rsx! {};
    }
    let Some(found) = ctx.library.find_for_ruleset(kind, source, name, ctx.use_2024) else {
        return render_inline(display, ctx);
    };
    let target = EntityTarget {
        kind,
        source: found.source().to_string(),
        name: found.name().to_string(),
    };

    let is_self = ctx.current.is_some_and(|(k, s, n)| {
        k == kind && s.eq_ignore_ascii_case(&target.source) && n.eq_ignore_ascii_case(&target.name)
    });

    // Computed before either closure below moves `target` into itself.
    let permalink = crate::routes::Route::EntityDetail {
        kind,
        source: target.source.clone(),
        name: crate::routes::NameSegment(target.name.clone()),
    };

    let modal = ctx.modal;
    let mut hover = ctx.hover.0;
    let onclick = {
        let target = target.clone();
        move |_| {
            hover.set(None);
            crate::modal_history::open_modal(modal, target.clone());
        }
    };
    let onmouseenter = move |evt: MouseEvent| {
        if is_self {
            return;
        }
        let c = evt.client_coordinates();
        hover.set(Some((target.clone(), c.x, c.y)));
    };
    let onmouseleave = move |_| hover.set(None);

    rsx! {
        span { onmouseenter, onmouseleave,
            Link {
                class: "cursor-pointer text-blue-700 underline decoration-dotted underline-offset-2 hover:decoration-solid",
                to: permalink,
                onclick_only: true,
                onclick,
                {render_inline(display, ctx)}
            }
        }
    }
}
