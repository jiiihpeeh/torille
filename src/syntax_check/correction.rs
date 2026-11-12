//! Turn a rule finding into concrete corrections.
//!
//! The inflection engine is the only source of replacement words. A correction
//! is a plain form of the lemma with the desired person/number (or case), not
//! a suffix edit. `Confidence::Plain` is required: `Repaired`/`Guessed` forms
//! are accepted words but must never be offered as corrections.

use crate::expand::forms_of;
use crate::kotus::sanalista;
use crate::spec::Catalogues;
use crate::tags::{
    Case, Confidence, Number, PartOfSpeech, Person, Polarity, Tags, Voice,
};
use crate::tokenise::Token;

use super::diagnostic::{Correction, Edit};

/// The finite verb/auxiliary form for a subject's person and number.
///
/// `reading` is the agreement bearer's finite reading; tense, mood and voice
/// are preserved, so a past-tense clause stays past and a conditional stays
/// conditional.
pub fn verb_agreement_corrections(
    lemma: &str,
    token: &Token,
    person: Person,
    number: Number,
    reading: &Tags,
    cat: &Catalogues,
) -> Vec<Correction> {
    let Some(row) = sanalista().by_word(lemma).next() else {
        return Vec::new();
    };
    let inflections: Vec<_> = row.inflections().collect();
    let pos = PartOfSpeech::parse_all(row.part_of_speech())
        .first()
        .copied()
        .unwrap_or(PartOfSpeech::Verb);
    let mut want = Tags::UNKNOWN;
    want.part_of_speech = Some(PartOfSpeech::Verb);
    want.person = Some(person);
    want.number = Some(number);
    want.tense = reading.tense;
    want.mood = reading.mood;
    want.voice = reading.voice.or(Some(Voice::Active));
    want.polarity = Polarity::Affirmative;

    let mut corrections: Vec<Correction> = Vec::new();
    for form in forms_of(lemma, &inflections, cat, None, &want, pos, false) {
        if form.confidence != Confidence::Plain || form.word == token.text {
            continue;
        }
        let exact = form.readings.iter().any(|candidate| {
            candidate.part_of_speech == Some(PartOfSpeech::Verb)
                && candidate.person == Some(person)
                && candidate.number == Some(number)
                && candidate.tense == reading.tense
                && candidate.mood == reading.mood
                && candidate.voice == reading.voice.or(Some(Voice::Active))
                && candidate.polarity == Polarity::Affirmative
        });
        if !exact {
            continue;
        }
        if corrections.iter().any(|seen| {
            seen.edits
                .first()
                .is_some_and(|edit| edit.replacement == form.word)
        }) {
            continue;
        }
        let distance = crate::levenshtein_with_ops(&token.text, &form.word).0;
        corrections.push(Correction::new(
            vec![Edit::new(token.start, token.end, form.word)],
            distance as f32,
            "subject_predicate_agreement",
        ));
    }
    corrections.sort_by(|a, b| {
        a.score
            .total_cmp(&b.score)
            .then_with(|| a.edits[0].replacement.cmp(&b.edits[0].replacement))
    });
    corrections
}

/// The singular/plural demonstrative lemma for the number the head needs.
///
/// The sanalista lists the singular and plural demonstratives as separate
/// lemmas (`tämä`/`nämä`, `tuo`/`nuo`, `se`/`ne`), so a correction cannot
/// inflect `nämä` to the singular; it has to switch to the other lemma.
fn demonstrative_lemma(lemma: &str, number: Number) -> &str {
    match (lemma, number) {
        ("nämä", Number::Singular) => "tämä",
        ("nuo", Number::Singular) => "tuo",
        ("ne", Number::Singular) => "se",
        ("tämä", Number::Plural) => "nämä",
        ("tuo", Number::Plural) => "nuo",
        ("se", Number::Plural) => "ne",
        _ => lemma,
    }
}

