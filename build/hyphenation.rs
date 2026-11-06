//! Compile the vendored hyphenation patterns into a static trie.
//!
//! `data/hyphenation/hyph-fi-x-school.tex` is kept verbatim as the upstream,
//! MIT-licensed artifact. Parsing it at build time means the shipped binary
//! has no parser and no `OnceLock`: `src/hyphenation/patterns.rs` includes a
//! generated `NODES` table and walks it directly.
//!
//! Invalid input fails the build. The checks are the ones the runtime parser
//! used to enforce: the `\patterns{...}` block must exist, tokens stay inside
//! the Finnish pattern alphabet, and a token with no score is dropped.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

#[derive(Default)]
struct Node {
    children: BTreeMap<char, usize>,
    values: Vec<u8>,
}

struct PatternSet {
    nodes: Vec<Node>,
    longest: usize,
}

impl PatternSet {
    fn parse(text: &str) -> Result<Self, String> {
        let mut set = Self {
            nodes: vec![Node::default()],
            longest: 0,
        };
        let mut in_patterns = false;
        let mut seen = 0usize;

        for (line_no, raw) in text.lines().enumerate() {
            let line = raw.split('%').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            if !in_patterns {
                if let Some(rest) = line.strip_prefix("\\patterns") {
                    let rest = rest.trim_start();
                    let Some(rest) = rest.strip_prefix('{') else {
                        return Err(format!(
                            "line {}: \\patterns is not followed by '{{'",
                            line_no + 1
                        ));
                    };
                    in_patterns = true;
                    if !rest.trim().is_empty()
                        && let Some(body) = rest.split('}').next()
                    {
                        seen += set.parse_tokens(body, line_no + 1)?;
                    }
                }
                continue;
            }
            if let Some(body) = line.split('}').next() {
                let body = body.trim();
                if body.is_empty() {
                    break;
                }
                seen += set.parse_tokens(body, line_no + 1)?;
                if line.contains('}') {
                    break;
                }
            }
        }

        if seen == 0 {
            return Err("no hyphenation patterns found".to_string());
        }
        Ok(set)
    }

    fn parse_tokens(&mut self, body: &str, line_no: usize) -> Result<usize, String> {
        let mut count = 0;
        for token in body.split_whitespace() {
            if token.is_empty() || token == "\\patterns{" {
                continue;
            }
            let token = token.trim_matches(|c: char| c == '{' || c == '}');
            if token.is_empty() {
                continue;
            }
            if !token
                .chars()
                .all(|c| matches!(c, 'a'..='z' | 'å' | 'ä' | 'ö' | 'š' | 'ž' | '.' | '0'..='9'))
            {
                return Err(format!(
                    "line {line_no}: pattern {token:?} contains a character outside the Finnish pattern alphabet"
                ));
            }
            if !token.chars().any(|c| c.is_ascii_digit()) {
                continue;
            }
            let mut letters = String::new();
            let mut values = vec![0u8];
            for c in token.chars() {
                if let Some(digit) = c.to_digit(10) {
                    *values.last_mut().expect("values is never empty") = digit as u8;
                } else {
                    letters.push(c);
                    values.push(0);
                }
            }
            if letters.is_empty() {
                continue;
            }
            self.longest = self.longest.max(letters.chars().count());
            self.insert(&letters, &values);
            count += 1;
        }
        Ok(count)
    }

    fn insert(&mut self, letters: &str, values: &[u8]) {
        let mut node = 0usize;
        for c in letters.chars() {
            let next = match self.nodes[node].children.get(&c) {
                Some(&next) => next,
                None => {
                    let next = self.nodes.len();
                    self.nodes.push(Node::default());
                    self.nodes[node].children.insert(c, next);
                    next
                }
            };
            node = next;
        }
        let slot = &mut self.nodes[node].values;
        if slot.len() < values.len() {
            slot.resize(values.len(), 0);
        }
        for (at, &value) in values.iter().enumerate() {
            slot[at] = slot[at].max(value);
        }
    }
}

