//! Parser for 5etools' inline `{@tag arg1|arg2|...}` markup embedded in
//! entry strings. Hand-written rather than regex-based because tags can nest (an arg can
//! itself contain another `{@tag}`), which needs brace depth tracking
//! rather than pattern matching.

#[derive(Clone, Debug, PartialEq)]
pub enum InlineNode {
    Text(String),
    /// `name` is the tag name as written after the `@`.
    Tag {
        name: String,
        args: Vec<String>,
    },
}

/// Whether `chars[i]` starts a tag: a `{` immediately followed by `@`.
fn is_tag_start(chars: &[char], i: usize) -> bool {
    chars.get(i) == Some(&'{') && chars.get(i + 1) == Some(&'@')
}

/// Parses a string into a flat sequence of plain text and tag nodes. A
/// tag's arguments are kept as raw strings (not recursively parsed) -
/// rendering re-parses an argument with [`parse_inline`] only when that
/// tag's semantics call for rendering it as rich text (e.g. `{@b ...}`).
pub fn parse_inline(input: &str) -> Vec<InlineNode> {
    let chars: Vec<char> = input.chars().collect();
    let mut nodes = Vec::new();
    let mut plain = String::new();
    let mut i = 0;
    while i < chars.len() {
        if is_tag_start(&chars, i) {
            if !plain.is_empty() {
                nodes.push(InlineNode::Text(std::mem::take(&mut plain)));
            }
            let (tag, next_i) = parse_tag(&chars, i);
            nodes.push(tag);
            i = next_i;
        } else {
            plain.push(chars[i]);
            i += 1;
        }
    }
    if !plain.is_empty() {
        nodes.push(InlineNode::Text(plain));
    }
    nodes
}

/// `is_tag_start(chars, start)` holds. Returns the parsed tag and the
/// index just past its closing `}` (or end of input if unterminated).
fn parse_tag(chars: &[char], start: usize) -> (InlineNode, usize) {
    let content_start = start + 2;
    let mut depth = 1;
    let mut i = content_start;
    while i < chars.len() {
        if is_tag_start(chars, i) {
            depth += 1;
            i += 2;
            continue;
        }
        if chars[i] == '}' {
            depth -= 1;
            if depth == 0 {
                break;
            }
        }
        i += 1;
    }
    let content_end = i;
    let content: String = chars[content_start..content_end].iter().collect();
    let next_i = if content_end < chars.len() {
        content_end + 1
    } else {
        chars.len()
    };

    let (name, rest) = match content.find(char::is_whitespace) {
        Some(pos) => (content[..pos].to_string(), content[pos..].trim_start().to_string()),
        None => (content, String::new()),
    };
    let args = split_top_level(&rest);
    (InlineNode::Tag { name, args }, next_i)
}

/// Splits on top-level `|` characters, ignoring any that fall inside a
/// nested `{@...}` span.
fn split_top_level(s: &str) -> Vec<String> {
    if s.is_empty() {
        return Vec::new();
    }
    let chars: Vec<char> = s.chars().collect();
    let mut parts = Vec::new();
    let mut depth = 0;
    let mut current = String::new();
    let mut i = 0;
    while i < chars.len() {
        if is_tag_start(&chars, i) {
            depth += 1;
            current.push_str("{@");
            i += 2;
            continue;
        }
        if chars[i] == '}' && depth > 0 {
            depth -= 1;
            current.push('}');
            i += 1;
            continue;
        }
        if chars[i] == '|' && depth == 0 {
            parts.push(std::mem::take(&mut current));
            i += 1;
            continue;
        }
        current.push(chars[i]);
        i += 1;
    }
    parts.push(current);
    parts
}

/// The text of `input` without markup: each tag is replaced by its first argument.
pub fn strip_tags(input: &str) -> String {
    parse_inline(input)
        .into_iter()
        .map(|node| match node {
            InlineNode::Text(t) => t,
            InlineNode::Tag { args, .. } => args.into_iter().next().map(|a| strip_tags(&a)).unwrap_or_default(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag(name: &str, args: &[&str]) -> InlineNode {
        InlineNode::Tag {
            name: name.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn plain_text_has_no_tags() {
        assert_eq!(
            parse_inline("just some text"),
            vec![InlineNode::Text("just some text".to_string())]
        );
    }

    #[test]
    fn single_tag_no_args() {
        assert_eq!(parse_inline("{@h}"), vec![tag("h", &[])]);
    }

    #[test]
    fn tag_with_single_arg() {
        assert_eq!(parse_inline("{@dice 1d6}"), vec![tag("dice", &["1d6"])]);
    }

    #[test]
    fn tag_with_multiple_args() {
        assert_eq!(
            parse_inline("{@creature Goblin|MM|goblins}"),
            vec![tag("creature", &["Goblin", "MM", "goblins"])]
        );
    }

    #[test]
    fn text_around_tags() {
        assert_eq!(
            parse_inline("You take {@damage 1d4} damage."),
            vec![
                InlineNode::Text("You take ".to_string()),
                tag("damage", &["1d4"]),
                InlineNode::Text(" damage.".to_string()),
            ]
        );
    }

    #[test]
    fn adjacent_tags() {
        assert_eq!(
            parse_inline("{@atk mw}{@h}4 damage"),
            vec![
                tag("atk", &["mw"]),
                tag("h", &[]),
                InlineNode::Text("4 damage".to_string()),
            ]
        );
    }

    #[test]
    fn nested_tag_inside_arg_stays_within_that_arg() {
        // The whole nested tag should be captured as part of the outer
        // tag's single argument, not split on the inner tag's own args.
        assert_eq!(
            parse_inline("{@bold deals {@damage 1d6} damage}"),
            vec![tag("bold", &["deals {@damage 1d6} damage"])]
        );
    }

    #[test]
    fn nested_tag_does_not_confuse_pipe_splitting() {
        assert_eq!(
            parse_inline("{@b text with {@i a|b} inside|second arg}"),
            vec![tag("b", &["text with {@i a|b} inside", "second arg"])]
        );
    }

    #[test]
    fn unterminated_tag_consumes_to_end_of_input() {
        assert_eq!(parse_inline("{@dice 1d6"), vec![tag("dice", &["1d6"])]);
    }

    #[test]
    fn empty_input_yields_no_nodes() {
        assert_eq!(parse_inline(""), Vec::new());
    }
}
