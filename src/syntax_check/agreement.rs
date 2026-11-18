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
use crate::tags::{
    Case, Degree, Mood, Number, PartOfSpeech, Person, Polarity, Tags, Tense, Voice,
};
use crate::tokenise::Token;

use super::correction;
use super::diagnostic::{
    Confidence, Correction, DiagnosticKind, Severity, SyntaxDiagnostic,
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
    diagnostics.extend(check_compound_participle(tokens, analysis));
    diagnostics.extend(check_attributes(tokens, analysis));
    diagnostics.extend(check_complements(tokens, analysis));
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
        if starts_with_subordinator(clause, tokens, analysis)
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
        // A spelling that is both a finite form and an instructive or
        // infinitive is not a clear predicate: `ihastellen` is the potential
        // 1sg of `ihastella` *and* its instructive, and the instructive is the
        // reading a text almost always means (`sinä rannassa, ihastellen
        // kalojani`). Diagnosing the accidental one is a false positive, and
        // this rule needs a clear bearer.
        if has_non_finite_verb_reading(&analysis.tokens[bearer.index]) {
            continue;
        }
        // `kaikki` is “everything” with a singular verb and “all (of them)”
        // with a plural one, so it never decides the number on its own.
        if analysis.tokens[subject.index].key == "kaikki" {
            continue;
        }
        // A 1st/2nd-person predicate takes only a pronoun as its subject;
        // a nominal there is an apposition to the dropped pronoun
        // (`Me suuremmat istumme`). Comparing person against the adjective
        // offered `istuvat`, and a line break can put `Me` in the previous
        // sentence, so the apposition is all the clause has.
        if matches!(bearer.person, Person::First | Person::Second)
            && !is_personal_pronoun_key(&analysis.tokens[subject.index].key)
        {
            continue;
        }
        if subject.person == bearer.person && subject.number == bearer.number {
            continue;
        }

        let confident = analysis.tokens[subject.index].confidence == RoleConfidence::Likely
            && analysis.tokens[bearer.index].confidence == RoleConfidence::Likely;
        let (severity, confidence) = if confident {
            (Severity::Error, Confidence::Certain)
        } else {
            (Severity::Warning, Confidence::Likely)
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

/// Past active participle agreement in a compound tense.
///
/// `olen lukenut` / `olemme lukeneet`: the auxiliary carries the person and
/// the participle agrees with the subject in number. Passive compound tenses
/// (`talo on rakennettu`, `talot on rakennettu`) do not agree, so they are
/// skipped.
fn check_compound_participle(
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
) -> Vec<SyntaxDiagnostic> {
    let mut diagnostics = Vec::new();
    for clause in &analysis.clauses {
        let Some(predicate) = clause.predicate else {
            continue;
        };
        if !crate::syntax::is_past_participle(predicate, &analysis.tokens)
            || crate::syntax::predicate_is_passive(predicate, &analysis.tokens)
        {
            continue;
        }
        let has_olla_auxiliary = clause.tokens.clone().any(|index| {
            tokens[index].is_word()
                && analysis.tokens[index].role == SyntacticRole::Auxiliary
                && analysis.tokens[index].lemma.as_deref() == Some("olla")
        });
        if !has_olla_auxiliary {
            continue;
        }
        let Some(subject) = subject_of(clause, tokens, analysis) else {
            continue;
        };
        if subject.index > predicate {
            continue;
        }
        let mut numbers: Vec<Number> = analysis.tokens[predicate]
            .readings
            .iter()
            .filter(|reading| {
                reading.part_of_speech == Some(PartOfSpeech::Verb)
                    && reading.tense == Some(Tense::Past)
                    && reading.case.is_some()
                    && reading.mood != Some(Mood::Infinitive)
                    // `mennyt`/`tullut` also have passive homonyms; the
                    // compound tense here is active.
                    && reading.voice != Some(Voice::Passive)
            })
            .filter_map(|reading| reading.number)
            .collect();
        numbers.sort_unstable();
        numbers.dedup();
        let [participle_number] = numbers.as_slice() else {
            continue;
        };
        if *participle_number == subject.number {
            continue;
        }
        let Some(reading) = analysis.tokens[predicate]
            .readings
            .iter()
            .find(|reading| {
                reading.part_of_speech == Some(PartOfSpeech::Verb)
                    && reading.tense == Some(Tense::Past)
                    && reading.case.is_some()
                    && reading.mood != Some(Mood::Infinitive)
                    && reading.voice != Some(Voice::Passive)
                    && reading.number == Some(*participle_number)
            })
            .copied()
        else {
            continue;
        };
        // The participle's role reading is often weak because the surface is
        // also an adjective/nominal (`lukenut`); the single participle number
        // established above is the disambiguation, so the subject's confidence
        // is the controlling one.
        let confident = analysis.tokens[subject.index].confidence == RoleConfidence::Likely;
        let (severity, confidence) = if confident {
            (Severity::Error, Confidence::Certain)
        } else {
            (Severity::Warning, Confidence::Likely)
        };
        let token = &tokens[predicate];
        // The chosen lemma can be the adjective homonym (`väsynyt`); the verb
        // paradigm is one of the `lemmas`. Take the first base that actually
        // produces the wanted participle cell.
        let mut corrections = Vec::new();
        for lemma in &analysis.tokens[predicate].lemmas {
            corrections = correction::participle_agreement_corrections(
                lemma,
                token,
                subject.number,
                &reading,
                crate::spec::Catalogues::embedded(),
            );
            if !corrections.is_empty() {
                break;
            }
        }
        if corrections.is_empty()
            && let Some(lemma) = analysis.tokens[predicate].lemma.as_deref()
        {
            corrections = correction::participle_agreement_corrections(
                lemma,
                token,
                subject.number,
                &reading,
                crate::spec::Catalogues::embedded(),
            );
        }
        let mut diagnostic = SyntaxDiagnostic::new(
            DiagnosticKind::SubjectPredicateAgreement,
            severity,
            confidence,
            token.start..token.end,
            "participle_agreement",
        );
        diagnostic.expected = Some(format!("{} participle", subject.number.name()));
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
    let has_predicate: Vec<bool> = analysis
        .clauses
        .iter()
        .map(|clause| clause.predicate.is_some())
        .collect();
    for (index, syntax) in analysis.tokens.iter().enumerate() {
        let predicate_less = has_predicate
            .get(syntax.clause)
            .is_none_or(|has_predicate| !has_predicate);
        if !tokens[index].is_word() {
            continue;
        }
        // A clause with a finite predicate has a role for everything that can
        // have one, and only a `Modifier` is an attribute. A pronoun in an
        // oblique case (`niille`, `minulle`), an object or a complement is not
        // compared with the following noun: `panna niille raha-arvo` and
        // `sai tuskin sanotuksi` are not agreement errors.
        //
        // A predicate-less clause (a heading, a fragment, `Punaisessa auto.`)
        // keeps the shape-based reading, because there is no role to rely on.
        let candidate = if predicate_less {
            matches!(
                syntax.role,
                SyntacticRole::Modifier | SyntacticRole::Unknown | SyntacticRole::Adverbial
            )
        } else {
            syntax.role == SyntacticRole::Modifier
        };
        if !candidate {
            continue;
        }
        // A relative pronoun (`joka`, `jotka`, `jossa`, ...) introduces a
        // clause; it is not an attributive modifier and has no head to agree
        // with. The rule is for adjectives, ordinals and demonstratives.
        if syntax
            .lemma
            .as_deref()
            .is_some_and(is_relative_pronoun_lemma)
        {
            continue;
        }
        // An invariant quantifier/determiner does not agree with its head:
        // `Te kaikki tiedätte` and `koko päivänä` keep the same nominative
        // shape whatever the number of the noun.
        if syntax
            .lemma
            .as_deref()
            .is_some_and(is_invariant_attribute_lemma)
        {
            continue;
        }
        // `paljon hyvää`: the adjective is the head of the quantity phrase,
        // not an attribute of the following noun. A closed adverb set, the
        // same shape as the other lexical cues in this module.
        if previous_word_is(index, &["paljon", "vähän", "runsaasti", "tarpeeksi"], tokens, analysis) {
            continue;
        }
        // Ask for an unambiguous attribute shape first: a token that is not an
        // adjective, ordinal or demonstrative never needs the degree lookup.
        let Some((attribute_case, attribute_number)) = attribute_shape(syntax) else {
            continue;
        };
        let Some(degree) = attribute_degree(syntax) else {
            continue;
        };
        // A degree surface the morphology could not tag is left quiet: the
        // correction helper must not replace a comparative with its positive.
        if degree.is_none() && crate::expand::degree_base(&syntax.key).is_some() {
            continue;
        }
        // `Kaksi pientä lasta`: the adjective belongs to a cardinal numeral
        // phrase and is partitive by numeral government. The counted head may
        // lack its partitive reading in this context, so the ordinary
        // attribute comparison would offer a nominative by mistake.
        if in_cardinal_phrase(index, tokens, syntax, analysis) {
            continue;
        }
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
            degree,
            pos,
            crate::spec::Catalogues::embedded(),
        );
        let number_mismatch = attribute_number != head_number;
        let (severity, confidence) = if number_mismatch {
            (Severity::Error, Confidence::Certain)
        } else {
            (Severity::Warning, Confidence::Likely)
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

/// Is the previous word one of `words`? Used for the quantity-phrase head
/// (`paljon hyvää`), where the adjective is not an attribute.
fn previous_word_is(
    index: usize,
    words: &[&str],
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
) -> bool {
    (0..index)
        .rev()
        .find(|&candidate| tokens[candidate].is_word())
        .is_some_and(|previous| words.contains(&analysis.tokens[previous].key.as_str()))
}

/// A predicative adjective agrees with the subject in number.
///
/// `Talot ovat punainen` and `Talo on punaisia` are the common learner errors.
/// The rule is number-only: `Talot ovat punaisia` (partitive plural) and
/// `Talot ovat punaiset` (nominative plural) are both legitimate, so the case
/// is not a correctness claim. Coordinated subjects, noun complements,
/// participles and existential clauses are skipped; an adjective degree keeps
/// its degree in the correction.
fn check_complements(tokens: &[Token], analysis: &SyntaxAnalysis) -> Vec<SyntaxDiagnostic> {
    let mut diagnostics = Vec::new();
    for clause in &analysis.clauses {
        if clause.existential {
            continue;
        }
        let Some(predicate) = clause.predicate else {
            continue;
        };
        if analysis.tokens[predicate].lemma.as_deref() != Some("olla") {
            continue;
        }
        let Some(complement) = clause.complement else {
            continue;
        };
        // A coordinated subject phrase is collapsed to a plural controller by
        // `subject_of`; it returns `None` when the person cannot be resolved.
        let Some(subject) = subject_of(clause, tokens, analysis) else {
            continue;
        };
        let syntax = &analysis.tokens[complement];
        let Some(lemma) = syntax.lemma.as_deref() else {
            continue;
        };
        if let Some((complement_case, complement_number)) = adjective_shape(syntax) {
            let Some(degree) = attribute_degree(syntax) else {
                continue;
            };
            if degree.is_none() && crate::expand::degree_base(&syntax.key).is_some() {
                continue;
            }
            if subject.number == complement_number {
                continue;
            }
            let target_case = if subject.number == Number::Singular {
                Case::Nominative
            } else {
                complement_case
            };
            let corrections = correction::attribute_agreement_corrections(
                lemma,
                &tokens[complement],
                target_case,
                subject.number,
                degree,
                PartOfSpeech::Adjective,
                crate::spec::Catalogues::embedded(),
            )
            .into_iter()
            .map(|mut correction| {
                correction.note = "predicative_agreement";
                correction
            })
            .collect();
            let confident = analysis.tokens[subject.index].confidence == RoleConfidence::Likely
                && syntax.confidence == RoleConfidence::Likely;
            let (severity, confidence) = if confident {
                (Severity::Error, Confidence::Certain)
            } else {
                (Severity::Warning, Confidence::Likely)
            };
            let mut diagnostic = SyntaxDiagnostic::new(
                DiagnosticKind::PredicativeAgreement,
                severity,
                confidence,
                tokens[complement].start..tokens[complement].end,
                "predicative_agreement",
            );
            diagnostic.expected = Some(format!(
                "{} {}",
                target_case.name(),
                subject.number.name()
            ));
            diagnostic.corrections = corrections;
            diagnostics.push(diagnostic);
            continue;
        }

        // A noun predicate has a semantic partitive/nominative choice, so this
        // separate rule only reports a clear number mismatch: a nominative
        // singular predicate with a plural subject (`He ovat opettaja.`) or a
        // nominative plural predicate with a singular subject. Partitive
        // predicates are left to the speaker (`He ovat opettajia.`,
        // `Ne ovat vettä.`).
        let Some((complement_case, complement_number)) = noun_predicative_shape(syntax) else {
            continue;
        };
        if complement_case != Case::Nominative || subject.number == complement_number {
            continue;
        }
        let targets: &[Case] = if subject.number == Number::Plural {
            &[Case::Partitive, Case::Nominative]
        } else {
            &[Case::Nominative]
        };
        let mut corrections: Vec<Correction> = Vec::new();
        for target in targets {
            corrections.extend(
                correction::attribute_agreement_corrections(
                    lemma,
                    &tokens[complement],
                    *target,
                    subject.number,
                    None,
                    PartOfSpeech::Noun,
                    crate::spec::Catalogues::embedded(),
                )
                .into_iter()
                .take(1)
                .map(|mut correction| {
                    correction.note = "predicative_agreement";
                    correction.with_confidence(Confidence::Likely)
                }),
            );
        }
        let mut seen: Vec<String> = Vec::new();
        corrections.retain(|correction| {
            let Some(edit) = correction.edits.first() else {
                return false;
            };
            if seen.contains(&edit.replacement) {
                return false;
            }
            seen.push(edit.replacement.clone());
            true
        });
        let confident = analysis.tokens[subject.index].confidence == RoleConfidence::Likely
            && syntax.confidence == RoleConfidence::Likely;
        let confidence = if confident {
            Confidence::Certain
        } else {
            Confidence::Likely
        };
        let mut diagnostic = SyntaxDiagnostic::new(
            DiagnosticKind::PredicativeAgreement,
            Severity::Warning,
            confidence,
            tokens[complement].start..tokens[complement].end,
            "predicative_noun_agreement",
        );
        diagnostic.expected = Some(if subject.number == Number::Plural {
            "partitive or nominative plural".to_string()
        } else {
            "nominative singular".to_string()
        });
        diagnostic.corrections = corrections;
        diagnostics.push(diagnostic);
    }
    diagnostics
}

/// The unique case/number of a noun predicate.
fn noun_predicative_shape(token: &TokenSyntax) -> Option<(Case, Number)> {
    let mut shape: Option<(Case, Number)> = None;
    for reading in &token.readings {
        if reading.part_of_speech != Some(PartOfSpeech::Noun) {
            continue;
        }
        let Some(candidate) = reading.case.zip(reading.number) else {
            continue;
        };
        match shape {
            None => shape = Some(candidate),
            Some(seen) if seen == candidate => {}
            Some(_) => return None,
        }
    }
    shape
}

/// Whether an attribute sits immediately after a cardinal numeral.
///
/// `yksi` and the plural numerals (`kahdet`) are excluded: they do not govern
/// the partitive singular.
fn in_cardinal_phrase(
    index: usize,
    tokens: &[Token],
    syntax: &TokenSyntax,
    analysis: &SyntaxAnalysis,
) -> bool {
    let Some(clause) = analysis.clauses.get(syntax.clause) else {
        return false;
    };
    let words: Vec<usize> = clause
        .tokens
        .clone()
        .filter(|&candidate| tokens[candidate].is_word())
        .collect();
    let Some(position) = words.iter().position(|&candidate| candidate == index) else {
        return false;
    };
    let Some(&previous) = position.checked_sub(1).and_then(|at| words.get(at)) else {
        return false;
    };
    let token = &analysis.tokens[previous];
    token.lemma.as_deref() != Some("yksi")
        && token.readings.iter().any(|reading| {
            reading.part_of_speech == Some(crate::tags::PartOfSpeech::Numeral)
                && reading.number != Some(Number::Plural)
        })
}

/// The unambiguous adjective case/number of a predicate complement.
fn adjective_shape(token: &TokenSyntax) -> Option<(Case, Number)> {
    let mut shape: Option<(Case, Number)> = None;
    for reading in &token.readings {
        if reading.part_of_speech != Some(PartOfSpeech::Adjective) {
            continue;
        }
        let Some(candidate) = reading.case.zip(reading.number) else {
            continue;
        };
        match shape {
            None => shape = Some(candidate),
            Some(seen) if seen == candidate => {}
            Some(_) => return None,
        }
    }
    shape
}

/// The degree every attribute reading agrees on.
///
/// `Some(None)` is the positive degree, `Some(Some(Comparative))` a comparative
/// and so on; `None` means the readings disagree and the rule is skipped.
fn attribute_degree(token: &TokenSyntax) -> Option<Option<Degree>> {
    let mut seen: Vec<Option<Degree>> = Vec::new();
    for reading in &token.readings {
        if !is_attribute_pos(&token.key, reading.part_of_speech) {
            continue;
        }
        seen.push(reading.degree);
    }
    let first = *seen.first()?;
    seen.iter().all(|degree| *degree == first).then_some(first)
}

fn attribute_shape(token: &TokenSyntax) -> Option<(Case, Number)> {
    let mut shape: Option<(Case, Number)> = None;
    for reading in &token.readings {
        if !is_attribute_pos(&token.key, reading.part_of_speech) {
            continue;
        }
        let Some(candidate) = reading.case.zip(reading.number) else {
            continue;
        };
        match shape {
            None => shape = Some(candidate),
            Some(seen) if seen == candidate => {}
            Some(_) => return None,
        }
    }
    shape
}

fn head_shape(token: &TokenSyntax) -> Option<(Case, Number)> {
    let mut shape: Option<(Case, Number)> = None;
    for reading in &token.readings {
        if !is_head_pos(reading.part_of_speech) {
            continue;
        }
        let Some(candidate) = reading.case.zip(reading.number) else {
            continue;
        };
        match shape {
            None => shape = Some(candidate),
            Some(seen) if seen == candidate => {}
            Some(_) => return None,
        }
    }
    shape
}

fn attribute_pos(token: &TokenSyntax) -> Option<PartOfSpeech> {
    token
        .readings
        .iter()
        .find_map(|reading| {
            is_attribute_pos(&token.key, reading.part_of_speech)
                .then_some(reading.part_of_speech?)
        })
}

/// Which readings can stand as an attributive modifier.
///
/// Only the demonstratives among the pronouns do: a personal or indefinite
/// pronoun in an oblique case (`minulle`, `häntä`, `niille`) is never the
/// attribute of the following noun, and treating it as one offered `minä` for
/// `Siellä uusiintui minulle ruumis ja sielu.`.
fn is_attribute_pos(key: &str, pos: Option<PartOfSpeech>) -> bool {
    match pos {
        Some(PartOfSpeech::Adjective | PartOfSpeech::Ordinal) => true,
        Some(PartOfSpeech::Pronoun) => matches!(
            key,
            "se" | "tämä" | "tuo" | "ne" | "nämä" | "nuo" | "sama" | "samat"
        ),
        _ => false,
    }
}

/// A relative pronoun, whose forms introduce a clause rather than modify a
/// head. The lexicon gives it the same `Pronoun` part of speech as a
/// demonstrative, so the lemma is the only distinction available here.
fn is_relative_pronoun_lemma(lemma: &str) -> bool {
    matches!(lemma, "joka" | "mikä" | "ken" | "kuka")
}

/// Determiners whose nominative is the same for singular and plural.
///
/// `kaikki` has a rare generated plural `kaiket`, but the standard appositive
/// `Te kaikki tiedätte` uses the invariant form. `koko` is likewise invariant.
fn is_invariant_attribute_lemma(lemma: &str) -> bool {
    matches!(lemma, "kaikki" | "koko")
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
        && can_head_attribute(head, tokens, analysis)
    {
        return Some(head);
    }
    let clause = analysis.clauses.get(syntax.clause)?;
    clause
        .tokens
        .clone()
        .find(|&candidate| candidate > index && can_head_attribute(candidate, tokens, analysis))
}

/// Whether a token can be the head of an attributive phrase.
///
/// `Nämä hurskaan miehen sanat`: the immediate next word is another modifier,
/// not the agreement head, so walk past modifiers to the noun the whole phrase
/// modifies. A function word is not a head even when the lexicon gives it a
/// nominal homonym: `kuin` is a comparison particle before it is the
/// instructive plural of `kuu`, and `herttainen kuin kaunis` must not agree
/// with an instructive plural.
fn can_head_attribute(index: usize, tokens: &[Token], analysis: &SyntaxAnalysis) -> bool {
    let Some(token) = tokens.get(index) else {
        return false;
    };
    if !token.is_word() {
        return false;
    }
    let syntax = &analysis.tokens[index];
    if matches!(
        syntax.role,
        SyntacticRole::Modifier | SyntacticRole::Adverbial
    ) {
        return false;
    }
    let key = syntax.key.as_str();
    key != "kuin"
        && !crate::syntax::is_coordinating(key)
        && !crate::syntax::is_subordinator(key)
}

fn starts_with_subordinator(
    clause: &crate::syntax::Clause,
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
) -> bool {
    clause
        .tokens
        .clone()
        .find(|&index| tokens[index].is_word())
        .is_some_and(|index| crate::syntax::is_subordinator(&analysis.tokens[index].key))
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
        if !has_coordinator(tokens, analysis, indices[0], *indices.last()?) {
            return None;
        }
        // Coordinated first/second-person subjects need a person-resolution
        // rule that this phase does not have. Do not guess.
        if indices.iter().any(|&index| {
            matches!(
                pronoun_person_number(&analysis.tokens[index].key),
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
    if let Some((person, number)) = pronoun_person_number(&token.key) {
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

/// `minä`/`sinä`/`hän`/`me`/`te`/`he`, the subjects that carry a person.
fn is_personal_pronoun_key(key: &str) -> bool {
    matches!(key, "minä" | "sinä" | "hän" | "me" | "te" | "he")
}

/// Whether the token also has an instructive or infinitive verb reading.
///
/// Used to keep the agreement rule off a spelling collision: the same surface
/// can be a finite form of one verb and a non-finite form of another.
fn has_non_finite_verb_reading(token: &TokenSyntax) -> bool {
    token.readings.iter().any(|reading| {
        reading.part_of_speech == Some(PartOfSpeech::Verb)
            && matches!(reading.mood, Some(Mood::Infinitive | Mood::Instructive))
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

pub(super) fn pronoun_person_number(word: &str) -> Option<(Person, Number)> {
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

fn has_coordinator(
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
    left: usize,
    right: usize,
) -> bool {
    let (start, end) = if left < right {
        (left + 1, right)
    } else {
        (right + 1, left)
    };
    tokens[start..end]
        .iter()
        .enumerate()
        .filter(|(_, token)| token.is_word())
        .any(|(offset, _)| matches!(analysis.tokens[start + offset].key.as_str(), "ja" | "sekä"))
}
