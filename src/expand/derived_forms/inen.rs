//! The productive `-inen` and `-lainen` adjective recognisers.

use crate::kotus::Inflection;
use crate::spec::Catalogues;
use crate::tags::{Confidence, PartOfSpeech};

use crate::expand::lexicon_index::LexiconIndex;
use crate::expand::{Generated, inflect_all};

use super::modifier_is_known;

/// True when `candidate`'s `-inen` base is a sanalista lemma. The `i` of
/// `-inen` may belong to the base (`mieli` + `nen`) or to the suffix
/// (`tahko` + `inen`), and `ruumis` + `-inen` drops the `s`, so both cuts
/// and the `s` variant are tried.
fn inen_base_is_lemma(candidate: &str) -> bool {
    let Some(base) = candidate.strip_suffix("inen") else {
        return false;
    };
    let shorter = candidate.strip_suffix("nen").unwrap_or("");
    let lexicon = crate::kotus::sanalista();
    let known = |b: &str| {
        !b.is_empty()
            && (lexicon.by_word(b).next().is_some()
                || lexicon.by_word(&format!("{b}s")).next().is_some())
    };
    [base, shorter].iter().any(|b| {
        // `pää` + `-inen` drops one of the long vowels (`päinen`), so try
        // the base with its final vowel doubled as well.
        known(b)
            || b.chars()
                .last()
                .filter(|c| crate::is_vowel(*c))
                .is_some_and(|c| known(&format!("{b}{c}")))
    })
}

/// Split a compound `-inen` adjective into `(modifier, head)` where the
/// head is a derived `-inen` adjective of a lemma: `vastenmielinen` ->
/// (`vasten`, `mielinen`), `henkiruumiinen` -> (`henki`, `ruumiinen`).
fn inen_compound_split(lemma: &str) -> Option<(&str, &str)> {
    for split in 1..lemma.len() {
        if !lemma.is_char_boundary(split) {
            continue;
        }
        let (modifier, head) = lemma.split_at(split);
        if !head.ends_with("inen") || !modifier_is_known(modifier) {
            continue;
        }
        if inen_base_is_lemma(head) {
            return Some((modifier, head));
        }
    }
    None
}

/// The forms of a classless `-inen` adjective (`kovakorvainen`,
/// `yksinkertainen`). Its head is a *derived* adjective -- `korva` +
/// `-inen`, `kerta` + `-inen` -- not a sanalista lemma, so
/// [`compound_head_forms`] cannot inflect it; but the whole word inflects
/// as the `-inen` class (38, `nainen`), whose `-inen` -> `-ise-`
/// alternation gives `kovakorvaisen`, `kovakorvaisia`, ... The `-llinen`
/// (`onnellinen`), `-lainen` (`suomalainen`) and `-mainen`
/// (`kaupunkimainen`) adjectives share the `inen` tail and the class.
///
/// Not run when the lemma is classed (it has its own paradigm) or when
/// `compound_head_forms` already resolved it (its head is a real lemma).
/// The forms are `Repaired`: real, but built by rule.
pub(crate) fn derived_inen_forms(
    lemma: &str,
    cat: &Catalogues,
    lexicon: Option<&LexiconIndex>,
    pos: PartOfSpeech,
    possessives: bool,
) -> Vec<Generated> {
    if !lemma.ends_with("inen") {
        return Vec::new();
    }
    // A compound `-inen` adjective takes its vowel harmony from the
    // *head*, not the whole word: `vastenmielinen` = `vasten` + `mielinen`
    // and `mielinen` is front-harmony, so the partitive plural is
    // `vastenmielisiä`, not `vastenmielisia` (which is what inflecting the
    // whole word would give, because of the `a` in `vasten`). Inflect the
    // head on its own (its harmony is then right) and prefix the modifier.
    let inflections = [Inflection {
        class: 38,
        gradation: None,
    }];
    if let Some((modifier, head)) = inen_compound_split(lemma) {
        let head_forms = inflect_all(head, &inflections, cat, lexicon, pos, possessives);
        if !head_forms.is_empty() {
            return head_forms
                .into_iter()
                .map(|f| {
                    Generated::new(format!("{modifier}{}", f.word), f.union())
                        .with(Confidence::Repaired)
                })
                .collect();
        }
    }
    // No compound split: the whole word as class 38.
    inflect_all(lemma, &inflections, cat, lexicon, pos, possessives)
        .into_iter()
        .map(|f| Generated::new(f.word.clone(), f.union()).with(Confidence::Repaired))
        .collect()
}

