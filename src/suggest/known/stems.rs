//! Stem-map and possessive probes used by the non-compound gates.

use crate::tags::PartOfSpeech;

use super::catalogue;

pub(crate) fn is_content_head(word: &str) -> bool {
    crate::compound::is_content_head(word)
}


/// The whole classless compound lemma `left + base`, if that is a listed
/// compound. `suurkaani` is a classless row, so the base should be the whole
/// word rather than its head `kaani`.
pub(super) fn classless_compound_whole(left: &str, base: &str) -> Option<String> {
    let whole = format!("{left}{base}");
    let row = crate::kotus::sanalista().by_word(&whole).next()?;
    row.inflections().next().is_none().then_some(whole)
}

/// The head a known classless compound lemma names, when the right half is
/// ambiguous.
///
/// The stem map is a flat surface -> lemma index, so a right half that two
/// lemmas share (`teiden` is the genitive plural of both `tee` and `tie`)
/// comes back with one of them, first-wins. The compound itself is the
/// disambiguator: `raitiotie` is a lemma, while `raitiotee` is not, so the
/// head is `tie`. Returns `None` when no base makes the whole word a lemma,
/// leaving the ordinary first-wins answer in place.
pub(super) fn compound_head_from_lemma(left: &str, right: &str) -> Option<String> {
    use crate::expand::check_normal::{ENDINGS, STEM_BUCKETS_NOUN};
    let lexicon = crate::kotus::sanalista();
    let mut bases: Vec<String> = Vec::new();
    for base in STEM_BUCKETS_NOUN.bases(right) {
        bases.push(base.to_string());
    }
    for ending in ENDINGS {
        if ending.is_empty() {
            continue;
        }
        let Some(stem) = right.strip_suffix(ending) else {
            continue;
        };
        for base in STEM_BUCKETS_NOUN.bases(stem) {
            bases.push(base.to_string());
        }
    }
    bases.sort();
    bases.dedup();
    for base in bases {
        if let Some(whole) = classless_compound_whole(left, &base) {
            // A classless row is a compound whose head is `base`; name the
            // whole lemma (`suurkaani`, `raitiotie`) rather than the head.
            return Some(whole);
        }
        if lexicon.by_word(&format!("{left}{base}")).next().is_some() {
            // A classed row is the head lemma itself and keeps the old
            // answer.
            return Some(base);
        }
    }
    None
}

/// Verify that `base` inflects to `word` under `pos`. The lib's
/// `find_base_via_*` helpers take the full word and do the
/// ending strip internally; here we already have the stripped
/// stem and the candidate base, so a direct `inflect_all` and
/// `any` probe is the right shape.
pub(super) fn find_base_via_stem_map_static(
    word: &str,
    _stem: &str,
    base: &str,
    pos: PartOfSpeech,
) -> Option<String> {
    use crate::kotus::sanalista;
    let lex = sanalista();
    let lemma = lex.by_word(base).next()?;
    let inflections: Vec<_> = lemma.inflections().collect();
    // `emits_form`, not `inflect_all`: the compound split search asks the same
    // (surface, base) pair many times -- once per pass, per possessive variant,
    // per recursion -- and `emits_form` caches the built paradigm. Listing it
    // rebuilt a 1 200-form verb paradigm per ask.
    let hit = crate::expand::emits_form(
        lemma.word(),
        &inflections,
        catalogue(),
        None,
        pos,
        false,
        word,
    );
    if hit {
        return Some(base.to_string());
    }
    None
}

/// Verify a possessive surface against the noun stem-map candidates of
/// its illative host. `kotiinsa` = `kotiin` + `-sa`; `kotiin` is also a
/// lemma, so the ordinary possessive probe tries the wrong paradigm.
pub(super) fn possessive_base_via_stem_map(word: &str, host: &str) -> Option<String> {
    for ending in crate::expand::check_normal::ENDINGS {
        if ending.is_empty() {
            continue;
        }
        let Some(stem) = host.strip_suffix(ending) else {
            continue;
        };
        if stem.is_empty() || !crate::expand::check_normal::STEM_BUCKETS_NOUN.contains(stem) {
            continue;
        }
        let mut fallback = None;
        for base in crate::expand::check_normal::STEM_BUCKETS_NOUN.bases(stem) {
            if let Some(b) = find_base_via_stem_map_static_possessive(word, base) {
                // `kotiinsa` is both `koti` + `-in` + `-sa` and the same
                // surface from `kota`; prefer the nominative that is
                // actually the start of the word (`koti`).
                if word.starts_with(&b) {
                    return Some(b);
                }
                fallback.get_or_insert(b);
            }
        }
        if fallback.is_some() {
            return fallback;
        }
    }
    None
}

/// Verify a possessive form against the noun paradigm.
///
/// Used for the contracted genitive plural (`talojensa`, `veljiensä`),
/// whose stem is one `e` shorter than the post-possessive host. Unlike the
/// direct stem-map probe, this asks the engine for the possessive forms, so
/// only a surface the paradigm really emits is accepted.
pub(super) fn find_base_via_stem_map_static_possessive(word: &str, base: &str) -> Option<String> {
    use crate::kotus::sanalista;
    // A base can be listed under more than one homonym row; the possessive
    // form may come from a later class (`kuusi` is both the spruce and the
    // numeral). Try every row before rejecting the stem.
    for lemma in sanalista().by_word(base) {
        let inflections: Vec<_> = lemma.inflections().collect();
        if crate::expand::emits_form(
            lemma.word(),
            &inflections,
            catalogue(),
            None,
            PartOfSpeech::Noun,
            true,
            word,
        ) {
            return Some(base.to_string());
        }
    }
    None
}

/// Verb counterpart. Both call into the same `inflect_all` and
/// differ on the `pos` argument.
fn find_base_via_verb_stem_map_static(
    word: &str,
    _stem: &str,
    base: &str,
    pos: PartOfSpeech,
) -> Option<String> {
    use crate::kotus::sanalista;
    let lex = sanalista();
    let lemma = lex.by_word(base).next()?;
    let inflections: Vec<_> = lemma.inflections().collect();
    let hit = crate::expand::emits_form(
        lemma.word(),
        &inflections,
        catalogue(),
        None,
        pos,
        false,
        word,
    );
    if hit {
        return Some(base.to_string());
    }
    None
}

/// Probe the verb stem map for `lower` at `stem`, also trying the
/// vowel-stripped stem. The builder stores a non-finite stem without its
/// trailing stem vowels (`lukeneen`, `lukeneita` and `lukenutta` all ->
/// `luken`), so one key covers a whole participle/infinitive paradigm.
/// The verify in `find_base_via_verb_stem_map_static` keeps it honest.
pub(super) fn verb_stem_probe(lower: &str, stem: &str) -> Option<String> {
    use crate::expand::check_normal::STEM_BUCKETS_VERB;
    let trimmed = stem.trim_end_matches(crate::is_vowel);
    for cand in [stem, trimmed] {
        if cand.is_empty() || !STEM_BUCKETS_VERB.contains(cand) {
            continue;
        }
        for base in STEM_BUCKETS_VERB.bases(cand) {
            if let Some(b) =
                find_base_via_verb_stem_map_static(lower, cand, base, PartOfSpeech::Verb)
            {
                return Some(b);
            }
        }
    }
    None
}
