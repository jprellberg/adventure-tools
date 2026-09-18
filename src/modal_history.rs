//! Wires the modal overlay into the browser's history: each
//! open pushes an entry carrying the entity it shows, so the back/forward
//! buttons step through subsequently-opened modals (e.g. clicking a
//! hyperlink while one is already open) the way they'd step through pages.
//!
//! `history.pushState` never fires `popstate` - only actual back/forward
//! navigation does - so an *open* sets the modal signal directly and pushes
//! a matching entry, while [`listen_for_modal_navigation`]'s `popstate`
//! listener applies the state of whichever entry back/forward lands on.
//! Dismissing the modal (Escape, backdrop, close button) is not navigation:
//! [`close_modal`] just clears the signal and leaves history alone.
//!
//! Every push reuses the current URL (`location.href` unchanged) rather
//! than encoding the entity into it, so dioxus-router - which treats the
//! full pathname+search+hash as "the route" - never sees the pathname
//! change and keeps rendering the same page underneath.
//!
//! The very first modal opened in a page session also pushes a plain
//! "no modal" baseline entry immediately before its own, so that going back
//! from it can never step past it into whatever history existed before the
//! app loaded (see `BASELINE_PUSHED`).

use std::cell::Cell;

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{EntityTarget, ModalSignal};

thread_local! {
    /// Whether this page session has ever pushed a modal-history entry yet.
    /// The *first* open also pushes a plain `null` "no modal" entry right
    /// before the modal's own one, so going back can never step past it
    /// into whatever history existed before the app loaded (e.g. the
    /// browser's new-tab page) - seeded once, it's the floor every
    /// subsequent back ultimately lands on.
    static BASELINE_PUSHED: Cell<bool> = const { Cell::new(false) };
}

#[derive(Serialize, Deserialize)]
struct ModalState {
    kind: String,
    source: String,
    name: String,
}

impl ModalState {
    fn into_target(self) -> Option<EntityTarget> {
        Some(EntityTarget {
            kind: self.kind.parse().ok()?,
            source: self.source,
            name: self.name,
        })
    }
}

impl From<&EntityTarget> for ModalState {
    fn from(target: &EntityTarget) -> Self {
        ModalState {
            kind: target.kind.to_string(),
            source: target.source.clone(),
            name: target.name.clone(),
        }
    }
}

/// Shows `target` in the modal and pushes a history entry for it.
pub fn open_modal(modal: ModalSignal, target: EntityTarget) {
    if !BASELINE_PUSHED.with(Cell::get) {
        BASELINE_PUSHED.with(|b| b.set(true));
        document::eval("history.pushState(null, '', location.href);");
    }

    let state = ModalState::from(&target);
    let mut modal = modal.0;
    modal.set(Some(target));
    if let Ok(state_json) = serde_json::to_string(&state) {
        document::eval(&format!("history.pushState({state_json}, '', location.href);"));
    }
}

/// Dismisses the modal immediately, without touching browser history.
pub fn close_modal(modal: ModalSignal) {
    let mut modal = modal.0;
    modal.set(None);
}

/// Starts listening for the `popstate` events a browser back/forward
/// produces, applying the resulting state to the modal signal. Call once,
/// from the root layout.
pub fn listen_for_modal_navigation(modal: ModalSignal) {
    let mut modal = modal.0;
    use_future(move || async move {
        let mut eval =
            document::eval("window.addEventListener('popstate', (event) => { dioxus.send(event.state ?? null); });");
        while let Ok(state) = eval.recv::<Option<ModalState>>().await {
            modal.set(state.and_then(ModalState::into_target));
        }
    });
}