/// The derived `-inen` adjective a surface form belongs to, if any.
///
/// `-inen` is the productive NOUN -> ADJECTIVE rule (`tahko` ->
/// `tahkoinen`, `kivi` -> `kivinen`; `derivation::RULES`'s `ADJ_INEN`),
/// but the derived word is not a sanalista lemma, so it is in neither the
/// stem maps nor `find_bases` -- `tahkoisella` is unknown while the noun
/// `tahko` is known. Reconstruct the `-inen` nominative from the surface
/// (the oblique stem is `-ise-` / `-is-` / `-isi-`), require its `-inen`
/// base to be a sanalista lemma, then inflect the whole word as class 38
/// and verify the surface. Returns the derived adjective (e.g.
/// `tahkoinen`), not the noun.
///
/// This is the derivation layer, not a suffix stripper: class 38 does the
/// inflection, and the `-inen` base being a lemma bounds it to real
/// `-inen` bases.
pub fn derived_inen_base(
    word: &str,
    cat: &Catalogues,
    lexicon: Option<&LexiconIndex>,
) -> Option<String> {
    for candidate in inen_candidates(word) {
        // A real `-inen` base, a compound whose head is one
        // (`henkiruumiinen` = `henki` + `ruumiinen`), or a whole word whose
        // `-inen` base is a lemma (`tahkoinen` of `tahko`).
        let is_lemma = crate::kotus::sanalista()
            .by_word(&candidate)
            .next()
            .is_some();
        if !is_lemma && inen_compound_split(&candidate).is_none() && !inen_base_is_lemma(&candidate)
        {
            continue;
        }
        let forms = derived_inen_forms(&candidate, cat, lexicon, PartOfSpeech::Adjective, true);
        if forms.iter().any(|f| f.word == word) {
            return Some(candidate);
        }
    }
    None
}
/// The derived `-lainen`/`-läinen` adjective a surface form belongs to, if
/// any.
///
/// `-lainen` is the productive PLACE/PERSON -> ADJECTIVE rule (`Turku` ->
/// `turkulainen`, `Alberta` -> `albertalainen`; `derivation::RULES`'s
/// `ADJ_LAINEN`). The derived adjective is not a sanalista lemma, so
/// `turkulaisia` is unknown while `Turku` is known. Reconstruct the
/// nominative from the `-lais-` oblique stem, reverse the rule to a base,
/// require that base to be a known name or lemma, and inflect the adjective
/// as class 38 to verify the surface. Returns the derived adjective, not the
/// place.
pub fn derived_lainen_base(
    word: &str,
    cat: &Catalogues,
    lexicon: Option<&LexiconIndex>,
) -> Option<String> {
    let rule = crate::derivation::RULES
        .iter()
        .find(|r| r.id == "ADJ_LAINEN")?;
    for candidate in inen_candidates(word) {
        for reverse in crate::derivation::reverse(&candidate) {
            if reverse.rule != rule.id {
                continue;
            }
            let base = reverse.source.as_str();
            let known = crate::expand::proper_names::contains(base)
                || crate::kotus::sanalista().by_word(base).next().is_some();
            // A base ending in a consonant takes the epenthetic `i`
            // (`Quebec` -> `quebecilainen`, `Texas` -> `texasilainen`);
            // the plain rule's `*quebeclainen`/`*texaslainen` is not the
            // standard spelling.
            if known
                && !ends_in_consonant(base)
                && crate::derivation::derive(base, rule).as_deref() == Some(candidate.as_str())
                && lainen_candidate_matches(&candidate, word, cat, lexicon)
            {
                return Some(candidate);
            }
            // Place names in `-i` often take `-elainen`/`-eläinen` rather
            // than `-ilainen`: `Lahti` -> `lahtelainen`, `Rovaniemi` ->
            // `rovaniemeläinen`, `Seinäjoki` -> `seinäjokelainen`. The
            // reverse rule hands back the `e` stem, so map it to the `i`
            // lemma and verify the adjective.
            let Some(stem) = base.strip_suffix('e') else {
                continue;
            };
            let i_base = format!("{stem}i");
            let known = crate::expand::proper_names::contains(&i_base)
                || crate::kotus::sanalista().by_word(&i_base).next().is_some();
            if known && lainen_candidate_matches(&candidate, word, cat, lexicon) {
                return Some(candidate);
            }
        }
        // Some names take an epenthetic `i` before `-lainen`
        // (`Quebec` -> `quebecilainen`, `Hessen` -> `hesseniläinen`),
        // which the plain `ADJ_LAINEN` rule does not spell. The rule's
        // exact stem is irrelevant here: the known base plus class 38 is
        // what verifies the surface.
        let base = candidate
            .strip_suffix("ilainen")
            .or_else(|| candidate.strip_suffix("iläinen"));
        if let Some(base) = base {
            let known = crate::expand::proper_names::contains(base)
                || crate::kotus::sanalista().by_word(base).next().is_some();
            if known && ends_in_consonant(base) {
                let forms = inflect_all(
                    &candidate,
                    &[Inflection {
                        class: 38,
                        gradation: None,
                    }],
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

/// Whether the derived adjective `candidate` inflects to `word` as class 38.
fn lainen_candidate_matches(
    candidate: &str,
    word: &str,
    cat: &Catalogues,
    lexicon: Option<&LexiconIndex>,
) -> bool {
    inflect_all(
        candidate,
        &[Inflection {
            class: 38,
            gradation: None,
        }],
        cat,
        lexicon,
        PartOfSpeech::Adjective,
        true,
    )
    .iter()
    .any(|f| f.word == word)
}

/// Whether `word` ends in a consonant, which decides whether `-lainen`
/// takes the epenthetic `i` (`Quebec` -> `quebecilainen` vs `Alberta` ->
/// `albertalainen`).
fn ends_in_consonant(word: &str) -> bool {
    word.chars().last().is_some_and(|c| !crate::is_vowel(c))
}

/// Candidate `-inen` nominatives for `word`: the word itself when it ends
/// in `inen`, otherwise the `-ise-`/`-is-`/`-isi-` oblique stem mapped
/// back (`tahkoise` -> `tahkoi` + `nen`, `punais` -> `punai` + `nen`).
/// Possessives are stripped first so a form like `henkiruumiisenne`
/// (`henkiruumiise` + `nne`) yields `henkiruumiinen`.
pub(super) fn inen_candidates(word: &str) -> Vec<String> {
    fn add(s: &str, out: &mut Vec<String>) {
        if s.ends_with("inen") {
            out.push(s.to_string());
        } else if let Some(p) = s.strip_suffix("se") {
            out.push(format!("{p}nen"));
        } else if let Some(p) = s.strip_suffix("si") {
            out.push(format!("{p}nen"));
        } else if let Some(p) = s.strip_suffix('s') {
            out.push(format!("{p}nen"));
        }
        // The comitative of an `-inen` adjective is `-isineen`/`-isineni`
        // (`laitilalaisineen`): drop the owner suffix, the `s`, and put the
        // `nen` back.
        for suffix in ["ineen", "ineni", "inesi", "inemme", "inenne", "inensa"] {
            if let Some(p) = s.strip_suffix(suffix)
                && let Some(q) = p.strip_suffix('s')
            {
                out.push(format!("{q}nen"));
            }
        }
    }
    let mut out = Vec::new();
    let mut bases: Vec<&str> = vec![word];
    for poss in ["nsa", "nsä", "nne", "mme", "ni", "si", "an", "än", "en"] {
        if let Some(inner) = word.strip_suffix(poss) {
            bases.push(inner);
        }
    }
    for base in bases {
        add(base, &mut out);
        for ending in crate::expand::check_normal::ENDINGS {
            if ending.is_empty() {
                continue;
            }
            if let Some(stem) = base.strip_suffix(ending) {
                add(stem, &mut out);
            }
        }
    }
    out
}
