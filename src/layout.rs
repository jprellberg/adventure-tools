use dioxus::prelude::*;

use crate::card::source_badge_classes;
use crate::data::Library;
use crate::pins::{PinList, Pins};
use crate::render::{render_entity_preview, RenderCtx};
use crate::routes::{EntityKind, Route, View};
use crate::search::{Facet, Filters, Scale};
use crate::state::LibrarySignal;
use crate::{EntityTarget, HoverSignal, LastDetail, ResyncRequest, SearchInput, UpdateAvailable, Use2024Rules};

/// How long to wait after the last keystroke before re-running the search:
/// long enough that fast typing doesn't re-filter the whole corpus on every
/// character, short enough to still feel immediate.
const SEARCH_DEBOUNCE_MS: u32 = 150;

/// The states of the view toggle: the view, its glyph and its tooltip.
const VIEW_TOGGLES: [(View, &str, &str); 3] = [
    (View::Results, "≡", "Search results"),
    (View::Pins, "📌", "Pinned entities"),
    (View::Detail, "▢", "Entity detail"),
];

/// The classes of a view toggle segment that is selected or not.
fn view_toggle_state(on: bool) -> &'static str {
    if on {
        "bg-gray-100 text-gray-900"
    } else {
        "bg-white text-gray-400 hover:text-gray-600"
    }
}

/// The entity the detail view shows. A book's page counts as showing the
/// book.
fn shown_detail(route: &Route, library: Option<&Library>) -> Option<EntityTarget> {
    match route {
        Route::Book { source, .. } => {
            let book = library?.book(source)?;
            Some(EntityTarget {
                kind: EntityKind::Books,
                source: book.source.clone(),
                name: book.name.clone(),
            })
        }
        Route::EntityDetail { detail } => detail.0.clone(),
        _ => None,
    }
}

/// Keeps the app state and the address in step whenever the route changes:
/// the pinned view's address spells out the current pins (rewritten in place
/// whenever they change), a search address puts its query into the search
/// bar, and the detail view's entity is kept for the way back to it.
fn follow_route() {
    let route = use_route::<Route>();
    let nav = use_navigator();
    let pins = PinList(use_context::<Pins>().list.read().clone());
    let library = use_context::<LibrarySignal>();
    let mut last_detail = use_context::<LastDetail>().0;
    let mut search_input = use_context::<SearchInput>().0;
    use_effect(use_reactive!(|route, pins| {
        if route.view() == Some(View::Pins) {
            let address = Route::Pins { pins };
            if route != address {
                nav.replace(address);
            }
        }
    }));
    use_effect(use_reactive!(|route| {
        match &route {
            Route::Search { query } if *search_input.peek() != query.0 => search_input.set(query.0.clone()),
            Route::EntityDetail { .. } | Route::Book { .. } => {
                let detail = shown_detail(&route, library.peek().as_deref());
                if *last_detail.peek() != detail {
                    last_detail.set(detail);
                }
            }
            _ => {}
        }
    }));
}

#[component]
pub fn RootLayout() -> Element {
    crate::viewport::provide_viewport();
    follow_route();

    rsx! {
        div { class: "min-h-screen bg-white",
            SearchHeader {}
            main { class: "px-4 pb-4 pt-20", Outlet::<Route> {} }
            HoverPopupOverlay {}
        }
    }
}

