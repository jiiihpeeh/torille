//! Subject–predicate agreement.
//!
//! The analysis layer already points a clause at its subject and predicate.
//! This module finds the **finite agreement bearer** (the main verb, the `olla`
//! auxiliary of a compound tense, or the negative auxiliary), reads the
//! subject's person and number, and compares them.
//!
//! The rule is deliberately conservative. Existential, passive, necessive and
//! numeral constructions have their own agreement behaviour and are skipped:
//! `Pöydällä on kirjat`, `Talo on rakennettu`, `Minun täytyy mennä` and
//! `Kaksi miestä tuli` are all correct.

use crate::syntax::{RoleConfidence, SyntacticRole, SyntaxAnalysis, TokenSyntax};
use crate::tags::{Case, Mood, Number, PartOfSpeech, Person, Polarity, Tags, Voice};
use crate::tokenise::Token;

use super::correction;
use super::diagnostic::{
    DiagnosticConfidence, DiagnosticKind, Severity, SyntaxDiagnostic,
};

#[derive(Debug, Clone, Copy)]
struct Subject {
    index: usize,
    person: Person,
    number: Number,
}

#[derive(Debug, Clone)]
struct Bearer {
    index: usize,
    reading: Tags,
    person: Person,
    number: Number,
}

pub(super) fn check(tokens: &[Token], analysis: &SyntaxAnalysis) -> Vec<SyntaxDiagnostic> {
    let mut diagnostics = check_subject_predicate(tokens, analysis);
    diagnostics.extend(check_attributes(tokens, analysis));
    diagnostics
}

fn check_subject_predicate(
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
) -> Vec<SyntaxDiagnostic> {
    let mut diagnostics = Vec::new();
    for clause in &analysis.clauses {
        // Existential plural subjects take a singular verb.
        if clause.existential {
            continue;
        }
        // A merged initial subordinate clause (`Koska olin väsynyt menin
        // kotiin`) has unreliable roles: the complement of the subordinate
        // verb looks like the main subject. The punctuation rule handles the
        // missing comma; agreement stays out of it.
        if starts_with_subordinator(clause, tokens)
            && clause
                .tokens
                .clone()
                .filter(|&index| tokens[index].is_word())
                .filter(|&index| finite_reading(&analysis.tokens[index]).is_some())
                .count()
                >= 2
        {
            continue;
        }
        let Some(subject) = subject_of(clause, tokens, analysis) else {
            continue;
        };
        let Some(bearer) = bearer_of(clause, tokens, analysis) else {
            continue;
        };
        // A post-predicate subject is a presentation/existential order
        // (`jossa roihusi tuli`) or a misread object (`Ostin kolme asiaa`),
        // not a normal agreement controller. Existential clauses are already
        // skipped above.
        if subject.index > bearer.index {
            continue;
        }
        if bearer.reading.voice == Some(Voice::Passive)
            || bearer.reading.mood == Some(Mood::Imperative)
        {
            continue;
        }
        if subject.person == bearer.person && subject.number == bearer.number {
            continue;
        }

        let confident = analysis.tokens[subject.index].confidence == RoleConfidence::Likely
            && analysis.tokens[bearer.index].confidence == RoleConfidence::Likely;
        let (severity, confidence) = if confident {
            (Severity::Error, DiagnosticConfidence::High)
        } else {
            (Severity::Warning, DiagnosticConfidence::Medium)
        };
        let token = &tokens[bearer.index];
        let original = token.text.as_str();
        let corrections = if bearer.reading.part_of_speech == Some(PartOfSpeech::NegativeVerb) {
            correction::negative_auxiliary_correction(
                token.start,
                token.end,
                original,
                subject.person,
                subject.number,
            )
            .into_iter()
            .collect()
        } else {
            let Some(lemma) = analysis.tokens[bearer.index].lemma.as_deref() else {
                continue;
            };
            correction::verb_agreement_corrections(
                lemma,
                token,
                subject.person,
                subject.number,
                &bearer.reading,
                crate::spec::Catalogues::embedded(),
            )
        };

        let mut diagnostic = SyntaxDiagnostic::new(
            DiagnosticKind::SubjectPredicateAgreement,
            severity,
            confidence,
            token.start..token.end,
            "subject_predicate_agreement",
        );
        diagnostic.expected = Some(format!(
            "{} {}",
            subject.person.name(),
            subject.number.name()
        ));
        diagnostic.corrections = corrections;
        diagnostics.push(diagnostic);
    }
    diagnostics
}

