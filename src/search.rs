//! Live search across every loaded entity, with fuzzy
//! matching against names plus each entity's kind and source so a query
//! like "blight bes" finds bestiary Blights and "smogre" finds the Smoke
//! Grenade even though its letters aren't contiguous.
//!
//! Everything a query compares against is lowercased once, when the
//! library is loaded ([`SearchIndex`]), so a search allocates nothing per
//! entity; only the best [`MAX_RESULTS`] matches are kept, in a bounded heap.

use std::collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap, HashSet};

use serde_json::Value;

use crate::data::{is_base_source, ruleset_excluded_sources, EntityRef, Library};
use crate::routes::EntityKind;
use crate::statblock::format::{cr_string, cr_to_number, habitat_name, title_case};

pub struct SearchHit<'a> {
    pub kind: EntityKind,
    pub entity: EntityRef<'a>,
}

/// How many cards a single search renders at once. The corpus can hold tens
/// of thousands of entities across all sources; without a cap a broad query
/// (e.g. a single letter) would try to render all of them at once.
const MAX_RESULTS: usize = 60;

/// Extra cost added when a query word only matched an entity's source or
/// kind rather than its name, so a name match always outranks a
/// kind/source match of otherwise equal quality.
const FIELD_PENALTY: i32 = 1000;

/// Keywords (aliases, an owner's name) rank between a name match and a
/// kind/source match.
const KEYWORD_PENALTY: i32 = FIELD_PENALTY / 2;

/// A property of one kind's entities that the filter menu narrows the
/// results by.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum Facet {
    Habitat,
    Rarity,
    SpellClass,
}

impl Facet {
    /// The kind whose entities have this property; selecting a value of it
    /// restricts the results to that kind.
    pub fn kind(self) -> EntityKind {
        match self {
            Facet::Habitat => EntityKind::Bestiary,
            Facet::Rarity => EntityKind::Items,
            Facet::SpellClass => EntityKind::Spells,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Facet::Habitat => "Habitat",
            Facet::Rarity => "Rarity",
            Facet::SpellClass => "Class",
        }
    }
}

/// An ordered property of one kind's entities that the filter menu narrows
/// the results to a range of.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum Scale {
    Cr,
    SpellLevel,
}

impl Scale {
    /// The kind whose entities lie on this scale; setting a bound restricts
    /// the results to that kind.
    pub fn kind(self) -> EntityKind {
        match self {
            Scale::Cr => EntityKind::Bestiary,
            Scale::SpellLevel => EntityKind::Spells,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Scale::Cr => "CR",
            Scale::SpellLevel => "Level",
        }
    }
}

/// An entity's value on its kind's scale, with its label: a monster's
/// challenge rating ("1/4" is 0.25), a spell's level ("Cantrip" is 0).
fn scale_value(entity: &EntityRef) -> Option<(Scale, Box<str>, f64)> {
    match entity {
        EntityRef::Monster(m) => {
            let cr = m.cr.as_ref().and_then(cr_string)?;
            let value = cr_to_number(&cr)?;
            Some((Scale::Cr, cr.into(), value))
        }
        EntityRef::Spell(s) => {
            let level = s.level?;
            let label = if level == 0 {
                "Cantrip".into()
            } else {
                level.to_string().into()
            };
            Some((Scale::SpellLevel, label, level.into()))
        }
        _ => None,
    }
}

/// Item rarities from least to most rare, the order the rarity filter lists
/// them in.
const RARITIES: [&str; 10] = [
    "None",
    "Common",
    "Uncommon",
    "Rare",
    "Very Rare",
    "Legendary",
    "Artifact",
    "Varies",
    "Unknown",
    "Unknown (Magic)",
];

