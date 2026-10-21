//! Pinned entities: the entities the user has chosen to keep in the pinned
//! card grid. The pins are stored in IndexedDB and every change is broadcast
//! to the app's other tabs, so all of them show the same pins; the address of
//! the pinned view (`/pins/:pins`) spells them out, so a bookmark or a shared
//! link carries them.
//!
//! A pin is a fixed-width id: a 42-bit hash of the entity's kind, source and
//! name, written as 7 base64url characters. Ids are simply concatenated, so
//! an address stays short (17 pins fit into 128 characters) and, since an id
//! depends only on what identifies the entity and not on its position in
//! the data, it stays valid across data updates. Among the ~50 000
//! entities 42 bits leave about 3e-4 colliding pairs to expect (a test checks
//! the current data has none); should two collide, the first one wins.

use std::fmt;

use dioxus::prelude::*;
use dioxus::router::{FromRouteSegments, ToRouteSegments};
use futures::StreamExt;
use js_sys::Reflect;

use crate::data::db;
use crate::routes::EntityKind;

const ID_CHARS: usize = 7;
const ID_BITS: u32 = 6 * ID_CHARS as u32;
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
/// The key of the pins in the meta store.
const DB_KEY: &str = "pins";
/// The `BroadcastChannel` the tabs announce pin changes on.
const CHANNEL: &str = "adventure-tools-pins";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PinId(u64);

impl PinId {
    /// Case-insensitive, like entity lookups.
    pub fn new(kind: EntityKind, source: &str, name: &str) -> Self {
        // FNV-1a over `kind \0 source \0 name`, then mixed so every input bit
        // reaches the bits that are kept.
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        let parts = [kind.slug(), &source.to_lowercase(), &name.to_lowercase()];
        for part in parts {
            for byte in part.bytes().chain([0]) {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
            }
        }
        hash ^= hash >> 32;
        hash = hash.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        hash ^= hash >> 29;
        PinId(hash & ((1 << ID_BITS) - 1))
    }
}

/// The pins in order, written as their concatenated ids (the route segment of
/// the pinned view, and the stored and broadcast form).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PinList(pub Vec<PinId>);

impl fmt::Display for PinList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for pin in &self.0 {
            for i in (0..ID_CHARS).rev() {
                write!(f, "{}", ALPHABET[(pin.0 >> (6 * i)) as usize & 63] as char)?;
            }
        }
        Ok(())
    }
}

impl PinList {
    /// Ids that are malformed are skipped, and repeats are dropped.
    fn parse(text: &str) -> Self {
        let mut pins = Vec::new();
        for chunk in text.as_bytes().as_chunks::<ID_CHARS>().0 {
            let id = chunk.iter().try_fold(0u64, |id, c| {
                let value = ALPHABET.iter().position(|a| a == c)?;
                Some(id << 6 | value as u64)
            });
            if let Some(id) = id.map(PinId) {
                if !pins.contains(&id) {
                    pins.push(id);
                }
            }
        }
        PinList(pins)
    }
}

impl ToRouteSegments for PinList {
    fn display_route_segments(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            return Ok(());
        }
        [self.to_string()].display_route_segments(f)
    }
}

impl FromRouteSegments for PinList {
    type Err = std::convert::Infallible;

    fn from_route_segments(segments: &[&str]) -> Result<Self, Self::Err> {
        Ok(PinList::parse(&segments.concat()))
    }
}

/// The pins, and the task that stores and broadcasts each change of them.
#[derive(Clone, Copy, PartialEq)]
pub struct Pins {
    pub list: Signal<Vec<PinId>>,
    saver: Coroutine<Vec<PinId>>,
}

impl Pins {
    /// Provides the pins as a context, initially none, and keeps them in sync
    /// with the other tabs. Call once, from the app root.
    pub fn provide() -> Pins {
        let mut list = use_signal(Vec::new);
        // One write after the other, so a later change can't be overtaken.
        let saver = use_coroutine(move |mut rx: UnboundedReceiver<Vec<PinId>>| async move {
            while let Some(pins) = rx.next().await {
                let text = PinList(pins).to_string();
                if let Err(e) = db::put(db::STORE_META, DB_KEY, &text).await {
                    dioxus::logger::tracing::warn!("storing the pins: {e}");
                }
                document::eval(&format!("new BroadcastChannel('{CHANNEL}').postMessage('{text}');"));
            }
        });
        use_future(move || async move {
            let mut eval = document::eval(&format!(
                "new BroadcastChannel('{CHANNEL}').onmessage = (e) => dioxus.send(e.data);"
            ));
            while let Ok(text) = eval.recv::<String>().await {
                list.set(PinList::parse(&text).0);
            }
        });
        use_context_provider(|| Pins { list, saver })
    }

