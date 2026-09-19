mod card;
#[cfg(test)]
mod corpus_tests;
mod data;
mod layout;
mod model;
mod pages;
mod pins;
mod render;
mod routes;
mod search;
mod statblock;
mod state;
mod tags;
mod viewport;

use std::rc::Rc;

use dioxus::prelude::*;
use futures::StreamExt;

use data::Library;
use routes::{EntityKind, Route};

fn main() {
    dioxus::launch(App);
}

/// Whether a newer data release is available for download - checked once in
/// the background after startup, surfaced as the update icon button in
/// the top right corner.
#[derive(Clone, Copy, PartialEq)]
pub struct UpdateAvailable(pub bool);

/// Sends a message to request a full resync + reparse of the library (the
/// action behind the update icon button).
pub type ResyncRequest = Coroutine<()>;

/// The search bar's raw text, updated on every keystroke so the input
/// itself never lags behind typing.
#[derive(Clone, Copy, PartialEq)]
pub struct SearchInput(pub Signal<String>);

/// The debounced search text the Home page actually filters by.
/// Kept separate from [`SearchInput`] so a fast typist doesn't
/// re-run a full-corpus search on every keystroke - see `layout::SearchHeader`.
#[derive(Clone, Copy, PartialEq)]
pub struct SearchQuery(pub Signal<String>);

/// Identifies one entity, wherever a signal needs to name "this one" rather
/// than hold its rendered content.
#[derive(Clone, Debug, PartialEq)]
pub struct EntityTarget {
    pub kind: EntityKind,
    pub source: String,
    pub name: String,
}

/// The entity currently previewed in the floating hover popup, plus where to anchor it - see `layout::HoverPopupOverlay`. Kept as
/// a single app-level signal (rather than a popup nested under each link,
/// as a pure-CSS `:hover` reveal would be) so the preview can float above
/// every card instead of being clipped by its scroll container.
#[derive(Clone, Copy, PartialEq)]
pub struct HoverSignal(pub Signal<Option<(EntityTarget, f64, f64)>>);

/// Which ruleset's core rulebooks to show. `true` (the default) shows the
/// 2024 revisions and hides their 2014 originals; `false` is the inverse.
/// Every other source is
/// unaffected - see `data::excluded_by_ruleset`.
#[derive(Clone, Copy, PartialEq)]
pub struct Use2024Rules(pub bool);

#[component]
fn App() -> Element {
    let mut library = use_signal(|| None::<Rc<Library>>);
    let mut load_error = use_signal(|| None::<String>);
    let progress = use_signal(|| "Starting…".to_string());
    let mut loading = use_signal(|| true);
    let mut update_available = use_signal(|| UpdateAvailable(false));

    use_context_provider(|| library);
    use_context_provider(|| pins::PinsSignal::new(pins::from_location()));
    use_context_provider(|| SearchInput(Signal::new(String::new())));
    use_context_provider(|| SearchQuery(Signal::new(String::new())));
    use_context_provider(|| update_available);
    use_context_provider(|| HoverSignal(Signal::new(None)));
    use_context_provider(|| Signal::new(Use2024Rules(true)));
    use_context_provider(|| Signal::new(search::Filters::default()));

    let resync: ResyncRequest = use_coroutine(move |mut rx: UnboundedReceiver<()>| async move {
        while rx.next().await.is_some() {
            loading.set(true);
            match data::resync_and_reload(progress).await {
                Ok(lib) => {
                    library.set(Some(Rc::new(lib)));
                    update_available.set(UpdateAvailable(false));
                }
                Err(e) => load_error.set(Some(e)),
            }
            loading.set(false);
        }
    });
    use_context_provider(|| resync);

    use_future(move || async move {
        match data::load_library(progress).await {
            Ok(lib) => {
                library.set(Some(Rc::new(lib)));
            }
            Err(e) => {
                load_error.set(Some(e));
                loading.set(false);
                return;
            }
        }
        loading.set(false);
        if let Ok(true) = data::update_available().await {
            update_available.set(UpdateAvailable(true));
        }
    });

    rsx! {
        document::Link { rel: "stylesheet", href: asset!("/assets/tailwind.css") }
        if let Some(err) = load_error() {
            div { class: "flex h-screen items-center justify-center p-8 text-center text-red-600",
                "Failed to load data: {err}"
            }
        } else if loading() {
            // Inline layout styles: the stylesheet is injected at runtime, so
            // without them the text flashes unstyled in the corner until it loads.
            div {
                class: "flex h-screen flex-col items-center justify-center gap-4 px-8 text-center",
                style: "display:flex;height:100vh;flex-direction:column;align-items:center;justify-content:center;gap:1rem;padding:0 2rem;text-align:center;color:#6b7280;font-family:sans-serif",
                div { class: "h-10 w-10 animate-spin rounded-full border-4 border-gray-200 border-t-gray-600" }
                p { class: "text-gray-500", "{progress}" }
            }
        } else {
            Router::<Route> {}
        }
    }
}
