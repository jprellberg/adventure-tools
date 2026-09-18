//! Resolves 5etools' `_copy`/`_mod` data-inheritance convention: many
//! entries - about a quarter of the bestiary - are defined only
//! as a diff against another entry (`_copy: { name, source, _mod? }`)
//! rather than carrying their own full data. This runs once per kind, on
//! the raw JSON tree, before it's deserialized into typed structs.

use std::collections::{HashMap, HashSet};

use serde_json::{Map, Value};

/// Identifies an entity - or a `_copy` target, which names the same fields -
/// so a copy can find its base. Most kinds are unique by name and source.
pub type KeyFn = fn(&Value) -> Option<String>;

pub fn name_source_key(v: &Value) -> Option<String> {
    Some(format!("{}|{}", v.get("name")?.as_str()?, v.get("source")?.as_str()?).to_ascii_lowercase())
}

/// Like [`name_source_key`] plus the given fields, for kinds whose name and
/// source alone repeat (subclasses across their classes, features across
/// levels).
pub fn key_with_fields<const N: usize>(v: &Value, fields: [&str; N]) -> Option<String> {
    let mut key = name_source_key(v)?;
    for field in fields {
        key.push('|');
        match v.get(field)? {
            Value::String(s) => key.push_str(&s.to_ascii_lowercase()),
            other => key.push_str(&other.to_string()),
        }
    }
    Some(key)
}

pub fn subclass_key(v: &Value) -> Option<String> {
    key_with_fields(v, ["className", "classSource"])
}

pub fn class_feature_key(v: &Value) -> Option<String> {
    key_with_fields(v, ["className", "classSource", "level"])
}

pub fn subclass_feature_key(v: &Value) -> Option<String> {
    key_with_fields(
        v,
        [
            "className",
            "classSource",
            "subclassShortName",
            "subclassSource",
            "level",
        ],
    )
}

/// Resolves every `_copy` in `items` against the other entries in the same
/// slice (copies may reference a base from a different sourcebook, which is
/// fine since every source is loaded together per kind).
/// Entries without `_copy` pass through unchanged; a `_copy` whose
/// target can't be found, or that forms a cycle, is left as-is rather than
/// failing the whole load.
pub fn resolve_copies(items: Vec<Value>) -> Vec<Value> {
    resolve_copies_by(items, name_source_key)
}

pub fn resolve_copies_by(items: Vec<Value>, key: KeyFn) -> Vec<Value> {
    let index: HashMap<String, usize> = items
        .iter()
        .enumerate()
        .filter_map(|(i, v)| key(v).map(|k| (k, i)))
        .collect();

    // Only entries with a `_copy` are rebuilt; every other entry is moved
    // through untouched, and is cloned only when a copy takes it as its base.
    let mut resolved: HashMap<usize, Value> = HashMap::new();
    let mut resolving: HashSet<usize> = HashSet::new();
    for i in 0..items.len() {
        if items[i].get("_copy").is_some() {
            resolve_one(i, &items, &index, key, &mut resolved, &mut resolving);
        }
    }
    items
        .into_iter()
        .enumerate()
        .map(|(i, item)| resolved.remove(&i).unwrap_or(item))
        .collect()
}

/// The resolved form of `items[i]`: a copy is resolved into `resolved`
/// (which also memoizes it), anything else is the item itself.
fn resolve_one(
    i: usize,
    items: &[Value],
    index: &HashMap<String, usize>,
    key: KeyFn,
    resolved: &mut HashMap<usize, Value>,
    resolving: &mut HashSet<usize>,
) -> Value {
    if let Some(v) = resolved.get(&i) {
        return v.clone();
    }
    let raw = &items[i];
    let Some(copy_spec) = raw.get("_copy") else {
        return raw.clone();
    };

    let base_idx = key(copy_spec).and_then(|k| index.get(&k));

    let merged = match base_idx {
        Some(&base_idx) if !resolving.contains(&base_idx) && base_idx != i => {
            resolving.insert(i);
            let base = resolve_one(base_idx, items, index, key, resolved, resolving);
            resolving.remove(&i);

            let mut merged = match base {
                Value::Object(map) => map,
                _ => Map::new(),
            };
            if let Some(own) = raw.as_object() {
                for (k, v) in own {
                    if k != "_copy" {
                        merged.insert(k.clone(), v.clone());
                    }
                }
            }
            if let Some(m) = copy_spec.get("_mod") {
                apply_mod(&mut merged, m);
            }
            Value::Object(merged)
        }
        // Target missing or a cycle: can't merge, so keep the entry as-is
        // (minus the now-meaningless `_copy` marker).
        _ => {
            let mut own = raw.as_object().cloned().unwrap_or_default();
            own.remove("_copy");
            Value::Object(own)
        }
    };

    resolved.insert(i, merged.clone());
    merged
}

fn apply_mod(entity: &mut Map<String, Value>, mod_spec: &Value) {
    let Some(mod_obj) = mod_spec.as_object() else { return };
    for (field, op_value) in mod_obj {
        // "_" targets whole-entity concerns this app doesn't model or
        // render (spellcasting lists, skill bonuses) - nothing to apply.
        if field == "_" {
            continue;
        }
        let ops: Vec<&Value> = match op_value {
            Value::Array(a) => a.iter().collect(),
            other => vec![other],
        };
        for op in ops {
            apply_single_mod(entity, field, op);
        }
    }
}

