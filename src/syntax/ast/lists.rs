//! Phase 3a: attach the flat list arena as `List` and `ListItem` nodes.
//!
//! This pass is deliberately conservative. It attaches a list only when the
//! flat parser called it `Nominal` and every item is a `Word` or `Phrase`
//! (never a nested list or a clause item). A mixed, clause or predicate list
//! stays flat; its words and phrases keep their existing shape.
//!
//! Ownership is the smallest existing `Clause` or `Phrase` that contains the
//! list. The item children are the top-level existing nodes inside the item;
//! a phrase keeps its own word leaves, so the item owns the phrase rather than
//! duplicating the leaf.

use std::ops::Range;

use crate::syntax::SyntaxAnalysis;
use crate::syntax_list::{List, ListItemKind, ListKind};
use crate::tokenise::Token;

use super::{Builder, Node, NodeId, NodeKind};

/// Whether phase 3a is allowed to represent this list.
pub(super) fn eligible(list: &List) -> bool {
    list.parent.is_none()
        && list.kind == ListKind::Nominal
        && list
            .items
            .iter()
            .all(|item| matches!(item.kind, ListItemKind::Word | ListItemKind::Phrase))
}

pub(super) fn attach(
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
    builder: &mut Builder,
    clause_nodes: &[NodeId],
) {
    let mut plans = Vec::new();
    for (index, list) in analysis.lists.iter().enumerate() {
        if !eligible(list) {
            continue;
        }
        let Some(owner) = owner(builder, clause_nodes, list) else {
            continue;
        };
        let mut items = Vec::new();
        let mut valid = true;
        for item in &list.items {
            match item_candidates(tokens, builder, owner, item.tokens.clone()) {
                Some(candidates) => items.push(candidates),
                None => {
                    valid = false;
                    break;
                }
            }
        }
        let conjunction = list.conjunction.and_then(|index| word_leaf(builder, index));
        if list.conjunction.is_some() && conjunction.is_none() {
            valid = false;
        }
        if !valid || !covers_list(tokens, builder, list, &items) {
            continue;
        }
        plans.push((index, owner, items, conjunction));
    }

    for (index, owner, items, conjunction) in plans {
        let list = &analysis.lists[index];
        let list_node = builder.push(Node::list(list, index));
        builder.attach(owner, list_node);
        for (item_index, item) in list.items.iter().enumerate() {
            let item_node = builder.push(Node::list_item(list, item, index, item_index));
            builder.attach(list_node, item_node);
            for candidate in &items[item_index] {
                builder.reparent(item_node, *candidate);
            }
        }
        if let Some(conjunction) = conjunction {
            builder.reparent(list_node, conjunction);
        }
    }

    builder.sort_children();
}

fn owner(builder: &Builder, clause_nodes: &[NodeId], list: &List) -> Option<NodeId> {
    let mut best: Option<(usize, NodeId)> = None;
    for (index, node) in builder.nodes.iter().enumerate() {
        if !matches!(node.kind, NodeKind::Clause(_) | NodeKind::Phrase(_)) {
            continue;
        }
        if !contains(&node.tokens, &list.tokens) {
            continue;
        }
        let width = node.tokens.end.saturating_sub(node.tokens.start);
        if best.is_none_or(|(best_width, _)| width < best_width) {
            best = Some((width, NodeId::new(index)));
        }
    }
    best.map(|(_, node)| node)
        .or_else(|| clause_nodes.get(list.clause).copied())
}

fn item_candidates(
    tokens: &[Token],
    builder: &Builder,
    owner: NodeId,
    item_tokens: Range<usize>,
) -> Option<Vec<NodeId>> {
    let mut found = Vec::new();
    for (index, node) in builder.nodes.iter().enumerate() {
        let id = NodeId::new(index);
        if id == owner {
            continue;
        }
        if !matches!(
            node.kind,
            NodeKind::Word | NodeKind::Phrase(_) | NodeKind::Clause(_)
        ) {
            continue;
        }
        if !contains(&item_tokens, &node.tokens) {
            continue;
        }
        let Some(parent) = node.parent else {
            continue;
        };
        if parent != owner && contains(&item_tokens, &builder.nodes[parent.index()].tokens) {
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

    // The item must be covered by its own children, and the children must not
    // overlap. A node that spans two items is not a valid child of either.
    let mut previous_end = 0usize;
    for &node in &nodes {
        let range = &builder.nodes[node.index()].tokens;
        if range.start < previous_end {
            return None;
        }
        previous_end = range.end;
    }
    for index in item_tokens {
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

fn covers_list(tokens: &[Token], builder: &Builder, list: &List, items: &[Vec<NodeId>]) -> bool {
    let mut skip = vec![false; tokens.len()];
    for &separator in &list.separators {
        if let Some(slot) = skip.get_mut(separator) {
            *slot = true;
        }
    }
    for index in list.conjunction.into_iter().chain(list.introducer) {
        if let Some(slot) = skip.get_mut(index) {
            *slot = true;
        }
    }
    for index in list.tokens.clone() {
        if !tokens.get(index).is_some_and(Token::is_word)
            || skip.get(index).copied().unwrap_or(false)
        {
            continue;
        }
        let covered = list.items.iter().enumerate().any(|(item_index, item)| {
            item.tokens.contains(&index)
                && items[item_index]
                    .iter()
                    .any(|node| builder.nodes[node.index()].tokens.contains(&index))
        });
        if !covered {
            return false;
        }
    }
    true
}

fn contains(outer: &Range<usize>, inner: &Range<usize>) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

fn word_leaf(builder: &Builder, token_index: usize) -> Option<NodeId> {
    builder
        .nodes
        .iter()
        .enumerate()
        .find(|(_, node)| node.kind == NodeKind::Word && node.tokens.start == token_index)
        .map(|(index, _)| NodeId::new(index))
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