/// The app's single search bar, fixed at the top of every
/// route so it stays reachable - and stays focused - no matter where the
/// user navigates. Typing while another view is showing jumps to the search
/// results so the results it produces are visible immediately. The toggle
/// to its left switches between the views.
#[component]
fn SearchHeader() -> Element {
    let mut search_input = use_context::<SearchInput>().0;
    let update_available = use_context::<Signal<UpdateAvailable>>();
    let resync = use_context::<ResyncRequest>();
    let mut pins = use_context::<Pins>();
    let last_detail = use_context::<LastDetail>().0;
    let mut filters_open = use_signal(|| false);
    let nav = use_navigator();
    let router = router();
    let view = use_route::<Route>().view();
    // The filter menu belongs to the search results and closes with them.
    use_effect(use_reactive!(|view| {
        if view != Some(View::Results) && *filters_open.peek() {
            filters_open.set(false);
        }
    }));
    // The view the search results were opened from, which ESC returns to.
    let mut came_from = use_signal(|| View::Pins);
    let mut show = move |target: View| {
        let route = router.current::<Route>();
        if target == View::Results && route.view() != Some(View::Results) {
            came_from.set(route.view().unwrap_or(View::Pins));
        }
        nav.push(match target {
            View::Results => Route::search(&search_input.peek()),
            View::Pins => Route::Pins {
                pins: PinList(pins.list.peek().clone()),
            },
            View::Detail => Route::detail_of(last_detail.peek().clone()),
        });
    };
    // Bumped on every keystroke so a pending debounce task can tell it's
    // been superseded and skip committing its now-stale value.
    let mut debounce_generation = use_signal(|| 0u32);

    // Instead of holding focus (which makes text unselectable), the search
    // input takes it back when the user types anywhere outside another text
    // field - or, inside a filter list, that list's search field does.
    // Escape anywhere goes to the search input.
    use_hook(|| {
        document::eval(
            r#"
            if (!window.__searchTypeAhead) {
                window.__searchTypeAhead = true;
                // A list re-renders when a checkbox in it is clicked, which
                // drops the focus, so the list last clicked in is remembered.
                let lastList = null;
                document.addEventListener("pointerdown", (e) => {
                    lastList = e.target.closest ? e.target.closest("[data-filter-list]") : null;
                });
                document.addEventListener("keydown", (e) => {
                    const search = document.getElementById("search-input");
                    const t = e.target;
                    if (!search || e.ctrlKey || e.metaKey || e.altKey) return;
                    if (e.key === "Escape") {
                        if (t !== search) {
                            search.focus();
                            search.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
                        }
                        return;
                    }
                    const list = (t.closest && t.closest("[data-filter-list]")) || (lastList && lastList.isConnected && lastList);
                    const input = (list && list.querySelector(".filter-search")) || search;
                    if (t === input) return;
                    const field = "input:not([type=checkbox]):not([type=range]), textarea, select, [contenteditable]";
                    if (t.closest && t.closest(field)) return;
                    if (e.key.length === 1 && e.key !== " ") input.focus();
                });
            }
            "#,
        );
    });

    // Empties the search (Escape, or the clear button - there's no Escape
    // key on a phone) and leaves the results for the view they were opened
    // from.
    let mut clear = move || {
        search_input.set(String::new());
        debounce_generation.set(debounce_generation() + 1);
        if view == Some(View::Results) {
            show(came_from());
        }
    };

    let filters = use_context::<Signal<Filters>>();
    let rules = use_context::<Signal<Use2024Rules>>();
    let dots_state = if filters_open() {
        "bg-gray-100 text-gray-900"
    } else if !filters.read().is_default_for(rules().0) {
        // Something beyond the ruleset is filtered out.
        "border-amber-400 bg-amber-100 text-amber-800"
    } else {
        "bg-white text-gray-600"
    };

    // While the filter menu is open the header only frames the menu, so the
    // page stays visible (dimmed by the backdrop) on either side of it.
    let header_state = if filters_open() {
        "pointer-events-none"
    } else {
        "bg-white/95 backdrop-blur"
    };
    let card_state = if filters_open() {
        "border-gray-300 bg-white"
    } else {
        "border-transparent"
    };

    rsx! {
        if filters_open() {
            div {
                class: "fixed inset-0 z-30 bg-black/10",
                onclick: move |_| filters_open.set(false),
            }
        }
        header { class: "fixed inset-x-0 top-0 z-40 flex items-start justify-center gap-1 px-4 py-2.5 {header_state}",
            div { class: "pointer-events-auto min-w-0 max-w-xl flex-1 rounded-2xl border p-1.5 {card_state}",
                div { class: "flex items-center gap-2",
                    div { class: "flex shrink-0 overflow-hidden rounded-full border border-gray-300",
                        for (target, glyph, title) in VIEW_TOGGLES {
                            button {
                                key: "{glyph}",
                                class: "h-[2.125rem] w-8 text-sm leading-none {view_toggle_state(view == Some(target))}",
                                title,
                                aria_label: title,
                                aria_pressed: view == Some(target),
                                onclick: move |_| show(target),
                                "{glyph}"
                            }
                        }
                    }
                    div { class: "relative min-w-0 flex-1",
                        input {
                            class: "w-full rounded-full border border-gray-300 py-2 pl-4 pr-10 text-sm outline-none focus:border-gray-400 [&::-webkit-search-cancel-button]:appearance-none",
                            r#type: "search",
                            placeholder: "Search the reference database…",
                            value: "{search_input}",
                            id: "search-input",
                            autofocus: true,
                            oninput: move |evt| {
                                let value = evt.value();
                                search_input.set(value.clone());
                                if view != Some(View::Results) {
                                    show(View::Results);
                                    return;
                                }
                                let generation = debounce_generation() + 1;
                                debounce_generation.set(generation);
                                // The results follow the query in the address.
                                spawn(async move {
                                    gloo_timers::future::TimeoutFuture::new(SEARCH_DEBOUNCE_MS).await;
                                    let showing_results = router.current::<Route>().view()
                                        == Some(View::Results);
                                    if debounce_generation() == generation && showing_results {
                                        nav.replace(Route::search(&value));
                                    }
                                });
                            },
                            onkeydown: move |evt| {
                                // Escape pressed elsewhere is forwarded here by the
                                // type-ahead listener above.
                                if evt.key() == Key::Escape {
                                    if filters_open() {
                                        filters_open.set(false);
                                        return;
                                    }
                                    clear();
                                }
                            },
                        }
                        if !search_input.read().is_empty() {
                            button {
                                class: "absolute inset-y-0 right-1 my-auto h-8 w-8 rounded-full text-lg leading-none text-gray-400 hover:text-gray-600",
                                title: "Clear search",
                                aria_label: "Clear search",
                                onclick: move |_| clear(),
                                "✕"
                            }
                        }
                    }
                    match view {
                        Some(View::Results) => rsx! {
                            button {
                                class: "h-9 w-9 shrink-0 rounded-full border border-gray-300 text-lg leading-none hover:border-gray-400 {dots_state}",
                                title: "Filters",
                                aria_label: "Filters",
                                aria_expanded: filters_open(),
                                onclick: move |_| filters_open.set(!filters_open()),
                                "⋮"
                            }
                        },
                        Some(View::Pins) => rsx! {
                            button {
                                class: "h-9 w-9 shrink-0 rounded-full border border-gray-300 bg-white text-base leading-none text-gray-600 hover:border-gray-400 disabled:text-gray-300 disabled:hover:border-gray-300",
                                title: "Unpin everything",
                                aria_label: "Unpin everything",
                                disabled: pins.list.read().is_empty(),
                                onclick: move |_| async move {
                                    let confirmed = document::eval("return confirm('Unpin every entity?')")
                                        .join::<bool>()
                                        .await
                                        .unwrap_or(false);
                                    if confirmed {
                                        pins.set(Vec::new());
                                    }
                                },
                                svg {
                                    class: "mx-auto h-4 w-4",
                                    view_box: "0 0 24 24",
                                    fill: "none",
                                    stroke: "currentColor",
                                    stroke_width: "1.75",
                                    stroke_linecap: "round",
                                    stroke_linejoin: "round",
                                    path { d: "M4 7h16M10 11v6M14 11v6M5 7l1 12a2 2 0 0 0 2 2h8a2 2 0 0 0 2-2l1-12M9 7V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v3" }
                                }
                            }
                        },
                        _ => rsx! {},
                    }
                }
                if filters_open() {
                    FilterPanel {}
                }
            }
            if update_available().0 {
                button {
                    class: "pointer-events-auto mt-1.5 shrink-0 rounded-full border border-blue-300 bg-blue-50 px-3 py-2 text-sm text-blue-700 hover:bg-blue-100",
                    title: "A data update is available - click to download it",
                    aria_label: "Update data",
                    onclick: move |_| resync.send(()),
                    "⭯"
                }
            }
        }
    }
}

