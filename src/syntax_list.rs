//! Lists as a syntactic structure.
//!
//! A comma-separated nominal run is not a sequence of clauses and not a
//! punctuation coincidence. This pass finds the run inside a clause, splits it
//! on top-level commas, attaches the final conjunction, classifies the items,
//! and records the list for the diagnostic layer.
//!
//! It is deliberately conservative: a list needs at least one top-level comma
//! or semicolon, or a colon introducer, and the parser only calls something a
//! `Nominal` list when every item is nominal and none is a finite clause.
//! `Mixed` lists are kept for analysis but never produce an automatic
//! punctuation correction.
//!
//! A semicolon is a clause boundary, so the clause layer has already split a
//! semicolon list into one clause per group. The parser therefore joins
//! adjacent clauses that a top-level semicolon separates (within one sentence)
//! into one list whose items are the clause groups; commas inside a group are
//! item-internal, not list separators.

use std::ops::Range;

use crate::sentence::SentenceSpan;
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
    /// Separator token indices that divide top-level items.
    pub separators: Vec<usize>,
    /// How each entry of `separators` is written. Parallel to `separators`.
    pub separator_kinds: Vec<ListSeparator>,
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
    /// Word range extended over trailing separators, up to the next word.
    /// A nested list needs the closing bracket that trails the item's last
    /// word; diagnostics keep using `tokens`.
    pub(crate) scan: Range<usize>,
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
pub enum ListSeparator {
    Comma,
    Semicolon,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListItemKind {
    Word,
    Phrase,
    Clause,
    /// Nested lists, indices into `analysis.lists`.
    List(Vec<usize>),
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
    pub fn name(&self) -> &'static str {
        match self {
            ListItemKind::Word => "word",
            ListItemKind::Phrase => "phrase",
            ListItemKind::Clause => "clause",
            ListItemKind::List(_) => "list",
        }
    }
}

/// Find lists in every clause.
///
/// This is the compatibility entry point: it derives the sentence spans from
/// the token stream and then calls [`parse_with_sentences`]. A caller that
/// already has the syntax analysis should use the latter.
pub fn parse(tokens: &[Token], clauses: &[Clause], out: &[TokenSyntax]) -> Vec<List> {
    let contexts = crate::sentence::analyze(tokens);
    let sentences = crate::sentence::spans_from_contexts(tokens, &contexts);
    parse_with_sentences(tokens, clauses, &sentences, out)
}

/// Find lists in every clause with a caller-supplied sentence map.
///
/// The arena is post-order: a nested list is pushed before the list that
/// contains it, so `parent` indices are always smaller than the list itself.
/// `ListItemKind::List(indices)` names the children from the parent's
/// `items`.
///
/// `sentences` is used only to keep a semicolon run inside one sentence; a
/// semicolon never ends a sentence, so a run that crosses a period is not one
/// list.
pub fn parse_with_sentences(
    tokens: &[Token],
    clauses: &[Clause],
    sentences: &[SentenceSpan],
    out: &[TokenSyntax],
) -> Vec<List> {
    let mut lists = Vec::new();
    let mut index = 0;
    while index < clauses.len() {
        let end = semicolon_run_end(index, clauses, tokens, sentences);
        if end > index + 1
            && parse_semicolon_run(index..end, clauses, tokens, out, &mut lists).is_some()
        {
            index = end;
            continue;
        }
        parse_clause(index, &clauses[index], tokens, out, &mut lists);
        index += 1;
    }
    lists
}

/// One past the last clause joined to `start` by top-level semicolons.
fn semicolon_run_end(
    start: usize,
    clauses: &[Clause],
    tokens: &[Token],
    sentences: &[SentenceSpan],
) -> usize {
    let mut end = start + 1;
    while end < clauses.len() {
        let previous = &clauses[end - 1];
        let next = &clauses[end];
        if sentence_index(previous.start, sentences) != sentence_index(next.start, sentences) {
            break;
        }
        if separator_between(tokens, previous, next, ';').is_none() {
            break;
        }
        end += 1;
    }
    end
}

fn sentence_index(byte: usize, sentences: &[SentenceSpan]) -> Option<usize> {
    sentences
        .iter()
        .find(|sentence| byte >= sentence.start && byte < sentence.end)
        .map(|sentence| sentence.index)
}