/// Attributive adjective, ordinal, and demonstrative agreement.
///
/// The attribute must have an unambiguous case/number reading and must not be
/// a genitive modifier (`Punaisen auton` is not an agreement error), a
/// cardinal numeral (`kaksi miestä` governs the partitive), or a comparative/
/// superlative (`punaisempi auto`). Only then is the head's case/number a fair
/// comparison.
fn check_attributes(tokens: &[Token], analysis: &SyntaxAnalysis) -> Vec<SyntaxDiagnostic> {
    let mut diagnostics = Vec::new();
    for (index, syntax) in analysis.tokens.iter().enumerate() {
        let predicate_less = analysis
            .clauses
            .get(syntax.clause)
            .is_none_or(|clause| clause.predicate.is_none());
        if !tokens[index].is_word()
            || !(matches!(
                syntax.role,
                SyntacticRole::Modifier | SyntacticRole::Unknown
            ) || (predicate_less && syntax.role == SyntacticRole::Adverbial))
        {
            continue;
        }
        if crate::expand::degree_base(&tokens[index].text.to_lowercase()).is_some() {
            continue;
        }
        let Some((attribute_case, attribute_number)) = attribute_shape(syntax) else {
            continue;
        };
        if attribute_case == Case::Genitive {
            continue;
        }
        let Some(head) = attribute_head(index, syntax, tokens, analysis) else {
            continue;
        };
        let Some((head_case, head_number)) = head_shape(&analysis.tokens[head]) else {
            continue;
        };
        if attribute_case == head_case && attribute_number == head_number {
            continue;
        }
        let Some(pos) = attribute_pos(syntax) else {
            continue;
        };
        let Some(lemma) = syntax.lemma.as_deref() else {
            continue;
        };
        let token = &tokens[index];
        let corrections = correction::attribute_agreement_corrections(
            lemma,
            token,
            head_case,
            head_number,
            pos,
            crate::spec::Catalogues::embedded(),
        );
        let number_mismatch = attribute_number != head_number;
        let (severity, confidence) = if number_mismatch {
            (Severity::Error, DiagnosticConfidence::High)
        } else {
            (Severity::Warning, DiagnosticConfidence::Medium)
        };
        let mut diagnostic = SyntaxDiagnostic::new(
            DiagnosticKind::AttributeAgreement,
            severity,
            confidence,
            token.start..token.end,
            "attribute_agreement",
        );
        diagnostic.expected = Some(format!("{} {}", head_case.name(), head_number.name()));
        diagnostic.corrections = corrections;
        diagnostics.push(diagnostic);
    }
    diagnostics
}

fn attribute_shape(token: &TokenSyntax) -> Option<(Case, Number)> {
    let mut shapes: Vec<(Case, Number)> = token
        .readings
        .iter()
        .filter(|reading| is_attribute_pos(reading.part_of_speech))
        .filter_map(|reading| Some((reading.case?, reading.number?)))
        .collect();
    shapes.sort_unstable();
    shapes.dedup();
    match shapes.as_slice() {
        [shape] => Some(*shape),
        _ => None,
    }
}

fn head_shape(token: &TokenSyntax) -> Option<(Case, Number)> {
    let mut shapes: Vec<(Case, Number)> = token
        .readings
        .iter()
        .filter(|reading| is_head_pos(reading.part_of_speech))
        .filter_map(|reading| Some((reading.case?, reading.number?)))
        .collect();
    shapes.sort_unstable();
    shapes.dedup();
    match shapes.as_slice() {
        [shape] => Some(*shape),
        _ => None,
    }
}