    /// Loads the pins: those of the address the page was opened with (which
    /// then become everyone's pins), or else the stored ones.
    pub async fn load(mut self) {
        let from_url = from_location();
        if !from_url.is_empty() {
            self.set(from_url);
            return;
        }
        match db::get(db::STORE_META, DB_KEY).await {
            Ok(text) => self.list.set(PinList::parse(&text.unwrap_or_default()).0),
            Err(e) => dioxus::logger::tracing::warn!("loading the pins: {e}"),
        }
    }

    pub fn is_pinned(&self, kind: EntityKind, source: &str, name: &str) -> bool {
        self.list.read().contains(&PinId::new(kind, source, name))
    }

    /// Replaces the pins, everywhere.
    pub fn set(&mut self, pins: Vec<PinId>) {
        self.list.set(pins.clone());
        self.saver.send(pins);
    }

    /// Toggles whether an entity is pinned. The card's pin button is the
    /// only way to pin/unpin.
    pub fn toggle(&mut self, kind: EntityKind, source: &str, name: &str) {
        let id = PinId::new(kind, source, name);
        let mut list = self.list.read().clone();
        match list.iter().position(|p| *p == id) {
            Some(i) => {
                list.remove(i);
            }
            None => list.push(id),
        }
        self.set(list);
    }

    /// Moves `pin` to where `target` is, in this tab only: a drag shows the
    /// new order as it goes and [`Pins::save`]s it when it ends.
    pub fn move_to(&mut self, pin: PinId, target: PinId) {
        let mut list = self.list.write();
        let (Some(from), Some(to)) = (
            list.iter().position(|p| *p == pin),
            list.iter().position(|p| *p == target),
        ) else {
            return;
        };
        let pin = list.remove(from);
        list.insert(to, pin);
    }

    /// Stores and broadcasts the pins as they are in this tab.
    pub fn save(&self) {
        self.saver.send(self.list.read().clone());
    }
}

/// The pins in the address the page was opened with (`…/pins/:pins`).
fn from_location() -> Vec<PinId> {
    let path = Reflect::get(&js_sys::global(), &"location".into())
        .and_then(|location| Reflect::get(&location, &"pathname".into()))
        .ok()
        .and_then(|path| path.as_string())
        .unwrap_or_default();
    path.rsplit_once("/pins/")
        .map(|(_, pins)| PinList::parse(pins).0)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_stable() {
        // Changing the hash or the id layout breaks every shared link.
        let id = PinId::new(EntityKind::Bestiary, "XMM", "Ghoul");
        assert_eq!(PinList(vec![id]).to_string(), "gLG8UYm");
    }

    #[test]
    fn ids_ignore_case_and_differ_by_kind() {
        let ghoul = PinId::new(EntityKind::Bestiary, "XMM", "Ghoul");
        assert_eq!(ghoul, PinId::new(EntityKind::Bestiary, "xmm", "GHOUL"));
        assert_ne!(ghoul, PinId::new(EntityKind::Spells, "XMM", "Ghoul"));
        assert_ne!(ghoul, PinId::new(EntityKind::Bestiary, "MM", "Ghoul"));
    }

    #[test]
    fn pins_round_trip_through_the_url() {
        let pins = PinList(
            (0..17)
                .map(|i| PinId::new(EntityKind::Spells, "XPHB", &format!("Spell {i}")))
                .collect(),
        );
        let route = crate::routes::Route::Pins { pins: pins.clone() };
        assert!(route.to_string().len() <= 128, "{route}");
        assert_eq!(route.to_string().parse::<crate::routes::Route>().unwrap(), route);
    }

    #[test]
    fn malformed_and_repeated_ids_are_dropped() {
        let id = PinId::new(EntityKind::Items, "XDMG", "Bag of Holding");
        let text = format!("{0}!!!!!!!{0}abc", PinList(vec![id]));
        assert_eq!(PinList::parse(&text).0, vec![id]);
    }
}
