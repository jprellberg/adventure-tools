//! The `/books/:source/:..section` page: a book's table of contents next to
//! the text of the part of the book selected in it. Each of the two scrolls
//! on its own.
//!
//! The table of contents lists the book's chapters and, below each, its
//! named sections. A part is addressed in the URL by its position: the
//! chapter's index, then the section's index within the chapter
//! (`/books/SRC/2/1`); without one the first chapter is shown.

use std::collections::HashSet;

use dioxus::prelude::*;
use serde_json::Value;

use crate::card::kind_source_tags;
use crate::data;
use crate::model::Entry;
use crate::render::{render_entry, RenderCtx};
use crate::routes::{EntityKind, Route};
use crate::statblock::format::str_at;
use crate::state::LibrarySignal;
use crate::tags::strip_tags;
use crate::{HoverSignal, ModalSignal};

/// A chapter or one of its sections, as listed in the table of contents.
struct TocNode {
    name: String,
    children: Vec<TocNode>,
}

fn is_block(node: &Value) -> bool {
    matches!(node.get("type").and_then(Value::as_str), Some("entries" | "section"))
}

fn block_name(block: &Value) -> Option<&str> {
    block
        .get("name")
        .and_then(Value::as_str)
        .filter(|name| !name.trim().is_empty())
}

/// The named blocks inside `block`, looking through unnamed ones.
fn sections(block: &Value) -> Vec<&Value> {
    let mut found = Vec::new();
    collect_sections(block, &mut found);
    found
}

fn collect_sections<'a>(block: &'a Value, found: &mut Vec<&'a Value>) {
    let entries = block.get("entries").and_then(Value::as_array).into_iter().flatten();
    for entry in entries.filter(|e| is_block(e)) {
        if block_name(entry).is_some() {
            found.push(entry);
        } else {
            collect_sections(entry, found);
        }
    }
}

fn toc_node(block: &Value, with_sections: bool) -> TocNode {
    TocNode {
        name: strip_tags(block_name(block).unwrap_or("Untitled")),
        children: if with_sections {
            sections(block).into_iter().map(|s| toc_node(s, false)).collect()
        } else {
            Vec::new()
        },
    }
}

fn toc(chapters: &[Value]) -> Vec<TocNode> {
    chapters.iter().map(|chapter| toc_node(chapter, true)).collect()
}

/// The chapter at `path[0]`, or its section at `path[1]`.
fn select<'a>(chapters: &'a [Value], path: &[usize]) -> Option<&'a Value> {
    let chapter = chapters.get(*path.first()?)?;
    match path.get(1) {
        None => Some(chapter),
        Some(&section) => sections(chapter).get(section).copied(),
    }
}

fn book_route(source: &str, path: &[usize]) -> Route {
    Route::Book {
        source: source.to_string(),
        section: path.iter().map(usize::to_string).collect(),
    }
}

#[component]
pub fn BookPage(source: String, section: Vec<String>) -> Element {
    let library = use_context::<LibrarySignal>();
    let modal = use_context::<ModalSignal>();
    let hover = use_context::<HoverSignal>();
    let book_id = library
        .read()
        .as_ref()
        .and_then(|lib| {
            lib.book(&source)
                .and_then(|book| str_at(&book.extra, "id").map(str::to_string))
        })
        .unwrap_or_default();
    let chapters = use_resource(use_reactive!(|book_id| async move { data::load_book(&book_id).await }));
    let path: Vec<usize> = section.iter().map_while(|part| part.parse().ok()).take(2).collect();
    let selected = if path.is_empty() { vec![0] } else { path };

    // The selected chapter is always open; the others stay as the user left them.
    let mut expanded = use_signal(HashSet::<usize>::new);
    let chapter = selected[0];
    use_effect(use_reactive!(|chapter| {
        expanded.write().insert(chapter);
    }));
    let location = book_route(&source, &selected);
    use_effect(use_reactive!(|location| {
        let _ = location;
        document::eval("document.getElementById('book-content')?.scrollTo(0, 0);");
    }));

    let Some(lib) = library.read().clone() else {
        return rsx! {};
    };
    let Some(book) = lib.book(&source) else {
        return rsx! {
            p { class: "mt-32 text-center text-gray-400", "No book with source {source}." }
        };
    };
    let title = book.name.clone();
    let ctx = RenderCtx::new(&lib, modal, hover);

    let (contents, text) = match &*chapters.read() {
        None => (
            rsx! {},
            rsx! {
                p { class: "text-gray-400", "Loading…" }
            },
        ),
        Some(Err(error)) => (
            rsx! {},
            rsx! {
                p { class: "text-red-600", "Failed to load the book: {error}" }
            },
        ),
        Some(Ok(chapters)) => (
            contents_list(&toc(chapters), &source, &selected, expanded),
            match select(chapters, &selected) {
                Some(part) => {
                    let depth = if selected.len() == 1 { -1 } else { 0 };
                    render_entry(&Entry::from_value(part.clone()), ctx.with_depth(depth))
                }
                None => rsx! {
                    p { class: "text-gray-400", "This part of the book doesn't exist." }
                },
            },
        ),
    };

    rsx! {
        div { class: "mx-auto flex h-[calc(100dvh-6rem)] w-full max-w-[90rem] flex-col justify-center gap-4 lg:flex-row",
            nav { class: "flex max-h-[35%] shrink-0 flex-col bg-white lg:max-h-none lg:w-72",
                h2 { class: "p-2 text-base font-bold leading-tight",
                    "{title}"
                    {kind_source_tags(EntityKind::Books, &source, &lib)}
                }
                div { class: "min-h-0 flex-1 overflow-y-auto py-1", {contents} }
            }
            article {
                id: "book-content",
                class: "min-h-0 w-full flex-1 overflow-y-auto bg-white p-4 text-sm lg:max-w-[63rem]",
                {text}
            }
        }
    }
}

