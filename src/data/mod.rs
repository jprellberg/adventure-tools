pub mod books;
pub mod db;
mod github;
pub mod loader;
mod props;
pub mod snapshot;
mod sync;

use std::collections::HashMap;

use crate::model::{
    Action, Background, BookMeta, Class, ClassFeature, Condition, Feat, Hazard, Item, Monster, Object, Other, Race,
    Spell,
};
use crate::routes::EntityKind;
use dioxus::prelude::{Signal, WritableExt};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::pins::PinId;
use crate::search::SearchIndex;

/// The full parsed reference library: every source found in the cached
/// `data/` tree, read-only.
#[derive(Clone, Serialize, Deserialize)]
pub struct Library {
    pub monsters: Vec<Monster>,
    pub spells: Vec<Spell>,
    pub items: Vec<Item>,
    pub item_types: Vec<ItemType>,
    pub classes: Vec<Class>,
    pub class_features: Vec<ClassFeature>,
    pub subclass_features: Vec<ClassFeature>,
    pub races: Vec<Race>,
    pub backgrounds: Vec<Background>,
    pub feats: Vec<Feat>,
    pub conditions: Vec<Condition>,
    pub objects: Vec<Object>,
    pub actions: Vec<Action>,
    pub hazards: Vec<Hazard>,
    /// Every kind that shares the minimal name/source/page/entries shape,
    /// keyed by the kind it belongs to.
    pub other: HashMap<EntityKind, Vec<Other>>,
    pub spell_lookup: SpellLookup,
    pub books: Vec<BookMeta>,
    /// Derived from the entities above, so it is rebuilt rather than
    /// stored (see [`Library::indexed`]).
    #[serde(skip)]
    pub search_index: SearchIndex,
    /// Where each pinnable entity is, by its pin id; rebuilt like the search
    /// index.
    #[serde(skip)]
    pin_index: HashMap<PinId, (EntityKind, usize)>,
}

/// One entry of `items-base.json`'s `itemType` array - only the code to
/// name mapping is used (to spell out an item's type).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ItemType {
    pub abbreviation: String,
    pub source: String,
    pub name: String,
}

/// Which classes/subclasses/etc. a spell is available to, keyed by the
/// spell's lowercased source then lowercased name (`generated/
/// gendata-spell-source-lookup.json`).
pub type SpellLookup = HashMap<String, HashMap<String, Value>>;

