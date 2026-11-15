//! Morphological predicates shared by segmentation and role resolution.

use crate::tags::{Case, Mood, PartOfSpeech, Polarity, Tags};
use crate::tokenise::Token;

use super::{RoleConfidence, SyntacticRole, TokenSyntax};

/// A reading that can be the finite predicate of a clause.
pub(crate) fn is_finite_predicate(reading: &Tags) -> bool {
    match reading.part_of_speech {
        Some(PartOfSpeech::NegativeVerb) => true,
        // A single-token `Verb` reading with negative polarity is a
        // connegative (`syö` in `en syö`, `mennä` in `ei mennä`), not a
        // finite predicate. The finite negative is the auxiliary token.
        Some(PartOfSpeech::Verb) => {
            reading.polarity == Polarity::Affirmative
                && matches!(
                    reading.mood,
                    Some(Mood::Indicative | Mood::Conditional | Mood::Imperative | Mood::Potential)
                )
        }
        _ => false,
    }
}

/// Cases that license an adverbial reading without clause context.
///
/// Essive and translative are deliberately absent: they are also the cases of
/// a predicate complement, and phase 6 needs the predicate's valency to tell
/// `Hän tuli iloiseksi` from an ordinary adverbial.
pub(super) fn is_adverbial_case(case: Case) -> bool {
    matches!(
        case,
        Case::Inessive
            | Case::Elative
            | Case::Illative
            | Case::Adessive
            | Case::Ablative
            | Case::Allative
            | Case::Abessive
            | Case::Comitative
            | Case::Prolative
            | Case::Instructive
    )
}

/// Whether any reading of a token is a verb.
pub(crate) fn is_verb_reading(reading: &Tags) -> bool {
    matches!(
        reading.part_of_speech,
        Some(PartOfSpeech::Verb | PartOfSpeech::NegativeVerb)
    )
}

/// Whether a part of speech is nominal.
pub(crate) fn is_nominal_pos(pos: Option<PartOfSpeech>) -> bool {
    matches!(
        pos,
        Some(
            PartOfSpeech::Noun
                | PartOfSpeech::Adjective
                | PartOfSpeech::Pronoun
                | PartOfSpeech::Numeral
                | PartOfSpeech::Ordinal
        )
    )
}

/// A participle used as a prenominal attribute (`Helsingissä asuva mies`).
pub(super) fn is_participle_attribute(index: usize, out: &[TokenSyntax]) -> bool {
    out[index].readings.iter().any(|reading| {
        reading.part_of_speech == Some(PartOfSpeech::Verb)
            && reading.case.is_some()
            && reading.mood != Some(Mood::Infinitive)
    })
}

/// Whether any reading of a token is nominal.
pub(super) fn is_nominal(syntax: &TokenSyntax) -> bool {
    syntax
        .readings
        .iter()
        .any(|reading| is_nominal_pos(reading.part_of_speech))
}

/// Whether one reading is both nominal and in one of `cases`.
///
/// Asking [`is_nominal`] and [`has_case`] separately can combine two readings:
/// `Ilman` has a `Preposition/Nominative` reading and a `Noun/Genitive` one,
/// and `sinut` has an `Unknown/Nominative` reading plus a `Pronoun/Accusative`
/// one. Subject and object selection must not take the case from the reading
/// that is not the nominal.
pub(super) fn nominal_has_case(syntax: &TokenSyntax, cases: &[Case]) -> bool {
    syntax.readings.iter().any(|reading| {
        is_nominal_pos(reading.part_of_speech)
            && reading.case.is_some_and(|case| cases.contains(&case))
    })
}

/// A nominal role is only `Likely` when the word is not also a verb.
pub(super) fn nominal_confidence(syntax: &TokenSyntax) -> RoleConfidence {
    let has_verb = syntax.readings.iter().any(is_verb_reading);
    let has_nominal = is_nominal(syntax);
    if has_verb && has_nominal {
        RoleConfidence::Weak
    } else {
        RoleConfidence::Likely
    }
}