/// The filter options below the search bar: a checkbox list each for the
/// kinds and sources shown (the latter with the ruleset toggle, which
/// switches the core rulebooks), and the kind-specific filters, each of
/// which restricts the results to its own kind.
#[component]
fn FilterPanel() -> Element {
    let library = use_context::<LibrarySignal>();
    let mut rules = use_context::<Signal<Use2024Rules>>();
    let mut filters = use_context::<Signal<Filters>>();
    let Some(lib) = library.read().clone() else {
        return rsx! {};
    };
    let current = filters.read().clone();

    let kinds: Vec<SelectOption> = EntityKind::all()
        .map(|kind| SelectOption {
            label: kind.label().to_string(),
            title: None,
            classes: Some(kind.badge_classes()),
            checked: current.kind_shown(kind),
        })
        .collect();
    let sources: Vec<SelectOption> = lib
        .search_index
        .sources
        .iter()
        .map(|source| SelectOption {
            label: source.to_string(),
            title: lib.source_full_name(source).map(str::to_string),
            classes: Some(source_badge_classes(source)),
            checked: current.source_shown(source),
        })
        .collect();
    let range_slider = |scale: Scale| {
        rsx! {
            RangeSlider {
                scale,
                steps: lib.search_index.scale_steps(scale).to_vec(),
            }
        }
    };
    let facet_select = |facet: Facet| {
        let options: Vec<SelectOption> = lib
            .search_index
            .facet_options(facet)
            .iter()
            .map(|value| SelectOption {
                label: value.to_string(),
                title: None,
                classes: None,
                checked: current.facet_selected(facet, value),
            })
            .collect();
        let values = lib.search_index.facet_options(facet).to_vec();
        rsx! {
            FilterRow { label: facet.label(),
                MultiSelect {
                    options,
                    columns: match facet {
                        Facet::Habitat | Facet::Rarity => "columns-2",
                        Facet::SpellClass => "columns-3",
                    },
                    on_toggle: move |i: usize| {
                        filters.write().toggle_facet(facet, &values[i]);
                    },
                    ListAction {
                        label: "clear",
                        active: current.facet_set(facet),
                        disabled: !current.facet_set(facet),
                        onclick: move |_| {
                            filters.write().clear_facet(facet);
                        },
                    }
                }
            }
        }
    };

    rsx! {
        div { class: "mt-1.5 max-h-[70vh] space-y-3 overflow-y-auto border-t border-gray-200 px-1 pb-1 pt-2.5",
            if let Some(warning) = filter_warning(&current) {
                p { class: "rounded-lg border border-amber-300 bg-amber-50 px-2.5 py-1.5 text-xs text-amber-800",
                    "⚠ {warning}"
                }
            }
            div { class: "space-y-3",
                FilterGroup { label: "Show",
                    FilterRow { label: "Kinds",
                        MultiSelect {
                            options: kinds,
                            on_toggle: move |i: usize| {
                                if let Some(kind) = EntityKind::all().nth(i) {
                                    filters.write().toggle_kind(kind);
                                }
                            },
                            for (shown, label) in [(true, "all"), (false, "none")] {
                                ListAction {
                                    key: "{label}",
                                    label,
                                    onclick: move |_| {
                                        filters.write().set_all_kinds(shown);
                                    },
                                }
                            }
                        }
                    }
                    FilterRow { label: "Sources",
                        MultiSelect {
                            options: sources,
                            on_toggle: {
                                let lib = lib.clone();
                                move |i: usize| {
                                    filters.write().toggle_source(&lib.search_index.sources[i]);
                                }
                            },
                            for (shown, label) in [(true, "all"), (false, "none")] {
                                ListAction {
                                    key: "{label}",
                                    label,
                                    onclick: {
                                        let lib = lib.clone();
                                        move |_| {
                                            filters.write().set_all_sources(shown, &lib.search_index.sources);
                                        }
                                    },
                                }
                            }
                            for (use_2024, label) in [(false, "2014"), (true, "2024")] {
                                ListAction {
                                    key: "{label}",
                                    label,
                                    title: "Show the {label} core rulebooks and hide the other edition's",
                                    pressed: rules().0 == use_2024,
                                    onclick: move |_| {
                                        rules.set(Use2024Rules(use_2024));
                                        filters.write().set_ruleset(use_2024);
                                    },
                                }
                            }
                        }
                    }
                }
                div { class: "space-y-3",
                    FilterGroup { label: EntityKind::Bestiary.label(),
                        {range_slider(Scale::Cr)}
                        {facet_select(Facet::Habitat)}
                    }
                    FilterGroup { label: EntityKind::Items.label(), {facet_select(Facet::Rarity)} }
                    FilterGroup { label: EntityKind::Spells.label(),
                        {range_slider(Scale::SpellLevel)}
                        {facet_select(Facet::SpellClass)}
                    }
                }
            }
        }
    }
}

