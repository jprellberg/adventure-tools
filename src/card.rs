//! The entity card: a fixed-width, fixed-height (~3:4) rendering of one
//! entity's statblock, with a pin toggle in the top-right corner (the
//! statblock's title flows around it). Used for the pinned grid / search
//! results, at full size for the `/entities/:kind/:source/:name` permalink
//! page, and again (also at full size) inside the modal overlay a
//! title/hyperlink click opens.

use dioxus::prelude::*;

use crate::data::{is_base_source, Library};
use crate::pins::{is_pinned, toggle_pin, PinsSignal};
use crate::render::RenderCtx;
use crate::routes::{EntityKind, Route};
use crate::state::LibrarySignal;
use crate::viewport::Viewport;
use crate::{EntityTarget, HoverSignal, ModalSignal};

/// The kind + source tag pair shown inline after an entity's title; they
/// wrap along with the title text. The three core rulebooks get
/// their own gold tag color since nearly every card traces back to one of
/// them; every source tag's tooltip names its sourcebook in full where
/// known.
pub fn kind_source_tags(kind: EntityKind, source: &str, lib: &Library) -> Element {
    let source_classes = if is_base_source(source) {
        "bg-yellow-100 text-yellow-800 ring-1 ring-inset ring-yellow-300"
    } else {
        "bg-indigo-50 text-indigo-700"
    };
    let source_title = lib.source_full_name(source).unwrap_or(source).to_string();
    rsx! {
        span { class: "ml-1.5 rounded px-1.75 py-0.5 align-middle text-[10px] font-medium uppercase tracking-wide {kind.badge_classes()}",
            title: "{kind.label()}",
            "{kind.label()}"
        }
        span { class: "ml-1 rounded px-1.75 py-0.5 align-middle text-[10px] font-medium uppercase tracking-wide {source_classes}",
            title: "{source_title}",
            "{source}"
        }
    }
}

/// A grid card's size, as factors (height x width) of the base card.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CardSize {
    /// 1 x 1
    Base,
    /// 2 x 1
    Tall,
    /// 2 x 2
    Large,
}

/// Base card and grid gap, in rem; `Base.classes()` and the grids' `gap-4`
/// must agree with them.
const CARD_WIDTH: f64 = 20.0;
const CARD_HEIGHT: f64 = 29.0;
const GAP: f64 = 1.0;
/// What the page takes from the window around the grid (`layout::RootLayout`):
/// `px-4` on each side, `pt-20` above for the search bar and `pb-4` below.
const PAGE_MARGIN_X: f64 = 2.0;
const PAGE_MARGIN_Y: f64 = 6.0;

impl CardSize {
    /// (rows, columns) of base cards this size spans.
    fn factors(self) -> (f64, f64) {
        match self {
            CardSize::Base => (1.0, 1.0),
            CardSize::Tall => (2.0, 1.0),
            CardSize::Large => (2.0, 2.0),
        }
    }

    fn classes(self) -> &'static str {
        match self {
            CardSize::Base => "h-[29rem] w-[20rem]",
            CardSize::Tall => "h-[59rem] w-[20rem]",
            CardSize::Large => "h-[59rem] w-[41rem]",
        }
    }

    /// Whether `count` cards of this size fit into the window at once,
    /// wrapped into as many columns as the width allows.
    fn fits(self, count: usize, viewport: Viewport) -> bool {
        let (rows, columns) = self.factors();
        let width = columns * CARD_WIDTH + (columns - 1.0) * GAP;
        let height = rows * CARD_HEIGHT + (rows - 1.0) * GAP;
        let per_row = ((viewport.width - PAGE_MARGIN_X + GAP) / (width + GAP)).floor();
        if per_row < 1.0 {
            return false;
        }
        let grid_rows = (count as f64 / per_row).ceil();
        grid_rows * (height + GAP) - GAP <= viewport.height - PAGE_MARGIN_Y
    }

    /// Whether the window is too narrow to lay out two base cards side by
    /// side. Unmeasured (zero-width) windows don't count.
    pub fn single_column(viewport: Viewport) -> bool {
        let two_cards = 2.0 * CARD_WIDTH + GAP;
        viewport.width > 0.0 && viewport.width - PAGE_MARGIN_X < two_cards
    }

    /// The largest size at which all `count` cards are on screen without
    /// scrolling, or `Base` when none is.
    pub fn fitting(count: usize, viewport: Viewport) -> Self {
        [CardSize::Large, CardSize::Tall]
            .into_iter()
            .find(|size| size.fits(count, viewport))
            .unwrap_or(CardSize::Base)
    }
}