/// The facet values of an entity: a monster's habitats,
/// an item's rarity, the classes that can cast a spell.
fn facet_values(entity: &EntityRef, library: &Library) -> Vec<(Facet, Box<str>)> {
    match entity {
        EntityRef::Monster(m) => m
            .environment
            .iter()
            .map(|e| (Facet::Habitat, habitat_name(e).into()))
            .collect(),
        EntityRef::Item(i) => i.rarity.iter().map(|r| (Facet::Rarity, title_case(r).into())).collect(),
        EntityRef::Spell(s) => {
            let classes = library
                .spell_lookup
                .get(&s.source.to_lowercase())
                .and_then(|by_name| by_name.get(&s.name.to_lowercase()))
                .and_then(|entry| entry.get("class"))
                .and_then(Value::as_object);
            let mut names: Vec<&str> = classes
                .into_iter()
                .flat_map(|by_source| by_source.values())
                .filter_map(Value::as_object)
                .flat_map(|list| list.keys().map(String::as_str))
                .collect();
            names.sort_unstable();
            names.dedup();
            names.into_iter().map(|c| (Facet::SpellClass, c.into())).collect()
        }
        _ => Vec::new(),
    }
}

/// The order a facet's values are listed in: rarities by rank, everything
/// else alphabetically.
fn facet_order(facet: Facet, value: &str) -> (usize, String) {
    let rank = match facet {
        Facet::Rarity => RARITIES.iter().position(|r| *r == value).unwrap_or(RARITIES.len()),
        Facet::Habitat | Facet::SpellClass => 0,
    };
    (rank, value.to_lowercase())
}

/// What the filter menu lets through: the kinds and sources not switched
/// off, a range per scale and, per facet, the selected values. The default
/// is the 2024 ruleset: the 2014 core rulebooks are off, everything else is
/// on, and neither a range bound nor a facet value is set.
#[derive(Clone, PartialEq)]
pub struct Filters {
    hidden_kinds: HashSet<EntityKind>,
    /// Lowercased source codes.
    hidden_sources: HashSet<Box<str>>,
    /// The lowest and highest value shown on each scale; `None` leaves that
    /// end open. Only scales with a bound set are present.
    ranges: BTreeMap<Scale, [Option<f64>; 2]>,
    /// Only facets with at least one selected value are present.
    facets: BTreeMap<Facet, BTreeSet<Box<str>>>,
}

impl Default for Filters {
    fn default() -> Self {
        let mut filters = Filters {
            hidden_kinds: HashSet::new(),
            hidden_sources: HashSet::new(),
            ranges: BTreeMap::new(),
            facets: BTreeMap::new(),
        };
        filters.set_ruleset(true);
        filters
    }
}

impl Filters {
    /// Switches off the core rulebooks of the other edition and on those of
    /// the selected one; every other source keeps its state.
    pub fn set_ruleset(&mut self, use_2024: bool) {
        for source in ruleset_excluded_sources(use_2024) {
            self.hidden_sources.insert(source.to_lowercase().into());
        }
        for source in ruleset_excluded_sources(!use_2024) {
            self.hidden_sources.remove(source.to_lowercase().as_str());
        }
    }

    /// Whether nothing is filtered out beyond the other edition's core
    /// rulebooks: every kind and every other source is on, and neither a
    /// range bound nor a facet value is set.
    pub fn is_default_for(&self, use_2024: bool) -> bool {
        let excluded: HashSet<Box<str>> = ruleset_excluded_sources(use_2024)
            .iter()
            .copied()
            .map(|source| source.to_lowercase().into())
            .collect();
        self.hidden_kinds.is_empty()
            && self.hidden_sources == excluded
            && self.ranges.is_empty()
            && self.facets.is_empty()
    }

    /// The kinds the ranges and the selected facet values restrict the
    /// results to, in `EntityKind::all()` order. More than one lets nothing
    /// through.
    pub fn facet_kinds(&self) -> Vec<EntityKind> {
        let kinds: HashSet<EntityKind> = self
            .facets
            .keys()
            .map(|f| f.kind())
            .chain(self.ranges.keys().map(|s| s.kind()))
            .collect();
        EntityKind::all().filter(|k| kinds.contains(k)).collect()
    }

    /// The lowest (`max` false) or highest value shown on `scale`, if set.
    pub fn range_bound(&self, scale: Scale, max: bool) -> Option<f64> {
        self.ranges.get(&scale).and_then(|range| range[max as usize])
    }

    /// Sets the lowest (`max` false) or highest value shown on `scale`;
    /// `None` leaves that end of the range open.
    pub fn set_range_bound(&mut self, scale: Scale, max: bool, value: Option<f64>) {
        let range = self.ranges.entry(scale).or_default();
        range[max as usize] = value;
        if range.iter().all(Option::is_none) {
            self.ranges.remove(&scale);
        }
    }

