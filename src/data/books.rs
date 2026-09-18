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
    out: BookContent,
}

/// Rules and tables of one book (`data` is the file's chapter list).
pub fn extract(source: &str, book_name: &str, data: &[Value], with_rules: bool) -> BookContent {
    let mut walker = Walker {
        source,
        book_name,
        with_rules,
        out: BookContent::default(),
    };
    for chapter in data {
        walker.walk(chapter, 0, &mut Vec::new(), None);
    }
    walker.out
}

fn is_block(kind: &str) -> bool {
    matches!(kind, "entries" | "section")
}

impl Walker<'_> {
    /// `path` holds the names of the enclosing named blocks; `page` the
    /// nearest page number seen on the way down.
    fn walk(&mut self, node: &Value, depth: usize, path: &mut Vec<String>, page: Option<i64>) {
        match node {
            Value::Array(items) => items.iter().for_each(|n| self.walk(n, depth, path, page)),
            Value::Object(map) => {
                let kind = map.get("type").and_then(Value::as_str).unwrap_or("");
                let page = map.get("page").and_then(Value::as_i64).or(page);
                if kind == "table" {
                    self.table(map, path, page);
                }
                let named = is_block(kind)
                    .then(|| map.get("name").and_then(Value::as_str))
                    .flatten();
                if let Some(name) = named.filter(|_| self.with_rules && RULE_DEPTHS.contains(&depth)) {
                    self.rule(map, name, path, page);
                }
                let pushed = named.map(strip_tags);
                let child_depth = if is_block(kind) { depth + 1 } else { depth };
                if let Some(name) = &pushed {
                    path.push(name.clone());
                }
                for value in map.values() {
                    self.walk(value, child_depth, path, page);
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
    fn rule(&mut self, block: &Map<String, Value>, name: &str, path: &[String], page: Option<i64>) {
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
            "entries": block["entries"],
        }));
    }

    fn table(&mut self, table: &Map<String, Value>, path: &[String], page: Option<i64>) {
        let Some(caption) = table.get("caption").and_then(Value::as_str) else {
            return;
        };
        self.out.tables.push(json!({
            "name": strip_tags(caption),
            "source": self.source,
            "page": page,
            "path": self.breadcrumb(path),
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
        assert_eq!(content.tables.len(), 1);
        assert_eq!(content.tables[0]["name"], "Weapon Costs");
    }
}
