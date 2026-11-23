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
    Case, Confidence, Degree, Mood, Number, PartOfSpeech, Person, Polarity, Tags, Tense, Voice,
};
use crate::tokenise::Token;

use super::diagnostic::{Confidence as DiagnosticConfidence, Correction, CorrectionLicense, Edit};

/// A cached [`forms_of`] result for one (lemma, wanted tags, POS).
///
/// The lemma's class list is not part of the key: it is a function of the
/// lemma, so two calls can only differ by the wanted tags or the POS. A
/// correction pass inflects the whole paradigm of every lemma it offers a
/// replacement for, and a warm re-check of the same text asks for exactly the
/// same paradigms again. The cache is thread-local and bounded by the number
/// of cached forms, so a long document cannot grow it without bound.
#[derive(PartialEq, Eq, Hash)]
struct FormsKey {
    lemma: String,
    want: Tags,
    pos: PartOfSpeech,
}

const FORM_CACHE_CAP: usize = 20_000;

/// The class list the sanalista gives `lemma`, or `None` when it is not a
/// lemma at all.
fn lemma_inflections(lemma: &str) -> Option<Vec<crate::kotus::Inflection>> {
    let row = sanalista().by_word(lemma).next()?;
    Some(row.inflections().collect())
}

/// The part of speech a lemma's correction paradigm is inflected as.
///
/// The sanalista's first reading decides; `fallback` covers a row that names
/// none (the verb corrections pass `Verb`). The nominal corrections already
/// know the POS from the reading and do not call this.
fn lemma_pos(lemma: &str, fallback: PartOfSpeech) -> Option<PartOfSpeech> {
    let row = sanalista().by_word(lemma).next()?;
    Some(
        PartOfSpeech::parse_all(row.part_of_speech())
            .first()
            .copied()
            .unwrap_or(fallback),
    )
}

fn cached_forms(
    lemma: &str,
    cat: &Catalogues,
    want: &Tags,
    pos: PartOfSpeech,
) -> std::rc::Rc<Vec<crate::tags::Form>> {
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;
    use std::rc::Rc;

    thread_local! {
        static CACHE: RefCell<HashMap<FormsKey, Rc<Vec<crate::tags::Form>>>> =
            RefCell::new(HashMap::new());
        static FORMS: Cell<usize> = const { Cell::new(0) };
    }

    let key = FormsKey {
        lemma: lemma.to_string(),
        want: *want,
        pos,
    };
    if let Some(forms) = CACHE.with_borrow(|cache| cache.get(&key).cloned()) {
        return forms;
    }
    let Some(inflections) = lemma_inflections(lemma) else {
        return Rc::new(Vec::new());
    };
    let forms = Rc::new(forms_of(lemma, &inflections, cat, want, pos, false));
    CACHE.with_borrow_mut(|cache| {
        if FORMS.with(Cell::get) + forms.len() > FORM_CACHE_CAP {
            cache.clear();
            FORMS.with(|count| count.set(0));
        }
        FORMS.with(|count| count.set(count.get() + forms.len()));
        cache.insert(key, Rc::clone(&forms));
    });
    forms
}

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
    let Some(pos) = lemma_pos(lemma, PartOfSpeech::Verb) else {
        return Vec::new();
    };
    let mut want = Tags::UNKNOWN;
    want.part_of_speech = Some(PartOfSpeech::Verb);
    want.person = Some(person);
    want.number = Some(number);
    want.tense = reading.tense;
    want.mood = reading.mood;
    want.voice = reading.voice.or(Some(Voice::Active));
    want.polarity = Polarity::Affirmative;

    let mut corrections: Vec<Correction> = Vec::new();
    let forms = cached_forms(lemma, cat, &want, pos);
    for form in forms.iter() {
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
        corrections.push(
            Correction::new(
                vec![Edit::new(token.start, token.end, form.word.clone())],
                distance as f32,
                "subject_predicate_agreement",
            )
            .licensed_by(CorrectionLicense::Inflection)
            .with_confidence(DiagnosticConfidence::Certain),
        );
    }
    corrections.sort_by(|a, b| {
        a.score
            .total_cmp(&b.score)
            .then_with(|| a.edits[0].replacement.cmp(&b.edits[0].replacement))
    });
    corrections
}

