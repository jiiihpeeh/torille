//! List and clause punctuation diagnostics.
//!
//! The rules consume the list arena and the clause spans produced by the
//! analysis layer. They never guess a boundary from the comma alone: a
//! subordinate clause needs a subordinator, a relative clause needs a relative
//! pronoun, and a list needs the parallel items that `syntax_list` found.

use crate::sentence;
use crate::syntax::{Clause, SyntacticRole, SyntaxAnalysis, TokenSyntax};
use crate::syntax_list::{List, ListItem, ListItemKind, ListKind};
use crate::tags::{Clitic, Mood, PartOfSpeech, Polarity};
use crate::tokenise::Token;

use super::diagnostic::{
    Correction, DiagnosticConfidence, DiagnosticKind, Edit, Severity, SyntaxDiagnostic,
};

pub(super) fn check(tokens: &[Token], analysis: &SyntaxAnalysis) -> Vec<SyntaxDiagnostic> {
    let contexts = sentence::analyze(tokens);
    let mut diagnostics = list_diagnostics(tokens, analysis);
    diagnostics.extend(clause_diagnostics(tokens, analysis));
    diagnostics.extend(question_diagnostics(tokens, analysis));
    diagnostics.extend(quote_diagnostics(tokens, &contexts));
    diagnostics.extend(parenthetical_diagnostics(tokens, analysis, &contexts));
    diagnostics
}

// ── lists ────────────────────────────────────────────────────────────────

fn list_diagnostics(tokens: &[Token], analysis: &SyntaxAnalysis) -> Vec<SyntaxDiagnostic> {
    let mut out = Vec::new();
    for list in analysis.lists.iter().filter(|list| list.kind == ListKind::Nominal) {
        let Some(first_item) = list.items.first() else {
            continue;
        };

        if let Some(colon) = list.introducer {
            let commas = comma_positions(tokens, colon, first_item.tokens.start);
            if let Some(&(start, _)) = commas.first() {
                out.push(remove_comma(
                    DiagnosticKind::ListIntroducerComma,
                    Severity::Error,
                    start,
                    "list_introducer_comma",
                ));
            }
        }

        for pair in list.items.windows(2) {
            let [left, right] = pair else {
                continue;
            };
            let commas = comma_positions(tokens, left.tokens.end, right.tokens.start);
            if commas.len() >= 2 {
                let (start, _) = commas[1];
                out.push(remove_comma(
                    DiagnosticKind::ListSeparatorDuplicate,
                    Severity::Error,
                    start,
                    "list_separator_duplicate",
                ));
            }
        }

        for item in &list.items {
            out.extend(missing_separators(
                item,
                tokens,
                &analysis.tokens,
                &analysis.lists,
            ));
        }

        if let Some(conjunction) = list.conjunction {
            let previous = list
                .items
                .iter()
                .rev()
                .find(|item| item.tokens.end <= conjunction);
            if let Some(previous) = previous {
                let commas = comma_positions(tokens, previous.tokens.end, conjunction);
                if commas.len() == 1 {
                    let (start, _) = commas[0];
                    out.push(remove_comma(
                        DiagnosticKind::ListCommaBeforeConjunction,
                        Severity::Suggestion,
                        start,
                        "list_comma_before_conjunction",
                    ));
                }
            }
        }
    }
    out
}

fn missing_separators(
    item: &ListItem,
    tokens: &[Token],
    out: &[TokenSyntax],
    lists: &[List],
) -> Vec<SyntaxDiagnostic> {
    let words: Vec<usize> = item
        .tokens
        .clone()
        .filter(|&index| tokens[index].is_word())
        .collect();
    // A nested item's own words are checked by the child list. The words of
    // the host item before the bracket still need the parent-level check.
    let child = match item.kind {
        ListItemKind::List(index) => lists.get(index).map(|list| list.tokens.clone()),
        _ => None,
    };
    let mut diagnostics = Vec::new();
    for pair in words.windows(2) {
        let [left, right] = pair else {
            continue;
        };
        if child
            .as_ref()
            .is_some_and(|range| range.contains(left) || range.contains(right))
        {
            continue;
        }
        if is_common_noun(*left, tokens, out) && is_common_noun(*right, tokens, out) {
            let boundary = tokens[*left].end;
            let mut diagnostic = SyntaxDiagnostic::new(
                DiagnosticKind::ListSeparatorMissing,
                Severity::Error,
                DiagnosticConfidence::High,
                boundary..boundary,
                "list_separator_missing",
            );
            diagnostic.corrections = vec![Correction::new(
                vec![Edit::insertion(boundary, ",")],
                0.0,
                "list_separator_missing",
            )];
            diagnostics.push(diagnostic);
        }
    }
    diagnostics
}