/// The first case from `cases` that the token actually has.
pub(super) fn best_case(syntax: &TokenSyntax, cases: &[Case]) -> Option<Case> {
    cases.iter().copied().find(|case| {
        syntax
            .readings
            .iter()
            .any(|reading| reading.case == Some(*case))
    })
}

/// A finite predicate candidate, excluding the negative auxiliary.
///
/// `ei` is also a noun and `älä` has a class-99 noun reading, so "all readings
/// are `NegativeVerb`" is not a reliable test. A token is the auxiliary when
/// it has a `NegativeVerb` reading and no non-auxiliary finite reading; the
/// main verb is what the clause predicate should point at.
pub(super) fn is_negative_auxiliary(index: usize, out: &[TokenSyntax]) -> bool {
    out[index]
        .readings
        .iter()
        .any(|reading| reading.part_of_speech == Some(PartOfSpeech::NegativeVerb))
        && !out[index].readings.iter().any(|reading| {
            reading.part_of_speech != Some(PartOfSpeech::NegativeVerb)
                && is_finite_predicate(reading)
        })
}

pub(super) fn is_predicate_candidate(index: usize, out: &[TokenSyntax]) -> bool {
    out[index].role == SyntacticRole::Predicate && !is_negative_auxiliary(index, out)
}

/// A clear infinitive reading, which can head an infinitival clause.
///
/// A word that is also a nominal is not one: `pieniä` has an infinitive
/// homonym, but in `omenoita (pieniä, makeita)` it is an adjective and must
/// not split the clause.
pub(super) fn is_infinitive(index: usize, out: &[TokenSyntax]) -> bool {
    out[index]
        .readings
        .iter()
        .any(|reading| reading.mood == Some(Mood::Infinitive))
        && !out[index]
            .readings
            .iter()
            .any(|reading| is_nominal_pos(reading.part_of_speech))
}

/// A past participle, the verb of a compound tense or a passive construction.
///
/// It has a case and past tense and is not an infinitive. The active form
/// (`lukenut`, `tullut`) carries the compound tense with `olla` or the
/// negative auxiliary; the passive form (`luettu`, `rakennettu`) makes the
/// clause passive. A present participle (`mentävä`) is not one of these.
pub(super) fn is_past_participle(index: usize, out: &[TokenSyntax]) -> bool {
    out[index].readings.iter().any(|reading| {
        reading.part_of_speech == Some(PartOfSpeech::Verb)
            && reading.tense == Some(crate::tags::Tense::Past)
            && reading.case.is_some()
            && reading.mood != Some(Mood::Infinitive)
    })
}

/// Whether the verb readings of a token are passive.
///
/// A participle can carry a spurious passive homonym (`tullut` has active and
/// passive readings), so one passive reading is not enough. If any verb
/// reading is active, the token is treated as active; a genuinely passive
/// form (`rakennettu`, `luettu`, `rakennettiin`) has no active reading.
pub(crate) fn predicate_is_passive(index: usize, out: &[TokenSyntax]) -> bool {
    let mut saw_verb = false;
    for reading in &out[index].readings {
        if reading.part_of_speech != Some(PartOfSpeech::Verb) {
            continue;
        }
        saw_verb = true;
        if reading.voice != Some(crate::tags::Voice::Passive) {
            return false;
        }
    }
    saw_verb
}


pub(crate) fn is_coordinating(word: &str) -> bool {
    matches!(
        word,
        "ja" | "mutta" | "tai" | "eli" | "sekä" | "vai" | "vaan"
    )
}

pub(crate) fn is_subordinator(word: &str) -> bool {
    matches!(
        word,
        "että" | "kun" | "jos" | "vaikka" | "koska" | "jotta" | "mikäli" | "ellei"
    )
}