/// A borrowed reference into whichever typed collection an entity lives in,
/// so search/link-resolution/rendering can work generically across kinds.
pub enum EntityRef<'a> {
    Monster(&'a Monster),
    Spell(&'a Spell),
    Item(&'a Item),
    Class(&'a Class),
    Race(&'a Race),
    Background(&'a Background),
    Feat(&'a Feat),
    Condition(&'a Condition),
    Object(&'a Object),
    Action(&'a Action),
    Hazard(&'a Hazard),
    Other(EntityKind, &'a Other),
}

impl<'a> EntityRef<'a> {
    pub fn name(&self) -> &'a str {
        match self {
            EntityRef::Monster(e) => &e.name,
            EntityRef::Spell(e) => &e.name,
            EntityRef::Item(e) => &e.name,
            EntityRef::Class(e) => &e.name,
            EntityRef::Race(e) => &e.name,
            EntityRef::Background(e) => &e.name,
            EntityRef::Feat(e) => &e.name,
            EntityRef::Condition(e) => &e.name,
            EntityRef::Object(e) => &e.name,
            EntityRef::Action(e) => &e.name,
            EntityRef::Hazard(e) => &e.name,
            EntityRef::Other(_, e) => &e.name,
        }
    }

    pub fn source(&self) -> &'a str {
        match self {
            EntityRef::Monster(e) => &e.source,
            EntityRef::Spell(e) => &e.source,
            EntityRef::Item(e) => &e.source,
            EntityRef::Class(e) => &e.source,
            EntityRef::Race(e) => &e.source,
            EntityRef::Background(e) => &e.source,
            EntityRef::Feat(e) => &e.source,
            EntityRef::Condition(e) => &e.source,
            EntityRef::Object(e) => &e.source,
            EntityRef::Action(e) => &e.source,
            EntityRef::Hazard(e) => &e.source,
            EntityRef::Other(_, e) => &e.source,
        }
    }

    /// The fields the typed struct doesn't model, for generic access.
    pub fn extra(&self) -> &'a Map<String, Value> {
        match self {
            EntityRef::Monster(e) => &e.extra,
            EntityRef::Spell(e) => &e.extra,
            EntityRef::Item(e) => &e.extra,
            EntityRef::Class(e) => &e.extra,
            EntityRef::Race(e) => &e.extra,
            EntityRef::Background(e) => &e.extra,
            EntityRef::Feat(e) => &e.extra,
            EntityRef::Condition(e) => &e.extra,
            EntityRef::Object(e) => &e.extra,
            EntityRef::Action(e) => &e.extra,
            EntityRef::Hazard(e) => &e.extra,
            EntityRef::Other(_, e) => &e.extra,
        }
    }

    /// Extra words search matches besides the name: alternative names
    /// (`alias`, `altNames`) and, for entities that belong to another
    /// (subclasses, subraces, cards), the owner's name.
    pub fn keywords(&self) -> Vec<&'a str> {
        let extra = self.extra();
        let mut words = Vec::new();
        for key in ["alias", "altNames"] {
            if let Some(Value::Array(a)) = extra.get(key) {
                words.extend(a.iter().filter_map(Value::as_str));
            }
        }
        for key in ["className", "raceName", "set", "path"] {
            if let Some(Value::String(s)) = extra.get(key) {
                words.push(s);
            }
        }
        words
    }
}

impl Library {
    /// Builds the search and pin indexes; every constructor of a usable
    /// library (parsing, decoding a snapshot) ends with this.
    pub fn indexed(mut self) -> Self {
        self.search_index = SearchIndex::build(&self);
        let mut pin_index = HashMap::new();
        for kind in EntityKind::all() {
            for (position, entity) in self.entities_of(kind).enumerate() {
                pin_index
                    .entry(PinId::new(kind, entity.source(), entity.name()))
                    .or_insert((kind, position));
            }
        }
        self.pin_index = pin_index;
        self
    }

    /// The entity a pin refers to, if the library has it.
    pub fn pinned(&self, id: PinId) -> Option<(EntityKind, EntityRef<'_>)> {
        let (kind, position) = *self.pin_index.get(&id)?;
        Some((kind, self.entity_at(kind, position)?))
    }

    /// The entity at `position` in the iteration order of `entities_of(kind)`.
    pub fn entity_at(&self, kind: EntityKind, position: usize) -> Option<EntityRef<'_>> {
        Some(match kind {
            EntityKind::Bestiary => EntityRef::Monster(self.monsters.get(position)?),
            EntityKind::Spells => EntityRef::Spell(self.spells.get(position)?),
            EntityKind::Items => EntityRef::Item(self.items.get(position)?),
            EntityKind::Classes => EntityRef::Class(self.classes.get(position)?),
            EntityKind::Races => EntityRef::Race(self.races.get(position)?),
            EntityKind::Backgrounds => EntityRef::Background(self.backgrounds.get(position)?),
            EntityKind::Feats => EntityRef::Feat(self.feats.get(position)?),
            EntityKind::Conditions => EntityRef::Condition(self.conditions.get(position)?),
            EntityKind::Objects => EntityRef::Object(self.objects.get(position)?),
            EntityKind::Actions => EntityRef::Action(self.actions.get(position)?),
            EntityKind::Hazards => EntityRef::Hazard(self.hazards.get(position)?),
            other => EntityRef::Other(other, self.other.get(&other)?.get(position)?),
        })
    }

    pub fn entities_of(&self, kind: EntityKind) -> Box<dyn Iterator<Item = EntityRef<'_>> + '_> {
        match kind {
            EntityKind::Bestiary => Box::new(self.monsters.iter().map(EntityRef::Monster)),
            EntityKind::Spells => Box::new(self.spells.iter().map(EntityRef::Spell)),
            EntityKind::Items => Box::new(self.items.iter().map(EntityRef::Item)),
            EntityKind::Classes => Box::new(self.classes.iter().map(EntityRef::Class)),
            EntityKind::Races => Box::new(self.races.iter().map(EntityRef::Race)),
            EntityKind::Backgrounds => Box::new(self.backgrounds.iter().map(EntityRef::Background)),
            EntityKind::Feats => Box::new(self.feats.iter().map(EntityRef::Feat)),
            EntityKind::Conditions => Box::new(self.conditions.iter().map(EntityRef::Condition)),
            EntityKind::Objects => Box::new(self.objects.iter().map(EntityRef::Object)),
            EntityKind::Actions => Box::new(self.actions.iter().map(EntityRef::Action)),
            EntityKind::Hazards => Box::new(self.hazards.iter().map(EntityRef::Hazard)),
            other => Box::new(
                self.other
                    .get(&other)
                    .into_iter()
                    .flatten()
                    .map(move |o| EntityRef::Other(other, o)),
            ),
        }
    }

    /// Case-insensitive lookup, used both to resolve routes and to resolve
    /// `{@tag name|source}` markup - 5etools authors write these names
    /// casually (e.g. `{@creature air elemental}`), unlike routes which
    /// come from an exact name already in hand. `source` is matched
    /// case-insensitively too, and ignored (searching all sources) when
    /// absent - a reasonable simplification of 5etools' more elaborate
    /// default-source rules.
    pub fn find_ci(&self, kind: EntityKind, source: Option<&str>, name: &str) -> Option<EntityRef<'_>> {
        self.entities_of(kind)
            .find(|e| e.name().eq_ignore_ascii_case(name) && source.is_none_or(|s| e.source().eq_ignore_ascii_case(s)))
    }

    /// [`Self::find_ci`] for link markup: with no explicit `source`, a match
    /// from a book the ruleset toggle hides (a 2014 core rulebook under 2024
    /// rules, ...) loses to one it shows, falling back to the hidden one
    /// only when it's the sole match.
    pub fn find_for_ruleset(
        &self,
        kind: EntityKind,
        source: Option<&str>,
        name: &str,
        use_2024: bool,
    ) -> Option<EntityRef<'_>> {
        let mut matches = self.entities_of(kind).filter(|e| {
            e.name().eq_ignore_ascii_case(name) && source.is_none_or(|s| e.source().eq_ignore_ascii_case(s))
        });
        let first = matches.next()?;
        if source.is_some() || !excluded_by_ruleset(first.source(), use_2024) {
            return Some(first);
        }
        Some(
            matches
                .find(|e| !excluded_by_ruleset(e.source(), use_2024))
                .unwrap_or(first),
        )
    }

    /// The book printed under `source`.
    pub fn book(&self, source: &str) -> Option<&Other> {
        self.other
            .get(&EntityKind::Books)?
            .iter()
            .find(|b| b.source.eq_ignore_ascii_case(source))
    }

    /// The full title of a source code, where known - for the source tag's tooltip.
    pub fn source_full_name(&self, code: &str) -> Option<&str> {
        self.books
            .iter()
            .find(|b| b.source.eq_ignore_ascii_case(code))
            .map(|b| b.name.as_str())
    }
}

/// The core rulebooks, in either edition, called out with their own
/// source-tag color since nearly every card traces back to
/// one of them.
pub fn is_base_source(code: &str) -> bool {
    matches!(
        code.to_ascii_uppercase().as_str(),
        "PHB" | "DMG" | "MM" | "XPHB" | "XDMG" | "XMM"
    )
}

/// Whether `source` should be hidden under the given 2014/2024 rules
/// selection (the ruleset toggle). Only the core rulebooks
/// have both a 2014 and a 2024 edition in the corpus; every other source is shown regardless of the
/// toggle.
pub fn excluded_by_ruleset(source: &str, use_2024: bool) -> bool {
    ruleset_excluded_sources(use_2024)
        .iter()
        .any(|s| s.eq_ignore_ascii_case(source))
}

/// The core rulebooks the given ruleset selection hides.
pub fn ruleset_excluded_sources(use_2024: bool) -> &'static [&'static str] {
    if use_2024 {
        &["PHB", "DMG", "MM"]
    } else {
        &["XPHB", "XDMG", "XMM"]
    }
}

/// Whether a newer commit of the tracked 5etools-src branch is available
/// than what's cached locally. Used to decide whether to show the update
/// icon button.
pub async fn update_available() -> Result<bool, String> {
    let stored = sync::stored_sha().await?;
    let remote = sync::remote_sha().await?;
    Ok(stored.as_deref() != Some(remote.as_str()))
}

/// Loads the library for app startup: if no data is cached yet, downloads
/// the full `data/` tree first (reporting progress); then loads the cached
/// library snapshot, preparing (parsing and saving) it first when there is
/// none for this data and build. It doesn't check for updates - that's a
/// separate, explicit action (see [`update_available`] and
/// [`resync_and_reload`]).
pub async fn load_library(mut progress: Signal<String>) -> Result<Library, String> {
    let stored = sync::stored_sha().await?;
    let sha = match stored {
        Some(sha) if sync::cached_file_count().await? > 0 => sha,
        _ => {
            progress.set("Checking for updates...".to_string());
            let sha = sync::remote_sha().await?;
            sync::sync_files(&sha, progress).await?;
            sha
        }
    };
    progress.set("Loading library...".to_string());
    match snapshot::load(&sha).await? {
        Some(library) => Ok(library),
        None => prepare_library(&sha, progress).await,
    }
}

/// The chapters of the book or adventure with the given `books.json` or
/// `adventures.json` id, read from the cached files: its text is far larger
/// than the rest of the library, so it is parsed only when its page is opened.
pub async fn load_book(id: &str) -> Result<Vec<Value>, String> {
    let id = id.to_lowercase();
    let mut path = format!("book/book-{id}.json");
    let mut cached = db::get(db::STORE_FILES, &path).await?;
    if cached.is_none() {
        path = format!("adventure/adventure-{id}.json");
        cached = db::get(db::STORE_FILES, &path).await?;
    }
    let raw = cached.ok_or_else(|| format!("{path} missing from the local data cache"))?;
    let mut file: Value = serde_json::from_str(&raw).map_err(|e| format!("parsing {path}: {e}"))?;
    Ok(loader::take_array(&mut file, "data"))
}

/// Downloads what changed in the latest `data/` tree and prepares the
/// library from it - the action behind the update icon button.
pub async fn resync_and_reload(mut progress: Signal<String>) -> Result<Library, String> {
    progress.set("Checking for updates...".to_string());
    let sha = sync::remote_sha().await?;
    sync::sync_files(&sha, progress).await?;
    prepare_library(&sha, progress).await
}

/// Parses the cached files into a library and caches it as the snapshot for
/// `data_sha`.
async fn prepare_library(data_sha: &str, mut progress: Signal<String>) -> Result<Library, String> {
    let library = loader::parse_library(progress).await?;
    progress.set("Caching the prepared library...".to_string());
    // The snapshot only speeds up later starts, so failing to save it (e.g.
    // storage quota) must not fail this one.
    let _ = snapshot::save(&library, data_sha).await;
    Ok(library)
}

#[cfg(test)]
mod tests {
    use super::excluded_by_ruleset;

    #[test]
    fn ruleset_2024_excludes_2014_core_books_only() {
        for code in ["PHB", "DMG", "MM", "phb"] {
            assert!(excluded_by_ruleset(code, true));
        }
        for code in ["XPHB", "XDMG", "XMM", "CoS", "PSX"] {
            assert!(!excluded_by_ruleset(code, true));
        }
    }

    #[test]
    fn ruleset_2014_excludes_2024_core_books_only() {
        for code in ["XPHB", "XDMG", "XMM"] {
            assert!(excluded_by_ruleset(code, false));
        }
        for code in ["PHB", "DMG", "MM", "CoS"] {
            assert!(!excluded_by_ruleset(code, false));
        }
    }
}
