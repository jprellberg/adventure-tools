//! The app's landing page: pinned entity cards when the
//! search bar is empty, live search-result cards while it has text.

use dioxus::prelude::*;

use crate::card::{CardSize, EntityCard, EntityRow};
use crate::data::Library;
use crate::pins::PinsSignal;
use crate::routes::EntityKind;
use crate::search::{self, Filters};
use crate::state::LibrarySignal;
use crate::viewport::Viewport;
use crate::{SearchInput, SearchQuery};

#[component]
pub fn HomePage() -> Element {
    // Whether to show the pinned grid vs. results reacts to the raw input
    // instantly; the results themselves use the debounced query so a fast
    // typist isn't re-searching the whole corpus on every keystroke (see
    // `layout::SearchHeader`).
    let search_input = use_context::<SearchInput>().0;
    let search_query = use_context::<SearchQuery>().0;
    let library = use_context::<LibrarySignal>();
    let pins = use_context::<PinsSignal>();
    let filters = use_context::<Signal<Filters>>();
    let viewport = use_context::<Signal<Viewport>>();
    // The hits are recomputed only when the query, the filters
    // or the library change, not on every render of this page.
    let hits = use_memo(move || {
        let Some(lib) = library.read().clone() else {
            return Vec::new();
        };
        search::search(&lib, &search_query.read(), &filters.read())
            .into_iter()
            .map(|hit| (hit.kind, hit.entity.source().to_string(), hit.entity.name().to_string()))
            .collect::<Vec<_>>()
    });
    let Some(lib) = library.read().clone() else {
        return rsx! {};
    };

    let typed = search_input.read().clone();
    let q = search_query.read().clone();
    rsx! {
        div { class: "w-full",
            if typed.trim().is_empty() {
                {pinned_grid(&lib, pins, &filters.read(), viewport())}
            } else {
                {results_grid(&hits.read(), &q, viewport())}
            }
        }
    }
}

fn pinned_grid(lib: &Library, pins: PinsSignal, filters: &Filters, viewport: Viewport) -> Element {
    if pins.read().is_empty() {
        return rsx! {
            p { class: "mt-32 text-center text-gray-400",
                "Search above, then pin entities with the 📍 button to keep them here."
            }
        };
    }
    // Only entities the library actually has (a pin can outlive a data
    // update that renamed or dropped its entity) and that the current
    // filters don't hide.
    let known: Vec<_> = pins
        .read()
        .iter()
        .filter_map(|id| lib.pinned(*id))
        .filter(|(kind, entity)| filters.allows(*kind, entity.source()))
        .map(|(kind, entity)| (kind, entity.source().to_string(), entity.name().to_string()))
        .collect();
    entities(&known, viewport)
}

fn results_grid(hits: &[(EntityKind, String, String)], q: &str, viewport: Viewport) -> Element {
    if hits.is_empty() {
        return rsx! {
            p { class: "mt-32 text-center text-gray-400", "No results for \"{q}\"." }
        };
    }
    entities(hits, viewport)
}

/// The given entities as a grid of cards, or - in single-column windows,
/// where cards would show hardly more than one at a time - as a list of
/// one-line rows.
fn entities(items: &[(EntityKind, String, String)], viewport: Viewport) -> Element {
    if CardSize::single_column(viewport) {
        return rsx! {
            div { class: "mx-auto w-full max-w-xl",
                for (kind , source , name) in items.iter().cloned() {
                    EntityRow { key: "{kind}|{source}|{name}", kind, source, name }
                }
            }
        };
    }
    let size = CardSize::fitting(items.len(), viewport);
    rsx! {
        div { class: "flex flex-wrap justify-center gap-4",
            for (kind , source , name) in items.iter().cloned() {
                EntityCard { key: "{kind}|{source}|{name}", kind, source, name, size }
            }
        }
    }
}