fn is_common_noun(index: usize, tokens: &[Token], out: &[TokenSyntax]) -> bool {
    if tokens[index].is_capitalised() {
        return false;
    }
    let readings = &out[index].readings;
    let noun = readings
        .iter()
        .any(|reading| reading.part_of_speech == Some(PartOfSpeech::Noun));
    let competing = readings.iter().any(|reading| {
        matches!(
            reading.part_of_speech,
            Some(
                PartOfSpeech::Adjective
                    | PartOfSpeech::Numeral
                    | PartOfSpeech::Ordinal
                    | PartOfSpeech::Verb
                    | PartOfSpeech::NegativeVerb
            )
        )
    });
    noun && !competing
}

fn remove_comma(
    kind: DiagnosticKind,
    severity: Severity,
    start: usize,
    message: &'static str,
) -> SyntaxDiagnostic {
    let mut diagnostic = SyntaxDiagnostic::new(
        kind,
        severity,
        DiagnosticConfidence::High,
        start..start + 1,
        message,
    );
    diagnostic.corrections = vec![Correction::new(
        vec![Edit::new(start, start + 1, "")],
        0.0,
        message,
    )];
    diagnostic
}

// ── clauses ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClauseKind {
    Main,
    Subordinate,
    Relative,
    Coordinate,
    Infinitive,
}

fn clause_diagnostics(tokens: &[Token], analysis: &SyntaxAnalysis) -> Vec<SyntaxDiagnostic> {
    let mut out = Vec::new();
    for (index, clause) in analysis.clauses.iter().enumerate() {
        let kind = clause_kind(clause, tokens, analysis);
        if kind == ClauseKind::Subordinate {
            out.extend(subordinate_comma(index, clause, kind, tokens, analysis));
        }
        if kind == ClauseKind::Relative {
            out.extend(relative_comma(clause, tokens, analysis));
        }
    }
    out
}

/// A subordinate clause after a main clause, or a main clause after an initial
/// subordinate clause, needs a comma at the boundary. A merged initial
/// subordinate clause (`Koska olin väsynyt menin kotiin`) is split at its last
/// finite predicate because the clause parser could not see the boundary.
fn subordinate_comma(
    index: usize,
    clause: &Clause,
    _kind: ClauseKind,
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
) -> Vec<SyntaxDiagnostic> {
    let mut out = Vec::new();
    if index > 0 && same_sentence(&analysis.clauses[index - 1], clause, analysis) {
        let previous = &analysis.clauses[index - 1];
        if let (Some(left), Some(right)) = (last_word(previous, tokens), first_word(clause, tokens)) {
            let previous_kind = clause_kind(previous, tokens, analysis);
            let boundary_needs_comma = previous_kind != ClauseKind::Subordinate
                || clause_kind(clause, tokens, analysis) == ClauseKind::Subordinate;
            if boundary_needs_comma && !comma_between(tokens, left, right) {
                out.push(comma_missing(tokens[left].end, DiagnosticKind::CommaMissing));
            }
        }
    }
    if let Some(next) = analysis.clauses.get(index + 1)
        && same_sentence(clause, next, analysis)
        && let (Some(left), Some(right)) = (last_word(clause, tokens), first_word(next, tokens))
        && clause_kind(next, tokens, analysis) == ClauseKind::Main
        && !comma_between(tokens, left, right)
    {
        out.push(comma_missing(tokens[left].end, DiagnosticKind::CommaMissing));
    }

    // The subordinate clause may still be merged with its main clause in one
    // `Clause`: `Koska olin väsynyt menin kotiin`. Use the last finite
    // predicate as the main clause's head and insert the comma before it.
    let finite = finite_words(clause, tokens, analysis);
    if finite.len() >= 2
        && let Some(&main) = finite.last()
        && let Some(before) = previous_word(tokens, main)
        && !comma_between(tokens, before, main)
    {
        out.push(comma_missing(tokens[before].end, DiagnosticKind::CommaMissing));
    }
    out
}