fn apply_single_mod(entity: &mut Map<String, Value>, field: &str, op: &Value) {
    match op {
        Value::String(s) if s == "remove" => {
            entity.remove(field);
        }
        Value::Object(op_map) => {
            match op_map.get("mode").and_then(Value::as_str).unwrap_or("") {
                "setProp" => {
                    let prop = op_map.get("prop").and_then(Value::as_str).unwrap_or(field);
                    entity.insert(prop.to_string(), op_map.get("value").cloned().unwrap_or(Value::Null));
                }
                "replaceTxt" => {
                    let replace = op_map.get("replace").and_then(Value::as_str).unwrap_or("");
                    let with = op_map.get("with").and_then(Value::as_str).unwrap_or("");
                    let ci = op_map
                        .get("flags")
                        .and_then(Value::as_str)
                        .is_some_and(|f| f.contains('i'));
                    if field == "*" {
                        for v in entity.values_mut() {
                            replace_text(v, replace, with, ci);
                        }
                    } else if let Some(target) = entity.get_mut(field) {
                        replace_text(target, replace, with, ci);
                    }
                }
                mode @ ("appendArr"
                | "prependArr"
                | "insertArr"
                | "removeArr"
                | "replaceArr"
                | "appendIfNotExistsArr") => array_op(entity, field, mode, op_map),
                // addSpells/removeSpells/replaceSpells/addSkills and similar
                // target nested structures (spellcasting, skills) this app
                // doesn't render, so applying them wouldn't change output.
                _ => {}
            }
        }
        _ => {}
    }
}

fn replace_text(value: &mut Value, from: &str, to: &str, case_insensitive: bool) {
    match value {
        Value::String(s) => {
            *s = if case_insensitive {
                replace_ci(s, from, to)
            } else {
                s.replace(from, to)
            };
        }
        Value::Array(items) => items
            .iter_mut()
            .for_each(|v| replace_text(v, from, to, case_insensitive)),
        Value::Object(map) => map
            .values_mut()
            .for_each(|v| replace_text(v, from, to, case_insensitive)),
        _ => {}
    }
}

fn replace_ci(haystack: &str, from: &str, to: &str) -> String {
    if from.is_empty() {
        return haystack.to_string();
    }
    let lower_hay = haystack.to_lowercase();
    let lower_from = from.to_lowercase();
    let mut out = String::with_capacity(haystack.len());
    let mut rest = haystack;
    let mut rest_lower = lower_hay.as_str();
    while let Some(pos) = rest_lower.find(&lower_from) {
        out.push_str(&rest[..pos]);
        out.push_str(to);
        rest = &rest[pos + from.len()..];
        rest_lower = &rest_lower[pos + from.len()..];
    }
    out.push_str(rest);
    out
}

/// Whether an array item (a plain string like a language, or a
/// `{name, entries}`-shaped object like a trait) matches `needle` by value
/// or by its `name`.
fn item_matches(item: &Value, needle: &str) -> bool {
    match item {
        Value::String(s) => s.eq_ignore_ascii_case(needle),
        Value::Object(m) => m
            .get("name")
            .and_then(Value::as_str)
            .is_some_and(|n| n.eq_ignore_ascii_case(needle)),
        _ => false,
    }
}

/// `items`/`value` in a mod op can be a single value or an array of them.
fn as_list(v: Option<&Value>) -> Vec<Value> {
    match v {
        Some(Value::Array(a)) => a.clone(),
        Some(other) => vec![other.clone()],
        None => Vec::new(),
    }
}

/// `names` in a mod op can be a single string or an array of strings.
fn as_str_list(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::Array(a)) => a.iter().filter_map(|x| x.as_str().map(String::from)).collect(),
        Some(Value::String(s)) => vec![s.clone()],
        _ => Vec::new(),
    }
}

fn array_op(entity: &mut Map<String, Value>, field: &str, mode: &str, op_map: &Map<String, Value>) {
    let arr = match entity
        .entry(field.to_string())
        .or_insert_with(|| Value::Array(Vec::new()))
    {
        Value::Array(a) => a,
        _ => return,
    };

    match mode {
        "appendArr" => arr.extend(as_list(op_map.get("items"))),
        "prependArr" => {
            let mut items = as_list(op_map.get("items"));
            items.extend(std::mem::take(arr));
            *arr = items;
        }
        "insertArr" => {
            let index = op_map.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
            let items = as_list(op_map.get("items"));
            let at = index.min(arr.len());
            arr.splice(at..at, items);
        }
        "removeArr" => {
            let names = as_str_list(op_map.get("names").or_else(|| op_map.get("items")));
            arr.retain(|item| !names.iter().any(|n| item_matches(item, n)));
        }
        "replaceArr" => {
            let Some(target) = op_map.get("replace").and_then(Value::as_str) else {
                return;
            };
            let items = as_list(op_map.get("items"));
            if let Some(pos) = arr.iter().position(|item| item_matches(item, target)) {
                arr.splice(pos..=pos, items);
            }
        }
        "appendIfNotExistsArr" => {
            for item in as_list(op_map.get("items")) {
                let exists = arr.iter().any(|existing| existing == &item);
                if !exists {
                    arr.push(item);
                }
            }
        }
        _ => {}
    }
}
