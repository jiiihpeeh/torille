//! Phase 1: head-driven phrases inside a clause.
//!
//! A phrase root is a token that fills a clause slot, is an adverbial, or is
//! the target of a `Modifier` edge. Only `Modifier` and `Auxiliary` edges
//! become phrase-internal links; an object is not a child of the verb phrase
//! merely because the verb governs it.
//!
//! The pass is deliberately contiguous-only. A dependent separated from its
//! head by a relative clause, a parenthetical or a list leaves the words as
//! leaves and keeps the flat `head` edge as the node's `governor`.

use crate::syntax::{SyntacticRole, SyntaxAnalysis};
use crate::tags::{Mood, PartOfSpeech};
use crate::tokenise::Token;

use super::seed::Seed;
use super::{Builder, Node, NodeId, PhraseKind};

pub(super) fn attach(
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
    builder: &mut Builder,
    seed: &Seed,
    clause_nodes: &[NodeId],
) {
    for (clause_index, clause) in analysis.clauses.iter().enumerate() {
        let Some(&clause_node) = clause_nodes.get(clause_index) else {
            continue;
        };

        let mut words = Vec::new();
        let mut word_set = vec![false; tokens.len()];
        for index in clause.tokens.clone() {
            let (Some(token), Some(syntax)) = (tokens.get(index), analysis.tokens.get(index))
            else {
                continue;
            };
            if token.is_word() && syntax.clause == clause_index {
                word_set[index] = true;
                words.push(index);
            }
        }
        if words.is_empty() {
            continue;
        }

        // The only phrase-internal edges. An adposition's complement is also
        // internal (`Ilman rahaa`, `Pöydän alla`); a verb's complement is not.
        let mut dependents = vec![Vec::new(); tokens.len()];
        for &word in &words {
            let syntax = &analysis.tokens[word];
            let Some(head) = syntax.head else {
                continue;
            };
            if head == word || !word_set.get(head).copied().unwrap_or(false) {
                continue;
            }
            let head_is_adposition = analysis.tokens.get(head).is_some_and(|head| {
                head.readings.iter().any(|reading| {
                    matches!(
                        reading.part_of_speech,
                        Some(PartOfSpeech::Postposition | PartOfSpeech::Preposition)
                    )
                })
            });
            let internal = matches!(
                syntax.role,
                SyntacticRole::Modifier | SyntacticRole::Auxiliary
            ) || (syntax.role == SyntacticRole::Complement && head_is_adposition);
            if internal {
                dependents[head].push(word);
            }
        }

        let mut roots = Vec::new();
        for &word in &words {
            let role = analysis.tokens[word].role;
            if matches!(
                role,
                SyntacticRole::Predicate
                    | SyntacticRole::Subject
                    | SyntacticRole::Object
                    | SyntacticRole::Complement
                    | SyntacticRole::Adverbial
            ) {
                roots.push(word);
            }
            if !matches!(role, SyntacticRole::Modifier | SyntacticRole::Auxiliary)
                && !dependents[word].is_empty()
            {
                roots.push(word);
            }
        }
        roots.sort_unstable();
        roots.dedup();

        let mut claimed = vec![false; tokens.len()];
        for root in roots {
            if !word_set[root] || claimed[root] {
                continue;
            }
            let set = collect(root, &dependents, tokens.len());
            try_attach(
                root,
                &set,
                tokens,
                analysis,
                builder,
                seed,
                clause_node,
                clause_index,
                &mut claimed,
            );
        }

        // A predicate-less fragment has no clause slot to root a phrase. A
        // narrow shape run still gives a heading or caption its noun phrase:
        // adjective/ordinal/demonstrative modifiers immediately before a noun.
        // It is deliberately weak and never invents a role.
        if clause.predicate.is_none() {
            let shape_roots = shape_roots(&words, analysis, &claimed);
            for (head, set) in shape_roots {
                if claimed[head] {
                    continue;
                }
                try_attach(
                    head,
                    &set,
                    tokens,
                    analysis,
                    builder,
                    seed,
                    clause_node,
                    clause_index,
                    &mut claimed,
                );
            }
        }

        builder.sort_children();
    }
}

fn collect(root: usize, dependents: &[Vec<usize>], token_count: usize) -> Vec<bool> {
    let mut set = vec![false; token_count];
    let mut stack = vec![root];
    while let Some(token) = stack.pop() {
        if set[token] {
            continue;
        }
        set[token] = true;
        stack.extend(dependents[token].iter().copied());
    }
    set
}

#[allow(clippy::too_many_arguments)]
fn try_attach(
    root: usize,
    set: &[bool],
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
    builder: &mut Builder,
    seed: &Seed,
    clause_node: NodeId,
    clause_index: usize,
    claimed: &mut [bool],
) -> bool {
    let mut min = usize::MAX;
    let mut max = 0usize;
    let mut any = false;
    for (index, &in_set) in set.iter().enumerate() {
        if in_set {
            min = min.min(index);
            max = max.max(index);
            any = true;
        }
    }
    if !any {
        return false;
    }
    let contiguous =
        (min..=max).all(|index| !tokens.get(index).is_some_and(Token::is_word) || set[index]);
    if !contiguous {
        return false;
    }
    if (0..tokens.len()).any(|index| set[index] && claimed[index]) {
        return false;
    }
    let Some(kind) = phrase_kind(set, root, analysis) else {
        return false;
    };

    for (index, &in_set) in set.iter().enumerate() {
        if in_set {
            claimed[index] = true;
        }
    }
    let span = tokens[min].start..tokens[max].end;
    let syntax = &analysis.tokens[root];
    let phrase = builder.push(Node::phrase(
        kind,
        min..max + 1,
        span,
        root,
        syntax,
        clause_index,
    ));
    builder.attach(clause_node, phrase);
    for (index, &in_set) in set.iter().enumerate() {
        if !in_set {
            continue;
        }
        if let Some(leaf) = seed.words.get(index).copied().flatten() {
            builder.reparent(phrase, leaf);
        }
    }
    true
}

