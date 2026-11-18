//! Phase 0: the document, sentence and word-leaf skeleton.

use std::ops::Range;

use crate::syntax::SyntaxAnalysis;
use crate::tokenise::Token;

use super::{Builder, Node, NodeId};

/// The skeleton a later pass fills.
pub(super) struct Seed {
    pub(super) root: NodeId,
    /// Byte span and node id for each logical sentence.
    pub(super) sentences: Vec<(Range<usize>, NodeId)>,
    /// One leaf per token index; only word tokens have one.
    pub(super) words: Vec<Option<NodeId>>,
}

impl Seed {
    /// The logical sentence that contains `byte`, or the document root.
    pub(super) fn sentence_for(&self, byte: usize) -> NodeId {
        self.sentences
            .iter()
            .find(|(span, _)| span.start <= byte && byte < span.end)
            .map_or(self.root, |(_, id)| *id)
    }
}

pub(super) fn build(tokens: &[Token], analysis: &SyntaxAnalysis, builder: &mut Builder) -> Seed {
    let words: Vec<usize> = tokens
        .iter()
        .enumerate()
        .filter(|(_, token)| token.is_word())
        .map(|(index, _)| index)
        .collect();

    let word_tokens = match (words.first(), words.last()) {
        (Some(&first), Some(&last)) => first..last + 1,
        _ => 0..0,
    };
    let document_end = tokens.last().map_or(0, |token| token.end);
    let root = builder.push(Node::document(word_tokens, document_end));

    // The sentence layer splits at a bracket for capitalisation. A syntax
    // clause may span those pieces, so the tree's sentence is the union of the
    // raw spans that one clause touches. This keeps a bracket parenthetical
    // inside one tree sentence while the case layer keeps its raw spans.
    let mut raw: Vec<(Range<usize>, Vec<usize>)> = Vec::new();
    for span in &analysis.sentences {
        let inside: Vec<usize> = words
            .iter()
            .copied()
            .filter(|&index| {
                let token = &tokens[index];
                token.start >= span.start && token.end <= span.end
            })
            .collect();
        if !inside.is_empty() {
            raw.push((span.start..span.end, inside));
        }
    }

    let mut parent: Vec<usize> = (0..raw.len()).collect();
    for clause in &analysis.clauses {
        let overlapping: Vec<usize> = raw
            .iter()
            .enumerate()
            .filter(|(_, (span, _))| clause.start < span.end && span.start < clause.end)
            .map(|(index, _)| index)
            .collect();
        if let Some(&first) = overlapping.first() {
            for &other in &overlapping[1..] {
                union(&mut parent, first, other);
            }
        }
    }

    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut root_group: Vec<Option<usize>> = vec![None; raw.len()];
    for index in 0..raw.len() {
        let root = find(&mut parent, index);
        let group = match root_group[root] {
            Some(group) => group,
            None => {
                let group = groups.len();
                groups.push(Vec::new());
                root_group[root] = Some(group);
                group
            }
        };
        groups[group].push(index);
    }

    let mut sentences = Vec::new();
    for group in &groups {
        let start = group
            .iter()
            .map(|&index| raw[index].0.start)
            .min()
            .expect("a sentence group is not empty");
        let end = group
            .iter()
            .map(|&index| raw[index].0.end)
            .max()
            .expect("a sentence group is not empty");
        let first = group
            .iter()
            .flat_map(|&index| raw[index].1.iter())
            .min()
            .expect("a sentence group has a word");
        let last = group
            .iter()
            .flat_map(|&index| raw[index].1.iter())
            .max()
            .expect("a sentence group has a word");
        let node = builder.push(Node::sentence(start..end, *first..*last + 1));
        builder.attach(root, node);
        sentences.push((start..end, node));
    }

    let mut leaves = vec![None; tokens.len()];
    for &index in &words {
        let token = &tokens[index];
        let syntax = analysis.tokens.get(index).cloned().unwrap_or_default();
        let leaf = builder.push(Node::word(token, index, &syntax));
        leaves[index] = Some(leaf);
        let parent = sentences
            .iter()
            .find(|(span, _)| span.start <= token.start && token.start < span.end)
            .map_or(root, |(_, id)| *id);
        builder.attach(parent, leaf);
    }

    Seed {
        root,
        sentences,
        words: leaves,
    }
}

fn find(parent: &mut [usize], mut index: usize) -> usize {
    while parent[index] != index {
        parent[index] = parent[parent[index]];
        index = parent[index];
    }
    index
}

fn union(parent: &mut [usize], left: usize, right: usize) {
    let left = find(parent, left);
    let right = find(parent, right);
    if left != right {
        parent[right] = left;
    }
}
