//! The productive `-ton`/`-tön` privative adjective recogniser.

use crate::kotus::Inflection;
use crate::spec::Catalogues;
use crate::tags::PartOfSpeech;

use crate::expand::inflect_all;
use crate::expand::lexicon_index::LexiconIndex;

/// The derived privative `-ton`/`-tön` adjective a surface form belongs
/// to, if any.
///
/// `-ton` is the productive NOUN -> ADJECTIVE rule (`koriste` ->
/// `koristeeton`, `taide` -> `taiteeton`; `derivation::RULES`'s
/// `PRIV_TON*`). The derived adjective is not a sanalista lemma, so its
/// cases and degrees are in neither the stem maps nor the form set.
///
/// The form is built on the noun's genitive/weak stem, which may lengthen
/// the final vowel (`koriste` -> `koristee-`) or gradate the consonant
/// (`taide` -> `taitee-`). Reconstruct the nominative by replacing the
/// oblique `-ttoma-`/`-toma-` tail with `-ton`, recover the noun through
/// the noun stem map, and verify the exact surface against the class-34
/// paradigm.
pub fn derived_ton_base(
    word: &str,
    cat: &Catalogues,
    lexicon: Option<&LexiconIndex>,
) -> Option<String> {
    for candidate in ton_candidates(word) {
        let Some(prefix) = candidate
            .strip_suffix("ton")
            .or_else(|| candidate.strip_suffix("tön"))
        else {
            continue;
        };
        let noun = crate::expand::check_normal::STEM_BUCKETS_NOUN
            .bases(prefix)
            .any(|base| {
                crate::kotus::sanalista().by_word(base).any(|row| {
                    crate::tags::PartOfSpeech::parse_all(row.part_of_speech())
                        .contains(&PartOfSpeech::Noun)
                })
            });
        if !noun {
            continue;
        }
        for inflection in [
            Inflection {
                class: 34,
                gradation: None,
            },
            Inflection {
                class: 34,
                gradation: Some(crate::kotus::Gradation(3)),
            },
        ] {
            let forms = inflect_all(
                &candidate,
                &[inflection],
                cat,
                lexicon,
                PartOfSpeech::Adjective,
                true,
            );
            if forms.iter().any(|f| f.word == word) {
                return Some(candidate);
            }
        }
        // The superlative and comparative are productive for the privative
        // adjective too (`koristeeton` -> `koristeettomin` ->
        // `koristeettominta`), but the derived nominative is not a
        // sanalista lemma, so `degree_forms` never runs for it. Build the
        // two degree nominatives here and verify the surface against them.
        if let Some((comparative, superlative)) = ton_degrees(&candidate) {
            for (nominal, class, gradation) in [
                (
                    comparative,
                    16u16,
                    crate::kotus::Gradation::from_letter('H'),
                ),
                (superlative, 36u16, None),
            ] {
                let forms = inflect_all(
                    &nominal,
                    &[Inflection { class, gradation }],
                    cat,
                    lexicon,
                    PartOfSpeech::Adjective,
                    false,
                );
                if forms.iter().any(|f| f.word == word) {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

/// The comparative and superlative nominatives of a `-ton`/`-tön`
/// adjective: `koristeeton` -> (`koristeettomampi`, `koristeettomin`).
fn ton_degrees(lemma: &str) -> Option<(String, String)> {
    let (stem, back) = if let Some(stem) = lemma.strip_suffix("ton") {
        (stem, true)
    } else {
        (lemma.strip_suffix("tön")?, false)
    };
    if stem.is_empty() {
        return None;
    }
    if back {
        Some((format!("{stem}ttomampi"), format!("{stem}ttomin")))
    } else {
        Some((format!("{stem}ttömämpi"), format!("{stem}ttömin")))
    }
}

/// Candidate nominatives for a derived privative `-ton`/`-tön` surface.
///
/// The nominative ends in `-ton`/`-tön`; the oblique stem replaces that
/// with `-toma-` (class 34) or `-ttoma-` (class 34*C). So every `tom`/
/// `töm` in the word is a possible tail, with the doubled `tt` variant
/// tried as well (`koristeettominta` -> `koristeeton`).
fn ton_candidates(word: &str) -> Vec<String> {
    let mut out = Vec::new();
    if word.ends_with("ton") || word.ends_with("tön") {
        out.push(word.to_string());
    }
    for (needle, suffix) in [
        ("ttom", "ton"),
        ("ttöm", "tön"),
        ("tom", "ton"),
        ("töm", "tön"),
        ("tonta", "ton"),
        ("töntä", "tön"),
    ] {
        for (i, _) in word.match_indices(needle) {
            let prefix = &word[..i];
            if prefix.chars().count() >= 3 {
                out.push(format!("{prefix}{suffix}"));
            }
        }
    }
    out.sort();
    out.dedup();
    out
}
