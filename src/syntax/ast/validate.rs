//! Tree invariants.
//!
//! A violation is a bug in the builder, not a user-facing diagnostic. The
//! builder runs this under `debug_assert!`; tests and the CLI run it
//! explicitly so a malformed tree cannot be silently accepted.

use crate::syntax::SyntaxAnalysis;
use crate::tokenise::Token;

use super::clauses::kind_of;
use super::{Ast, NodeId, NodeKind, Violation};

/// Check every phase-0 invariant.
///
/// List completeness is deliberately not checked yet: phase 0 does not emit
/// list nodes. A list node that is present must at least point at a real list
/// and item.
pub fn validate(
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
    ast: &Ast,
) -> Result<(), Vec<Violation>> {
    let mut out = Vec::new();

    if ast.nodes.is_empty() {
        fail(&mut out, None, "root", "the arena is empty");
        return Err(out);
    }
    let Some(root) = ast.nodes.get(ast.root.index()) else {
        fail(
            &mut out,
            Some(ast.root),
            "root",
            "the root id is out of range",
        );
        return Err(out);
    };
    if root.kind != NodeKind::Document {
        fail(
            &mut out,
            Some(ast.root),
            "root",
            "the root is not a document",
        );
    }
    if root.parent.is_some() {
        fail(&mut out, Some(ast.root), "root", "the root has a parent");
    }

    check_links(ast, &mut out);
    check_cycles(ast, &mut out);
    check_words(tokens, analysis, ast, &mut out);
    check_segments(tokens, ast, &mut out);
    check_coverage(tokens, ast, &mut out);
    check_clauses(tokens, analysis, ast, &mut out);
    check_roles(analysis, ast, &mut out);
    check_lists(analysis, ast, &mut out);

    if out.is_empty() { Ok(()) } else { Err(out) }
}

fn fail(
    out: &mut Vec<Violation>,
    node: Option<NodeId>,
    rule: &'static str,
    detail: impl Into<String>,
) {
    out.push(Violation {
        node,
        rule,
        detail: detail.into(),
    });
}

fn check_links(ast: &Ast, out: &mut Vec<Violation>) {
    let mut parent_counts = vec![0usize; ast.nodes.len()];
    for (index, node) in ast.nodes.iter().enumerate() {
        let id = NodeId::new(index);
        for &child in &node.children {
            if child.index() >= ast.nodes.len() {
                fail(
                    out,
                    Some(id),
                    "link",
                    format!("child {} is out of range", child.index()),
                );
                continue;
            }
            parent_counts[child.index()] += 1;
            if ast.nodes[child.index()].parent != Some(id) {
                fail(
                    out,
                    Some(child),
                    "link",
                    format!("parent pointer does not name node {}", index),
                );
            }
        }
        if let Some(parent) = node.parent {
            if parent.index() >= ast.nodes.len() {
                fail(
                    out,
                    Some(id),
                    "link",
                    format!("parent {} is out of range", parent.index()),
                );
            } else if !ast.nodes[parent.index()].children.contains(&id) {
                fail(
                    out,
                    Some(id),
                    "link",
                    format!("parent {} does not list this child", parent.index()),
                );
            }
        }
    }
    for (index, count) in parent_counts.iter().enumerate() {
        if index == ast.root.index() {
            if *count != 0 {
                fail(out, Some(NodeId::new(index)), "root", "the root is a child");
            }
        } else if *count != 1 {
            fail(
                out,
                Some(NodeId::new(index)),
                "link",
                format!("node has {count} parents, expected 1"),
            );
        }
    }
}

fn check_cycles(ast: &Ast, out: &mut Vec<Violation>) {
    for start in 0..ast.nodes.len() {
        let mut current = Some(NodeId::new(start));
        let mut steps = 0;
        while let Some(id) = current {
            if id.index() >= ast.nodes.len() {
                break;
            }
            steps += 1;
            if steps > ast.nodes.len() {
                fail(
                    out,
                    Some(NodeId::new(start)),
                    "cycle",
                    "the parent chain does not terminate",
                );
                break;
            }
            current = ast.nodes[id.index()].parent;
        }
    }
}