/// Why the filters let nothing through, when it is the kind-specific
/// filters' doing: they are set for more than one kind, or for a kind that
/// is switched off.
fn filter_warning(filters: &Filters) -> Option<String> {
    let kinds = filters.facet_kinds();
    let labels: Vec<&str> = kinds.iter().map(|k| k.label()).collect();
    match kinds.as_slice() {
        [] => None,
        [kind] if !filters.kind_shown(*kind) => Some(format!(
            "{} filters are set, but {} is switched off under Kinds, so nothing matches.",
            labels[0], labels[0]
        )),
        [_] => None,
        _ => Some(format!(
            "Filters are set for {}. Each limits the results to its own kind, so nothing matches.",
            labels.join(" and ")
        )),
    }
}

/// A headed group of filter rows.
#[component]
fn FilterGroup(label: &'static str, children: Element) -> Element {
    rsx! {
        section { class: "space-y-1.5",
            div { class: "text-[10px] font-medium uppercase tracking-wide text-gray-400",
                "{label}"
            }
            {children}
        }
    }
}

/// One filter control with its label to the left.
#[component]
fn FilterRow(label: &'static str, children: Element) -> Element {
    rsx! {
        div { class: "flex items-start gap-2",
            span { class: "w-14 shrink-0 pt-1 text-xs text-gray-500", "{label}" }
            div { class: "min-w-0 flex-1", {children} }
        }
    }
}