fn shape_roots(
    words: &[usize],
    analysis: &SyntaxAnalysis,
    claimed: &[bool],
) -> Vec<(usize, Vec<bool>)> {
    let mut out = Vec::new();
    for (position, &head) in words.iter().enumerate() {
        if claimed.get(head).copied().unwrap_or(false) || !is_noun_head(head, analysis) {
            continue;
        }
        let mut set = vec![false; analysis.tokens.len()];
        let mut modifiers = 0usize;
        for &previous in words[..position].iter().rev() {
            if claimed.get(previous).copied().unwrap_or(false)
                || !is_shape_modifier(&analysis.tokens[previous])
            {
                break;
            }
            set[previous] = true;
            modifiers += 1;
        }
        if modifiers == 0 {
            continue;
        }
        set[head] = true;
        out.push((head, set));
    }
    out
}

fn is_noun_head(index: usize, analysis: &SyntaxAnalysis) -> bool {
    analysis.tokens.get(index).is_some_and(|token| {
        token
            .readings
            .iter()
            .any(|reading| reading.part_of_speech == Some(PartOfSpeech::Noun))
    })
}

fn is_shape_modifier(token: &crate::syntax::TokenSyntax) -> bool {
    token.readings.iter().any(|reading| {
        matches!(
            reading.part_of_speech,
            Some(PartOfSpeech::Adjective | PartOfSpeech::Ordinal)
        ) || (reading.part_of_speech == Some(PartOfSpeech::Pronoun) && is_demonstrative(&token.key))
    })
}

fn is_demonstrative(key: &str) -> bool {
    matches!(
        key,
        "se" | "tämä" | "tuo" | "ne" | "nämä" | "nuo" | "sama" | "samat"
    )
}

/// The lexical kind of a phrase, from its head and the role the flat model
/// chose. The role breaks POS homonyms: `tuli` is a verb predicate but a noun
/// subject, and `kirjaa` is a noun object but a verb form.
fn phrase_kind(set: &[bool], head: usize, analysis: &SyntaxAnalysis) -> Option<PhraseKind> {
    let syntax = analysis.tokens.get(head)?;
    let readings = &syntax.readings;
    let has = |pos: PartOfSpeech| {
        readings
            .iter()
            .any(|reading| reading.part_of_speech == Some(pos))
    };

    let nominal = if has(PartOfSpeech::Pronoun) {
        Some(PhraseKind::Pronoun)
    } else if has(PartOfSpeech::Adjective) {
        Some(PhraseKind::Adjective)
    } else if has(PartOfSpeech::Numeral) || has(PartOfSpeech::Ordinal) {
        Some(PhraseKind::Numeral)
    } else if has(PartOfSpeech::Noun) {
        Some(PhraseKind::Noun)
    } else {
        None
    };
    let verb = if readings.iter().any(|reading| {
        reading.part_of_speech == Some(PartOfSpeech::Verb) && reading.mood == Some(Mood::Infinitive)
    }) {
        Some(PhraseKind::Infinitive)
    } else if readings.iter().any(|reading| {
        reading.part_of_speech == Some(PartOfSpeech::Verb)
            && reading.case.is_some()
            && reading.mood != Some(Mood::Infinitive)
    }) {
        Some(PhraseKind::Participle)
    } else if has(PartOfSpeech::Verb) || has(PartOfSpeech::NegativeVerb) {
        Some(PhraseKind::Verb)
    } else {
        None
    };
    let adposition = if has(PartOfSpeech::Postposition) {
        Some(PhraseKind::Postposition)
    } else if has(PartOfSpeech::Preposition) {
        Some(PhraseKind::Preposition)
    } else {
        None
    };
    let adverb = has(PartOfSpeech::Adverb).then_some(PhraseKind::Adverb);

    let kind = match syntax.role {
        SyntacticRole::Predicate | SyntacticRole::Auxiliary => {
            verb.or(nominal).or(adposition).or(adverb)
        }
        SyntacticRole::Subject
        | SyntacticRole::Object
        | SyntacticRole::Complement
        | SyntacticRole::Modifier => adposition.or(nominal).or(verb).or(adverb),
        SyntacticRole::Adverbial => adposition.or(adverb).or(nominal).or(verb),
        SyntacticRole::Unknown => nominal.or(verb).or(adposition).or(adverb),
    }?;

    // A nominal head with a numeral dependent is the numeral phrase the
    // agreement rules want, even when the flat model made the noun the head.
    if kind == PhraseKind::Noun
        && set.iter().enumerate().any(|(index, &in_set)| {
            in_set
                && index != head
                && analysis.tokens.get(index).is_some_and(|token| {
                    token.readings.iter().any(|reading| {
                        matches!(
                            reading.part_of_speech,
                            Some(PartOfSpeech::Numeral | PartOfSpeech::Ordinal)
                        )
                    })
                })
        })
    {
        return Some(PhraseKind::Numeral);
    }

    Some(kind)
}
