//! Live search across every loaded entity, with fuzzy
//! matching against names plus each entity's kind and source so a query
//! like "blight bes" finds bestiary Blights and "smogre" finds the Smoke
//! Grenade even though its letters aren't contiguous.
//!
//! Everything a query compares against is lowercased once, when the
//! library is loaded ([`SearchIndex`]), so a search allocates nothing per
//! entity; only the best [`MAX_RESULTS`] matches are kept, in a bounded heap.

use std::collections::BinaryHeap;

use crate::data::{excluded_by_ruleset, EntityRef, Library};
use crate::routes::EntityKind;

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

/// One entity's searchable text, lowercased.
#[derive(Clone)]
struct IndexEntry {
    kind: EntityKind,
    /// The entity's position in `Library::entities_of(kind)`.
    position: usize,
    name: Box<str>,
    source: Box<str>,
    keywords: Box<[Box<str>]>,
}

/// The lowercased search text of every entity, in `EntityKind::all()` order.
#[derive(Clone, Default)]
pub struct SearchIndex {
    entries: Vec<IndexEntry>,
    /// Lowercased kind labels, indexed by `EntityKind as usize`.
    kind_labels: Vec<Box<str>>,
}

impl SearchIndex {
    pub fn build(library: &Library) -> Self {
        let mut entries = Vec::new();
        for kind in EntityKind::all() {
            for (position, entity) in library.entities_of(kind).enumerate() {
                entries.push(IndexEntry {
                    kind,
                    position,
                    name: entity.name().to_lowercase().into(),
                    source: entity.source().to_lowercase().into(),
                    keywords: entity.keywords().into_iter().map(|k| k.to_lowercase().into()).collect(),
                });
            }
        }
        let kind_labels = EntityKind::all().map(|k| k.label().to_lowercase().into()).collect();
        SearchIndex { entries, kind_labels }
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
/// (lower is better - see [`fuzzy_cost`]), then alphabetically. `use_2024`
/// hides whichever core rulebooks the ruleset toggle currently
/// excludes (see [`crate::data::excluded_by_ruleset`]).
pub fn search<'a>(library: &'a Library, query: &str, use_2024: bool) -> Vec<SearchHit<'a>> {
    let needles: Vec<Needle> = query.split_whitespace().map(Needle::new).collect();
    if needles.is_empty() {
        return Vec::new();
    }
    let index = &library.search_index;

    let mut best: BinaryHeap<Candidate> = BinaryHeap::with_capacity(MAX_RESULTS + 1);
    for (i, entry) in index.entries.iter().enumerate() {
        if excluded_by_ruleset(&entry.source, use_2024) {
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
    use super::{fuzzy_cost, Needle};

    fn cost(haystack: &str, needle: &str) -> Option<i32> {
        fuzzy_cost(haystack, &Needle::new(needle))
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