/// A small text button acting on a whole filter ("all", "none", "clear", a
/// ruleset); `pressed` marks the one in effect, and `active` tints a
/// "clear" amber, like the filter button, while there is something to clear.
#[component]
fn ListAction(
    label: &'static str,
    title: Option<String>,
    pressed: Option<bool>,
    active: Option<bool>,
    disabled: Option<bool>,
    onclick: EventHandler<()>,
) -> Element {
    let state = match (pressed, active) {
        (Some(true), _) => "font-semibold text-gray-900",
        // The padding is offset by the margin so the label stays in place.
        (_, Some(true)) => "-mx-1 rounded bg-amber-100 px-1 text-amber-800 hover:bg-amber-200",
        _ => "text-gray-400 hover:text-gray-700 disabled:text-gray-200 disabled:hover:text-gray-200",
    };
    rsx! {
        button {
            class: "text-[10px] uppercase tracking-wide {state}",
            title,
            aria_pressed: pressed,
            disabled: disabled.unwrap_or(false),
            onclick: move |_| onclick.call(()),
            "{label}"
        }
    }
}

/// One checkbox of a [`MultiSelect`].
#[derive(Clone, PartialEq)]
struct SelectOption {
    label: String,
    title: Option<String>,
    /// Shows the label as a tag with these classes rather than as plain text.
    classes: Option<&'static str>,
    checked: bool,
}