fn attribute_pos(token: &TokenSyntax) -> Option<PartOfSpeech> {
    token
        .readings
        .iter()
        .find_map(|reading| {
            is_attribute_pos(reading.part_of_speech).then_some(reading.part_of_speech?)
        })
}

fn is_attribute_pos(pos: Option<PartOfSpeech>) -> bool {
    matches!(
        pos,
        Some(PartOfSpeech::Adjective | PartOfSpeech::Ordinal | PartOfSpeech::Pronoun)
    )
}

fn is_head_pos(pos: Option<PartOfSpeech>) -> bool {
    matches!(
        pos,
        Some(PartOfSpeech::Noun | PartOfSpeech::Adjective | PartOfSpeech::Pronoun)
    )
}

/// The head of an attribute: the parser's link when there is one, otherwise the
/// next word in the same clause (a predicate-less phrase such as
/// `Punainen autot`).
fn attribute_head(
    index: usize,
    syntax: &TokenSyntax,
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
) -> Option<usize> {
    if let Some(head) = syntax.head
        && head != index
        && tokens.get(head).is_some_and(Token::is_word)
    {
        return Some(head);
    }
    let clause = analysis.clauses.get(syntax.clause)?;
    clause
        .tokens
        .clone()
        .find(|&candidate| candidate > index && tokens[candidate].is_word())
}

fn starts_with_subordinator(clause: &crate::syntax::Clause, tokens: &[Token]) -> bool {
    clause
        .tokens
        .clone()
        .find(|&index| tokens[index].is_word())
        .is_some_and(|index| {
            crate::syntax::is_subordinator(&tokens[index].text.to_lowercase())
        })
}

/// The subject of a clause, including a shared subject from a preceding
/// clause and coordinated subjects inside this one.
fn subject_of(
    clause: &crate::syntax::Clause,
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
) -> Option<Subject> {
    let mut indices: Vec<usize> = Vec::new();
    if let Some(index) = clause.subject
        && tokens.get(index).is_some_and(Token::is_word)
    {
        indices.push(index);
        // A shared subject can bring its coordinated partners from the
        // clause it came from: `Sami ja Liisa söivät ja joivat` must stay
        // plural in the second clause.
        if let Some(source) = analysis
            .clauses
            .iter()
            .find(|source| source.tokens.contains(&index))
        {
            for other in source.tokens.clone() {
                if other != index
                    && tokens[other].is_word()
                    && analysis.tokens[other].role == SyntacticRole::Subject
                    && !indices.contains(&other)
                {
                    indices.push(other);
                }
            }
        }
    }
    for index in clause.tokens.clone() {
        if index != clause.subject.unwrap_or(usize::MAX)
            && tokens[index].is_word()
            && analysis.tokens[index].role == SyntacticRole::Subject
            && !indices.contains(&index)
        {
            indices.push(index);
        }
    }
    if indices.is_empty() {
        return None;
    }

    // Genitive subjects belong to necessive constructions (`Minun on
    // mentävä`, `Minun täytyy mennä`) and do not agree with the predicate.
    if indices
        .iter()
        .any(|&index| analysis.tokens[index].case == Some(Case::Genitive))
    {
        return None;
    }

    if indices.len() > 1 {
        if !has_coordinator(tokens, indices[0], *indices.last()?) {
            return None;
        }
        // Coordinated first/second-person subjects need a person-resolution
        // rule that this phase does not have. Do not guess.
        if indices.iter().any(|&index| {
            matches!(
                pronoun_person_number(&tokens[index].text),
                Some((Person::First | Person::Second, _))
            )
        }) {
            return None;
        }
        return Some(Subject {
            index: indices[0],
            person: Person::Third,
            number: Number::Plural,
        });
    }

    let index = indices[0];
    let token = &analysis.tokens[index];
    if let Some((person, number)) = pronoun_person_number(&tokens[index].text) {
        return Some(Subject {
            index,
            person,
            number,
        });
    }

    let mut numbers: Vec<Number> = selected_readings(token)
        .filter_map(|reading| reading.number)
        .collect();
    numbers.sort_unstable();
    numbers.dedup();
    let number = match numbers.as_slice() {
        [number] => *number,
        _ => return None,
    };
    if number == Number::Collective {
        return None;
    }
    let mut persons: Vec<Person> = selected_readings(token)
        .filter_map(|reading| reading.person)
        .collect();
    persons.sort_unstable();
    persons.dedup();
    let person = match persons.as_slice() {
        [person] => *person,
        [] => Person::Third,
        _ => return None,
    };
    Some(Subject {
        index,
        person,
        number,
    })
}