/// The table of contents: chapters, each folding its sections away.
fn contents_list(toc: &[TocNode], source: &str, selected: &[usize], mut expanded: Signal<HashSet<usize>>) -> Element {
    rsx! {
        ul { class: "px-2 text-sm",
            for (i , chapter) in toc.iter().enumerate() {
                li { key: "{i}",
                    div { class: "flex items-start",
                        if chapter.children.is_empty() {
                            span { class: "w-6 shrink-0" }
                        } else {
                            button {
                                class: "w-6 shrink-0 py-1 text-gray-400 hover:text-gray-700",
                                title: if expanded.read().contains(&i) { "Collapse" } else { "Expand" },
                                aria_expanded: expanded.read().contains(&i),
                                onclick: move |_| {
                                    let mut open = expanded.write();
                                    if !open.remove(&i) {
                                        open.insert(i);
                                    }
                                },
                                if expanded.read().contains(&i) {
                                    "▾"
                                } else {
                                    "▸"
                                }
                            }
                        }
                        Link {
                            class: toc_link_class(selected == [i], "font-semibold"),
                            to: book_route(source, &[i]),
                            "{chapter.name}"
                        }
                    }
                    if expanded.read().contains(&i) {
                        ul { class: "ml-9",
                            for (j , section) in chapter.children.iter().enumerate() {
                                li { key: "{j}",
                                    Link {
                                        class: toc_link_class(selected == [i, j], ""),
                                        to: book_route(source, &[i, j]),
                                        "{section.name}"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn toc_link_class(selected: bool, extra: &str) -> String {
    let state = if selected {
        "bg-blue-50 text-blue-800"
    } else {
        "text-gray-700 hover:bg-gray-50"
    };
    format!("block min-w-0 flex-1 rounded px-2 py-1 leading-tight {state} {extra}")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn book() -> Vec<Value> {
        vec![
            json!({ "type": "section", "name": "One", "entries": [
                "intro",
                { "type": "entries", "name": "First", "entries": ["a"] },
                { "type": "entries", "entries": [
                    { "type": "entries", "name": "Hidden In Unnamed", "entries": ["b"] } ] },
                { "type": "inset", "name": "Not A Section", "entries": ["c"] },
            ] }),
            json!({ "type": "section", "name": "Two {@b bold}", "entries": ["only text"] }),
        ]
    }

    #[test]
    fn contents_list_chapters_and_their_named_sections() {
        let toc = toc(&book());
        assert_eq!(toc.len(), 2);
        assert_eq!(toc[0].name, "One");
        let names: Vec<&str> = toc[0].children.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["First", "Hidden In Unnamed"]);
        assert_eq!(toc[1].name, "Two bold");
        assert!(toc[1].children.is_empty());
    }

    #[test]
    fn a_path_selects_a_chapter_or_one_of_its_sections() {
        let chapters = book();
        assert_eq!(select(&chapters, &[1]).unwrap()["name"], "Two {@b bold}");
        assert_eq!(select(&chapters, &[0, 1]).unwrap()["name"], "Hidden In Unnamed");
        assert!(select(&chapters, &[0, 2]).is_none());
        assert!(select(&chapters, &[2]).is_none());
    }
}