/// A scrolling list of checkboxes, filtered by what is typed into its
/// search field, which typing anywhere in the list goes to; Enter toggles
/// the first match. `on_toggle` gets the index of the option clicked;
/// `columns` is a Tailwind `columns-*` class to lay the options out in
/// several columns; `children` are [`ListAction`]s beside the search field.
#[component]
fn MultiSelect(
    options: Vec<SelectOption>,
    columns: Option<&'static str>,
    on_toggle: EventHandler<usize>,
    children: Element,
) -> Element {
    // Options laid out in columns are few enough to show in full; a
    // height limit would overflow the columns sideways.
    let layout = columns.map_or("max-h-40 overflow-y-auto".to_string(), |c| format!("{c} gap-0"));
    let mut query = use_signal(String::new);
    let needle = query().to_lowercase();
    let visible: Vec<(usize, SelectOption)> = options
        .iter()
        .cloned()
        .enumerate()
        .filter(|(_, o)| {
            o.label.to_lowercase().contains(&needle)
                || o.title.as_ref().is_some_and(|t| t.to_lowercase().contains(&needle))
        })
        .collect();
    let first_visible = visible.first().map(|(i, _)| *i);

    rsx! {
        div {
            class: "rounded-lg border border-gray-200 bg-white",
            "data-filter-list": true,
            div { class: "flex items-center gap-2 border-b border-gray-200 px-2 py-1",
                input {
                    class: "filter-search min-w-0 flex-1 bg-transparent py-0.5 text-xs outline-none",
                    r#type: "text",
                    placeholder: "Type to filter…",
                    value: "{query}",
                    oninput: move |e| query.set(e.value()),
                    onkeydown: move |e| {
                        if e.key() == Key::Enter {
                            if let Some(i) = first_visible {
                                on_toggle.call(i);
                            }
                        }
                    },
                }
                div { class: "flex shrink-0 gap-2", {children} }
            }
            div { class: "py-1 {layout}",
                for (i, option) in visible {
                    label {
                        key: "{option.label}",
                        class: "flex break-inside-avoid cursor-pointer items-center gap-2 px-2 py-0.5 text-xs hover:bg-gray-50",
                        title: option.title.clone().unwrap_or_else(|| option.label.clone()),
                        input {
                            r#type: "checkbox",
                            class: "accent-gray-700",
                            checked: option.checked,
                            onchange: move |_| on_toggle.call(i),
                        }
                        if let Some(classes) = option.classes {
                            span { class: "shrink-0 rounded px-1.75 py-0.5 text-[10px] font-medium uppercase tracking-wide {classes}",
                                "{option.label}"
                            }
                        } else {
                            span { "{option.label}" }
                        }
                        if let (Some(title), Some(_)) = (&option.title, option.classes) {
                            span { class: "truncate text-gray-400", "{title}" }
                        }
                    }
                }
                if first_visible.is_none() {
                    p { class: "px-2 py-0.5 text-xs text-gray-400", "No matches" }
                }
            }
        }
    }
}