fn check_words(tokens: &[Token], analysis: &SyntaxAnalysis, ast: &Ast, out: &mut Vec<Violation>) {
    let mut counts = vec![0usize; tokens.len()];
    for node in &ast.nodes {
        if node.kind != NodeKind::Word {
            continue;
        }
        let len = node.tokens.end.saturating_sub(node.tokens.start);
        if len != 1 {
            fail(out, None, "word", format!("a word node spans {len} tokens"));
            continue;
        }
        let index = node.tokens.start;
        let Some(token) = tokens.get(index) else {
            fail(
                out,
                None,
                "word",
                format!("word token {index} is out of range"),
            );
            continue;
        };
        if !token.is_word() {
            fail(out, None, "word", format!("token {index} is a separator"));
            continue;
        }
        counts[index] += 1;
        if node.head != Some(index) {
            fail(
                out,
                None,
                "word",
                format!("word {index} does not head itself"),
            );
        }
        let Some(syntax) = analysis.tokens.get(index) else {
            continue;
        };
        if node.governor != syntax.head {
            fail(
                out,
                None,
                "word",
                format!("word {index} governor differs from the flat model"),
            );
        }
        if node.clause != Some(syntax.clause) {
            fail(
                out,
                None,
                "word",
                format!("word {index} clause differs from the flat model"),
            );
        }
    }
    for (index, token) in tokens.iter().enumerate() {
        if token.is_word() && counts[index] != 1 {
            fail(
                out,
                None,
                "word",
                format!(
                    "word token {index} has {} leaves, expected 1",
                    counts[index]
                ),
            );
        } else if !token.is_word() && counts[index] != 0 {
            fail(out, None, "word", format!("separator {index} has a leaf"));
        }
    }
}

fn check_segments(tokens: &[Token], ast: &Ast, out: &mut Vec<Violation>) {
    for (index, node) in ast.nodes.iter().enumerate() {
        let id = NodeId::new(index);
        if node.tokens.start > node.tokens.end || node.tokens.end > tokens.len() {
            fail(
                out,
                Some(id),
                "segment",
                "the token envelope is out of range",
            );
            continue;
        }
        if node.segments.is_empty() && node.tokens.start < node.tokens.end {
            fail(out, Some(id), "segment", "a non-empty node has no segments");
        }
        for segment in &node.segments {
            if segment.start >= segment.end {
                fail(out, Some(id), "segment", "an empty segment");
            }
            if segment.start < node.tokens.start || segment.end > node.tokens.end {
                fail(
                    out,
                    Some(id),
                    "segment",
                    "a segment escapes the token envelope",
                );
            }
        }
        for pair in node.segments.windows(2) {
            if pair[0].end > pair[1].start {
                fail(out, Some(id), "segment", "segments overlap or are unsorted");
            }
        }
        for token_index in node.tokens.clone() {
            let Some(token) = tokens.get(token_index) else {
                fail(out, Some(id), "span", "the token envelope is out of range");
                break;
            };
            if node.span.start > token.start || node.span.end < token.end {
                fail(
                    out,
                    Some(id),
                    "span",
                    "the byte span does not cover a token",
                );
                break;
            }
        }
        for &child in &node.children {
            let Some(child_node) = ast.nodes.get(child.index()) else {
                continue;
            };
            if child_node.tokens.start < node.tokens.start
                || child_node.tokens.end > node.tokens.end
            {
                fail(
                    out,
                    Some(child),
                    "span",
                    "a child escapes its parent's token envelope",
                );
            }
        }
    }
}

fn check_coverage(tokens: &[Token], ast: &Ast, out: &mut Vec<Violation>) {
    let mut marks = vec![0usize; tokens.len()];
    for (index, node) in ast.nodes.iter().enumerate() {
        if node.tokens.start > node.tokens.end || node.tokens.end > tokens.len() {
            continue;
        }
        let mark = index + 1;
        let mut stack = vec![NodeId::new(index)];
        while let Some(id) = stack.pop() {
            let Some(current) = ast.nodes.get(id.index()) else {
                continue;
            };
            if current.kind == NodeKind::Word {
                if let Some(slot) = marks.get_mut(current.tokens.start) {
                    *slot = mark;
                }
            } else {
                stack.extend(current.children.iter().rev().copied());
            }
        }
        for token_index in node.tokens.clone() {
            if tokens.get(token_index).is_some_and(Token::is_word)
                && marks.get(token_index) != Some(&mark)
            {
                fail(
                    out,
                    Some(NodeId::new(index)),
                    "coverage",
                    format!("word token {token_index} is not covered by a descendant leaf"),
                );
                break;
            }
        }
    }
}

