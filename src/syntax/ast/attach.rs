//! Parenthetical and quote nodes.
//!
//! A parenthetical is a balanced bracket pair. A quote is a maximal run of
//! word tokens whose `TokenContext::quote_depth` is positive. Both are
//! containers: their children are the top-level existing nodes inside the run,
//! so a quoted clause or a list inside a parenthetical keeps its own shape.

use std::ops::Range;

use crate::sentence;
use crate::syntax::SyntaxAnalysis;
use crate::tokenise::Token;

use super::seed::Seed;
use super::{Builder, Node, NodeId, NodeKind};

pub(super) fn parentheticals(
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
    builder: &mut Builder,
    seed: &Seed,
) {
    let mut pairs = bracket_pairs(tokens);
    // Inner pairs first: an outer parenthetical then sees the inner one as an
    // ordinary child.
    pairs.sort_unstable_by_key(|pair| std::cmp::Reverse(pair.0));

    for (open, close) in pairs {
        let words: Vec<usize> = (open + 1..close)
            .filter(|&index| tokens.get(index).is_some_and(Token::is_word))
            .collect();
        let (Some(&first), Some(&last)) = (words.first(), words.last()) else {
            continue;
        };
        let token_range = first..last + 1;
        if seed.sentence_for(tokens[first].start) != seed.sentence_for(tokens[last].start) {
            continue;
        }
        let span = tokens[open].start..tokens[close].end;
        let clause = analysis.tokens.get(first).map(|syntax| syntax.clause);
        wrap(
            tokens,
            builder,
            seed,
            Node::parenthetical(span, token_range, clause),
            first,
        );
    }
}

pub(super) fn quotes(
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
    builder: &mut Builder,
    seed: &Seed,
) {
    let contexts = sentence::analyze(tokens);
    let mut run: Vec<usize> = Vec::new();
    let mut run_sentence: Option<usize> = None;
    for (index, token) in tokens.iter().enumerate() {
        if !token.is_word() {
            continue;
        }
        let context = contexts.get(index);
        let quoted = context.is_some_and(|context| context.quote_depth > 0);
        if quoted {
            let sentence = context.map(|context| context.sentence_index);
            if !run.is_empty() && sentence != run_sentence {
                wrap_run(tokens, analysis, builder, seed, &run);
                run.clear();
            }
            run_sentence = sentence;
            run.push(index);
        } else if !run.is_empty() {
            wrap_run(tokens, analysis, builder, seed, &run);
            run.clear();
            run_sentence = None;
        }
    }
    if !run.is_empty() {
        wrap_run(tokens, analysis, builder, seed, &run);
    }
}

fn wrap_run(
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
    builder: &mut Builder,
    seed: &Seed,
    run: &[usize],
) {
    let (Some(&first), Some(&last)) = (run.first(), run.last()) else {
        return;
    };
    let token_range = first..last + 1;
    let span = tokens[first].start..tokens[last].end;
    let clause = analysis.tokens.get(first).map(|syntax| syntax.clause);
    wrap(
        tokens,
        builder,
        seed,
        Node::quote(span, token_range, clause),
        first,
    );
}

fn wrap(tokens: &[Token], builder: &mut Builder, seed: &Seed, node: Node, first: usize) {
    let Some(owner) = owner(builder, seed, node.tokens.clone(), tokens, first) else {
        return;
    };
    let Some(candidates) = candidates(tokens, builder, owner, node.tokens.clone()) else {
        return;
    };
    let node_id = builder.push(node);
    builder.attach(owner, node_id);
    for candidate in candidates {
        builder.reparent(node_id, candidate);
    }
    builder.sort_children();
}