    pub fn clear_range(&mut self, scale: Scale) {
        self.ranges.remove(&scale);
    }

    /// Whether an entity with the given value on its kind's scale lies in
    /// every range that is set.
    fn ranges_admit(&self, value: Option<f64>) -> bool {
        self.ranges
            .values()
            .all(|[min, max]| value.is_some_and(|v| min.is_none_or(|min| v >= min) && max.is_none_or(|max| v <= max)))
    }

    pub fn facet_selected(&self, facet: Facet, value: &str) -> bool {
        self.facets.get(&facet).is_some_and(|values| values.contains(value))
    }

    pub fn toggle_facet(&mut self, facet: Facet, value: &str) {
        let values = self.facets.entry(facet).or_default();
        if !values.remove(value) {
            values.insert(value.into());
        }
        if values.is_empty() {
            self.facets.remove(&facet);
        }
    }

    /// Whether any value of `facet` is selected.
    pub fn facet_set(&self, facet: Facet) -> bool {
        self.facets.contains_key(&facet)
    }

    pub fn clear_facet(&mut self, facet: Facet) {
        self.facets.remove(&facet);
    }

    /// Whether an entity with the given facet values passes every facet
    /// with a selection: it has at least one of the selected values of each.
    fn facets_admit(&self, values: &[(Facet, Box<str>)]) -> bool {
        self.facets
            .iter()
            .all(|(facet, selected)| values.iter().any(|(f, v)| f == facet && selected.contains(v)))
    }

    pub fn kind_shown(&self, kind: EntityKind) -> bool {
        !self.hidden_kinds.contains(&kind)
    }

    /// `source` must be lowercased.
    fn source_shown_lowercased(&self, source: &str) -> bool {
        !self.hidden_sources.contains(source)
    }

    pub fn source_shown(&self, source: &str) -> bool {
        self.source_shown_lowercased(&source.to_lowercase())
    }

    pub fn toggle_kind(&mut self, kind: EntityKind) {
        if !self.hidden_kinds.remove(&kind) {
            self.hidden_kinds.insert(kind);
        }
    }

    pub fn toggle_source(&mut self, source: &str) {
        let source: Box<str> = source.to_lowercase().into();
        if !self.hidden_sources.remove(&source) {
            self.hidden_sources.insert(source);
        }
    }

    pub fn set_all_kinds(&mut self, shown: bool) {
        self.hidden_kinds = if shown {
            HashSet::new()
        } else {
            EntityKind::all().collect()
        };
    }

    pub fn set_all_sources(&mut self, shown: bool, sources: &[Box<str>]) {
        self.hidden_sources = if shown {
            HashSet::new()
        } else {
            sources.iter().map(|s| s.to_lowercase().into()).collect()
        };
    }
}

/// One entity's searchable text, lowercased.
#[derive(Clone)]
struct IndexEntry {
    kind: EntityKind,
    /// The entity's position in `Library::entities_of(kind)`.
    position: usize,
    name: Box<str>,
    source: Box<str>,
    keywords: Box<[Box<str>]>,
    /// The entity's value on its kind's scale.
    scale_value: Option<f64>,
    /// As displayed, not lowercased.
    facets: Box<[(Facet, Box<str>)]>,
}

/// The lowercased search text of every entity, in `EntityKind::all()` order.
#[derive(Clone, Default)]
pub struct SearchIndex {
    entries: Vec<IndexEntry>,
    /// Lowercased kind labels, indexed by `EntityKind as usize`.
    kind_labels: Vec<Box<str>>,
    /// Every source code in the library: the core rulebooks first, then the rest, each sorted case-insensitively.
    pub sources: Vec<Box<str>>,
    /// Every value of each facet in the library, in [`facet_order`].
    facet_options: HashMap<Facet, Vec<Box<str>>>,
    /// Every value on each scale in the library with its label, from lowest
    /// to highest.
    scale_steps: HashMap<Scale, Vec<(Box<str>, f64)>>,
}