/// A search result reduced to one line - the title and its tags - for
/// single-column windows, where full cards would leave next to nothing of
/// the result list visible (especially with the on-screen keyboard open).
/// Clicking the title maximizes the entity like a card's title does.
#[component]
pub fn EntityRow(kind: EntityKind, source: String, name: String) -> Element {
    let library = use_context::<LibrarySignal>();
    let modal = use_context::<ModalSignal>();
    let hover = use_context::<HoverSignal>();
    let Some(lib) = library.read().clone() else {
        return rsx! {};
    };
    let Some(entity) = lib.find_ci(kind, Some(&source), &name) else {
        return rsx! {};
    };
    let target = EntityTarget {
        kind,
        source: entity.source().to_string(),
        name: entity.name().to_string(),
    };
    let permalink = Route::EntityDetail {
        kind,
        source: target.source.clone(),
        name: target.name.clone(),
    };
    let ctx = RenderCtx::new(&lib, modal, hover);

    rsx! {
        div { class: "border-b border-gray-200 py-2 text-base font-bold leading-tight",
            Link {
                class: "cursor-pointer hover:underline",
                to: permalink,
                onclick_only: true,
                onclick: {
                    let target = target.clone();
                    move |_| crate::modal_history::open_modal(modal, target.clone())
                },
                {crate::render::render_inline(&target.name, ctx)}
            }
            {kind_source_tags(kind, &target.source, &lib)}
        }
    }
}

#[component]
pub fn EntityCard(
    kind: EntityKind,
    source: String,
    name: String,
    #[props(default = false)] full: bool,
    #[props(default = CardSize::Base)] size: CardSize,
) -> Element {
    let library = use_context::<LibrarySignal>();
    let pins = use_context::<PinsSignal>();
    let modal = use_context::<ModalSignal>();
    let hover = use_context::<HoverSignal>();
    // A card's body is only built once the card has scrolled into view: a
    // search shows up to 60 cards, but only the few on screen are worth
    // rendering. Cards have a fixed size, so the empty ones keep the layout.
    let mut visible = use_signal(|| full);
    let Some(lib) = library.read().clone() else {
        return rsx! {};
    };

    let size_class = if full {
        "relative w-full overflow-x-hidden rounded border border-gray-200 bg-white p-4 shadow-sm".to_string()
    } else {
        format!(
            "relative {} shrink-0 overflow-y-auto overflow-x-hidden rounded border border-gray-200 bg-white p-2 shadow-sm",
            size.classes()
        )
    };

    let Some(entity) = lib.find_ci(kind, Some(&source), &name) else {
        return rsx! {
            div { class: size_class, p { class: "text-gray-400", "{name} ({source}) not found." } }
        };
    };
    let canonical_source = entity.source().to_string();
    let canonical_name = entity.name().to_string();
    let pinned = is_pinned(pins, kind, &canonical_source, &canonical_name);
    let onclick = {
        let source = canonical_source.clone();
        let name = canonical_name.clone();
        move |_| toggle_pin(pins, kind, &source, &name)
    };

    let ctx = RenderCtx::new(&lib, modal, hover);

    rsx! {
        div {
            class: size_class,
            onvisible: move |evt: VisibleEvent| {
                if !visible() && evt.is_intersecting().unwrap_or(false) {
                    visible.set(true);
                }
            },
            button {
                class: if pinned {
                    "float-right ml-1 rounded-full bg-blue-600 px-1.75 py-0.5 text-xs text-white"
                } else {
                    "float-right ml-1 rounded-full border border-gray-300 bg-white px-1.75 py-0.5 text-xs text-gray-500 hover:border-gray-400"
                },
                title: if pinned { "Unpin" } else { "Pin" },
                onclick,
                if pinned { "📌" } else { "📍" }
            }
            if visible() {
                {crate::statblock::render(&entity, kind, &canonical_name, ctx)}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CardSize;
    use crate::viewport::Viewport;

    fn full_hd() -> Viewport {
        Viewport {
            width: 120.0,
            height: 67.5,
        }
    }

    #[test]
    fn few_cards_grow_to_fill_the_window() {
        assert_eq!(CardSize::fitting(1, full_hd()), CardSize::Large);
        assert_eq!(CardSize::fitting(2, full_hd()), CardSize::Large);
        assert_eq!(CardSize::fitting(4, full_hd()), CardSize::Tall);
    }

    #[test]
    fn six_tall_cards_fit() {
        assert_eq!(
            CardSize::fitting(
                6,
                Viewport {
                    width: 127.0,
                    height: 65.0
                }
            ),
            CardSize::Tall
        );
        assert_eq!(
            CardSize::fitting(
                6,
                Viewport {
                    width: 126.9,
                    height: 65.0
                }
            ),
            CardSize::Base
        );
        assert_eq!(
            CardSize::fitting(
                6,
                Viewport {
                    width: 127.0,
                    height: 64.9
                }
            ),
            CardSize::Base
        );
    }

    #[test]
    fn many_cards_stay_base() {
        assert_eq!(CardSize::fitting(60, full_hd()), CardSize::Base);
    }

    #[test]
    fn a_window_too_small_for_the_larger_sizes_stays_base() {
        // Fits one base card but not a 2 x 1 card.
        assert_eq!(
            CardSize::fitting(
                1,
                Viewport {
                    width: 30.0,
                    height: 50.0
                }
            ),
            CardSize::Base
        );
        assert_eq!(CardSize::fitting(1, Viewport::default()), CardSize::Base);
    }
}