/// The separator token that writes `needle` between two clauses.
fn separator_between(
    tokens: &[Token],
    left: &Clause,
    right: &Clause,
    needle: char,
) -> Option<usize> {
    let start = left.tokens.end;
    let end = right.tokens.start;
    if start >= end {
        return None;
    }
    let mut depth = 0i32;
    for (index, token) in tokens.iter().enumerate().take(end).skip(start) {
        if token.is_word() {
            continue;
        }
        for character in token.text.chars() {
            match character {
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => depth = (depth - 1).max(0),
                c if c == needle && depth == 0 => return Some(index),
                _ => {}
            }
        }
    }
    None
}

/// Join a semicolon-separated clause run into one list.
///
/// Each clause group is one item. The first group may hold the host predicate;
/// the list starts after it (`Ostin omenoita; banaaneja` lists the objects).
/// A leading `ja`/`sekä`/`tai`/`vai` in the last group becomes the list
/// conjunction. Commas inside a group stay inside its item, so the group is a
/// phrase (`Virtanen, toimitusjohtaja`), not a nested list.
fn parse_semicolon_run(
    run: Range<usize>,
    clauses: &[Clause],
    tokens: &[Token],
    out: &[TokenSyntax],
    lists: &mut Vec<List>,
) -> Option<usize> {
    let first_clause = &clauses[run.start];
    let first_words = clause_words(first_clause, tokens);
    // `Ostin omenoita; …` starts the list at `omenoita`, not at the predicate.
    let first_comma = top_level_commas(tokens, first_clause.tokens.start, first_clause.tokens.end)
        .into_iter()
        .next()
        .unwrap_or(first_clause.tokens.end);
    let before_separator: Vec<usize> = first_words
        .iter()
        .copied()
        .filter(|&index| index < first_comma)
        .collect();
    let start_position = before_separator
        .iter()
        .rposition(|&index| is_finite(&out[index]))
        .map_or(0, |position| position + 1);
    let start = *first_words.get(start_position)?;

    let mut items = Vec::new();
    let mut separators = Vec::new();
    let mut conjunction = None;
    for (offset, clause) in clauses[run.clone()].iter().enumerate() {
        let (mut first, end) = word_run(tokens, clause.tokens.start, clause.tokens.end)?;
        if offset == 0 {
            first = start;
        }
        if offset + 1 == run.len() && is_conjunction_word(first, tokens) {
            conjunction = Some(first);
            first = word_run(tokens, first + 1, end)?.0;
        }
        if offset > 0 {
            let previous = &clauses[run.start + offset - 1];
            let separator = separator_between(tokens, previous, clause, ';')?;
            separators.push(separator);
        }
        items.push(list_item(first, end, clause.tokens.end, tokens, out));
    }
    if items.len() < 2 {
        return None;
    }
    let introducer = introducer_before(tokens, items[0].tokens.start);
    let kind = classify(&items, out);
    let first = items[0].tokens.start;
    let last = items[items.len() - 1].tokens.end;
    let span = tokens[first].start..tokens[last - 1].end;
    let role = out.get(first).map(|syntax| syntax.role).filter(|role| {
        matches!(
            role,
            SyntacticRole::Subject
                | SyntacticRole::Object
                | SyntacticRole::Adverbial
                | SyntacticRole::Complement
        )
    });
    let head = out.get(first).and_then(|syntax| syntax.head);
    let index = lists.len();
    lists.push(List {
        span,
        tokens: first..last,
        clause: run.start,
        role,
        head,
        kind,
        items,
        separator_kinds: vec![ListSeparator::Semicolon; separators.len()],
        separators,
        conjunction,
        introducer,
        parent: None,
    });
    Some(index)
}

fn clause_words(clause: &Clause, tokens: &[Token]) -> Vec<usize> {
    clause
        .tokens
        .clone()
        .filter(|&index| tokens[index].is_word())
        .collect()
}

fn introducer_before(tokens: &[Token], first_token: usize) -> Option<usize> {
    let mut before = first_token;
    while before > 0 && !tokens[before - 1].is_word() {
        before -= 1;
        if tokens[before].text.contains(':') {
            return Some(before);
        }
    }
    None
}