/// A relative clause that already has an opening comma and is followed by its
/// host predicate needs the closing comma. A restrictive relative without an
/// opening comma is left quiet.
fn relative_comma(
    clause: &Clause,
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
) -> Vec<SyntaxDiagnostic> {
    let Some(first) = first_word(clause, tokens) else {
        return Vec::new();
    };
    let Some(antecedent) = previous_word(tokens, first) else {
        return Vec::new();
    };
    if !comma_between(tokens, antecedent, first) {
        return Vec::new();
    }
    let finite = finite_predicate_words(clause, tokens, analysis);
    if finite.len() < 2 {
        return Vec::new();
    }
    let first_finite = finite[0];
    let host = *finite.last().unwrap_or(&first_finite);
    if host == first_finite || coordinated_between(tokens, first_finite, host) {
        return Vec::new();
    }
    let Some(before) = previous_word(tokens, host) else {
        return Vec::new();
    };
    if comma_between(tokens, before, host) {
        return Vec::new();
    }
    vec![comma_missing(
        tokens[before].end,
        DiagnosticKind::CommaPairUnbalanced,
    )]
}

fn comma_missing(at: usize, kind: DiagnosticKind) -> SyntaxDiagnostic {
    let message = if kind == DiagnosticKind::CommaPairUnbalanced {
        "comma_pair_unbalanced"
    } else {
        "comma_missing"
    };
    let mut diagnostic = SyntaxDiagnostic::new(
        kind,
        Severity::Error,
        DiagnosticConfidence::High,
        at..at,
        message,
    );
    diagnostic.corrections = vec![Correction::new(
        vec![Edit::insertion(at, ",")],
        0.0,
        "comma_missing",
    )];
    diagnostic
}

fn clause_kind(clause: &Clause, tokens: &[Token], analysis: &SyntaxAnalysis) -> ClauseKind {
    let Some(first) = first_word(clause, tokens) else {
        return ClauseKind::Main;
    };
    let lower = tokens[first].text.to_lowercase();
    if crate::syntax::is_subordinator(&lower) {
        return ClauseKind::Subordinate;
    }
    if crate::syntax::is_relative_word(first, tokens, &analysis.tokens) {
        return ClauseKind::Relative;
    }
    if crate::syntax::is_coordinating(&lower) {
        return ClauseKind::Coordinate;
    }
    if is_infinitive(&analysis.tokens[first]) {
        return ClauseKind::Infinitive;
    }
    ClauseKind::Main
}

fn first_word(clause: &Clause, tokens: &[Token]) -> Option<usize> {
    clause
        .tokens
        .clone()
        .find(|&index| tokens[index].is_word())
}

fn last_word(clause: &Clause, tokens: &[Token]) -> Option<usize> {
    clause
        .tokens
        .clone()
        .rev()
        .find(|&index| tokens[index].is_word())
}

fn previous_word(tokens: &[Token], index: usize) -> Option<usize> {
    (0..index).rev().find(|&candidate| tokens[candidate].is_word())
}

fn finite_predicate_words(
    clause: &Clause,
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
) -> Vec<usize> {
    clause
        .tokens
        .clone()
        .filter(|&index| {
            tokens[index].is_word()
                && matches!(
                    analysis.tokens[index].role,
                    SyntacticRole::Predicate | SyntacticRole::Auxiliary
                )
                && is_finite(&analysis.tokens[index])
        })
        .collect()
}

fn finite_words(clause: &Clause, tokens: &[Token], analysis: &SyntaxAnalysis) -> Vec<usize> {
    clause
        .tokens
        .clone()
        .filter(|&index| tokens[index].is_word() && is_finite(&analysis.tokens[index]))
        .collect()
}

fn is_finite(syntax: &TokenSyntax) -> bool {
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
}

fn is_infinitive(syntax: &TokenSyntax) -> bool {
    syntax
        .readings
        .iter()
        .any(|reading| reading.mood == Some(Mood::Infinitive))
}

