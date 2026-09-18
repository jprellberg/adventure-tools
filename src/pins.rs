//! Pinned entities: the entities the user has chosen to keep
//! visible in the card grid when the search bar is empty. The pins live in
//! the URL (`?p=`), so a bookmark or a shared link carries them.
//!
//! A pin is a fixed-width id: a 42-bit hash of the entity's kind, source and
//! name, written as 7 base64url characters. Ids are simply concatenated, so
//! a URL stays short (17 pins fit into 128 characters) and, since an id
//! depends only on what identifies the entity and not on its position in
//! the data, it stays valid across data updates. Among the ~50 000
//! entities 42 bits leave about 3e-4 colliding pairs to expect (a test checks
//! the current data has none); should two collide, the first one wins.

use dioxus::prelude::*;
use js_sys::Reflect;

use crate::routes::EntityKind;

const ID_CHARS: usize = 7;
const ID_BITS: u32 = 6 * ID_CHARS as u32;
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
const QUERY_KEY: &str = "p";

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

pub type PinsSignal = Signal<Vec<PinId>>;

fn encode(pins: &[PinId]) -> String {
    let mut out = String::with_capacity(pins.len() * ID_CHARS);
    for pin in pins {
        for i in (0..ID_CHARS).rev() {
            out.push(ALPHABET[(pin.0 >> (6 * i)) as usize & 63] as char);
        }
    }
    out
}

/// Ids that are malformed are skipped, and repeats are dropped.
fn decode(text: &str) -> Vec<PinId> {
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
    pins
}

/// The URL query carrying `pins`: empty when there are none.
pub fn query(pins: &[PinId]) -> String {
    if pins.is_empty() {
        String::new()
    } else {
        format!("?{QUERY_KEY}={}", encode(pins))
    }
}

/// The pins in the URL the page was opened with.
pub fn from_location() -> Vec<PinId> {
    let search = Reflect::get(&js_sys::global(), &"location".into())
        .and_then(|location| Reflect::get(&location, &"search".into()))
        .ok()
        .and_then(|search| search.as_string())
        .unwrap_or_default();
    search
        .trim_start_matches('?')
        .split('&')
        .find_map(|pair| pair.strip_prefix(QUERY_KEY)?.strip_prefix('='))
        .map(decode)
        .unwrap_or_default()
}

/// Keeps the address bar showing the current pins: the query is rewritten in
/// place (no history entry) whenever the pins or the route change, including
/// after a navigation that dropped it. Call from the root layout.
pub fn mirror_in_url(pins: PinsSignal) {
    let route = use_route::<crate::routes::Route>();
    let query = query(&pins.read());
    use_effect(use_reactive!(|(route, query)| {
        let _ = route;
        document::eval(&format!(
            "history.replaceState(history.state, '', location.pathname + '{query}');"
        ));
    }));
}

pub fn is_pinned(pins: PinsSignal, kind: EntityKind, source: &str, name: &str) -> bool {
    pins.read().contains(&PinId::new(kind, source, name))
}

/// Toggles whether an entity is pinned. The card's pin button is the only
/// way to pin/unpin.
pub fn toggle_pin(mut pins: PinsSignal, kind: EntityKind, source: &str, name: &str) {
    let id = PinId::new(kind, source, name);
    let mut list = pins.read().clone();
    match list.iter().position(|p| *p == id) {
        Some(i) => {
            list.remove(i);
        }
        None => list.push(id),
    }
    pins.set(list);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_stable() {
        // Changing the hash or the id layout breaks every shared link.
        let id = PinId::new(EntityKind::Bestiary, "XMM", "Ghoul");
        assert_eq!(encode(&[id]), "gLG8UYm");
    }

    #[test]
    fn ids_ignore_case_and_differ_by_kind() {
        let ghoul = PinId::new(EntityKind::Bestiary, "XMM", "Ghoul");
        assert_eq!(ghoul, PinId::new(EntityKind::Bestiary, "xmm", "GHOUL"));
        assert_ne!(ghoul, PinId::new(EntityKind::Spells, "XMM", "Ghoul"));
        assert_ne!(ghoul, PinId::new(EntityKind::Bestiary, "MM", "Ghoul"));
    }

    #[test]
    fn pins_round_trip_through_the_query() {
        let pins: Vec<PinId> = (0..17)
            .map(|i| PinId::new(EntityKind::Spells, "XPHB", &format!("Spell {i}")))
            .collect();
        let query = query(&pins);
        assert!(format!("/{query}").len() <= 128, "{}", query.len());
        assert_eq!(decode(query.strip_prefix("?p=").unwrap()), pins);
    }

    #[test]
    fn malformed_and_repeated_ids_are_dropped() {
        let id = PinId::new(EntityKind::Items, "XDMG", "Bag of Holding");
        let text = format!("{0}!!!!!!!{0}abc", encode(&[id]));
        assert_eq!(decode(&text), vec![id]);
    }
}
