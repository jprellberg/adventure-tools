//! Extracts the two kinds of entity that live only inside book and adventure
//! text: captioned tables (at any depth) and rules (named `entries` blocks
//! one or two levels below a chapter). Whole chapters, deeper blocks and
//! everything else in the books stay unindexed.

use serde_json::{json, Map, Value};

use crate::tags::strip_tags;

/// The extracted entities of one book, as JSON objects ready for the
/// generic loader.
#[derive(Default)]
pub struct BookContent {
    pub rules: Vec<Value>,
    pub tables: Vec<Value>,
}

/// Rules are the named blocks at these depths, counting a chapter as 0.
const RULE_DEPTHS: std::ops::RangeInclusive<usize> = 1..=2;

struct Walker<'a> {
    source: &'a str,
    book_name: &'a str,
    /// Whether rules (not just tables) are wanted; adventures only supply tables.
    with_rules: bool,
    /// The index of the chapter being walked.
    chapter: usize,
    /// The sections of that chapter, as the book page lists them.
    sections: Vec<*const Value>,
    out: BookContent,
}

/// Rules and tables of one book (`data` is the file's chapter list).
pub fn extract(source: &str, book_name: &str, data: &[Value], with_rules: bool) -> BookContent {
    let mut walker = Walker {
        source,
        book_name,
        with_rules,
        chapter: 0,
        sections: Vec::new(),
        out: BookContent::default(),
    };
    for (index, chapter) in data.iter().enumerate() {
        walker.chapter = index;
        walker.sections = sections(chapter).into_iter().map(|s| s as *const Value).collect();
        walker.walk(chapter, 0, &mut Vec::new(), None, None);
    }
    walker.out
}

fn is_block(kind: &str) -> bool {
    matches!(kind, "entries" | "section")
}

fn is_block_node(node: &Value) -> bool {
    node.get("type").and_then(Value::as_str).is_some_and(is_block)
}

/// The name of a chapter or section, if it has one.
pub fn block_name(block: &Value) -> Option<&str> {
    block
        .get("name")
        .and_then(Value::as_str)
        .filter(|name| !name.trim().is_empty())
}

/// The named blocks inside `block`, looking through unnamed ones: the
/// sections the book page lists below a chapter.
pub fn sections(block: &Value) -> Vec<&Value> {
    let mut found = Vec::new();
    collect_sections(block, &mut found);
    found
}

fn collect_sections<'a>(block: &'a Value, found: &mut Vec<&'a Value>) {
    let entries = block.get("entries").and_then(Value::as_array).into_iter().flatten();
    for entry in entries.filter(|e| is_block_node(e)) {
        if block_name(entry).is_some() {
            found.push(entry);
        } else {
            collect_sections(entry, found);
        }
    }
}

impl Walker<'_> {
    /// `path` holds the names of the enclosing named blocks; `page` the
    /// nearest page number seen on the way down; `section` the index of the
    /// innermost enclosing section of the chapter.
    fn walk(&mut self, node: &Value, depth: usize, path: &mut Vec<String>, page: Option<i64>, section: Option<usize>) {
        match node {
            Value::Array(items) => items.iter().for_each(|n| self.walk(n, depth, path, page, section)),
            Value::Object(map) => {
                let kind = map.get("type").and_then(Value::as_str).unwrap_or("");
                let page = map.get("page").and_then(Value::as_i64).or(page);
                let section = if is_block(kind) {
                    self.sections.iter().position(|s| std::ptr::eq(*s, node)).or(section)
                } else {
                    section
                };
                if kind == "table" {
                    self.table(map, path, page, section);
                }
                let named = is_block(kind)
                    .then(|| map.get("name").and_then(Value::as_str))
                    .flatten();
                if let Some(name) = named.filter(|_| self.with_rules && RULE_DEPTHS.contains(&depth)) {
                    self.rule(map, name, path, page, section);
                }
                let pushed = named.map(strip_tags);
                let child_depth = if is_block(kind) { depth + 1 } else { depth };
                if let Some(name) = &pushed {
                    path.push(name.clone());
                }
                for value in map.values() {
                    self.walk(value, child_depth, path, page, section);
                }
                if pushed.is_some() {
                    path.pop();
                }
            }
            _ => {}
        }
    }

    fn breadcrumb(&self, path: &[String]) -> String {
        std::iter::once(self.book_name)
            .chain(path.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" › ")
    }

    /// A rule needs prose of its own; a block that only groups sub-blocks
    /// (or statblocks) is covered by its children.
    fn rule(
        &mut self,
        block: &Map<String, Value>,
        name: &str,
        path: &[String],
        page: Option<i64>,
        section: Option<usize>,
    ) {
        let has_prose = block
            .get("entries")
            .and_then(Value::as_array)
            .is_some_and(|e| e.iter().any(Value::is_string));
        if !has_prose {
            return;
        }
        self.out.rules.push(json!({
            "name": strip_tags(name),
            "source": self.source,
            "page": page,
            "id": block.get("id"),
            "path": self.breadcrumb(path),
            "chapter": self.chapter,
            "section": section,
            "entries": block["entries"],
        }));
    }

    fn table(&mut self, table: &Map<String, Value>, path: &[String], page: Option<i64>, section: Option<usize>) {
        let Some(caption) = table.get("caption").and_then(Value::as_str) else {
            return;
        };
        self.out.tables.push(json!({
            "name": strip_tags(caption),
            "source": self.source,
            "page": page,
            "path": self.breadcrumb(path),
            "chapter": self.chapter,
            "section": section,
            "entries": [Value::Object(table.clone())],
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_depth_one_and_two_rules_and_captioned_tables() {
        let data = vec![json!({
            "type": "section", "name": "Equipment", "page": 10, "entries": [
                "chapter intro",
                { "type": "entries", "name": "Weapons", "id": "a", "entries": [
                    "Weapon text.",
                    { "type": "entries", "name": "Mastery", "entries": [
                        "Mastery text.",
                        { "type": "entries", "name": "Too Deep", "entries": ["skipped"] } ] },
                    { "type": "table", "caption": "Weapon Costs", "colLabels": ["a"], "rows": [["1"]] } ] },
                { "type": "entries", "name": "Only Children", "entries": [ { "type": "entries", "name": "Nested", "entries": ["x"] } ] },
            ]
        })];
        let content = extract("TST", "Test Book", &data, true);
        let names: Vec<&str> = content.rules.iter().filter_map(|r| r["name"].as_str()).collect();
        assert_eq!(names, ["Weapons", "Mastery", "Nested"]);
        assert_eq!(content.rules[1]["path"], "Test Book › Equipment › Weapons");
        assert_eq!(content.rules[0]["page"], 10);
        // Where the book page finds them: the chapter, and the section the
        // rule is or sits in.
        let places: Vec<(&Value, &Value)> = content.rules.iter().map(|r| (&r["chapter"], &r["section"])).collect();
        assert_eq!(
            places,
            [(&json!(0), &json!(0)), (&json!(0), &json!(0)), (&json!(0), &json!(1))]
        );
        assert_eq!(content.tables[0]["section"], 0);
        assert_eq!(content.tables.len(), 1);
        assert_eq!(content.tables[0]["name"], "Weapon Costs");
    }
}