fn coordinated_between(tokens: &[Token], left: usize, right: usize) -> bool {
    let (start, end) = if left < right {
        (left + 1, right)
    } else {
        (right + 1, left)
    };
    tokens[start..end]
        .iter()
        .filter(|token| token.is_word())
        .any(|token| {
            matches!(
                token.text.to_lowercase().as_str(),
                "ja" | "sekä" | "tai" | "vai"
            )
        })
}

fn comma_between(tokens: &[Token], left: usize, right: usize) -> bool {
    let (start, end) = if left < right {
        (left + 1, right)
    } else {
        (right + 1, left)
    };
    tokens[start..end]
        .iter()
        .filter(|token| !token.is_word())
        .any(|token| token.text.contains(','))
}

fn comma_positions(tokens: &[Token], start: usize, end: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for token in tokens.iter().take(end).skip(start).filter(|token| !token.is_word()) {
        for (offset, character) in token.text.char_indices() {
            if character == ',' {
                let start = token.start + offset;
                out.push((start, start + 1));
            }
        }
    }
    out
}

// ── questions and quotes ─────────────────────────────────────────────────

/// Interrogative surfaces that open a direct question.
///
/// The inflected forms of `kuka`/`ken` and `mikä` are included because the
/// sentence opener is often `Kenen`, `Missä` or `Mitä`, not the citation form.
/// `koska` is deliberately absent: it is also the subordinator “because”.
const INTERROGATIVE_SURFACES: &[&str] = &[
    "kuka", "ken", "ketä", "kenen", "kelle", "keneltä", "kenelle", "kenessä", "kenestä",
    "kenellä", "ketkä", "keiden", "keitä", "keissä", "keistä", "keihin", "keillä", "keiltä",
    "keille", "mikä", "mitä", "minkä", "missä", "mistä", "mihin", "millä", "miltä", "mille",
    "miksi", "mitkä", "millainen", "millaisia", "millaiset", "kumpi", "kumman", "kumpaa",
    "kummat", "kumpia", "milloin", "miten", "kuinka", "montako",
];

fn question_diagnostics(tokens: &[Token], analysis: &SyntaxAnalysis) -> Vec<SyntaxDiagnostic> {
    let mut out = Vec::new();
    for sentence in &analysis.sentences {
        let clauses: Vec<&Clause> = analysis
            .clauses
            .iter()
            .filter(|clause| clause.start >= sentence.start && clause.start < sentence.end)
            .collect();
        // A question with a second clause is too ambiguous to punctuate
        // automatically (`Missä olet ja mitä teet`); stay quiet.
        if clauses.len() != 1 {
            continue;
        }
        let clause = clauses[0];
        let Some(first) = first_word(clause, tokens) else {
            continue;
        };
        let lower = tokens[first].text.to_lowercase();
        let interrogative = INTERROGATIVE_SURFACES.contains(&lower.as_str());
        let clitic = has_ko_clitic(first, analysis)
            || clause
                .predicate
                .is_some_and(|predicate| has_ko_clitic(predicate, analysis));
        if !interrogative && !clitic {
            continue;
        }
        if interrogative && is_indefinite(tokens, first) {
            continue;
        }
        if crate::syntax::is_subordinator(&lower) {
            continue;
        }
        let Some(last) = last_word_in_sentence(tokens, sentence) else {
            continue;
        };
        let edit = match terminal_punctuation(tokens, last) {
            Terminal::Question | Terminal::Skip => continue,
            Terminal::Dot(at) => Edit::new(at, at + 1, "?"),
            Terminal::Insert(at) => Edit::insertion(at, "?"),
        };
        let mut diagnostic = SyntaxDiagnostic::new(
            DiagnosticKind::QuestionMarkMissing,
            Severity::Error,
            DiagnosticConfidence::High,
            edit.start..edit.end,
            "question_mark_missing",
        );
        diagnostic.expected = Some("?".to_string());
        diagnostic.corrections = vec![Correction::new(
            vec![edit],
            0.0,
            "question_mark_missing",
        )];
        out.push(diagnostic);
    }
    out
}

fn has_ko_clitic(index: usize, analysis: &SyntaxAnalysis) -> bool {
    analysis.tokens[index]
        .readings
        .iter()
        .any(|reading| reading.clitic == Some(Clitic::Ko))
}

