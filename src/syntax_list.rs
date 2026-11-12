//! Lists as a syntactic structure.
//!
//! A comma-separated nominal run is not a sequence of clauses and not a
//! punctuation coincidence. This pass finds the run inside a clause, splits it
//! on top-level commas, attaches the final conjunction, classifies the items,
//! and records the list for the diagnostic layer.
//!
//! It is deliberately conservative: a list needs at least one top-level comma
//! or a colon introducer, and the parser only calls something a `Nominal` list
//! when every item is nominal and none is a finite clause. `Mixed` lists are
//! kept for analysis but never produce an automatic punctuation correction.

use std::ops::Range;

use crate::syntax::{Clause, SyntacticRole, TokenSyntax};
use crate::tags::{Mood, PartOfSpeech, Polarity, Tags};
use crate::tokenise::Token;

/// One recognised list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct List {
    /// Byte span of the whole list, introducer excluded.
    pub span: Range<usize>,
    /// Token range, introducer excluded.
    pub tokens: Range<usize>,
    /// Clause the list belongs to.
    pub clause: usize,
    /// The list's grammatical function, when the clause makes it clear.
    pub role: Option<SyntacticRole>,
    /// Governing token, when one is known.
    pub head: Option<usize>,
    pub kind: ListKind,
    pub items: Vec<ListItem>,
    /// Comma token indices that separate top-level items.
    pub separators: Vec<usize>,
    /// Final `ja`/`sekä`/`tai`/`vai`, when present.
    pub conjunction: Option<usize>,
    /// Colon that introduces the list, when present.
    pub introducer: Option<usize>,
    /// Parent list when this list is nested inside an item.
    pub parent: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListItem {
    pub tokens: Range<usize>,
    pub span: Range<usize>,
    pub kind: ListItemKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListKind {
    /// `omena, appelsiini ja banaani`
    Nominal,
    /// `Ostin omenoita, Liisa osti banaaneja`
    Clause,
    /// `Sami söi, joi ja nukkui`
    Predicate,
    /// Do not apply list punctuation automatically.
    Mixed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListItemKind {
    Word,
    Phrase,
    Clause,
    /// Nested list, index into `analysis.lists`.
    List(usize),
}

impl ListKind {
    pub fn name(self) -> &'static str {
        match self {
            ListKind::Nominal => "nominal",
            ListKind::Clause => "clause",
            ListKind::Predicate => "predicate",
            ListKind::Mixed => "mixed",
        }
    }
}

impl ListItemKind {
    pub fn name(self) -> &'static str {
        match self {
            ListItemKind::Word => "word",
            ListItemKind::Phrase => "phrase",
            ListItemKind::Clause => "clause",
            ListItemKind::List(_) => "list",
        }
    }
}

/// Find lists in every clause. The returned arena is in source order; nested
/// lists are not produced yet (the top-level introduced list is).
pub fn parse(tokens: &[Token], clauses: &[Clause], out: &[TokenSyntax]) -> Vec<List> {
    let mut lists = Vec::new();
    for (clause_index, clause) in clauses.iter().enumerate() {
        parse_clause(clause_index, clause, tokens, out, &mut lists);
    }
    lists
}

