use dioxus::prelude::*;

use crate::card::EntityCard;
use crate::pages::book::BookPage;
use crate::routes::EntityKind;
use crate::EntityTarget;

/// The detail view (`/entities/:kind/:source/:name`, also the permalink of an
/// entity): the same card as the grid, rendered at full width, or a book's
/// page for a book. Without an entity it says how to pick one.
#[component]
pub fn EntityDetailPage(target: Option<EntityTarget>) -> Element {
    let Some(EntityTarget { kind, source, name }) = target else {
        return rsx! {
            p { class: "mt-32 text-center text-gray-400", "Click a title to see the entity here." }
        };
    };
    if kind == EntityKind::Books {
        return rsx! {
            BookPage { source, section: Vec::new() }
        };
    }
    rsx! {
        div { class: "mx-auto max-w-2xl", EntityCard { kind, source, name, full: true } }
    }
}
