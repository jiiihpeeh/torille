//! The productive `-llinen` adjective recogniser.

use crate::kotus::Inflection;
use crate::spec::Catalogues;
use crate::tags::PartOfSpeech;

use crate::expand::inflect_all;
use crate::expand::lexicon_index::LexiconIndex;

use super::inen::inen_candidates;

/// The derived `-llinen` adjective a surface form belongs to, when the base
/// is a noun lemma that does not fit the `i`/`ni`/`de` alternations of the
/// `-llinen` derivation (`kiulu` -> `kiulullinen`, not `kiuluinen`).
///
/// The nominative is reconstructed from the `-ise-`/`-is-` oblique spelling
/// the same way [`derived_inen_base`] does, then the whole adjective is
/// inflected as class 38 and the surface is verified.
pub fn derived_llinen_base(
    word: &str,
    cat: &Catalogues,
    lexicon: Option<&LexiconIndex>,
) -> Option<String> {
    for candidate in inen_candidates(word) {
        let Some(base) = candidate.strip_suffix("llinen") else {
            continue;
        };
        if base.is_empty() {
            continue;
        }
        // The base is often a stem, not a lemma: `orsi` -> `orre` ->
        // `orrellinen`. The noun stem map names the lemma behind `orre`;
        // a listed base (`onni`) is accepted directly.
        let base_known = crate::kotus::sanalista().by_word(base).next().is_some()
            || crate::expand::check_normal::STEM_BUCKETS_NOUN.contains(base);
        if !base_known {
            continue;
        }
        let forms = inflect_all(
            &candidate,
            &[Inflection {
                class: 38,
                gradation: None,
            }],
            cat,
            lexicon,
            PartOfSpeech::Adjective,
            true,
        );
        if forms.iter().any(|f| f.word == word) {
            return Some(candidate);
        }
    }
    None
}