/// The finite agreement bearer: the negative auxiliary first, then the `olla`
/// auxiliary of a compound tense, then the predicate itself.
fn bearer_of(
    clause: &crate::syntax::Clause,
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
) -> Option<Bearer> {
    let words: Vec<usize> = clause
        .tokens
        .clone()
        .filter(|&index| tokens[index].is_word())
        .collect();

    if clause.negative {
        for &index in &words {
            if analysis.tokens[index].role == SyntacticRole::Auxiliary
                && analysis.tokens[index]
                    .readings
                    .iter()
                    .any(|reading| reading.part_of_speech == Some(PartOfSpeech::NegativeVerb))
                && let Some((person, number)) = crate::expand::negation::person_number(&tokens[index].text)
            {
                return Some(Bearer {
                    index,
                    reading: Tags {
                        part_of_speech: Some(PartOfSpeech::NegativeVerb),
                        polarity: Polarity::Negative,
                        ..Tags::UNKNOWN
                    },
                    person,
                    number,
                });
            }
        }
    }

    for &index in &words {
        if analysis.tokens[index].role == SyntacticRole::Auxiliary
            && analysis.tokens[index].head == clause.predicate
            && let Some(reading) = finite_reading(&analysis.tokens[index])
            && let (Some(person), Some(number)) = (reading.person, reading.number)
        {
            return Some(Bearer {
                index,
                reading,
                person,
                number,
            });
        }
    }

    let predicate = clause.predicate?;
    let reading = finite_reading(&analysis.tokens[predicate])?;
    let (person, number) = (reading.person?, reading.number?);
    Some(Bearer {
        index: predicate,
        reading,
        person,
        number,
    })
}

pub(super) fn finite_reading(token: &TokenSyntax) -> Option<Tags> {
    token.readings.iter().copied().find(|reading| {
        match reading.part_of_speech {
            Some(PartOfSpeech::Verb) => {
                reading.polarity == Polarity::Affirmative
                    && matches!(
                        reading.mood,
                        Some(Mood::Indicative | Mood::Conditional | Mood::Potential)
                    )
            }
            Some(PartOfSpeech::NegativeVerb) => true,
            _ => false,
        }
    })
}

fn selected_readings(token: &TokenSyntax) -> impl Iterator<Item = &Tags> {
    token
        .readings
        .iter()
        .filter(move |reading| token.case.is_none() || reading.case == token.case)
}

fn pronoun_person_number(word: &str) -> Option<(Person, Number)> {
    Some(match word.to_lowercase().as_str() {
        "minä" => (Person::First, Number::Singular),
        "sinä" => (Person::Second, Number::Singular),
        "hän" => (Person::Third, Number::Singular),
        "me" => (Person::First, Number::Plural),
        "te" => (Person::Second, Number::Plural),
        "he" => (Person::Third, Number::Plural),
        _ => return None,
    })
}

fn has_coordinator(tokens: &[Token], left: usize, right: usize) -> bool {
    let (start, end) = if left < right {
        (left + 1, right)
    } else {
        (right + 1, left)
    };
    tokens[start..end]
        .iter()
        .filter(|token| token.is_word())
        .any(|token| matches!(token.text.to_lowercase().as_str(), "ja" | "sekä"))
}