fn parse_clause(
    clause_index: usize,
    clause: &Clause,
    tokens: &[Token],
    out: &[TokenSyntax],
    lists: &mut Vec<List>,
) {
    let words: Vec<usize> = clause
        .tokens
        .clone()
        .filter(|&index| tokens[index].is_word())
        .collect();
    if words.len() < 2 {
        return;
    }

    let (start_word, introducer) = list_start(&words, tokens, clause, out);
    let Some(start_word) = start_word else {
        return;
    };
    let start_token = words[start_word];
    let Some(end_token) = words.last().copied() else {
        return;
    };
    if start_token >= end_token {
        return;
    }

    let commas = top_level_commas(tokens, start_token, end_token + 1);
    if commas.is_empty() && introducer.is_none() {
        return;
    }

    // Split word runs at the top-level commas. Empty runs are kept out; a
    // duplicate comma is reported by the punctuation layer.
    let mut bounds: Vec<(usize, usize)> = Vec::new();
    let mut run_start = start_token;
    for comma in &commas {
        if let Some(run) = word_run(tokens, run_start, *comma) {
            bounds.push(run);
        }
        run_start = comma + 1;
    }
    if let Some(run) = word_run(tokens, run_start, end_token + 1) {
        bounds.push(run);
    }
    if bounds.len() < 2 {
        return;
    }

    // The final chunk can carry the conjunction. Split the last item at the
    // last top-level conjunction in that chunk.
    let mut conjunction = None;
    if let Some((last_start, last_end)) = bounds.pop() {
        if let Some(conjunction_index) = final_conjunction(tokens, last_start, last_end) {
            let (left, right) = split_at_word(tokens, last_start, last_end, conjunction_index);
            if let Some(left) = left {
                bounds.push(left);
            }
            if let Some(right) = right {
                bounds.push(right);
            }
            conjunction = Some(conjunction_index);
        } else {
            bounds.push((last_start, last_end));
        }
    }
    if bounds.len() < 2 {
        return;
    }

    let items: Vec<ListItem> = bounds
        .iter()
        .map(|&(first, end)| list_item(first, end, tokens, out))
        .collect();
    let kind = classify(&items, out);
    let first = items.first().map(|item| item.tokens.start).unwrap_or(start_token);
    let last = items.last().map(|item| item.tokens.end).unwrap_or(end_token + 1);
    let span = tokens[first].start..tokens[last.saturating_sub(1)].end;
    let role = out
        .get(first)
        .map(|syntax| syntax.role)
        .filter(|role| {
            matches!(
                role,
                SyntacticRole::Subject
                    | SyntacticRole::Object
                    | SyntacticRole::Adverbial
                    | SyntacticRole::Complement
            )
        });
    let head = out.get(first).and_then(|syntax| syntax.head);
    lists.push(List {
        span,
        tokens: first..last,
        clause: clause_index,
        role,
        head,
        kind,
        items,
        separators: commas,
        conjunction,
        introducer,
        parent: None,
    });
}

/// Where the list begins: after a colon introducer, or after the last finite
/// predicate before the first comma. Returns the word position in `words`.
fn list_start(
    words: &[usize],
    tokens: &[Token],
    clause: &Clause,
    out: &[TokenSyntax],
) -> (Option<usize>, Option<usize>) {
    if let Some(colon) = top_level_colon(tokens, clause.tokens.start, clause.tokens.end) {
        let after = words.iter().position(|&index| index > colon);
        return (after, Some(colon));
    }
    // A colon is a sentence boundary in the sentence layer, so the introducer
    // can sit just before this clause rather than inside its token range.
    let mut before = clause.tokens.start;
    while before > 0 && !tokens[before - 1].is_word() {
        before -= 1;
        if tokens[before].text.contains(':') {
            return (Some(0), Some(before));
        }
    }
    let Some(first_comma) = top_level_commas(tokens, clause.tokens.start, clause.tokens.end)
        .into_iter()
        .next()
    else {
        return (None, None);
    };
    let predicate = words
        .iter()
        .rposition(|&index| index < first_comma && is_finite(&out[index]));
    (Some(predicate.map_or(0, |position| position + 1)), None)
}

fn list_item(first: usize, end: usize, tokens: &[Token], out: &[TokenSyntax]) -> ListItem {
    let words = word_run(tokens, first, end).unwrap_or((first, end));
    let count = tokens[words.0..words.1]
        .iter()
        .filter(|token| token.is_word())
        .count();
    let kind = if has_finite(words.0, words.1, out) {
        ListItemKind::Clause
    } else if count > 1 {
        ListItemKind::Phrase
    } else {
        ListItemKind::Word
    };
    ListItem {
        tokens: words.0..words.1,
        span: tokens[words.0].start..tokens[words.1 - 1].end,
        kind,
    }
}

fn classify(items: &[ListItem], out: &[TokenSyntax]) -> ListKind {
    let mut nominal = true;
    let mut clause = true;
    let mut predicate = true;
    for item in items {
        let words: Vec<&TokenSyntax> = out[item.tokens.clone()].iter().collect();
        if !words.iter().any(|syntax| is_nominal(syntax)) || has_finite(item.tokens.start, item.tokens.end, out) {
            nominal = false;
        }
        if !has_finite(item.tokens.start, item.tokens.end, out) {
            clause = false;
            predicate = false;
        } else if !words.iter().any(|syntax| is_nominal(syntax)) {
            // A bare predicate list (`söi, joi ja nukkui`) has no nominal
            // subject inside each item.
        } else {
            predicate = false;
        }
    }
    if nominal {
        ListKind::Nominal
    } else if clause {
        ListKind::Clause
    } else if predicate {
        ListKind::Predicate
    } else {
        ListKind::Mixed
    }
}

