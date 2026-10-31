//! The productive `-ja` agent-noun and `-tar` feminine recognisers.

use crate::kotus::Inflection;
use crate::spec::Catalogues;
use crate::tags::PartOfSpeech;

use crate::expand::inflect_all;
use crate::expand::lexicon_index::LexiconIndex;

/// The derived `-ja`/`-jä` agent noun a surface form belongs to, if any.
///
/// `-ja` is the productive VERB -> NOUN agent rule (`heiluttaa` ->
/// `heiluttaja`, `laulaa` -> `laulaja`; `derivation::RULES`'s `AGENT_JA`),
/// but the derived noun is usually not a sanalista lemma, so it is in
/// neither the stem maps nor `find_bases` -- `heiluttajan` is unknown while
/// `heiluttaa` is known. Reconstruct the agent nominative from the surface,
/// reverse the rule to a verb lemma, and inflect the derived noun as class
/// 10 (back) or 12 (front) to verify the surface. Returns the derived noun,
/// not the verb.
/// The productive feminine `-tar`/`-tär` noun (`siivooja` ->
/// `siivoojatar`, `näyttelijä` -> `näyttelijätär`), and its inflected forms.
///
/// `-tar` attaches to an agent noun or another noun and inflects as class
/// 32*C (`siivoojattaren`). The source must be a known word; this is what
/// separates the productive derivation from the lexicalised `valtiatar`
/// (which the sanalista lists directly).
pub fn derived_tar_base(
    word: &str,
    cat: &Catalogues,
    lexicon: Option<&LexiconIndex>,
) -> Option<String> {
    let lower = word.to_lowercase();
    if !(lower.contains("tar") || lower.contains("tär")) {
        return None;
    }
    let mut candidates: Vec<String> = Vec::new();
    if lower.ends_with("tar") || lower.ends_with("tär") {
        candidates.push(lower.clone());
    }
    // Genitive/partitive stems double the t: `siivoojattaren`,
    // `siivoojatarta`. Reconstruct the nominative from either shape.
    for marker in ["ttar", "ttär"] {
        if let Some(i) = lower.rfind(marker) {
            candidates.push(format!("{}{}", &lower[..i], &marker[1..]));
        }
    }
    for marker in ["tar", "tär"] {
        if let Some(i) = lower.rfind(marker) {
            candidates.push(format!("{}{}", &lower[..i], marker));
        }
    }
    candidates.sort();
    candidates.dedup();
    for candidate in candidates {
        let suffix = if candidate.ends_with("tär") {
            "tär"
        } else if candidate.ends_with("tar") {
            "tar"
        } else {
            continue;
        };
        let source = &candidate[..candidate.len() - suffix.len()];
        if source.is_empty() {
            continue;
        }
        let harmonic = crate::harmony::pick_harmony(crate::harmony::front(source), "tar", "tär");
        if suffix != harmonic {
            continue;
        }
        // The source must be a noun; `-tar` on the conjunction `ja` would
        // otherwise fabricate `jatar`, which then turns `valtijatar` into a
        // compound.
        let source_is_noun = crate::kotus::sanalista().by_word(source).any(|row| {
            crate::tags::PartOfSpeech::parse_all(row.part_of_speech()).contains(&PartOfSpeech::Noun)
        });
        if !source_is_noun {
            continue;
        }
        let inflection = Inflection {
            class: 32,
            gradation: Some(crate::kotus::Gradation(3)),
        };
        let forms = inflect_all(
            &candidate,
            &[inflection],
            cat,
            lexicon,
            PartOfSpeech::Noun,
            true,
        );
        if forms.iter().any(|f| f.word == lower) {
            return Some(candidate);
        }
    }
    None
}

pub fn derived_agent_base(
    word: &str,
    cat: &Catalogues,
    lexicon: Option<&LexiconIndex>,
) -> Option<String> {
    // Cheap shape guard: the agent suffix is `-ja`/`-jä`, and an inflected
    // form keeps the `j` within a few characters of the end.
    if !word.chars().rev().take(8).any(|c| c == 'j') {
        return None;
    }
    let rule = crate::derivation::RULES
        .iter()
        .find(|r| r.id == "AGENT_JA")?;
    for candidate in agent_candidates(word) {
        for reverse in crate::derivation::reverse(&candidate) {
            if reverse.rule != rule.id {
                continue;
            }
            let Some(source) = crate::kotus::sanalista().by_word(&reverse.source).next() else {
                continue;
            };
            if !crate::tags::PartOfSpeech::parse_all(source.part_of_speech())
                .contains(&PartOfSpeech::Verb)
            {
                continue;
            }
            if crate::derivation::derive(&reverse.source, rule).as_deref()
                != Some(candidate.as_str())
            {
                continue;
            }
            for class in [10u16, 12u16] {
                let inflection = Inflection {
                    class,
                    gradation: None,
                };
                let forms = inflect_all(
                    &candidate,
                    &[inflection],
                    cat,
                    lexicon,
                    PartOfSpeech::Noun,
                    true,
                );
                if forms.iter().any(|f| f.word == word) {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

/// Candidate agent nominatives for `word`: the word itself, and the word
/// with a possessive and/or case ending peeled off. Only candidates ending
/// in `-ja`/`-jä` survive; the paradigm verify rejects the rest.
fn agent_candidates(word: &str) -> Vec<String> {
    let mut out = vec![word.to_string()];
    let peel = |base: &str, out: &mut Vec<String>| {
        for ending in crate::expand::check_normal::ENDINGS
            .iter()
            .copied()
            .chain(["n", "t", "a", "ä", "i", "e"])
        {
            if ending.is_empty() {
                continue;
            }
            if let Some(stem) = base.strip_suffix(ending) {
                out.push(stem.to_string());
            }
        }
    };
    peel(word, &mut out);
    for possessive in ["nsa", "nsä", "nne", "mme", "si", "ni", "an", "än", "en"] {
        if let Some(stem) = word.strip_suffix(possessive) {
            out.push(stem.to_string());
            peel(stem, &mut out);
        }
    }
    out.retain(|candidate| candidate.ends_with("ja") || candidate.ends_with("jä"));
    out.sort();
    out.dedup();
    out
}
