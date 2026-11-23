//! Vocative and address comma diagnostics.
//!
//! A form of address is a nominal that is not an argument of the clause. The
//! rules here are deliberately narrow: they only fire on the two positions the
//! morphology can identify without a parser, a sentence-initial greeting and an
//! address next to an imperative. Every finding is a `Suggestion`: an address
//! is a matter of intended structure, and the checker must not overstate it.

use crate::syntax::{Clause, SyntacticRole, SyntaxAnalysis};
use crate::tags::{Mood, PartOfSpeech, Person, Polarity};
use crate::tokenise::Token;

use super::super::analysis::{first_word, is_finite, is_imperative, last_word, looks_like_address};
use super::super::diagnostic::{
    Confidence, Correction, CorrectionLicense, DiagnosticKind, Edit, Severity, SyntaxDiagnostic,
};

const GREETINGS: &[&str] = &[
    "hei",
    "terve",
    "moi",
    "moikka",
    "moro",
    "morjens",
    "kiitos",
    "tervehdys",
];

/// The second half of `Hyvää huomenta` / `Hyvää päivää`.
const GOOD_DAY_WORDS: &[&str] = &["huomenta", "päivää", "iltaa", "yötä"];

const PERSONAL_PRONOUNS: &[&str] = &["minä", "sinä", "hän", "me", "te", "he"];

/// Negative imperatives: `Elä koskaan tee` is `älä` + connegative, not a
/// positive command whose object could be an address.
const NEGATIVE_IMPERATIVES: &[&str] = &["elä", "älä"];

pub(super) fn vocative_diagnostics(
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
) -> Vec<SyntaxDiagnostic> {
    let mut out = greeting_comma(tokens, analysis);
    out.extend(leading_address_comma(tokens, analysis));
    out.extend(trailing_address_comma(tokens, analysis));
    out
}

/// `Hei Maija!`, `Terve Pekka!`, `Hyvää huomenta kaikki.` — a sentence-initial
/// greeting followed directly by the person addressed.
fn greeting_comma(tokens: &[Token], analysis: &SyntaxAnalysis) -> Vec<SyntaxDiagnostic> {
    let mut out = Vec::new();
    for (index, clause) in analysis.clauses.iter().enumerate() {
        if !is_first_clause_in_sentence(index, &analysis.clauses, analysis) {
            continue;
        }
        let words: Vec<usize> = clause
            .tokens
            .clone()
            .filter(|&token| tokens[token].is_word())
            .collect();
        let Some(&first) = words.first() else {
            continue;
        };

        // `Hyvää huomenta, kaikki.` is one greeting in two words.
        if tokens[first].text.eq_ignore_ascii_case("hyvää") {
            let (Some(&second), Some(&address)) = (words.get(1), words.get(2)) else {
                continue;
            };
            if GOOD_DAY_WORDS.contains(&crate::text::lowercase(&tokens[second].text).as_ref())
                && is_greeting_address(address, tokens, analysis)
                && plain_space_between(tokens, second, address)
            {
                out.push(vocative_missing(tokens[second].end, "greeting"));
            }
            continue;
        }

        if !GREETINGS.contains(&crate::text::lowercase(&tokens[first].text).as_ref()) {
            continue;
        }
        let Some(&next) = words.get(1) else {
            continue;
        };
        if is_greeting_address(next, tokens, analysis) && plain_space_between(tokens, first, next) {
            out.push(vocative_missing(tokens[first].end, "greeting"));
        }
    }
    out
}

/// `Maija tule tänne.` / `Pekka tule!` — an address before an imperative.
fn leading_address_comma(tokens: &[Token], analysis: &SyntaxAnalysis) -> Vec<SyntaxDiagnostic> {
    let mut out = Vec::new();
    for clause in &analysis.clauses {
        if clause.negative {
            continue;
        }
        let Some(predicate) = clause.predicate else {
            continue;
        };
        if !is_imperative_predicate(predicate, analysis)
            || NEGATIVE_IMPERATIVES.contains(&crate::text::lowercase(&tokens[predicate].text).as_ref())
        {
            continue;
        }
        let words: Vec<usize> = clause
            .tokens
            .clone()
            .filter(|&token| tokens[token].is_word())
            .collect();
        let (Some(&first), Some(&second)) = (words.first(), words.get(1)) else {
            continue;
        };
        if second != predicate
            || !same_sentence(predicate, first, tokens, analysis)
            || !matches!(
                analysis.tokens[first].role,
                SyntacticRole::Subject | SyntacticRole::Unknown
            )
            || !is_leading_address(first, tokens, analysis)
            || !plain_space_between(tokens, first, predicate)
        {
            continue;
        }
        out.push(vocative_missing(tokens[first].end, "leading"));
    }
    out
}