fn owner(
    builder: &Builder,
    seed: &Seed,
    token_range: Range<usize>,
    tokens: &[Token],
    first: usize,
) -> Option<NodeId> {
    let mut best: Option<(usize, usize, NodeId)> = None;
    for (index, node) in builder.nodes.iter().enumerate() {
        if !matches!(
            node.kind,
            NodeKind::Clause(_)
                | NodeKind::Phrase(_)
                | NodeKind::List { .. }
                | NodeKind::ListItem { .. }
                | NodeKind::Parenthetical
                | NodeKind::Quote
                | NodeKind::Sentence
                | NodeKind::Document
        ) {
            continue;
        }
        if !contains(&node.tokens, &token_range) {
            continue;
        }
        let width = node.span.end.saturating_sub(node.span.start);
        let rank = match node.kind {
            NodeKind::Sentence => 1,
            NodeKind::Document => 2,
            _ => 0,
        };
        if best.is_none_or(|(best_width, best_rank, _)| (width, rank) < (best_width, best_rank)) {
            best = Some((width, rank, NodeId::new(index)));
        }
    }
    best.map(|(_, _, node)| node).or_else(|| {
        tokens
            .get(first)
            .map(|token| seed.sentence_for(token.start))
    })
}

fn candidates(
    tokens: &[Token],
    builder: &Builder,
    owner: NodeId,
    token_range: Range<usize>,
) -> Option<Vec<NodeId>> {
    let mut found = Vec::new();
    for (index, node) in builder.nodes.iter().enumerate() {
        let id = NodeId::new(index);
        if id == owner {
            continue;
        }
        if !matches!(
            node.kind,
            NodeKind::Word
                | NodeKind::Phrase(_)
                | NodeKind::Clause(_)
                | NodeKind::List { .. }
                | NodeKind::ListItem { .. }
                | NodeKind::Parenthetical
                | NodeKind::Quote
        ) {
            continue;
        }
        if !contains(&token_range, &node.tokens) {
            continue;
        }
        let Some(parent) = node.parent else {
            continue;
        };
        if parent != owner && overlaps(&token_range, &builder.nodes[parent.index()].tokens) {
            continue;
        }
        if !is_descendant(builder, owner, id) {
            continue;
        }
        found.push((node.tokens.start, node.tokens.end, id));
    }
    found.sort_unstable_by_key(|(start, end, _)| (*start, *end));
    let nodes: Vec<NodeId> = found.into_iter().map(|(_, _, id)| id).collect();
    if nodes.is_empty() {
        return None;
    }

    let mut previous_end = 0usize;
    for &node in &nodes {
        let range = &builder.nodes[node.index()].tokens;
        if range.start < previous_end {
            return None;
        }
        previous_end = range.end;
    }
    for index in token_range {
        if !tokens.get(index).is_some_and(Token::is_word) {
            continue;
        }
        if !nodes
            .iter()
            .any(|node| builder.nodes[node.index()].tokens.contains(&index))
        {
            return None;
        }
    }
    Some(nodes)
}

fn bracket_pairs(tokens: &[Token]) -> Vec<(usize, usize)> {
    let mut stack: Vec<(usize, char)> = Vec::new();
    let mut pairs = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if token.is_word() {
            continue;
        }
        for character in token.text.chars() {
            match character {
                '(' | '[' | '{' => stack.push((index, character)),
                ')' | ']' | '}' => {
                    let Some((open, opener)) = stack.pop() else {
                        continue;
                    };
                    if matches!((opener, character), ('(', ')') | ('[', ']') | ('{', '}')) {
                        pairs.push((open, index));
                    }
                }
                _ => {}
            }
        }
    }
    pairs
}

fn contains(outer: &Range<usize>, inner: &Range<usize>) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

fn overlaps(left: &Range<usize>, right: &Range<usize>) -> bool {
    left.start < right.end && right.start < left.end
}

fn is_descendant(builder: &Builder, ancestor: NodeId, mut node: NodeId) -> bool {
    loop {
        if node == ancestor {
            return true;
        }
        let Some(parent) = builder.nodes[node.index()].parent else {
            return false;
        };
        node = parent;
    }
}
