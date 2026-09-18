use dioxus::prelude::*;

use crate::card::EntityCard;
use crate::routes::EntityKind;

/// The `/entities/:kind/:source/:name` permalink page: the same card as the
/// grid, rendered at full width.
#[component]
pub fn EntityDetailPage(kind: EntityKind, source: String, name: String) -> Element {
    rsx! {
        div { class: "mx-auto max-w-2xl", EntityCard { kind, source, name, full: true } }
    }
}