/// Write the generated `NODES` table, read by `src/hyphenation/patterns.rs`.
pub(crate) fn emit(source: &Path, out: &Path) {
    let text = fs::read_to_string(source).unwrap_or_else(|e| {
        panic!(
            "build.rs: could not read {}: {e}; the vendored patterns are required",
            source.display()
        )
    });
    let set = PatternSet::parse(&text)
        .unwrap_or_else(|e| panic!("build.rs: {}: {e}", source.display()));

    let mut code = String::new();
    write!(
        code,
        "// @generated by build.rs from data/hyphenation/hyph-fi-x-school.tex.\n\
         // Do not edit; change the vendored source and rebuild.\n\n\
         pub(crate) const LONGEST: usize = {};\n\n\
         pub(crate) static NODES: &[Node] = &[\n",
        set.longest
    )
    .unwrap();
    for node in &set.nodes {
        code.push_str("    Node { children: &[");
        for (at, (c, child)) in node.children.iter().enumerate() {
            if at > 0 {
                code.push_str(", ");
            }
            write!(code, "({c:?}, {child})").unwrap();
        }
        code.push_str("], values: &[");
        for (at, value) in node.values.iter().enumerate() {
            if at > 0 {
                code.push_str(", ");
            }
            write!(code, "{value}").unwrap();
        }
        code.push_str("] },\n");
    }
    code.push_str("];\n");

    fs::write(out, code).unwrap_or_else(|e| panic!("{}: {e}", out.display()));
    eprintln!(
        "build.rs: hyphenation: {} trie nodes, longest pattern {} -> {}",
        set.nodes.len(),
        set.longest,
        out.display()
    );
}

/// Write the generated `EXCEPTIONS` table, read by
/// `src/hyphenation/exceptions.rs`.
pub(crate) fn emit_exceptions(source: &Path, out: &Path) {
    let text = fs::read_to_string(source).unwrap_or_else(|e| {
        panic!(
            "build.rs: could not read {}: {e}; the vendored exceptions are required",
            source.display()
        )
    });
    let mut rules: BTreeMap<String, (Vec<usize>, Vec<usize>)> = BTreeMap::new();
    for (line_no, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("word,") {
            continue;
        }
        let mut fields = line.split(',');
        let word = fields.next().unwrap_or("").trim().to_lowercase();
        let allow = fields.next().unwrap_or("").trim();
        let deny = fields.next().unwrap_or("").trim();
        if word.is_empty() {
            panic!("build.rs: {}:{}: missing word", source.display(), line_no + 1);
        }
        let entry = rules.entry(word).or_default();
        for offset in parse_offsets(allow, line_no + 1) {
            if !entry.0.contains(&offset) {
                entry.0.push(offset);
            }
        }
        for offset in parse_offsets(deny, line_no + 1) {
            if !entry.1.contains(&offset) {
                entry.1.push(offset);
            }
        }
    }

    let mut code = String::from(
        "// @generated by build.rs from data/hyphenation/exceptions.csv.\n\
         // Do not edit; change the vendored source and rebuild.\n\n\
         pub(crate) static EXCEPTIONS: &[Builtin] = &[\n",
    );
    for (word, (allow, deny)) in &rules {
        write!(code, "    Builtin {{ word: {word:?}, allow: &[").unwrap();
        for (at, offset) in allow.iter().enumerate() {
            if at > 0 {
                code.push_str(", ");
            }
            write!(code, "{offset}").unwrap();
        }
        code.push_str("], deny: &[");
        for (at, offset) in deny.iter().enumerate() {
            if at > 0 {
                code.push_str(", ");
            }
            write!(code, "{offset}").unwrap();
        }
        code.push_str("] },\n");
    }
    code.push_str("];\n");

    fs::write(out, code).unwrap_or_else(|e| panic!("{}: {e}", out.display()));
    eprintln!(
        "build.rs: hyphenation exceptions: {} rules -> {}",
        rules.len(),
        out.display()
    );
}

fn parse_offsets(field: &str, line_no: usize) -> Vec<usize> {
    if field.is_empty() {
        return Vec::new();
    }
    field
        .split(';')
        .map(|part| {
            part.trim().parse::<usize>().unwrap_or_else(|_| {
                panic!("build.rs: line {line_no}: {part:?} is not a character offset")
            })
        })
        .collect()
}