/// The attributive adjective/ordinal/demonstrative form that agrees with its
/// head in case and number.
pub fn attribute_agreement_corrections(
    lemma: &str,
    token: &Token,
    case: Case,
    number: Number,
    pos: PartOfSpeech,
    cat: &Catalogues,
) -> Vec<Correction> {
    let lemma = if pos == PartOfSpeech::Pronoun {
        demonstrative_lemma(lemma, number)
    } else {
        lemma
    };
    let Some(row) = sanalista().by_word(lemma).next() else {
        return Vec::new();
    };
    let inflections: Vec<_> = row.inflections().collect();
    let mut want = Tags::UNKNOWN;
    want.part_of_speech = Some(pos);
    want.case = Some(case);
    want.number = Some(number);

    let mut corrections: Vec<Correction> = Vec::new();
    for form in forms_of(lemma, &inflections, cat, None, &want, pos, false) {
        if form.confidence != Confidence::Plain || form.word == token.text {
            continue;
        }
        let exact = form.readings.iter().any(|candidate| {
            candidate.part_of_speech == Some(pos)
                && candidate.case == Some(case)
                && candidate.number == Some(number)
        });
        if !exact {
            continue;
        }
        if corrections.iter().any(|seen| {
            seen.edits
                .first()
                .is_some_and(|edit| edit.replacement == form.word)
        }) {
            continue;
        }
        let distance = crate::levenshtein_with_ops(&token.text, &form.word).0;
        corrections.push(Correction::new(
            vec![Edit::new(token.start, token.end, form.word)],
            distance as f32,
            "attribute_agreement",
        ));
    }
    corrections.sort_by(|a, b| {
        a.score
            .total_cmp(&b.score)
            .then_with(|| a.edits[0].replacement.cmp(&b.edits[0].replacement))
    });
    corrections
}

/// The nominal form for the case a verb governs.
pub fn case_corrections(
    lemma: &str,
    token: &Token,
    case: Case,
    number: Option<Number>,
    pos: PartOfSpeech,
    cat: &Catalogues,
) -> Vec<Correction> {
    let Some(row) = sanalista().by_word(lemma).next() else {
        return Vec::new();
    };
    let inflections: Vec<_> = row.inflections().collect();
    let mut want = Tags::UNKNOWN;
    want.part_of_speech = Some(pos);
    want.case = Some(case);
    want.number = number;

    let mut corrections: Vec<Correction> = Vec::new();
    for form in forms_of(lemma, &inflections, cat, None, &want, pos, false) {
        if form.confidence != Confidence::Plain || form.word == token.text {
            continue;
        }
        let exact = form.readings.iter().any(|candidate| {
            candidate.part_of_speech == Some(pos)
                && candidate.case == Some(case)
                && number.is_none_or(|number| candidate.number == Some(number))
        });
        if !exact {
            continue;
        }
        if corrections.iter().any(|seen| {
            seen.edits
                .first()
                .is_some_and(|edit| edit.replacement == form.word)
        }) {
            continue;
        }
        let distance = crate::levenshtein_with_ops(&token.text, &form.word).0;
        corrections.push(Correction::new(
            vec![Edit::new(token.start, token.end, form.word)],
            distance as f32,
            "verb_rection",
        ));
    }
    corrections.sort_by(|a, b| {
        a.score
            .total_cmp(&b.score)
            .then_with(|| a.edits[0].replacement.cmp(&b.edits[0].replacement))
    });
    corrections
}

/// The negative auxiliary for a subject's person and number.
pub fn negative_auxiliary_correction(
    start: usize,
    end: usize,
    original: &str,
    person: Person,
    number: Number,
) -> Option<Correction> {
    let word = crate::expand::negation::auxiliary(person, number)?;
    if word == original {
        return None;
    }
    Some(Correction::new(
        vec![Edit::new(start, end, word)],
        0.0,
        "subject_predicate_agreement",
    ))
}