/// `kuka tahansa`, `mikä vain` and similar are indefinite pronouns, not
/// questions.
fn is_indefinite(tokens: &[Token], first: usize) -> bool {
    let next = tokens
        .iter()
        .skip(first + 1)
        .find(|token| token.is_word())
        .map(|token| token.text.to_lowercase());
    matches!(
        next.as_deref(),
        Some("tahansa" | "hyvänsä" | "vain" | "ikin")
    )
}

fn last_word_in_sentence(tokens: &[Token], sentence: &sentence::SentenceSpan) -> Option<usize> {
    (0..tokens.len()).rev().find(|&index| {
        tokens[index].is_word()
            && tokens[index].start >= sentence.start
            && tokens[index].start < sentence.end
    })
}

enum Terminal {
    /// The sentence already ends with `?`.
    Question,
    /// A comma, semicolon, exclamation or ellipsis: do not guess.
    Skip,
    /// A period that can be replaced.
    Dot(usize),
    /// No terminal punctuation; insert at this byte.
    Insert(usize),
}

fn terminal_punctuation(tokens: &[Token], last_word: usize) -> Terminal {
    let mut dot = None;
    let mut question = false;
    let mut skip = false;
    for token in tokens.iter().skip(last_word + 1) {
        if token.is_word() {
            break;
        }
        if token.text.contains("...") || token.text.contains('…') {
            skip = true;
        }
        for (offset, character) in token.text.char_indices() {
            match character {
                '?' => question = true,
                '!' => skip = true,
                ',' | ';' | ':' => skip = true,
                '.' if dot.is_none() => dot = Some(token.start + offset),
                _ => {}
            }
        }
    }
    if question {
        Terminal::Question
    } else if skip {
        Terminal::Skip
    } else if let Some(at) = dot {
        Terminal::Dot(at)
    } else {
        Terminal::Insert(tokens[last_word].end)
    }
}

/// One warning for a quote left open at the end of the document.
fn quote_diagnostics(
    tokens: &[Token],
    contexts: &[sentence::TokenContext],
) -> Vec<SyntaxDiagnostic> {
    let depth = contexts
        .last()
        .map(|context| context.quote_depth)
        .unwrap_or(0);
    if depth == 0 {
        return Vec::new();
    }
    let at = tokens.last().map(|token| token.end).unwrap_or(0);
    vec![SyntaxDiagnostic::new(
        DiagnosticKind::UnbalancedQuote,
        Severity::Warning,
        DiagnosticConfidence::Medium,
        at..at,
        "unbalanced_quote",
    )]
}

/// Adverbs that are clearly parenthetical on their own.
///
/// `kuten` is absent because its parenthetical is the following phrase, not
/// the cue word: `Omena, kuten päärynä, on hedelmä`. A pair around `kuten`
/// would be wrong, and phrase extent is a later rule.
const PARENTHETICAL_CUES: &[&str] = &["esimerkiksi"];

fn parenthetical_diagnostics(
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
    contexts: &[sentence::TokenContext],
) -> Vec<SyntaxDiagnostic> {
    let mut out = Vec::new();
    for clause in &analysis.clauses {
        let words: Vec<usize> = clause
            .tokens
            .clone()
            .filter(|&index| tokens[index].is_word())
            .collect();
        for (position, &index) in words.iter().enumerate() {
            let lower = tokens[index].text.to_lowercase();
            if !PARENTHETICAL_CUES.contains(&lower.as_str()) {
                continue;
            }
            if position == 0 || position + 1 >= words.len() {
                continue;
            }
            if inside_brackets(tokens, clause.tokens.start, index)
                || contexts
                    .get(index)
                    .is_some_and(|context| context.quote_depth > 0)
            {
                continue;
            }
            let left = words[position - 1];
            let right = words[position + 1];
            if comma_between(tokens, left, index) || comma_between(tokens, index, right) {
                continue;
            }
            let mut diagnostic = SyntaxDiagnostic::new(
                DiagnosticKind::CommaMissing,
                Severity::Suggestion,
                DiagnosticConfidence::Medium,
                tokens[index].start..tokens[index].end,
                "parenthetical_comma",
            );
            diagnostic.corrections = vec![Correction::new(
                vec![
                    Edit::insertion(tokens[left].end, ","),
                    Edit::insertion(tokens[index].end, ","),
                ],
                0.0,
                "parenthetical_comma",
            )];
            out.push(diagnostic);
        }
    }
    out
}

