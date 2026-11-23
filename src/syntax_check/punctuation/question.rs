//! Missing question marks.

use crate::sentence;
use crate::syntax::{Clause, SyntaxAnalysis};
use crate::tags::Clitic;
use crate::tokenise::Token;

use super::clause::{ClauseKind, clause_kind};
use super::super::analysis::first_word;
use super::super::diagnostic::{
    Confidence, Correction, CorrectionLicense, DiagnosticKind, Edit, Severity, SyntaxDiagnostic,
};

/// Interrogative surfaces that open a direct question.
///
/// The list lives in the syntax layer because the clause features and the
/// missing-question-mark rule must agree on what opens a wh-question.
use crate::syntax::INTERROGATIVE_SURFACES;

pub(super) fn question_diagnostics(tokens: &[Token], analysis: &SyntaxAnalysis) -> Vec<SyntaxDiagnostic> {
    let mut out = Vec::new();
    for sentence in &analysis.sentences {
        let clauses: Vec<&Clause> = analysis
            .clauses
            .iter()
            .filter(|clause| clause.start >= sentence.start && clause.start < sentence.end)
            .collect();
        // A question with a second main or subordinate clause is too
        // ambiguous to punctuate automatically (`Missä olet ja mitä teet`,
        // `Mitä ... tulisinkaan ..., jos ...`). Relative clauses belong to the
        // question and do not make the final mark ambiguous
        // (`Muistatko ... kirjan, joka ...`).
        if clauses.is_empty()
            || clauses
                .iter()
                .skip(1)
                .any(|clause| clause_kind(clause, tokens, analysis) != ClauseKind::Relative)
        {
            continue;
        }
        let clause = clauses[0];
        // A correlative relative (`Minkä kerran täytyy tapahtua, se
        // tapahtuu.`) starts with a relative pronoun, not a direct question.
        if is_correlative_relative(&clauses, tokens) {
            continue;
        }
        let Some(first) = first_word(clause, tokens) else {
            continue;
        };
        // The interrogative must open the sentence, not merely its clause:
        // `Hän kysyi, mitä minä teen.` is reported speech. A line break can
        // also start a sentence mid-clause (`...,\nmiten ...`); a comma
        // immediately before the sentence marks that continuation.
        let Some(sentence_first) = tokens
            .iter()
            .position(|token| token.is_word() && token.start >= sentence.start)
        else {
            continue;
        };
        if sentence_first != first || preceded_by_comma(tokens, first) {
            continue;
        }
        let lower = crate::text::lowercase(&tokens[first].text);
        let interrogative = INTERROGATIVE_SURFACES.contains(&lower.as_ref());
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
            Confidence::Certain,
            edit.start..edit.end,
            "question_mark_missing",
        );
        diagnostic.expected = Some("?".to_string());
        diagnostic.corrections = vec![Correction::new(
            vec![edit],
            0.0,
            "question_mark_missing",
        )
        .licensed_by(CorrectionLicense::Punctuation)
        .with_confidence(Confidence::Certain)];
        out.push(diagnostic);
    }
    out
}

/// A relative pronoun with a following demonstrative resumptive
/// (`Minkä ... , se ...`) is a correlative construction, not a direct question.
fn is_correlative_relative(clauses: &[&Clause], tokens: &[Token]) -> bool {
    if clauses.len() < 2 {
        return false;
    }
    let Some(first) = first_word(clauses[0], tokens) else {
        return false;
    };
    let lower = crate::text::lowercase(&tokens[first].text);
    if !matches!(
        lower.as_ref(),
        "minkä" | "mikä" | "joka" | "jonka" | "jotka" | "mitä" | "jossa" | "josta"
    ) {
        return false;
    }
    clauses.iter().skip(1).any(|clause| {
        clause.subject.is_some_and(|subject| {
            matches!(
                crate::text::lowercase(&tokens[subject].text).as_ref(),
                "se" | "ne" | "tämä" | "nuo" | "hän" | "he"
            )
        })
    })
}

fn has_ko_clitic(index: usize, analysis: &SyntaxAnalysis) -> bool {
    analysis.tokens[index]
        .readings
        .iter()
        .any(|reading| reading.clitic == Some(Clitic::Ko))
}

/// A comma in the separator run before the word: the sentence started after
/// a line break inside a continuing clause, so the interrogative is reported,
/// not direct.
fn preceded_by_comma(tokens: &[Token], first: usize) -> bool {
    for token in tokens[..first].iter().rev() {
        if token.is_word() {
            break;
        }
        if token.text.contains(',') {
            return true;
        }
    }
    false
}

/// `kuka tahansa`, `mikä vain` and similar are indefinite pronouns, not
/// questions.
fn is_indefinite(tokens: &[Token], first: usize) -> bool {
    let next = tokens
        .iter()
        .skip(first + 1)
        .find(|token| token.is_word())
        .map(|token| crate::text::lowercase(&token.text));
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
    let mut newline = false;
    let mut next_word = false;
    let mut next_lower = false;
    for token in tokens.iter().skip(last_word + 1) {
        if token.is_word() {
            next_word = true;
            next_lower = token
                .text
                .chars()
                .next()
                .is_some_and(|character| character.is_lowercase());
            break;
        }
        if token.text.contains('\n') {
            newline = true;
        }
        if token.text.contains("...") || token.text.contains('…') {
            skip = true;
        }
        for (offset, character) in token.text.char_indices() {
            match character {
                '?' => question = true,
                '!' | ',' | ';' | ':' => skip = true,
                // A dash is an interruption, not a missing terminal mark:
                // `Muistatko--muistatko sen kirjan.` is a deliberate break,
                // not a question that ends at the dash.
                '-' | '–' | '—' => skip = true,
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
        // A period at the end of a wrapped line is a real terminal mark even
        // when the next line starts lowercase.
        Terminal::Dot(at)
    } else if newline && (!next_word || next_lower) {
        // The sentence splitter broke at a line wrap inside a direct
        // question (`Voitteko ... tehdä ...?`); the mark belongs at the end
        // of the continued sentence, not at the wrap. The per-sentence cache
        // sees only the slice, so a trailing newline with no next word is
        // treated the same way.
        Terminal::Skip
    } else {
        Terminal::Insert(tokens[last_word].end)
    }
}
