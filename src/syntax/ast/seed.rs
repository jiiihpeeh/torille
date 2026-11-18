//! Phase 0: the document, sentence and word-leaf skeleton.

use std::ops::Range;

use crate::syntax::SyntaxAnalysis;
use crate::tokenise::Token;

use super::{Builder, Node, NodeId};

/// The skeleton a later pass fills.
pub(super) struct Seed {
    pub(super) root: NodeId,
    /// Byte span and node id for each sentence that contains a word.
    pub(super) sentences: Vec<(Range<usize>, NodeId)>,
    /// One leaf per token index; only word tokens have one.
    pub(super) words: Vec<Option<NodeId>>,
}

impl Seed {
    /// The sentence node that contains `byte`, or the document root.
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

    let mut sentences = Vec::new();
    for span in &analysis.sentences {
        let inside: Vec<usize> = words
            .iter()
            .copied()
            .filter(|&index| {
                let token = &tokens[index];
                token.start >= span.start && token.end <= span.end
            })
            .collect();
        let (Some(&first), Some(&last)) = (inside.first(), inside.last()) else {
            continue;
        };
        let node = builder.push(Node::sentence(span, first..last + 1));
        builder.attach(root, node);
        sentences.push((span.start..span.end, node));
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