fn inside_brackets(tokens: &[Token], start: usize, index: usize) -> bool {
    let mut depth = 0i32;
    for token in tokens.iter().take(index).skip(start) {
        if token.is_word() {
            continue;
        }
        for character in token.text.chars() {
            match character {
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => depth = (depth - 1).max(0),
                _ => {}
            }
        }
    }
    depth > 0
}

fn same_sentence(left: &Clause, right: &Clause, analysis: &SyntaxAnalysis) -> bool {
    sentence_index(left.start, analysis) == sentence_index(right.start, analysis)
}

fn sentence_index(byte: usize, analysis: &SyntaxAnalysis) -> Option<usize> {
    analysis
        .sentences
        .iter()
        .find(|sentence| byte >= sentence.start && byte < sentence.end)
        .map(|sentence| sentence.index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenise::tokenise;

    fn diagnostics(text: &str) -> Vec<SyntaxDiagnostic> {
        let tokens = tokenise(text);
        let analysis = crate::syntax::analyze(&tokens);
        check(&tokens, &analysis)
    }

    fn kind(text: &str, wanted: DiagnosticKind) -> Vec<SyntaxDiagnostic> {
        diagnostics(text)
            .into_iter()
            .filter(|diagnostic| diagnostic.kind == wanted)
            .collect()
    }

    #[test]
    fn a_missing_question_mark_is_an_error() {
        let found = kind("Missä sinä asut.", DiagnosticKind::QuestionMarkMissing);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].severity, Severity::Error);
        assert_eq!(found[0].corrections[0].edits[0].replacement, "?");
    }

    #[test]
    fn a_ko_question_without_a_mark_is_an_error() {
        let found = kind(
            "Onko sinulla kirjaa.",
            DiagnosticKind::QuestionMarkMissing,
        );
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn indirect_questions_and_existing_marks_stay_quiet() {
        for text in [
            "Kysyin, missä hän asuu.",
            "Hän sanoi: »Missä sinä asut?»",
            "Missä sinä asut?",
        ] {
            assert!(
                kind(text, DiagnosticKind::QuestionMarkMissing).is_empty(),
                "{text}"
            );
        }
    }

    #[test]
    fn a_quoted_question_gets_the_mark_inside_the_quote() {
        let found = kind(
            "»Missä sinä asut.»",
            DiagnosticKind::QuestionMarkMissing,
        );
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].corrections[0].edits[0].start, 19);
        assert_eq!(found[0].corrections[0].edits[0].replacement, "?");
    }

    #[test]
    fn an_indefinite_pronoun_is_not_a_question() {
        assert!(
            kind("Kuka tahansa voi tulla.", DiagnosticKind::QuestionMarkMissing).is_empty()
        );
    }

    #[test]
    fn a_semicolon_is_not_a_punctuation_error() {
        assert!(diagnostics("Hän tuli; minä lähdin.").is_empty());
    }

    #[test]
    fn an_unbalanced_quote_is_a_warning() {
        let found = kind("Hän sanoi: »Elä tule.", DiagnosticKind::UnbalancedQuote);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].severity, Severity::Warning);
    }

    #[test]
    fn balanced_quotes_are_quiet() {
        assert!(kind("Hän sanoi: »Elä tule.»", DiagnosticKind::UnbalancedQuote).is_empty());
    }

    #[test]
    fn an_unmarked_parenthetical_adverb_gets_a_pair() {
        let found = kind("Omena esimerkiksi on hyvä.", DiagnosticKind::CommaMissing);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].severity, Severity::Suggestion);
        assert_eq!(found[0].corrections[0].edits.len(), 2);
        assert_eq!(found[0].corrections[0].edits[0].replacement, ",");
        assert_eq!(found[0].corrections[0].edits[1].replacement, ",");
    }

    #[test]
    fn a_marked_parenthetical_adverb_is_quiet() {
        assert!(diagnostics("Omena, esimerkiksi, on hyvä.").is_empty());
        assert!(diagnostics("Esimerkiksi omena on hyvä.").is_empty());
    }
}
