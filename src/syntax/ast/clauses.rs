//! Phase 0: one `Clause` node per flat clause.

use crate::syntax::{Clause, SyntaxAnalysis};
use crate::tags::{Mood, PartOfSpeech};
use crate::tokenise::Token;

use super::seed::Seed;
use super::{Builder, ClauseKind, Node, NodeId};

/// The structural kind of a flat clause.
///
/// The relative/subordinate refinement is deliberate: [`crate::syntax::ClauseType`]
/// says `Subordinate` for both, while a tree consumer wants to know whether the
/// clause is a relative.
pub(super) fn kind_of(tokens: &[Token], analysis: &SyntaxAnalysis, clause: &Clause) -> ClauseKind {
    if clause.parent.is_some() {
        return ClauseKind::Relative;
    }

    let first = clause
        .tokens
        .clone()
        .find(|&index| tokens.get(index).is_some_and(Token::is_word));
    if let Some(first) = first {
        if crate::syntax::starts_relative_clause(first, tokens, &analysis.tokens) {
            return ClauseKind::Relative;
        }
        if analysis
            .tokens
            .get(first)
            .is_some_and(|syntax| crate::syntax::is_subordinator(&syntax.key))
        {
            return ClauseKind::Subordinate;
        }
    }

    if let Some(predicate) = clause.predicate {
        let mut saw_verb = false;
        let mut all_infinitive = true;
        for reading in &analysis.tokens[predicate].readings {
            if matches!(
                reading.part_of_speech,
                Some(PartOfSpeech::Verb | PartOfSpeech::NegativeVerb)
            ) {
                saw_verb = true;
                if reading.mood != Some(Mood::Infinitive) {
                    all_infinitive = false;
                }
            }
        }
        if saw_verb && all_infinitive {
            return ClauseKind::Infinitive;
        }
    }

    ClauseKind::Main
}

pub(super) fn group(
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
    builder: &mut Builder,
    seed: &Seed,
) -> Vec<NodeId> {
    let mut clause_nodes = Vec::with_capacity(analysis.clauses.len());
    for (index, clause) in analysis.clauses.iter().enumerate() {
        let kind = kind_of(tokens, analysis, clause);
        let node = builder.push(Node::clause(clause, index, kind));
        clause_nodes.push(node);
    }

    // A relative clause is a child of the preceding host segment; every other
    // clause belongs to the sentence that contains its first word.
    //
    // Phase 0 keeps every clause under its sentence. The flat `parent` link is
    // still what makes `kind_of` call a relative a relative; phase 2 replaces
    // the relative + host pair with a nested host, which is the only place a
    // clause stops being a direct sentence child.
    for (index, clause) in analysis.clauses.iter().enumerate() {
        let parent = seed.sentence_for(clause.start);
        builder.attach(parent, clause_nodes[index]);
    }

    // Reparent the word leaves from their sentence to their clause. A token
    // whose clause index does not actually contain it stays with the sentence;
    // the validator will catch a bad flat model, not a bad tree.
    for (index, token) in tokens.iter().enumerate() {
        if !token.is_word() {
            continue;
        }
        let Some(leaf) = seed.words.get(index).copied().flatten() else {
            continue;
        };
        let parent = analysis
            .tokens
            .get(index)
            .map(|syntax| syntax.clause)
            .filter(|&clause| {
                analysis
                    .clauses
                    .get(clause)
                    .is_some_and(|clause| clause.tokens.contains(&index))
            })
            .and_then(|clause| clause_nodes.get(clause).copied())
            .unwrap_or_else(|| seed.sentence_for(token.start));
        builder.reparent(parent, leaf);
    }

    builder.sort_children();
    clause_nodes
}