/// The inflected forms of the relative pronouns. The engine resolves most of
/// them to `joka`/`mikä`/`kuka`; the surface list covers the ones whose own
/// lemma wins (`missä`, `mitä`) and keeps the check independent of the
/// resolution's homonym ranking.
const RELATIVE_SURFACES: &[&str] = &[
    // joka
    "joka", "jonka", "jota", "jossa", "josta", "johon", "jolla", "jolta", "jolle", "joina",
    "joiksi", "joitta", "jotka", "joiden", "joita", "joissa", "joista", "joihin", "joilla",
    "joilta", "joille", // mikä
    "mikä", "minkä", "mitä", "missä", "mistä", "mihin", "millä", "miltä", "mille", "miksi",
    "mitkä", // kuka / ken
    "kuka", "kenen", "ketä", "kenessä", "kestä", "kehen", "kenellä", "keneltä", "kenelle", "ketkä",
    "keiden", "keitä", "keissä", "keistä", "keihin", "keillä", "keiltä", "keille", "ken",
];

/// Whether this token is a relative pronoun, in any of its cases.
pub(crate) fn is_relative_word(index: usize, tokens: &[Token], out: &[TokenSyntax]) -> bool {
    let lower = tokens[index].text.to_lowercase();
    if RELATIVE_SURFACES.contains(&lower.as_str()) {
        return true;
    }
    // A personal pronoun is never a relative pronoun, even when the homonym
    // resolver gives it a relative lemma (`minä` is also an essive of `mikä`).
    if matches!(
        lower.as_str(),
        "minä" | "sinä" | "hän" | "me" | "te" | "he"
    ) {
        return false;
    }
    let lemma = out[index].lemma.as_deref().unwrap_or_default();
    matches!(lemma, "joka" | "mikä" | "ken" | "kuka")
}

/// Whether `joka`/`jokainen` is a determiner before a nominative noun.
///
/// `Joka päivä juon kahvia` and `Merkitse joka esine kirjaan` use `joka` as
/// “every”, not as a relative pronoun. A relative pronoun is followed by a
/// verb, an adverb or a non-nominative argument, or it is preceded by a comma.
/// The missing-comma construction `Mies joka kirjan luki` still has a genitive
/// noun after the pronoun, so it stays relative and the punctuation rule can
/// suggest the comma.
fn is_determiner_joka(index: usize, tokens: &[Token], out: &[TokenSyntax]) -> bool {
    let lower = tokens[index].text.to_lowercase();
    if !matches!(lower.as_str(), "joka" | "jokainen") {
        return false;
    }
    let Some(next) = (index + 1..tokens.len()).find(|&candidate| tokens[candidate].is_word())
    else {
        return false;
    };
    if !nominal_has_case(&out[next], &[Case::Nominative]) {
        return false;
    }
    !comma_before(tokens, index) && !comma_between(tokens, index, next)
}

/// Whether this token opens a relative clause rather than acting as a
/// determiner (`joka päivä`, `joka esine`).
pub(crate) fn starts_relative_clause(index: usize, tokens: &[Token], out: &[TokenSyntax]) -> bool {
    is_relative_word(index, tokens, out) && !is_determiner_joka(index, tokens, out)
}

/// Whether a comma sits in the separators immediately before `index`.
fn comma_before(tokens: &[Token], index: usize) -> bool {
    for token in tokens[..index].iter().rev() {
        if token.is_word() {
            break;
        }
        if token.text.contains(',') {
            return true;
        }
    }
    false
}

/// Whether a comma sits between two word tokens.
pub(super) fn comma_between(tokens: &[Token], left: usize, right: usize) -> bool {
    tokens
        .iter()
        .skip(left + 1)
        .take(right.saturating_sub(left + 1))
        .filter(|token| !token.is_word())
        .any(|token| token.text.contains(','))
}

/// A function word that must not be mistaken for a subject or object merely
/// because the lexicon also gives it a noun reading (`ja`, `kun`).
pub(super) fn is_clause_marker(index: usize, tokens: &[Token], out: &[TokenSyntax]) -> bool {
    let lower = tokens[index].text.to_lowercase();
    is_coordinating(&lower) || is_subordinator(&lower) || is_relative_word(index, tokens, out)
}
