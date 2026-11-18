//! Clause grouping: one node per flat clause, plus the multi-segment host.
//!
//! A relative clause in the middle of its host (`Mies, joka tuli, söi`) is a
//! flat pair of segments in `SyntaxAnalysis`: the host is clause `A`, the
//! relative is `R`, and the continuation is `B`. `group` may replace `A` and
//! `B` with one host node whose `segments` are `[A.tokens, B.tokens]` and
//! whose child order is `A`, `R`, `B`. The merge is deliberately conservative;
//! if any condition fails the tree keeps the flat three-clause shape.

use crate::syntax::{Clause, SyntaxAnalysis, predicate_matches_previous_subject};
use crate::tags::{Mood, PartOfSpeech};
use crate::tokenise::Token;

use super::seed::Seed;
use super::{Builder, ClauseKind, Node, NodeId};

#[derive(Debug, Clone, Copy)]
struct Merge {
    host: usize,
    relative: usize,
    continuation: usize,
}

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

/// The kind of a merged host. The first segment supplies the subordinator; the
/// continuation supplies the predicate, so an infinitive predicate still types
/// the clause.
pub(super) fn host_kind(
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
    first: &Clause,
    continuation: &Clause,
) -> ClauseKind {
    match kind_of(tokens, analysis, first) {
        ClauseKind::Subordinate => ClauseKind::Subordinate,
        ClauseKind::Infinitive => ClauseKind::Infinitive,
        _ => {
            if kind_of(tokens, analysis, continuation) == ClauseKind::Infinitive {
                ClauseKind::Infinitive
            } else {
                ClauseKind::Main
            }
        }
    }
}

pub(super) fn group(
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
    builder: &mut Builder,
    seed: &Seed,
) -> Vec<NodeId> {
    let merges = plan_merges(tokens, analysis);
    let mut nodes: Vec<Option<NodeId>> = vec![None; analysis.clauses.len()];

    for merge in &merges {
        let host = &analysis.clauses[merge.host];
        let relative = &analysis.clauses[merge.relative];
        let continuation = &analysis.clauses[merge.continuation];
        let kind = host_kind(tokens, analysis, host, continuation);
        let host_node = builder.push(Node::host_clause(
            kind,
            host,
            continuation,
            merge.continuation,
        ));
        let relative_node =
            builder.push(Node::clause(relative, merge.relative, ClauseKind::Relative));
        builder.attach(host_node, relative_node);
        nodes[merge.host] = Some(host_node);
        nodes[merge.relative] = Some(relative_node);
        nodes[merge.continuation] = Some(host_node);
        builder.attach(seed.sentence_for(host.start), host_node);
    }

    for (index, clause) in analysis.clauses.iter().enumerate() {
        if nodes[index].is_some() {
            continue;
        }
        let node = builder.push(Node::clause(
            clause,
            index,
            kind_of(tokens, analysis, clause),
        ));
        nodes[index] = Some(node);
        builder.attach(seed.sentence_for(clause.start), node);
    }

    // Reparent the word leaves from their sentence to their tree clause. The
    // host owns both of its segments' words; the relative owns its own.
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
            .and_then(|clause| nodes.get(clause).copied().flatten())
            .unwrap_or_else(|| seed.sentence_for(token.start));
        builder.reparent(parent, leaf);
    }

    builder.sort_children();
    nodes
        .into_iter()
        .map(|node| node.expect("every flat clause has a tree node"))
        .collect()
}

fn plan_merges(tokens: &[Token], analysis: &SyntaxAnalysis) -> Vec<Merge> {
    let mut used = vec![false; analysis.clauses.len()];
    let mut merges = Vec::new();

    for (relative_index, relative) in analysis.clauses.iter().enumerate() {
        let Some(host_index) = relative.parent else {
            continue;
        };
        let Some(host) = analysis.clauses.get(host_index) else {
            continue;
        };
        let continuation_index = relative_index + 1;
        let Some(continuation) = analysis.clauses.get(continuation_index) else {
            continue;
        };

        if used[host_index] || used[relative_index] || used[continuation_index] {
            continue;
        }
        if host.parent.is_some() {
            continue;
        }
        if host.children.len() != 1 || host.children[0] != relative_index {
            continue;
        }
        if !relative.children.is_empty() {
            continue;
        }
        if continuation.parent.is_some() || !continuation.children.is_empty() {
            continue;
        }
        if host.tokens.end > relative.tokens.start
            || relative.tokens.end > continuation.tokens.start
        {
            continue;
        }

        let Some(sentence) = sentence_containing(analysis, host.start) else {
            continue;
        };
        if sentence_containing(analysis, relative.start) != Some(sentence)
            || sentence_containing(analysis, continuation.start) != Some(sentence)
        {
            continue;
        }
        if last_clause_in_sentence(analysis, sentence) != Some(continuation_index) {
            continue;
        }

        if let Some(subject) = continuation.subject {
            if Some(subject) != host.subject {
                continue;
            }
        } else if !predicate_matches_previous_subject(continuation, host, tokens, &analysis.tokens)
        {
            continue;
        }

        if let Some(antecedent) = relative.antecedent
            && !host.tokens.contains(&antecedent)
            && !continuation.tokens.contains(&antecedent)
        {
            continue;
        }

        used[host_index] = true;
        used[relative_index] = true;
        used[continuation_index] = true;
        merges.push(Merge {
            host: host_index,
            relative: relative_index,
            continuation: continuation_index,
        });
    }

    merges
}

fn sentence_containing(analysis: &SyntaxAnalysis, byte: usize) -> Option<usize> {
    analysis
        .sentences
        .iter()
        .position(|span| span.start <= byte && byte < span.end)
}

fn last_clause_in_sentence(analysis: &SyntaxAnalysis, sentence: usize) -> Option<usize> {
    let span = analysis.sentences.get(sentence)?;
    analysis
        .clauses
        .iter()
        .enumerate()
        .rev()
        .find(|(_, clause)| span.start <= clause.start && clause.start < span.end)
        .map(|(index, _)| index)
}