impl SearchIndex {
    pub fn build(library: &Library) -> Self {
        let mut entries = Vec::new();
        let mut sources = HashSet::new();
        let mut facet_options: HashMap<Facet, HashSet<Box<str>>> = HashMap::new();
        let mut scale_steps: HashMap<Scale, Vec<(Box<str>, f64)>> = HashMap::new();
        for kind in EntityKind::all() {
            for (position, entity) in library.entities_of(kind).enumerate() {
                sources.insert(entity.source());
                // Nameless entities have no title to click, nor an address.
                if entity.name().trim().is_empty() {
                    continue;
                }
                let facets = facet_values(&entity, library);
                for (facet, value) in &facets {
                    facet_options.entry(*facet).or_default().insert(value.clone());
                }
                let scale = scale_value(&entity);
                if let Some((scale, label, value)) = &scale {
                    scale_steps.entry(*scale).or_default().push((label.clone(), *value));
                }
                entries.push(IndexEntry {
                    kind,
                    position,
                    name: entity.name().to_lowercase().into(),
                    source: entity.source().to_lowercase().into(),
                    keywords: entity.keywords().into_iter().map(|k| k.to_lowercase().into()).collect(),
                    scale_value: scale.map(|(_, _, value)| value),
                    facets: facets.into(),
                });
            }
        }
        let kind_labels = EntityKind::all().map(|k| k.label().to_lowercase().into()).collect();
        let mut sources: Vec<Box<str>> = sources.into_iter().map(Box::from).collect();
        sources.sort_by_key(|s| (!is_base_source(s), s.to_lowercase()));
        let facet_options = facet_options
            .into_iter()
            .map(|(facet, values)| {
                let mut values: Vec<Box<str>> = values.into_iter().collect();
                values.sort_by_cached_key(|v| facet_order(facet, v));
                (facet, values)
            })
            .collect();
        for steps in scale_steps.values_mut() {
            steps.sort_by(|a, b| a.1.total_cmp(&b.1));
            steps.dedup_by(|a, b| a.1 == b.1);
        }
        SearchIndex {
            entries,
            kind_labels,
            sources,
            facet_options,
            scale_steps,
        }
    }

    /// Every value on `scale` in the library with its label, from lowest to
    /// highest.
    pub fn scale_steps(&self, scale: Scale) -> &[(Box<str>, f64)] {
        self.scale_steps.get(&scale).map_or(&[], Vec::as_slice)
    }

    /// Every value of `facet` in the library, in the order the filter lists them.
    pub fn facet_options(&self, facet: Facet) -> &[Box<str>] {
        self.facet_options.get(&facet).map_or(&[], Vec::as_slice)
    }
}

/// A query word, with its characters split out once for the word-prefix
/// matcher.
struct Needle {
    text: String,
    chars: Vec<char>,
}

impl Needle {
    fn new(word: &str) -> Self {
        let text = word.to_lowercase();
        let chars = text.chars().collect();
        Needle { text, chars }
    }
}

/// A match in the bounded result heap. Ordered worst-first (highest cost,
/// then alphabetically last), so the heap's top is the one to evict.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Candidate<'a> {
    cost: i32,
    name: &'a str,
    entry: usize,
}