/// The past active participle form for the subject's number.
///
/// The participle agrees with the subject in number (`olen lukenut`,
/// `olemme lukeneet`). The target keeps the observed tense, voice, case and
/// mood, so the helper only changes the number.
pub fn participle_agreement_corrections(
    lemma: &str,
    token: &Token,
    number: Number,
    reading: &Tags,
    cat: &Catalogues,
) -> Vec<Correction> {
    let Some(pos) = lemma_pos(lemma, PartOfSpeech::Verb) else {
        return Vec::new();
    };
    let mut want = Tags::UNKNOWN;
    want.part_of_speech = Some(PartOfSpeech::Verb);
    want.number = Some(number);
    want.tense = reading.tense;
    want.case = reading.case;
    want.voice = reading.voice.or(Some(Voice::Active));
    want.mood = reading.mood;
    want.polarity = Polarity::Affirmative;

    let mut corrections: Vec<Correction> = Vec::new();
    for form in cached_forms(lemma, cat, &want, pos).iter() {
        if form.confidence != Confidence::Plain || form.word == token.text {
            continue;
        }
        let exact = form.readings.iter().any(|candidate| {
            candidate.part_of_speech == Some(PartOfSpeech::Verb)
                && candidate.number == Some(number)
                && candidate.tense == reading.tense
                && candidate.case == reading.case
                && candidate.voice == reading.voice.or(Some(Voice::Active))
                && candidate.mood == reading.mood
                && candidate.possessive.is_none()
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
        corrections.push(
            Correction::new(
                vec![Edit::new(token.start, token.end, form.word.clone())],
                distance as f32,
                "participle_agreement",
            )
            .licensed_by(CorrectionLicense::Inflection)
            .with_confidence(DiagnosticConfidence::Certain),
        );
    }
    corrections.sort_by(|a, b| {
        a.score
            .total_cmp(&b.score)
            .then_with(|| a.edits[0].replacement.cmp(&b.edits[0].replacement))
    });
    corrections
}

/// The connegative main-verb form for a negative clause.
///
/// The present connegative is the 2sg imperative surface (`en mene`), the
/// past negative is the past participle (`en mennyt`), and the conditional and
/// potential have their own connegatives. `reading` is the erroneous finite
/// verb's reading, so the mood and tense are preserved where the paradigm has
/// a matching negative form.
pub fn connegative_corrections(
    lemma: &str,
    token: &Token,
    reading: &Tags,
    cat: &Catalogues,
) -> Vec<Correction> {
    let Some(pos) = lemma_pos(lemma, PartOfSpeech::Verb) else {
        return Vec::new();
    };
    let mut want = Tags::UNKNOWN;
    want.part_of_speech = Some(PartOfSpeech::Verb);
    want.voice = Some(Voice::Active);
    want.polarity = Polarity::Negative;
    if reading.tense == Some(Tense::Past) {
        // `en menin` -> `en mennyt`, not `en mene`.
        want.tense = Some(Tense::Past);
        want.mood = Some(Mood::Indicative);
    } else {
        want.mood = reading.mood.or(Some(Mood::Indicative));
        want.tense = reading.tense;
    }

    let mut corrections: Vec<Correction> = Vec::new();
    for form in cached_forms(lemma, cat, &want, pos).iter() {
        if form.confidence != Confidence::Plain
            || form.word == token.text
            || form.word.split_whitespace().count() != 1
        {
            continue;
        }
        let exact = form.readings.iter().any(|candidate| {
            candidate.part_of_speech == Some(PartOfSpeech::Verb)
                && candidate.polarity == Polarity::Negative
                && candidate.mood == want.mood
                && candidate.tense == want.tense
                && candidate.voice == Some(Voice::Active)
        });
        if !exact
            || corrections.iter().any(|seen| {
                seen.edits
                    .first()
                    .is_some_and(|edit| edit.replacement == form.word)
            })
        {
            continue;
        }
        let distance = crate::levenshtein_with_ops(&token.text, &form.word).0;
        corrections.push(
            Correction::new(
                vec![Edit::new(token.start, token.end, form.word.clone())],
                distance as f32,
                "connegative",
            )
            .licensed_by(CorrectionLicense::Inflection)
            .with_confidence(DiagnosticConfidence::Certain),
        );
    }
    corrections.sort_by(|a, b| {
        a.score
            .total_cmp(&b.score)
            .then_with(|| a.edits[0].replacement.cmp(&b.edits[0].replacement))
    });
    corrections
}

/// The imperative connegative for the 2sg `älä` construction.
///
/// The plural/third-person imperative connegatives (`älkää menkö`,
/// `älköön menkö`) are `Repaired` forms in the engine, and `Repaired` forms
/// are never offered as corrections. The 2sg surface (`älä mene`) is the
/// plain imperative form `mene`, so it is the one safe repair.
pub fn imperative_connegative_corrections(
    lemma: &str,
    token: &Token,
    auxiliary: &Token,
    cat: &Catalogues,
) -> Vec<Correction> {
    let Some((Person::Second, Number::Singular)) =
        crate::expand::negation::person_number(&auxiliary.text)
    else {
        return Vec::new();
    };
    let Some(pos) = lemma_pos(lemma, PartOfSpeech::Verb) else {
        return Vec::new();
    };
    let mut want = Tags::UNKNOWN;
    want.part_of_speech = Some(PartOfSpeech::Verb);
    want.voice = Some(Voice::Active);
    want.mood = Some(Mood::Imperative);
    want.person = Some(Person::Second);
    want.number = Some(Number::Singular);
    want.tense = Some(Tense::Present);
    want.polarity = Polarity::Affirmative;

    let mut corrections: Vec<Correction> = Vec::new();
    for form in cached_forms(lemma, cat, &want, pos).iter() {
        if form.confidence != Confidence::Plain
            || form.word == token.text
            || form.word.split_whitespace().count() != 1
        {
            continue;
        }
        let exact = form.readings.iter().any(|candidate| {
            candidate.part_of_speech == Some(PartOfSpeech::Verb)
                && candidate.mood == Some(Mood::Imperative)
                && candidate.person == Some(Person::Second)
                && candidate.number == Some(Number::Singular)
                && candidate.voice == Some(Voice::Active)
        });
        if !exact
            || corrections.iter().any(|seen| {
                seen.edits
                    .first()
                    .is_some_and(|edit| edit.replacement == form.word)
            })
        {
            continue;
        }
        let distance = crate::levenshtein_with_ops(&token.text, &form.word).0;
        corrections.push(
            Correction::new(
                vec![Edit::new(token.start, token.end, form.word.clone())],
                distance as f32,
                "connegative",
            )
            .licensed_by(CorrectionLicense::Inflection)
            .with_confidence(DiagnosticConfidence::Certain),
        );
    }
    corrections.sort_by(|left, right| {
        left.score
            .total_cmp(&right.score)
            .then_with(|| left.edits[0].replacement.cmp(&right.edits[0].replacement))
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
    degree: Option<Degree>,
    pos: PartOfSpeech,
    cat: &Catalogues,
) -> Vec<Correction> {
    let lemma = if pos == PartOfSpeech::Pronoun {
        demonstrative_lemma(lemma, number)
    } else {
        lemma
    };
    let mut want = Tags::UNKNOWN;
    want.part_of_speech = Some(pos);
    want.case = Some(case);
    want.number = Some(number);
    want.degree = degree;

    let mut corrections: Vec<Correction> = Vec::new();
    let forms = cached_forms(lemma, cat, &want, pos);
    for form in forms.iter() {
        if form.confidence != Confidence::Plain || form.word == token.text {
            continue;
        }
        let exact = form.readings.iter().any(|candidate| {
            candidate.part_of_speech == Some(pos)
                && candidate.case == Some(case)
                && candidate.number == Some(number)
                && candidate.degree == degree
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
        corrections.push(
            Correction::new(
                vec![Edit::new(token.start, token.end, form.word.clone())],
                distance as f32,
                "attribute_agreement",
            )
            .licensed_by(CorrectionLicense::Inflection)
            .with_confidence(DiagnosticConfidence::Certain),
        );
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
    let mut want = Tags::UNKNOWN;
    want.part_of_speech = Some(pos);
    want.case = Some(case);
    want.number = number;

    let mut corrections: Vec<Correction> = Vec::new();
    let forms = cached_forms(lemma, cat, &want, pos);
    for form in forms.iter() {
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
        corrections.push(
            Correction::new(
                vec![Edit::new(token.start, token.end, form.word.clone())],
                distance as f32,
                "verb_rection",
            )
            .licensed_by(CorrectionLicense::Inflection)
            .with_confidence(DiagnosticConfidence::Certain),
        );
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
    Some(
        Correction::new(
            vec![Edit::new(start, end, word)],
            0.0,
            "subject_predicate_agreement",
        )
        .licensed_by(CorrectionLicense::Inflection)
        .with_confidence(DiagnosticConfidence::Certain),
    )
}
