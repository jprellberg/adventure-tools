use std::rc::Rc;

use dioxus::prelude::*;

use crate::card::EntityCard;
use crate::modal_history::{close_modal, listen_for_modal_navigation};
use crate::render::{render_entity_preview, RenderCtx};
use crate::routes::Route;
use crate::state::LibrarySignal;
use crate::{HoverSignal, ModalSignal, ResyncRequest, SearchInput, SearchQuery, UpdateAvailable, Use2024Rules};

/// How long to wait after the last keystroke before re-running the search:
/// long enough that fast typing doesn't re-filter the whole corpus on every
/// character, short enough to still feel immediate.
const SEARCH_DEBOUNCE_MS: u32 = 150;

#[component]
pub fn RootLayout() -> Element {
    listen_for_modal_navigation(use_context::<ModalSignal>());
    crate::viewport::provide_viewport();
    crate::pins::mirror_in_url(use_context::<crate::pins::PinsSignal>());

    rsx! {
        div { class: "min-h-screen bg-white",
            SearchHeader {}
            main { class: "px-4 pb-4 pt-20", Outlet::<Route> {} }
            HoverPopupOverlay {}
            ModalOverlay {}
        }
    }
}

/// The app's single search bar, fixed at the top of every
/// route so it stays reachable - and stays focused - no matter where the
/// user navigates. Typing while off the Home route jumps back there so the
/// results it produces are visible immediately.
#[component]
fn SearchHeader() -> Element {
    let mut search_input = use_context::<SearchInput>().0;
    let mut search_query = use_context::<SearchQuery>().0;
    let update_available = use_context::<Signal<UpdateAvailable>>();
    let resync = use_context::<ResyncRequest>();
    let modal = use_context::<ModalSignal>();
    let mut rules = use_context::<Signal<Use2024Rules>>();
    let nav = use_navigator();
    let route = use_route::<Route>();
    let is_home = matches!(route, Route::Home {});
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
    // key on a phone) and returns to the Home route.
    let mut clear = move || {
        search_input.set(String::new());
        search_query.set(String::new());
        debounce_generation.set(debounce_generation() + 1);
        if !is_home {
            nav.push(Route::Home {});
        }
    };

    rsx! {
        header { class: "fixed inset-x-0 top-0 z-40 flex items-center justify-center gap-2 bg-white/95 px-4 py-4 backdrop-blur",
            div { class: "relative min-w-0 max-w-xl flex-1",
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
                        if !is_home {
                            nav.push(Route::Home {});
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
                        if evt.key() == Key::Escape {
                            // The modal overlay's own key handler only fires while
                            // it holds focus; this input aggressively re-steals
                            // focus on blur (see `refocus`), so it - not the
                            // overlay - is the reliable place to catch Escape.
                            if modal.0.read().is_some() {
                                close_modal(modal);
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
                class: "shrink-0 rounded-full border border-gray-300 bg-white px-3 py-2 text-sm text-gray-600 hover:border-gray-400",
                title: "Switch between 2014 and 2024 core rulebooks",
                onclick: move |_| rules.set(Use2024Rules(!rules().0)),
                if rules().0 { "2024" } else { "2014" }
            }
            if update_available().0 {
                button {
                    class: "shrink-0 rounded-full border border-blue-300 bg-blue-50 px-3 py-2 text-sm text-blue-700 hover:bg-blue-100",
                    title: "A data update is available - click to download it",
                    aria_label: "Update data",
                    onclick: move |_| resync.send(()),
                    "⭯"
                }
            }
        }
    }
}

/// The floating entity preview a hyperlink shows on hover.
/// Rendered once here at the layout root - rather than nested inside each
/// card, as a pure-CSS `:hover` reveal would need to be - so it floats
/// above every card's own scroll/clip boundary instead of being clipped by
/// it. Its z-index sits above [`ModalOverlay`]'s so a link hovered from
/// *inside* the modal still previews on top of it, not underneath.
#[component]
fn HoverPopupOverlay() -> Element {
    let hover = use_context::<HoverSignal>().0;
    let library = use_context::<LibrarySignal>();
    let modal = use_context::<ModalSignal>();
    let Some((target, x, y)) = hover.read().clone() else {
        return rsx! {};
    };
    let Some(lib) = library.read().clone() else {
        return rsx! {};
    };
    let ctx = RenderCtx::new(&lib, modal, HoverSignal(hover));

    rsx! {
        div {
            class: "fixed z-[300] w-80 max-h-[70vh] overflow-y-auto rounded border border-gray-300 bg-white p-2 text-xs shadow-lg",
            style: "left: min({x}px, calc(100vw - 336px)); top: min({y + 16.0}px, calc(100vh - 300px));",
            {render_entity_preview(target.kind, &target.name, &target.source, ctx)}
        }
    }
}

/// The maximized entity view a card title or hyperlink click opens
/// Escape (handled in `SearchHeader`, see its comment),
/// clicking the backdrop, and the close button all dismiss it immediately;
/// only the browser's back/forward buttons walk the history of opened
/// modals (see `modal_history`).
#[component]
fn ModalOverlay() -> Element {
    let modal_ctx = use_context::<ModalSignal>();
    let Some(target) = modal_ctx.0.read().clone() else {
        return rsx! {};
    };

    rsx! {
        div {
            class: "fixed inset-0 z-[200] flex items-center justify-center bg-black/40 p-4 sm:p-6",
            onclick: move |_| close_modal(modal_ctx),
            div {
                class: "relative w-full max-w-2xl",
                onclick: move |evt| evt.stop_propagation(),
                button {
                    class: "absolute -right-3 -top-3 z-20 rounded-full border border-gray-300 bg-white px-2 py-1 text-sm text-gray-500 shadow hover:border-gray-400",
                    title: "Close",
                    onclick: move |_| close_modal(modal_ctx),
                    "✕"
                }
                div { class: "max-h-[85vh] overflow-y-auto",
                    EntityCard { kind: target.kind, source: target.source.clone(), name: target.name.clone(), full: true }
                }
            }
        }
    }
}