/// `Tule tänne Pekka.` / `Tule Pekka!` — an address after an imperative.
fn trailing_address_comma(tokens: &[Token], analysis: &SyntaxAnalysis) -> Vec<SyntaxDiagnostic> {
    let mut out = Vec::new();
    for clause in &analysis.clauses {
        if clause.negative {
            continue;
        }
        let Some(predicate) = clause.predicate else {
            continue;
        };
        if !is_imperative_predicate(predicate, analysis)
            || NEGATIVE_IMPERATIVES.contains(&crate::text::lowercase(&tokens[predicate].text).as_ref())
        {
            continue;
        }
        let (Some(first), Some(last)) = (first_word(clause, tokens), last_word(clause, tokens))
        else {
            continue;
        };
        if first == last || last == predicate {
            continue;
        }
        // An object (`Tapaa Pekka!`) keeps its argument role; an address is
        // left without one.
        if analysis.tokens[last].role != SyntacticRole::Unknown {
            continue;
        }
        if !is_trailing_address(last, tokens, analysis) {
            continue;
        }
        let Some(before) = (first..last).rev().find(|&token| tokens[token].is_word()) else {
            continue;
        };
        // `näe minun käteni` and `ilman Bathsebaa`: a possessive object and an
        // adposition complement are arguments, not addresses.
        if !same_sentence(predicate, last, tokens, analysis)
            || is_conjunction(before, analysis)
            || is_adposition(before, analysis)
            || analysis.tokens[before].head == Some(last)
            || !plain_space_between(tokens, before, last)
        {
            continue;
        }
        out.push(vocative_missing(tokens[before].end, "trailing"));
    }
    out
}

fn vocative_missing(at: usize, subrule: &str) -> SyntaxDiagnostic {
    let mut diagnostic = SyntaxDiagnostic::new(
        DiagnosticKind::CommaMissing,
        Severity::Suggestion,
        Confidence::Likely,
        at..at,
        "vocative_comma",
    );
    diagnostic.expected = Some(subrule.to_string());
    diagnostic.corrections = vec![
        Correction::new(vec![Edit::insertion(at, ",")], 0.0, "comma_missing")
            .licensed_by(CorrectionLicense::Punctuation)
            .with_confidence(Confidence::Likely),
    ];
    diagnostic
}

/// An imperative predicate, not an adjective that happens to share the
/// surface (`Kaksi pieni lapsi.` is not an address).
fn is_imperative_predicate(index: usize, analysis: &SyntaxAnalysis) -> bool {
    let syntax = &analysis.tokens[index];
    // Only a second-person imperative can be addressed. `Herra olkoon
    // kanssanne` and `Jumala johdattakoon` are third-person jussives whose
    // subject is the first word, not an address.
    is_imperative(syntax)
        && syntax
            .readings
            .iter()
            .any(|reading| reading.mood == Some(Mood::Imperative) && reading.person == Some(Person::Second))
        // `Kunta saa periä maksun` has an indicative `saa` and a 2sg
        // imperative homonym. A predicate that can be read as a finite
        // indicative is not an imperative for an address.
        && !syntax.readings.iter().any(|reading| {
            reading.part_of_speech == Some(PartOfSpeech::Verb)
                && reading.polarity == Polarity::Affirmative
                && matches!(
                    reading.mood,
                    Some(Mood::Indicative | Mood::Conditional | Mood::Potential)
                )
        })
        && !syntax.readings.iter().any(|reading| {
            matches!(
                reading.part_of_speech,
                Some(
                    PartOfSpeech::Adjective
                        | PartOfSpeech::Adverb
                        | PartOfSpeech::Pronoun
                        | PartOfSpeech::Preposition
                        | PartOfSpeech::Postposition
                        | PartOfSpeech::Conjunction
                        | PartOfSpeech::NegativeVerb
                )
            )
        })
        // An infinitive/instructive homonym (`hävetä`, `elää`) is not the
        // clause's imperative predicate.
        && !syntax.readings.iter().any(|reading| {
            matches!(reading.mood, Some(Mood::Infinitive | Mood::Instructive))
        })
}