fn top_level_commas(tokens: &[Token], start: usize, end: usize) -> Vec<usize> {
    top_level_punctuation(tokens, start, end, ',')
}

fn top_level_colon(tokens: &[Token], start: usize, end: usize) -> Option<usize> {
    top_level_punctuation(tokens, start, end, ':').into_iter().next()
}

fn top_level_punctuation(tokens: &[Token], start: usize, end: usize, needle: char) -> Vec<usize> {
    let mut depth = 0i32;
    let mut out = Vec::new();
    for (index, token) in tokens.iter().enumerate().take(end).skip(start) {
        if token.is_word() {
            continue;
        }
        for character in token.text.chars() {
            match character {
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => depth = (depth - 1).max(0),
                c if c == needle && depth == 0 => out.push(index),
                _ => {}
            }
        }
    }
    out
}

type WordRun = Option<(usize, usize)>;

fn word_run(tokens: &[Token], start: usize, end: usize) -> WordRun {
    let first = (start..end).find(|&index| tokens[index].is_word())?;
    let last = (start..end).rev().find(|&index| tokens[index].is_word())?;
    Some((first, last + 1))
}

fn final_conjunction(tokens: &[Token], start: usize, end: usize) -> Option<usize> {
    (start..end).rfind(|&index| {
        tokens[index].is_word()
            && matches!(
                tokens[index].text.to_lowercase().as_str(),
                "ja" | "sekä" | "tai" | "vai"
            )
    })
}

fn split_at_word(
    tokens: &[Token],
    start: usize,
    end: usize,
    at: usize,
) -> (WordRun, WordRun) {
    let left = word_run(tokens, start, at);
    let right = word_run(tokens, at + 1, end);
    (left, right)
}

fn has_finite(start: usize, end: usize, out: &[TokenSyntax]) -> bool {
    out[start..end].iter().any(|syntax| {
        syntax.readings.iter().any(|reading| {
            matches!(
                reading.part_of_speech,
                Some(PartOfSpeech::Verb | PartOfSpeech::NegativeVerb)
            ) && reading.polarity == Polarity::Affirmative
                && matches!(
                    reading.mood,
                    Some(Mood::Indicative | Mood::Conditional | Mood::Imperative | Mood::Potential)
                )
        })
    })
}

fn is_finite(syntax: &TokenSyntax) -> bool {
    syntax.readings.iter().any(|reading: &Tags| {
        matches!(
            reading.part_of_speech,
            Some(PartOfSpeech::Verb | PartOfSpeech::NegativeVerb)
        ) && reading.polarity == Polarity::Affirmative
            && matches!(
                reading.mood,
                Some(Mood::Indicative | Mood::Conditional | Mood::Imperative | Mood::Potential)
            )
    })
}

fn is_nominal(syntax: &TokenSyntax) -> bool {
    syntax.readings.iter().any(|reading| {
        matches!(
            reading.part_of_speech,
            Some(
                PartOfSpeech::Noun
                    | PartOfSpeech::Adjective
                    | PartOfSpeech::Pronoun
                    | PartOfSpeech::Numeral
                    | PartOfSpeech::Ordinal
            )
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax;

    fn lists(text: &str) -> (Vec<Token>, Vec<List>) {
        let tokens = crate::tokenise::tokenise(text);
        let analysis = syntax::analyze(&tokens);
        (tokens, analysis.lists)
    }

    #[test]
    fn a_nominal_list_is_recognised() {
        let (tokens, lists) = lists("Ostin omenoita, appelsiineja ja banaaneja.");
        assert_eq!(lists.len(), 1);
        let list = &lists[0];
        assert_eq!(list.kind, ListKind::Nominal);
        assert_eq!(list.items.len(), 3);
        assert_eq!(list.separators.len(), 1);
        assert!(list.conjunction.is_some());
        assert_eq!(&tokens[list.items[0].tokens.start].text, "omenoita");
    }

    #[test]
    fn an_introduced_list_is_recognised() {
        let (_, lists) = lists("Ostin kolme asiaa: omenoita, appelsiineja ja banaaneja.");
        assert_eq!(lists.len(), 1);
        assert!(lists[0].introducer.is_some());
        assert_eq!(lists[0].items.len(), 3);
    }

    #[test]
    fn a_relative_clause_is_not_a_list() {
        let (_, lists) = lists("Koira, joka nukkuu sohvalla, on minun.");
        assert!(lists.is_empty(), "{lists:#?}");
    }
}
