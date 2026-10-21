//! The grid views: the pinned entities and the search results, both as
//! entity cards.

use dioxus::prelude::*;

use crate::card::{CardSize, EntityCard, EntityRow};
use crate::pins::{PinId, Pins};
use crate::routes::EntityKind;
use crate::search::{self, Filters};
use crate::state::LibrarySignal;
use crate::viewport::Viewport;

/// The pinned entities, whatever the filters say. They are reordered by
/// dragging them.
#[component]
pub fn PinnedPage() -> Element {
    let library = use_context::<LibrarySignal>();
    let pins = use_context::<Pins>();
    let viewport = use_context::<Signal<Viewport>>();
    let dragged = use_signal(|| None::<PinId>);
    let Some(lib) = library.read().clone() else {
        return rsx! {};
    };
    if pins.list.read().is_empty() {
        return rsx! {
            p { class: "mt-32 text-center text-gray-400",
                "Search above, then pin entities with the 📍 button to keep them here."
            }
        };
    }
    // Only entities the library actually has (a pin can outlive a data
    // update that renamed or dropped its entity).
    let known: Vec<_> = pins
        .list
        .read()
        .iter()
        .filter_map(|id| lib.pinned(*id))
        .map(|(kind, entity)| (kind, entity.source().to_string(), entity.name().to_string()))
        .collect();
    rsx! {
        div { class: "w-full", {entities(&known, viewport(), Some(dragged))} }
    }
}

/// The search results for `query`; with nothing typed, everything the
/// filters let through, alphabetically.
#[component]
pub fn ResultsPage(query: ReadSignal<String>) -> Element {
    let library = use_context::<LibrarySignal>();
    let filters = use_context::<Signal<Filters>>();
    let viewport = use_context::<Signal<Viewport>>();
    // The hits are recomputed only when the query, the filters
    // or the library change, not on every render of this page.
    let hits = use_memo(move || {
        let Some(lib) = library.read().clone() else {
            return Vec::new();
        };
        search::search(&lib, &query.read(), &filters.read())
            .into_iter()
            .map(|hit| (hit.kind, hit.entity.source().to_string(), hit.entity.name().to_string()))
            .collect::<Vec<_>>()
    });
    if library.read().is_none() {
        return rsx! {};
    }
    let hits = hits.read();
    if hits.is_empty() {
        let q = query.read();
        return rsx! {
            p { class: "mt-32 text-center text-gray-400",
                if q.trim().is_empty() {
                    "Nothing matches the current filters."
                } else {
                    "No results for \"{q}\"."
                }
            }
        };
    }
    rsx! {
        div { class: "w-full", {entities(&hits, viewport(), None)} }
    }
}

/// The given entities as a grid of cards, or - in single-column windows,
/// where cards would show hardly more than one at a time - as a list of
/// one-line rows. With `dragged` (the pin being dragged), they are pins that
/// can be dragged into another order.
fn entities(
    items: &[(EntityKind, String, String)],
    viewport: Viewport,
    dragged: Option<Signal<Option<PinId>>>,
) -> Element {
    let size = (!CardSize::single_column(viewport)).then(|| CardSize::fitting(items.len(), viewport));
    let class = match size {
        Some(_) => "flex flex-wrap justify-center gap-4",
        None => "mx-auto w-full max-w-xl",
    };
    rsx! {
        div { class,
            for (kind , source , name) in items.iter().cloned() {
                GridItem { key: "{kind}|{source}|{name}", kind, source, name, size, dragged }
            }
        }
    }
}

/// One entity of a grid: its card at `size`, or its row without one.
#[component]
fn GridItem(
    kind: EntityKind,
    source: String,
    name: String,
    size: Option<CardSize>,
    dragged: Option<Signal<Option<PinId>>>,
) -> Element {
    let item = match size {
        Some(size) => rsx! {
            EntityCard { kind, source: source.clone(), name: name.clone(), size }
        },
        None => rsx! {
            EntityRow { kind, source: source.clone(), name: name.clone() }
        },
    };
    let mut pins = use_context::<Pins>();
    let Some(mut dragged) = dragged else {
        return item;
    };
    let id = PinId::new(kind, &source, &name);
    rsx! {
        div {
            class: if dragged() == Some(id) { "cursor-grab opacity-40" } else { "cursor-grab" },
            draggable: true,
            ondragstart: move |evt: DragEvent| {
                // Firefox only starts a drag that carries data.
                let _ = evt.data_transfer().set_data("text/plain", "");
                evt.data_transfer().set_effect_allowed("move");
                dragged.set(Some(id));
            },
            // The dragged pin takes the place of the one under it as it goes.
            ondragover: move |evt: DragEvent| {
                evt.prevent_default();
                if let Some(pin) = dragged() {
                    if pin != id {
                        pins.move_to(pin, id);
                    }
                }
            },
            ondrop: move |evt: DragEvent| evt.prevent_default(),
            ondragend: move |_| {
                dragged.set(None);
                pins.save();
            },
            {item}
        }
    }
}