fn parse_clause(
    clause_index: usize,
    clause: &Clause,
    tokens: &[Token],
    out: &[TokenSyntax],
    lists: &mut Vec<List>,
) -> Option<usize> {
    let words: Vec<usize> = clause
        .tokens
        .clone()
        .filter(|&index| tokens[index].is_word())
        .collect();
    if words.len() < 2 {
        return None;
    }

    let (start_word, introducer) = list_start(&words, tokens, clause, out);
    let start_word = start_word?;
    let start_token = words[start_word];
    let end_token = words.last().copied()?;
    if start_token >= end_token {
        return None;
    }

    let commas = top_level_commas(tokens, start_token, end_token + 1);
    if commas.is_empty() && introducer.is_none() {
        return None;
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
        return None;
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
        return None;
    }

    let mut items: Vec<ListItem> = bounds
        .iter()
        .map(|&(first, end)| list_item(first, end, clause.tokens.end, tokens, out))
        .collect();
    // Children are appended before the parent, so the arena stays post-order.
    let mut children_of: Vec<Vec<usize>> = vec![Vec::new(); items.len()];
    for (item_index, item) in items.iter().enumerate() {
        children_of[item_index] = nested_in_item(item, clause_index, tokens, out, lists);
    }
    let parent_index = lists.len();
    for (item_index, children) in children_of.iter().enumerate() {
        for &child in children {
            lists[child].parent = Some(parent_index);
        }
        if !children.is_empty() {
            items[item_index].kind = ListItemKind::List(children.clone());
        }
    }
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
    let separator_count = commas.len();
    lists.push(List {
        span,
        tokens: first..last,
        clause: clause_index,
        role,
        head,
        kind,
        items,
        separators: commas,
        separator_kinds: vec![ListSeparator::Comma; separator_count],
        conjunction,
        introducer,
        parent: None,
    });
    Some(parent_index)
}

