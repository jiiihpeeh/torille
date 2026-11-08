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
/// 32*C (`siivoojattaren`). The source must be a known noun; this is what
/// separates the productive derivation from the lexicalised `valtiatar`
/// (which the sanalista lists directly).
///
/// The suffix does not always attach to the source's nominative: it takes
/// the weak stem (`pappi` -> `papi-`, `henki` -> `henge-`, `aalto` ->
/// `aallo-`, `neuvos` -> `neuvokse-`), and some nominatives lose a final
/// `-s` (`kuningas` -> `kuningatar`, `ruhtinas` -> `ruhtinatar`). The noun
/// stem map recovers those sources from the prefix, so `papitar`,
/// `hengetär`, `aallotar` and `kuningatar` are derived and not merely read
/// off a sanalista row.
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
        let prefix = &candidate[..candidate.len() - suffix.len()];
        // `-tar` never attaches to a one-letter stem; keeping a floor also
        // keeps the stem-map probe from matching short function words.
        if prefix.chars().count() < 2 {
            continue;
        }
        // At least one possible source has to agree on the suffix's harmony;
        // the paradigm verify then runs once, not once per source. `pappi`
        // is a back word, so `papitär` is rejected before any inflection.
        let harmony_matches = tar_sources(prefix).iter().any(|source| {
            suffix == crate::harmony::pick_harmony(crate::harmony::front(source), "tar", "tär")
        });
        if !harmony_matches {
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

/// The noun lemmas a `-tar`/`-tär` prefix may belong to.
///
/// The prefix is the surface before the suffix, so it is not always a
/// nominative. Three routes recover the source:
///
/// * the prefix itself is the lemma (`laulaja`, `sankari`, `ranska`);
/// * it is the weak stem the suffix attached to, which the noun stem map
///   maps back to the lemma (`papi` -> `pappi`, `henge` -> `henki`,
///   `aallo` -> `aalto`, `neuvokse` -> `neuvos`);
/// * the nominative ends in `-s`, which the suffix replaces rather than
///   follows (`kuninga` + `s` -> `kuningas`, `ruhtina` + `s` -> `ruhtinas`).
///
/// Every candidate must be a noun in the lexicon; that is what keeps the
/// rule from fabricating a feminine from a conjunction (`ja` -> `jatar`) or
/// from any string that merely ends in the suffix.
fn tar_sources(prefix: &str) -> Vec<String> {
    let lexicon = crate::kotus::sanalista();
    let is_noun = |word: &str| {
        lexicon.by_word(word).any(|row| {
            crate::tags::PartOfSpeech::parse_all(row.part_of_speech()).contains(&PartOfSpeech::Noun)
        })
    };
    let mut out: Vec<String> = Vec::new();
    if is_noun(prefix) {
        out.push(prefix.to_string());
    }
    for base in crate::expand::check_normal::STEM_BUCKETS_NOUN.bases(prefix) {
        if is_noun(base) {
            out.push(base.to_string());
        }
    }
    let with_s = format!("{prefix}s");
    if is_noun(&with_s) {
        out.push(with_s);
    }
    out.sort();
    out.dedup();
    out
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

#[cfg(test)]
mod tests {
    use super::*;

    fn tar_base(word: &str) -> Option<String> {
        derived_tar_base(word, Catalogues::embedded(), None)
    }

    /// The established and productive `-tar`/`-tär` nouns from the word lists
    /// in the Kotus/Kielikello material: professions, royalty, mythology and
    /// nationality. Some are sanalista lemmas and some are not; the derived
    /// layer must recognise both.
    #[test]
    fn established_and_productive_feminine_nouns_are_recognised() {
        let words = [
            // People, professions and occupations.
            "näyttelijätär",
            "laulajatar",
            "kirjailijatar",
            "opettajatar",
            "myyjätär",
            "juoksijatar",
            "perijätär",
            "rakastajatar",
            "papitar",
            "suojelijatar",
            "kenraalitar",
            "neuvoksetar",
            "laamannitar",
            // Royalty, nobility and authority.
            "kuningatar",
            "ruhtinatar",
            "herttuatar",
            "valtiatar",
            "jumalatar",
            // Relationships, myth and poetry.
            "ystävätär",
            "sankaritar",
            "hengetär",
            "haltijatar",
            "onnetar",
            "kohtalotar",
            "pahatar",
            "aallotar",
            "kaunotar",
            // Nationality and origin.
            "saksatar",
            "ranskatar",
            "englantitar",
            "kreikatar",
            "italiatar",
            "tanssijatar",
            "urheilijatar",
            "taiteilijatar",
            "runoilijatar",
            "tutkijatar",
            "johtajatar",
            "hallitsijatar",
            "vartijatar",
            "palvelijatar",
            "voittajatar",
            "kilpailijatar",
            "seikkailijatar",
        ];
        let missing: Vec<&str> = words
            .iter()
            .copied()
            .filter(|word| tar_base(word).is_none())
            .collect();
        assert!(
            missing.is_empty(),
            "unrecognised -tar/-tär nouns: {missing:?}"
        );
    }

    /// The suffix attaches to the weak stem, not the nominative, and the
    /// noun stem map is what recovers the lemma from it.
    #[test]
    fn weak_stems_recover_the_source_noun() {
        for (prefix, source) in [
            ("papi", "pappi"),
            ("henge", "henki"),
            ("onne", "onni"),
            ("aallo", "aalto"),
            ("neuvokse", "neuvos"),
            ("kreika", "kreikka"),
            ("englanni", "englanti"),
            ("valtia", "valtias"),
        ] {
            let sources = tar_sources(prefix);
            assert!(
                sources.iter().any(|candidate| candidate == source),
                "{prefix} should recover {source}, got {sources:?}"
            );
        }
    }

    /// A nominative ending in `-s` loses it before `-tar`: the prefix is one
    /// letter short of the lemma, so the extra `s` route is what finds it.
    #[test]
    fn an_s_nominative_recovers_before_tar() {
        for (prefix, source) in [
            ("kuninga", "kuningas"),
            ("ruhtina", "ruhtinas"),
            ("valtia", "valtias"),
        ] {
            let sources = tar_sources(prefix);
            assert!(
                sources.iter().any(|candidate| candidate == source),
                "{prefix} should recover {source}, got {sources:?}"
            );
        }
    }

    /// The inflected forms resolve to the derived nominative, so `check` and
    /// `suggest` name the feminine noun rather than the surface.
    #[test]
    fn inflected_forms_resolve_to_the_nominative() {
        for (surface, expected) in [
            ("papittaren", "papitar"),
            ("papitarta", "papitar"),
            ("hengettären", "hengetär"),
            ("aallottaren", "aallotar"),
            ("neuvoksettaren", "neuvoksetar"),
            ("kuningattaren", "kuningatar"),
            ("ruhtinattaren", "ruhtinatar"),
            ("sankarittaren", "sankaritar"),
            ("laulajattaren", "laulajatar"),
            ("opettajattaren", "opettajatar"),
            ("näyttelijättären", "näyttelijätär"),
            ("myyjättären", "myyjätär"),
            ("ystävättären", "ystävätär"),
        ] {
            assert_eq!(
                tar_base(surface).as_deref(),
                Some(expected),
                "{surface} should resolve to {expected}"
            );
        }
    }

    /// A non-noun host must not fabricate a feminine: `-tar` on the
    /// conjunction `ja` is not `jatar`.
    #[test]
    fn a_non_noun_host_is_rejected() {
        assert!(tar_base("jatar").is_none());
        assert!(tar_base("jatarin").is_none());
    }

    /// The suffix's harmony is the source noun's, not the surface's: `pappi`
    /// is a back word, so `papitär` is not a form of it.
    #[test]
    fn the_suffix_must_match_the_source_harmony() {
        assert!(tar_base("papitär").is_none());
        assert!(tar_base("hengetar").is_none());
    }
}
