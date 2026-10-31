//! The productive `-uus`/`-yys` abstract-noun recogniser.

use crate::kotus::Inflection;
use crate::spec::Catalogues;
use crate::tags::PartOfSpeech;

use crate::expand::inflect_all;
use crate::expand::lexicon_index::LexiconIndex;

/// The derived `-uus`/`-yys` abstract noun a surface form belongs to, if
/// any.
///
/// `-uus` is the productive ADJECTIVE -> NOUN rule (`irstainen` ->
/// `irstaisuus`, `suomalainen` -> `suomalaisuus`; `derivation::RULES`'s
/// `ABSTRACT_UUS`). The derived noun is usually not a sanalista lemma, so
/// `irstaisuus`/`irstaisuutta` are in neither the stem maps nor the form
/// set. Reconstruct the nominative from any `uu`/`yy` in the surface,
/// reverse the rule to the adjective base, and inflect the noun as class
/// 40 to verify the exact surface.
pub fn derived_uus_base(
    word: &str,
    cat: &Catalogues,
    lexicon: Option<&LexiconIndex>,
) -> Option<String> {
    let rule = crate::derivation::RULES
        .iter()
        .find(|r| r.id == "ABSTRACT_UUS")?;
    for candidate in uus_candidates(word) {
        for reverse in crate::derivation::reverse(&candidate) {
            if reverse.rule != rule.id {
                continue;
            }
            let Some(source) = crate::kotus::sanalista().by_word(&reverse.source).next() else {
                continue;
            };
            if !crate::tags::PartOfSpeech::parse_all(source.part_of_speech())
                .contains(&PartOfSpeech::Adjective)
            {
                continue;
            }
            if crate::derivation::derive(&reverse.source, rule).as_deref()
                != Some(candidate.as_str())
            {
                continue;
            }
            let forms = inflect_all(
                &candidate,
                &[Inflection {
                    class: 40,
                    gradation: None,
                }],
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
    None
}

/// Candidate nominatives for a derived `-uus`/`-yys` noun surface.
///
/// The nominative is `...uu` + `s` (or `...yy` + `s`), so every `uu`/`yy`
/// in the word is a possible stem boundary: `irstaisuutta` -> `irstaisuus`,
/// `irstaisuudessa` -> `irstaisuus`. Both harmonies are tried and the
/// reverse-derivation check rejects the wrong one.
fn uus_candidates(word: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (i, _) in word.match_indices("uu").chain(word.match_indices("yy")) {
        let prefix = &word[..i];
        if prefix.chars().count() < 3 {
            continue;
        }
        out.push(format!("{prefix}uus"));
        out.push(format!("{prefix}yys"));
    }
    out.sort();
    out.dedup();
    out
}