/// Whitespace-separated words each fuzzy-match against name, kind label,
/// source code, or an entity's keywords (aliases, owning class/race); an
/// entity is a hit only if every word matches at least one of those fields
/// (in any combination), so "blight bes" needs "blight" somewhere and "bes"
/// somewhere, not necessarily in the same field. Ranked by summed match cost
/// (lower is better - see [`fuzzy_cost`]), then alphabetically; a query
/// without words matches everything, so the first entities alphabetically
/// are listed. `filters` hides the kinds and sources switched off in the
/// filter menu and, while facet values are selected, every entity of
/// another kind or without the selected values.
pub fn search<'a>(library: &'a Library, query: &str, filters: &Filters) -> Vec<SearchHit<'a>> {
    let needles: Vec<Needle> = query.split_whitespace().map(Needle::new).collect();
    let index = &library.search_index;
    let facet_kinds = filters.facet_kinds();
    if facet_kinds.len() > 1 {
        return Vec::new();
    }

    let mut best: BinaryHeap<Candidate> = BinaryHeap::with_capacity(MAX_RESULTS + 1);
    for (i, entry) in index.entries.iter().enumerate() {
        if !filters.kind_shown(entry.kind)
            || !filters.source_shown_lowercased(&entry.source)
            || facet_kinds.first().is_some_and(|kind| *kind != entry.kind)
            || !filters.facets_admit(&entry.facets)
            || !filters.ranges_admit(entry.scale_value)
        {
            continue;
        }
        let kind_label = &index.kind_labels[entry.kind as usize];
        let mut total = 0;
        let mut matched_all = true;
        for needle in &needles {
            let keywords = entry
                .keywords
                .iter()
                .map(|k| fuzzy_cost(k, needle).map(|c| c + KEYWORD_PENALTY));
            let best_cost = [
                fuzzy_cost(&entry.name, needle),
                fuzzy_cost(kind_label, needle).map(|c| c + FIELD_PENALTY),
                fuzzy_cost(&entry.source, needle).map(|c| c + FIELD_PENALTY),
            ]
            .into_iter()
            .chain(keywords)
            .flatten()
            .min();
            match best_cost {
                Some(cost) => total += cost,
                None => {
                    matched_all = false;
                    break;
                }
            }
        }
        if !matched_all {
            continue;
        }
        let candidate = Candidate {
            cost: total,
            name: &entry.name,
            entry: i,
        };
        if best.len() < MAX_RESULTS {
            best.push(candidate);
        } else if best.peek().is_some_and(|worst| candidate < *worst) {
            best.pop();
            best.push(candidate);
        }
    }

    best.into_sorted_vec()
        .into_iter()
        .filter_map(|c| {
            let entry = &index.entries[c.entry];
            Some(SearchHit {
                kind: entry.kind,
                entity: library.entity_at(entry.kind, entry.position)?,
            })
        })
        .collect()
}

/// Lower is a better match; `None` means `needle` doesn't match `haystack`
/// at all. The haystack must already be lowercased. Cheap exact/prefix/
/// substring cases are checked first since they're both the most common
/// queries and the best possible matches; anything else falls back to
/// [`word_prefix_cost`], which requires each piece of `needle` to align to
/// a haystack *word's* start (like "smogre" -> "smoke grenade") rather than
/// letting it scatter anywhere - a plain character-subsequence scan is too
/// permissive here, since almost any short needle turns out to be *some*
/// subsequence of a long enough haystack.
fn fuzzy_cost(haystack: &str, needle: &Needle) -> Option<i32> {
    if needle.chars.is_empty() {
        return Some(0);
    }
    if haystack == needle.text {
        return Some(0);
    }
    if let Some(pos) = haystack.find(&needle.text) {
        // Prefix beats an internal substring; both beat a word-prefix match.
        return Some(if pos == 0 { 1 } else { 2 + pos as i32 });
    }
    word_prefix_cost(haystack, &needle.chars)
}

/// Whether `needle` can be split into consecutive chunks that are each a
/// prefix of a haystack word, in order - "smogre" as "smo"+"gre" against
/// "smoke grenade", or "rss" as "r"+"s"+"s" against "ring of spell storing"
/// (skipping "of"). Greedy (each word consumes the longest run of
/// remaining needle it shares a prefix with, no backtracking) rather than
/// an exhaustive search: simple, fast enough to run across the whole
/// corpus on every keystroke, and right for the word-initial queries this
/// is meant to catch.
fn word_prefix_cost(haystack: &str, needle: &[char]) -> Option<i32> {
    let mut words = 0;
    let mut consumed = 0;
    let mut cost = 50; // always ranks behind an exact/prefix/substring match
    for word in haystack.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()) {
        words += 1;
        if consumed == needle.len() {
            continue;
        }
        let common = word
            .chars()
            .zip(&needle[consumed..])
            .take_while(|(h, n)| h == *n)
            .count();
        if common == 0 {
            cost += 3; // this word contributed nothing - skipped over
            continue;
        }
        if common < word.chars().count() {
            cost += 2; // only a partial prefix of the word was used
        }
        consumed += common;
    }
    // Single-word haystacks: the fast paths above already cover them.
    (words >= 2 && consumed == needle.len()).then_some(cost)
}

#[cfg(test)]
mod tests {
    use super::{fuzzy_cost, Facet, Filters, Needle, Scale};
    use crate::routes::EntityKind;

    fn cost(haystack: &str, needle: &str) -> Option<i32> {
        fuzzy_cost(haystack, &Needle::new(needle))
    }

