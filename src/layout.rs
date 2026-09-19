use std::rc::Rc;

use dioxus::prelude::*;

use crate::card::source_badge_classes;
use crate::data::Library;
use crate::render::{render_entity_preview, RenderCtx};
use crate::routes::{EntityKind, Route, View};
use crate::search::Filters;
use crate::state::LibrarySignal;
use crate::{EntityTarget, HoverSignal, ResyncRequest, SearchInput, SearchQuery, UpdateAvailable, Use2024Rules};

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

/// The entity the detail view shows or - while another view is showing -
/// keeps for the way back. A book's page counts as showing the book.
fn current_detail(route: &Route, library: Option<&Library>) -> Option<EntityTarget> {
    if let Route::Book { source, .. } = route {
        let book = library?.book(source)?;
        return Some(EntityTarget {
            kind: EntityKind::Books,
            source: book.source.clone(),
            name: book.name.clone(),
        });
    }
    route.detail().cloned()
}

#[component]
pub fn RootLayout() -> Element {
    crate::viewport::provide_viewport();
    crate::pins::mirror_in_url(use_context::<crate::pins::PinsSignal>());

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
    let mut search_query = use_context::<SearchQuery>().0;
    let update_available = use_context::<Signal<UpdateAvailable>>();
    let resync = use_context::<ResyncRequest>();
    let library = use_context::<LibrarySignal>();
    let mut filters_open = use_signal(|| false);
    let nav = use_navigator();
    let router = router();
    let view = use_route::<Route>().view();
    // The view the search results were opened from, which ESC returns to.
    let mut came_from = use_signal(|| View::Pins);
    // Switches to `target`, which carries the detail view's entity along.
    let mut show = move |target: View| {
        let route = router.current::<Route>();
        if target == View::Results && route.view() != Some(View::Results) {
            came_from.set(route.view().unwrap_or(View::Pins));
        }
        nav.push(Route::of_view(
            target,
            current_detail(&route, library.read().as_deref()),
        ));
    };
    let mut input_el: Signal<Option<Rc<MountedData>>> = use_signal(|| None);
    // Bumped on every keystroke so a pending debounce task can tell it's
    // been superseded and skip committing its now-stale value.
    let mut debounce_generation = use_signal(|| 0u32);

    let refocus = move |_| {
        if let Some(el) = input_el.read().clone() {
            spawn(async move {
                let _ = el.set_focus(true).await;
            });
        }
    };

    // Empties the search (Escape, or the clear button - there's no Escape
    // key on a phone) and leaves the results for the view they were opened
    // from.
    let mut clear = move || {
        search_input.set(String::new());
        search_query.set(String::new());
        debounce_generation.set(debounce_generation() + 1);
        if view == Some(View::Results) {
            show(came_from());
        }
    };

    let dots_state = if filters_open() {
        "bg-gray-100 text-gray-900"
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
            div { class: "fixed inset-0 z-30 bg-black/10", onclick: move |_| filters_open.set(false) }
        }
        header { class: "fixed inset-x-0 top-0 z-40 flex items-start justify-center gap-2 px-4 py-2.5 {header_state}",
            div { class: "pointer-events-auto min-w-0 max-w-xl flex-1 rounded-2xl border p-1.5 {card_state}",
                div { class: "flex items-center gap-2",
                    div { class: "flex shrink-0 overflow-hidden rounded-full border border-gray-300",
                        for (target , glyph , title) in VIEW_TOGGLES {
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
                            autofocus: true,
                            onmounted: move |evt| input_el.set(Some(evt.data())),
                            oninput: move |evt| {
                                let value = evt.value();
                                search_input.set(value.clone());
                                if view != Some(View::Results) {
                                    show(View::Results);
                                }
                                let generation = debounce_generation() + 1;
                                debounce_generation.set(generation);
                                spawn(async move {
                                    gloo_timers::future::TimeoutFuture::new(SEARCH_DEBOUNCE_MS).await;
                                    if debounce_generation() == generation {
                                        search_query.set(value);
                                    }
                                });
                            },
                            onkeydown: move |evt| {
                                // This input aggressively re-steals focus on blur (see
                                // `refocus`), so it is the reliable place to catch Escape.
                                if evt.key() == Key::Escape {
                                    if filters_open() {
                                        filters_open.set(false);
                                        return;
                                    }
                                    clear();
                                }
                            },
                            onblur: refocus,
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
                    button {
                        class: "h-9 w-9 shrink-0 rounded-full border border-gray-300 text-lg leading-none hover:border-gray-400 {dots_state}",
                        title: "Filters",
                        aria_label: "Filters",
                        aria_expanded: filters_open(),
                        onclick: move |_| filters_open.set(!filters_open()),
                        "⋮"
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

/// The filter options below the search bar: the ruleset toggle (which
/// switches the core rulebooks' source buttons) and one on/off button per
/// entity kind and per source, tinted like the tags on the cards.
#[component]
fn FilterPanel() -> Element {
    let library = use_context::<LibrarySignal>();
    let mut rules = use_context::<Signal<Use2024Rules>>();
    let mut filters = use_context::<Signal<Filters>>();
    let Some(lib) = library.read().clone() else {
        return rsx! {};
    };

    rsx! {
        div { class: "mt-1.5 max-h-[60vh] space-y-3 overflow-y-auto border-t border-gray-200 px-1 pb-1 pt-2.5",
            FilterSection { label: "Rules",
                for (use_2024 , label) in [(false, "2014"), (true, "2024")] {
                    button {
                        key: "{label}",
                        class: "rounded-full border px-3 py-0.5 text-xs {toggle_state(rules().0 == use_2024)}",
                        title: "Show the {label} core rulebooks and hide the other edition's",
                        aria_pressed: rules().0 == use_2024,
                        onclick: move |_| {
                            rules.set(Use2024Rules(use_2024));
                            filters.write().set_ruleset(use_2024);
                        },
                        "{label}"
                    }
                }
            }
            FilterSection {
                label: "Kinds",
                on_all: move |shown| filters.write().set_all_kinds(shown),
                for kind in EntityKind::all() {
                    FilterChip {
                        key: "{kind}",
                        label: kind.label(),
                        classes: kind.badge_classes(),
                        on: filters.read().kind_shown(kind),
                        onclick: move |_| filters.write().toggle_kind(kind),
                    }
                }
            }
            FilterSection {
                label: "Sources",
                on_all: {
                    let lib = lib.clone();
                    move |shown| filters.write().set_all_sources(shown, &lib.search_index.sources)
                },
                for source in lib.search_index.sources.iter() {
                    FilterChip {
                        key: "{source}",
                        label: source.to_string(),
                        title: lib.source_full_name(source).map(str::to_string),
                        classes: source_badge_classes(source),
                        on: filters.read().source_shown(source),
                        onclick: {
                            let source = source.clone();
                            move |_| filters.write().toggle_source(&source)
                        },
                    }
                }
            }
        }
    }
}

/// The classes of a filter control that is switched on or off.
fn toggle_state(on: bool) -> &'static str {
    if on {
        "border-gray-400 bg-gray-100 text-gray-900"
    } else {
        "border-gray-200 bg-white text-gray-400 hover:border-gray-300"
    }
}

/// A labelled group of filter controls; with `on_all`, the group also gets
/// "all" and "none" buttons that switch every control on or off.
#[component]
fn FilterSection(label: &'static str, on_all: Option<EventHandler<bool>>, children: Element) -> Element {
    rsx! {
        section {
            div { class: "mb-1 flex items-baseline gap-2 text-[10px] uppercase tracking-wide text-gray-400",
                span { class: "font-medium", "{label}" }
                if let Some(on_all) = on_all {
                    button { class: "hover:text-gray-700", onclick: move |_| on_all.call(true), "all" }
                    button { class: "hover:text-gray-700", onclick: move |_| on_all.call(false), "none" }
                }
            }
            div { class: "flex flex-wrap gap-1", {children} }
        }
    }
}

/// One entity kind or source as an on/off button, tinted with its tag colors
/// while on and greyed out while off.
#[component]
fn FilterChip(
    label: String,
    title: Option<String>,
    classes: &'static str,
    on: bool,
    onclick: EventHandler<()>,
) -> Element {
    let state = if on {
        classes
    } else {
        "bg-white text-gray-400 ring-1 ring-inset ring-gray-200 hover:ring-gray-300"
    };
    let title = title.unwrap_or_else(|| label.clone());
    rsx! {
        button {
            class: "cursor-pointer rounded px-1.75 py-0.5 text-[10px] font-medium uppercase tracking-wide {state}",
            title: "{title}",
            aria_pressed: on,
            onclick: move |_| onclick.call(()),
            "{label}"
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