/// A range slider over every value on `scale` in the library, with a
/// draggable thumb for the lowest and one for the highest shown. A thumb at
/// its end of the scale leaves that end of the range open.
#[component]
fn RangeSlider(scale: Scale, steps: Vec<(Box<str>, f64)>) -> Element {
    let mut filters = use_context::<Signal<Filters>>();
    let Some(last) = steps.len().checked_sub(1).filter(|last| *last > 0) else {
        return rsx! {};
    };
    let position = |max: bool, open: usize| {
        filters
            .read()
            .range_bound(scale, max)
            .and_then(|bound| steps.iter().position(|(_, value)| *value == bound))
            .unwrap_or(open)
    };
    let min = position(false, 0);
    let max = position(true, last);
    let is_open = min == 0 && max == last;
    let summary = match (min, max) {
        _ if is_open => "Any".to_string(),
        (a, b) if a == b => steps[a].0.to_string(),
        (a, b) => format!("{} – {}", steps[a].0, steps[b].0),
    };
    let percent = |i: usize| i as f64 * 100.0 / last as f64;
    let (left, right) = (percent(min), 100.0 - percent(max));
    // The thumb last moved toward the top end lies on top, so that two thumbs
    // pushed together at either end can still be pulled apart.
    let min_on_top = min > last / 2;
    let id = move |max: bool| format!("{}-{}", scale.label().to_lowercase(), if max { "max" } else { "min" });
    let mut set = {
        let steps = steps.clone();
        move |is_max: bool, value: String| {
            let Ok(i) = value.parse::<usize>() else {
                return;
            };
            let (i, open_end) = if is_max { (i.max(min), last) } else { (i.min(max), 0) };
            let bound = (i != open_end).then_some(steps[i].1);
            filters.write().set_range_bound(scale, is_max, bound);
            // The input may have been dragged past the other thumb; put it back.
            document::eval(&format!("document.getElementById('{}').value = {i}", id(is_max)));
        }
    };
    let label = scale.label().to_lowercase();
    let thumb = "pointer-events-none absolute inset-0 h-4 w-full cursor-grab appearance-none bg-transparent [&::-webkit-slider-thumb]:pointer-events-auto [&::-webkit-slider-thumb]:h-3.5 [&::-webkit-slider-thumb]:w-3.5 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:border [&::-webkit-slider-thumb]:border-gray-500 [&::-webkit-slider-thumb]:bg-white [&::-moz-range-thumb]:pointer-events-auto [&::-moz-range-thumb]:h-3.5 [&::-moz-range-thumb]:w-3.5 [&::-moz-range-thumb]:rounded-full [&::-moz-range-thumb]:border [&::-moz-range-thumb]:border-gray-500 [&::-moz-range-thumb]:bg-white";

    rsx! {
        FilterRow { label: scale.label(),
            div { class: "flex items-center gap-2 pl-1 pr-[calc(0.5rem+1px)] pt-1",
                div { class: "relative h-4 flex-1",
                    div { class: "absolute inset-x-0 top-1.5 h-1 rounded-full bg-gray-200" }
                    div {
                        class: "absolute top-1.5 h-1 rounded-full bg-gray-500",
                        style: "left: {left}%; right: {right}%",
                    }
                    input {
                        id: id(false),
                        class: "{thumb}",
                        style: if min_on_top { "z-index: 2" } else { "z-index: 1" },
                        r#type: "range",
                        min: "0",
                        max: "{last}",
                        value: "{min}",
                        aria_label: "Lowest {label}",
                        oninput: {
                            let mut set = set.clone();
                            move |e: FormEvent| set(false, e.value())
                        },
                    }
                    input {
                        id: id(true),
                        class: "{thumb}",
                        style: "z-index: 1",
                        r#type: "range",
                        min: "0",
                        max: "{last}",
                        value: "{max}",
                        aria_label: "Highest {label}",
                        oninput: move |e: FormEvent| set(true, e.value()),
                    }
                }
                span { class: "w-20 shrink-0 text-xs text-gray-700 tabular-nums", "{summary}" }
                ListAction {
                    label: "clear",
                    active: !is_open,
                    disabled: is_open,
                    onclick: move |_| {
                        filters.write().clear_range(scale);
                    },
                }
            }
        }
    }
}

/// The floating entity preview a hyperlink shows on hover.
/// Rendered once here at the layout root - rather than nested inside each
/// card, as a pure-CSS `:hover` reveal would need to be - so it floats
/// above every card's own scroll/clip boundary instead of being clipped by
/// it.
#[component]
fn HoverPopupOverlay() -> Element {
    let hover = use_context::<HoverSignal>().0;
    let library = use_context::<LibrarySignal>();
    let Some((target, x, y)) = hover.read().clone() else {
        return rsx! {};
    };
    let Some(lib) = library.read().clone() else {
        return rsx! {};
    };
    let ctx = RenderCtx::new(&lib, HoverSignal(hover));

    rsx! {
        div {
            class: "fixed z-[300] w-80 max-h-[70vh] overflow-y-auto rounded border border-gray-300 bg-white p-2 text-xs shadow-lg",
            style: "pointer-events: none; left: min({x}px, calc(100vw - 336px)); top: min({y + 16.0}px, calc(100vh - 300px));",
            {render_entity_preview(target.kind, &target.name, &target.source, ctx)}
        }
    }
}