fn check_clauses(tokens: &[Token], analysis: &SyntaxAnalysis, ast: &Ast, out: &mut Vec<Violation>) {
    for (index, clause) in analysis.clauses.iter().enumerate() {
        let found: Vec<&super::Node> = ast
            .nodes
            .iter()
            .filter(|node| matches!(node.kind, NodeKind::Clause(_)) && node.clause == Some(index))
            .collect();
        if found.len() != 1 {
            fail(
                out,
                None,
                "clause",
                format!("flat clause {index} has {} nodes, expected 1", found.len()),
            );
            continue;
        }
        let node = found[0];
        if node.tokens != clause.tokens {
            fail(
                out,
                None,
                "clause",
                format!("clause {index} token range differs"),
            );
        }
        if node.span != (clause.start..clause.end) {
            fail(
                out,
                None,
                "clause",
                format!("clause {index} byte span differs"),
            );
        }
        let expected = NodeKind::Clause(kind_of(tokens, analysis, clause));
        if node.kind != expected {
            fail(
                out,
                None,
                "clause",
                format!("clause {index} has the wrong kind"),
            );
        }
    }

    for node in &ast.nodes {
        if !matches!(node.kind, NodeKind::Clause(_)) {
            continue;
        }
        let Some(index) = node.clause else {
            fail(
                out,
                None,
                "clause",
                "a clause node has no flat clause index",
            );
            continue;
        };
        let Some(clause) = analysis.clauses.get(index) else {
            fail(
                out,
                None,
                "clause",
                format!("clause index {index} is out of range"),
            );
            continue;
        };
        if node.tokens != clause.tokens {
            fail(
                out,
                None,
                "clause",
                format!("clause {index} node has the wrong tokens"),
            );
        }
    }
}

fn check_roles(analysis: &SyntaxAnalysis, ast: &Ast, out: &mut Vec<Violation>) {
    for node in &ast.nodes {
        match node.kind {
            NodeKind::Word | NodeKind::Phrase(_) => {
                let Some(head) = node.head else {
                    fail(out, None, "role", "a word or phrase node has no head");
                    continue;
                };
                let Some(syntax) = analysis.tokens.get(head) else {
                    fail(
                        out,
                        None,
                        "role",
                        format!("head token {head} is out of range"),
                    );
                    continue;
                };
                if node.role != Some(syntax.role) {
                    fail(
                        out,
                        None,
                        "role",
                        format!("token {head} role differs from the flat model"),
                    );
                }
                if node.confidence != syntax.confidence {
                    fail(
                        out,
                        None,
                        "role",
                        format!("token {head} confidence differs from the flat model"),
                    );
                }
            }
            NodeKind::List { .. } => {}
            _ => {
                if node.role.is_some() {
                    fail(out, None, "role", "this node kind must not carry a role");
                }
            }
        }
    }
}

fn check_lists(analysis: &SyntaxAnalysis, ast: &Ast, out: &mut Vec<Violation>) {
    for node in &ast.nodes {
        match node.kind {
            NodeKind::List { index } => {
                if index >= analysis.lists.len() {
                    fail(
                        out,
                        None,
                        "list",
                        format!("list index {index} is out of range"),
                    );
                }
            }
            NodeKind::ListItem { list, item } => {
                let Some(entry) = analysis.lists.get(list) else {
                    fail(
                        out,
                        None,
                        "list",
                        format!("list index {list} is out of range"),
                    );
                    continue;
                };
                if item >= entry.items.len() {
                    fail(
                        out,
                        None,
                        "list",
                        format!("item {item} is out of range for list {list}"),
                    );
                }
            }
            _ => {}
        }
    }
}