/// Parse a bracketed list inside one item: `omena (pieniä, makeita)`.
///
/// Only the outermost bracket pairs are considered, each needs at least two
/// words and a top-level comma. Every child list is appended to the arena and
/// its index returned; the caller sets the item kind and the parent links.
fn nested_in_item(
    item: &ListItem,
    clause_index: usize,
    tokens: &[Token],
    out: &[TokenSyntax],
    lists: &mut Vec<List>,
) -> Vec<usize> {
    let mut stack: Vec<(usize, char)> = Vec::new();
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    for index in item.scan.clone() {
        if tokens[index].is_word() {
            continue;
        }
        for character in tokens[index].text.chars() {
            match character {
                '(' | '[' | '{' => stack.push((index, character)),
                ')' | ']' | '}' => {
                    let Some((open, opener)) = stack.pop() else {
                        continue;
                    };
                    let matches =
                        matches!((opener, character), ('(', ')') | ('[', ']') | ('{', '}'));
                    if matches && open + 1 < index {
                        pairs.push((open + 1, index));
                    }
                }
                _ => {}
            }
        }
    }

    // Keep only pairs that are not inside another pair. An outer pair's own
    // recursive parse will find the inner one, so scanning both would produce
    // the same child twice.
    let outer: Vec<(usize, usize)> = pairs
        .iter()
        .copied()
        .filter(|&(start, end)| {
            !pairs
                .iter()
                .any(|&(outer_start, outer_end)| outer_start < start && end < outer_end)
        })
        .collect();

    let mut children = Vec::new();
    for (start, end) in outer {
        if top_level_commas(tokens, start, end).is_empty() {
            continue;
        }
        let Some((first, last)) = word_run(tokens, start, end) else {
            continue;
        };
        let clause = Clause {
            start: tokens[first].start,
            end: tokens[last - 1].end,
            tokens: first..last,
            predicate: None,
            subject: None,
            object: None,
            complement: None,
            negative: false,
            existential: false,
            passive: false,
            features: crate::syntax::ClauseFeatures::default(),
            parent: None,
            children: Vec::new(),
            antecedent: None,
        };
        if let Some(child) = parse_clause(clause_index, &clause, tokens, out, lists) {
            children.push(child);
        }
    }
    children
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

fn list_item(
    first: usize,
    end: usize,
    clause_end: usize,
    tokens: &[Token],
    out: &[TokenSyntax],
) -> ListItem {
    let words = word_run(tokens, first, end).unwrap_or((first, end));
    let mut scan_end = words.1;
    while scan_end < clause_end && !tokens[scan_end].is_word() {
        scan_end += 1;
    }
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
        scan: words.0..scan_end,
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
    (start..end).rfind(|&index| is_conjunction_word(index, tokens))
}

fn is_conjunction_word(index: usize, tokens: &[Token]) -> bool {
    matches!(
        tokens[index].text.to_lowercase().as_str(),
        "ja" | "sekä" | "tai" | "vai"
    )
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

    #[test]
    fn a_parenthetical_list_is_a_child() {
        let (tokens, lists) = lists(
            "Ostin omenoita, appelsiineja (pieniä, makeita) ja banaaneja.",
        );
        assert_eq!(lists.len(), 2, "{lists:#?}");
        let parent = lists.iter().position(|list| list.parent.is_none()).unwrap();
        let child = lists.iter().position(|list| list.parent.is_some()).unwrap();
        assert!(child < parent, "the arena is post-order: {lists:#?}");
        assert_eq!(lists[child].parent, Some(parent));
        assert_eq!(lists[parent].items.len(), 3);
        assert_eq!(lists[child].items.len(), 2);
        assert!(matches!(
            &lists[parent].items[1].kind,
            ListItemKind::List(indices) if indices == &vec![child]
        ));
        assert_eq!(&tokens[lists[child].items[0].tokens.start].text, "pieniä");
        assert_eq!(&tokens[lists[child].items[1].tokens.start].text, "makeita");
    }

    #[test]
    fn one_item_can_hold_two_child_lists() {
        let (_, lists) = lists(
            "Ostin omenoita, appelsiineja (pieniä, makeita) (happamia, raikkaita) ja banaaneja.",
        );
        assert_eq!(lists.len(), 3, "{lists:#?}");
        let parent = lists.iter().position(|list| list.parent.is_none()).unwrap();
        let children: Vec<usize> = lists[parent]
            .items
            .iter()
            .find_map(|item| match &item.kind {
                ListItemKind::List(indices) => Some(indices.clone()),
                _ => None,
            })
            .expect("the middle item should hold children");
        assert_eq!(children.len(), 2, "{lists:#?}");
        for child in children {
            assert_eq!(lists[child].parent, Some(parent));
            assert_eq!(lists[child].items.len(), 2);
        }
    }

    #[test]
    fn a_bracketed_relative_clause_is_not_a_nested_list() {
        let (_, lists) = lists("Kuitenkin, ostin omenoita (jotka olivat kalliita) eilen.");
        assert!(lists.iter().all(|list| list.parent.is_none()), "{lists:#?}");
    }

    #[test]
    fn a_semicolon_makes_a_list_with_semicolon_separators() {
        let (tokens, lists) = lists("Ostin omenoita; appelsiineja ja banaaneja.");
        assert_eq!(lists.len(), 1, "{lists:#?}");
        let list = &lists[0];
        assert_eq!(list.kind, ListKind::Nominal);
        assert_eq!(list.items.len(), 2);
        assert_eq!(
            list.separators.len(),
            list.separator_kinds.len(),
            "the separator kinds are parallel to the separator tokens"
        );
        assert!(
            list.separator_kinds
                .iter()
                .all(|kind| *kind == ListSeparator::Semicolon)
        );
        assert_eq!(&tokens[list.items[0].tokens.start].text, "omenoita");
    }

    #[test]
    fn a_short_semicolon_list_is_one_nominal_list() {
        let (tokens, lists) = lists("Ostin omenoita; banaaneja; appelsiineja.");
        assert_eq!(lists.len(), 1, "{lists:#?}");
        let list = &lists[0];
        assert_eq!(list.kind, ListKind::Nominal);
        assert_eq!(list.items.len(), 3);
        assert_eq!(list.separator_kinds.len(), 2);
        assert_eq!(list.separators.len(), 2);
        assert_eq!(&tokens[list.items[0].tokens.start].text, "omenoita");
    }

    #[test]
    fn a_complex_semicolon_list_keeps_its_internal_commas() {
        let (_, lists) = lists(
            "Kokoukseen osallistuivat Virtanen, toimitusjohtaja; \
             Laine, projektipäällikkö; Korhonen, talousjohtaja.",
        );
        assert_eq!(lists.len(), 1, "{lists:#?}");
        let list = &lists[0];
        assert_eq!(list.items.len(), 3);
        assert!(
            list.items
                .iter()
                .all(|item| item.kind == ListItemKind::Phrase),
            "each semicolon group is one phrase: {lists:#?}"
        );
        assert_eq!(list.separator_kinds.len(), 2);
    }

    #[test]
    fn a_colon_introduced_semicolon_list_is_one_list() {
        let (_, lists) = lists("Tarvitsemme kolme asiaa: aikaa; rahaa; osaamista.");
        assert_eq!(lists.len(), 1, "{lists:#?}");
        let list = &lists[0];
        assert_eq!(list.kind, ListKind::Nominal);
        assert!(list.introducer.is_some());
        assert_eq!(list.items.len(), 3);
    }

    #[test]
    fn a_main_main_semicolon_is_not_a_list() {
        let (_, lists) = lists("Hän tuli; minä lähdin.");
        assert!(lists.is_empty(), "{lists:#?}");
    }
}