    #[test]
    fn filters_are_default_until_changed_beyond_the_ruleset() {
        let mut filters = Filters::default();
        assert!(filters.is_default_for(true));
        assert!(!filters.is_default_for(false));
        filters.set_ruleset(false);
        assert!(filters.is_default_for(false));
        filters.toggle_source("MPMM");
        assert!(!filters.is_default_for(false));
        filters.toggle_source("MPMM");
        filters.toggle_kind(EntityKind::Spells);
        assert!(!filters.is_default_for(false));
    }

    #[test]
    fn group_filters_restrict_the_results_to_their_kind() {
        let mut filters = Filters::default();
        filters.set_range_bound(Scale::Cr, false, Some(1.0));
        assert_eq!(filters.facet_kinds(), [EntityKind::Bestiary]);
        filters.toggle_facet(Facet::Habitat, "Forest");
        assert_eq!(filters.facet_kinds(), [EntityKind::Bestiary]);
        assert!(!filters.is_default_for(true));
        filters.set_range_bound(Scale::SpellLevel, true, Some(3.0));
        assert_eq!(filters.facet_kinds(), [EntityKind::Bestiary, EntityKind::Spells]);
        filters.set_range_bound(Scale::Cr, false, None);
        filters.clear_range(Scale::SpellLevel);
        filters.clear_facet(Facet::Habitat);
        assert!(filters.is_default_for(true));
    }

    #[test]
    fn ranges_are_open_at_an_unset_end() {
        let mut filters = Filters::default();
        assert!(filters.ranges_admit(None));
        filters.set_range_bound(Scale::Cr, true, Some(0.5));
        assert!(filters.ranges_admit(Some(0.0)));
        assert!(filters.ranges_admit(Some(0.5)));
        assert!(!filters.ranges_admit(Some(1.0)));
        assert!(!filters.ranges_admit(None));
        filters.set_range_bound(Scale::Cr, false, Some(0.125));
        assert!(!filters.ranges_admit(Some(0.0)));
    }

    #[test]
    fn facets_admit_entities_with_one_selected_value_of_each() {
        let mut filters = Filters::default();
        filters.toggle_facet(Facet::Habitat, "Forest");
        filters.toggle_facet(Facet::Habitat, "Hill");
        filters.toggle_facet(Facet::SpellClass, "Wizard");
        let forest_wizard = [(Facet::Habitat, "Forest".into()), (Facet::SpellClass, "Wizard".into())];
        let forest_only = [(Facet::Habitat, "Forest".into())];
        assert!(filters.facets_admit(&forest_wizard));
        assert!(!filters.facets_admit(&forest_only));
    }

    #[test]
    fn exact_match_is_free() {
        assert_eq!(cost("goblin", "goblin"), Some(0));
    }

    #[test]
    fn prefix_beats_substring() {
        let prefix = cost("bestiary", "bes").unwrap();
        let substring = cost("hobestiary", "bes").unwrap();
        assert!(prefix < substring);
    }

    #[test]
    fn word_initials_match_across_words() {
        // "smogre" <- "smo"+"gre", the start of each word in "smoke grenade".
        assert!(cost("smoke grenade", "smogre").is_some());
        // "rss" <- "r"+"s"+"s", skipping "of".
        assert!(cost("ring of spell storing", "rss").is_some());
    }

    #[test]
    fn word_initials_do_not_match_unrelated_words() {
        // Every character of "blight" happens to occur, in order, somewhere
        // in this phrase - but not at any word's start, so a plain
        // character-subsequence scan would wrongly accept it.
        assert_eq!(cost("blessing of health", "blight"), None);
    }

    #[test]
    fn single_word_gappy_match_is_rejected() {
        // "bes" is a subsequence of "blessy" (b, then e, then s) but not a
        // prefix of it or of any word within it (there's only one word).
        assert_eq!(cost("blessy", "bes"), None);
    }

    #[test]
    fn missing_characters_do_not_match() {
        assert_eq!(cost("goblin", "zzz"), None);
    }

    #[test]
    fn kind_prefix_matches_but_unrelated_kind_does_not() {
        assert!(cost("bestiary", "bes").is_some());
        assert_eq!(cost("spells", "bes"), None);
    }
}