fn is_address(index: usize, tokens: &[Token], analysis: &SyntaxAnalysis) -> bool {
    looks_like_address(index, tokens, analysis)
}

/// A greeting can address a personal pronoun (`Hei sinä!`), which the
/// argument-position guard deliberately rejects.
fn is_greeting_address(index: usize, tokens: &[Token], analysis: &SyntaxAnalysis) -> bool {
    is_address(index, tokens, analysis)
        || PERSONAL_PRONOUNS.contains(&crate::text::lowercase(&tokens[index].text).as_ref())
}

/// A trailing address must be a name or a non-personal pronoun. A possessive
/// noun is usually an object (`näe minun käteni`), so it is not a cue here.
fn is_trailing_address(index: usize, tokens: &[Token], analysis: &SyntaxAnalysis) -> bool {
    if !is_address(index, tokens, analysis) {
        return false;
    }
    tokens[index].is_capitalised() || tokens[index].text.eq_ignore_ascii_case("kaikki")
}

fn is_adposition(index: usize, analysis: &SyntaxAnalysis) -> bool {
    analysis.tokens[index].readings.iter().any(|reading| {
        matches!(
            reading.part_of_speech,
            Some(PartOfSpeech::Preposition | PartOfSpeech::Postposition)
        )
    })
}

fn is_conjunction(index: usize, analysis: &SyntaxAnalysis) -> bool {
    analysis.tokens[index]
        .readings
        .iter()
        .any(|reading| reading.part_of_speech == Some(PartOfSpeech::Conjunction))
}

/// The first word of an imperative is an address only when it is nominal. A
/// personal pronoun is excluded: `Te menkää!` and `Me menkäämme!` have a
/// subject, not an address.
fn is_leading_address(index: usize, tokens: &[Token], analysis: &SyntaxAnalysis) -> bool {
    let key = crate::text::lowercase(&tokens[index].text);
    if PERSONAL_PRONOUNS.contains(&key.as_ref()) || is_finite(&analysis.tokens[index]) {
        return false;
    }
    let syntax = &analysis.tokens[index];
    if syntax
        .readings
        .iter()
        .any(|reading| reading.part_of_speech == Some(PartOfSpeech::Adverb))
    {
        return false;
    }
    syntax.readings.iter().any(|reading| {
        matches!(
            reading.part_of_speech,
            Some(PartOfSpeech::Noun | PartOfSpeech::Adjective | PartOfSpeech::Pronoun)
        )
    }) || (syntax.readings.is_empty() && tokens[index].is_capitalised())
}

/// Only whitespace may sit between the greeting/address and the next word.
fn plain_space_between(tokens: &[Token], left: usize, right: usize) -> bool {
    if left >= right {
        return false;
    }
    tokens[left + 1..right]
        .iter()
        .filter(|token| !token.is_word())
        .all(|token| token.text.trim().is_empty() && !token.text.contains('\n'))
}

fn same_sentence(left: usize, right: usize, tokens: &[Token], analysis: &SyntaxAnalysis) -> bool {
    sentence_of(tokens[left].start, analysis) == sentence_of(tokens[right].start, analysis)
}

fn is_first_clause_in_sentence(
    index: usize,
    clauses: &[Clause],
    analysis: &SyntaxAnalysis,
) -> bool {
    let Some(sentence) = sentence_of(clauses[index].start, analysis) else {
        return false;
    };
    !clauses[..index]
        .iter()
        .any(|clause| sentence_of(clause.start, analysis) == Some(sentence))
}

fn sentence_of(byte: usize, analysis: &SyntaxAnalysis) -> Option<usize> {
    analysis
        .sentences
        .iter()
        .find(|sentence| byte >= sentence.start && byte < sentence.end)
        .map(|sentence| sentence.index)
}
