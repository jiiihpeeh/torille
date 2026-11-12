//! Missing question marks.

use crate::sentence;
use crate::syntax::{Clause, SyntaxAnalysis};
use crate::tags::Clitic;
use crate::tokenise::Token;

use super::super::analysis::first_word;
use super::super::diagnostic::{
    Correction, DiagnosticConfidence, DiagnosticKind, Edit, Severity, SyntaxDiagnostic,
};

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

pub(super) fn question_diagnostics(tokens: &[Token], analysis: &SyntaxAnalysis) -> Vec<SyntaxDiagnostic> {
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
