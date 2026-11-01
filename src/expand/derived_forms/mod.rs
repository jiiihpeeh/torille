//! Derived and classless forms: compound heads, numerals and adjective
//! degrees, plus the productive suffix recognisers re-exported below.
//!
//! These are the helpers `inflect_in` consults after the class engines have
//! had their say. The file was split by suffix family; the public paths are
//! re-exported from `expand`, so callers do not change.

use crate::kotus::{EmbeddedLemma, Inflection};
use crate::spec::Catalogues;
use crate::tags::{Confidence, PartOfSpeech};

use super::degrees;
use super::lexicon_index::LexiconIndex;
use super::{Generated, inflect_all, inflect_in};

mod agent;
mod inen;
mod kas;
mod llinen;
mod ton;
mod uus;

pub use agent::{derived_agent_base, derived_tar_base};
pub(crate) use inen::derived_inen_forms;
pub use inen::{derived_inen_base, derived_lainen_base};
pub use kas::derived_kas_base;
pub use llinen::derived_llinen_base;
pub use ton::derived_ton_base;
pub use uus::derived_uus_base;

/// The sanalista row for a compound head, including the suffix entries the
/// dictionary writes with a leading non-breaking hyphen (`‑mielisyys`,
/// `‑peräinen`).
///
/// Those rows are classed but not standalone words: they say how the suffix
/// inflects when it is the head of a compound (`kevytmielisyys`,
/// `salaperäinen`). The head passed to the class engine is the ordinary word
/// without the placeholder hyphen; only the row lookup uses the placeholder.
fn compound_head_row(head: &str) -> Option<EmbeddedLemma> {
    let lexicon = crate::kotus::sanalista();
    lexicon.get(head).or_else(|| {
        let mut bound = String::with_capacity(head.len() + '\u{2011}'.len_utf8());
        bound.push('\u{2011}');
        bound.push_str(head);
        lexicon.get(&bound)
    })
}

/// The forms of a classless lemma that is a compound with a classed head.
///
/// The sanalista gives a compound like `periaate` (= `peri` + `aate`) or
/// `täysi-ikäinen` (= `täysi` + `ikäinen`) no class, so nothing else makes
/// its forms; but the head is a classed word and the whole inflects as the
/// head with the modifier in front. The modifier must itself be a word or
/// a combining form (a stem-map key), which is what keeps this off the
/// classless words whose right half only coincidentally looks like a
/// lemma -- the over-generation that made this rule unusable without a
/// constraint.
///
/// The head must be a nominal (noun, adjective or numeral): a verb is not
/// a compound head. The spans are `Repaired`: real forms, but built by
/// rule rather than read from a catalogue.
pub(super) fn compound_head_forms(
    lemma: &str,
    cat: &Catalogues,
    lexicon: Option<&LexiconIndex>,
    pos: PartOfSpeech,
    possessives: bool,
) -> Vec<Generated> {
    for split in 1..lemma.len() {
        if !lemma.is_char_boundary(split) {
            continue;
        }
        let (modifier, head) = lemma.split_at(split);
        if modifier.is_empty() || head.is_empty() {
            continue;
        }
        let Some(row) = compound_head_row(head) else {
            continue;
        };
        let inflections: Vec<Inflection> = row.inflections().collect();
        if inflections.is_empty() {
            continue;
        }
        // The head's own field only decides whether it *can* head a nominal
        // compound. The compound's reading is the lemma's: `aamukylmä` and
        // `aamuvarhainen` are `adjektiivi, substantiivi` even though their
        // heads are adjective-first or adjective-only, and their possessive
        // forms must carry the noun reading. Taking the head's first reading
        // made `aamukylmienne` and `aamuvarhaiseemme` adjectives.
        let head_pos = crate::tags::PartOfSpeech::inflecting_reading(row.part_of_speech());
        if !matches!(
            head_pos,
            PartOfSpeech::Noun | PartOfSpeech::Adjective | PartOfSpeech::Numeral
        ) {
            continue;
        }
        if !modifier_is_known(modifier) {
            continue;
        }
        let pos = if pos == PartOfSpeech::Unknown {
            head_pos
        } else {
            pos
        };
        let mut out: Vec<Generated> = Vec::new();
        // A noun compound does not take the head's adjective degrees:
        // `happiradikaali` is a noun, and `happiradikaaleimmittaan` is not a
        // word. An adjective compound does take them -- `tummanpunaisempi` is
        // `tumman` + `punaisempi` -- so only the nominal case drops the derived
        // layers.
        let head_forms = if pos == PartOfSpeech::Adjective {
            inflect_all(head, &inflections, cat, lexicon, pos, possessives)
        } else {
            super::inflect_paradigms(head, &inflections, cat, lexicon, pos, possessives)
        };
        for form in head_forms {
            out.push(
                Generated::new(format!("{modifier}{}", form.word), form.union())
                    .with(Confidence::Repaired),
            );
        }
        if !out.is_empty() {
            return out;
        }
    }
    Vec::new()
}

/// Whether `modifier` can be the first member of a compound. A trailing
/// hyphen is ignored for the check, since it separates the members in
/// writing (`täysi-` of `täysi-ikäinen`).
///
/// The modifier is one of:
///   * a sanalista word (`täysi-` of `täysi-ikäinen`);
///   * a combining form, a noun/numeral stem-map key (`nais-` of `nainen`,
///     `kansalais-`, `kolmi-` of `kolme`); an adjective case stem is not a
///     combining form;
///   * a known inflected form -- the modifier of a compound is very often
///     the genitive singular (`luonnon-`, `kansan-`, `maan-`), which the
///     compile-time form set carries;
///   * a `-minen` action noun's `-mis-` stem (`edistämis-` of
///     `edistäminen`), which is neither a lemma nor an inflected form;
///   * a bound derivational prefix (`esi-`, `epä-`, `pika-`), see
///     [`PREFIXES`].
///
/// Only the noun map counts for the combining-form case: a compound
/// modifier is a nominal stem, never a verb stem or a verb case form.
pub(crate) fn modifier_is_known(modifier: &str) -> bool {
    let bare = modifier.strip_suffix('-').unwrap_or(modifier);
    if bare.is_empty() {
        return false;
    }
    if crate::kotus::sanalista().by_word(bare).next().is_some() {
        return true;
    }
    // A combining form can be its own classless sanalista row, written
    // with the non-breaking hyphen the dictionary uses for bound
    // suffixes/prefixes (`yhteis‑` of `yhteiskunta`, `‑mielisyys`).
    let bound = format!("{bare}\u{2011}");
    if crate::kotus::sanalista().by_word(&bound).next().is_some() {
        return true;
    }
    // A plural-only lemma contributes its dropped-`t` stem as the modifier:
    // `housut` -> `housu` + `kauppa` (`housukauppa`), `käräjät` ->
    // `käräjä` + `tupa` (`käräjätupa`). The stem is not a lemma on its own,
    // so the direct probe above cannot see it. The `-t` convention marks a
    // plural-only citation form in every Kotus class, not just class 1.
    if crate::kotus::sanalista()
        .by_word(&format!("{bare}t"))
        .next()
        .is_some()
    {
        return true;
    }
    // The modifier of a `-minen` action noun is its `-mis-` stem:
    // `edistäminen` -> `edistämis-` (`edistämisvaroista`), `lukeminen` ->
    // `lukemis-` (`lukemisharjoitus`). The genitive (`edistämisen`) is not
    // the compound form, and the `-mis` stem is neither a lemma nor an
    // inflected form, so every probe above misses it. `edistäminen` itself
    // is often a productive derivation rather than a sanalista row, so ask
    // the non-compound gates for the `-minen` word instead of the lemma set.
    if let Some(stem) = bare.strip_suffix("mis")
        && !stem.is_empty()
        && crate::suggest::is_known_word_no_compound(&format!("{stem}minen")).is_some()
    {
        return true;
    }
    // The productive `-nen` combining stem is its `s` stem: `yhteinen` ->
    // `yhteis-` (`yhteiskunta`), `nainen` -> `nais-`, `kansalainen` ->
    // `kansalais-`. This is a derivational combining form, so it is
    // accepted even though the base is an adjective.
    if let Some(stem) = bare.strip_suffix('s')
        && !stem.is_empty()
        && crate::kotus::sanalista()
            .by_word(&format!("{stem}nen"))
            .next()
            .is_some()
    {
        return true;
    }
    // A combining form is also a nominal stem (`lanki-` of `lankki`,
    // `kolmi-` of `kolme`). The noun stem map contains every case stem,
    // including the adjective stem `irstai` (partitive plural of
    // `irstas`); accepting those split nonsense like `irstai` + `suu`.
    // Only a noun/numeral base licenses the stem.
    for base in crate::expand::check_normal::STEM_BUCKETS_NOUN.bases(bare) {
        if let Some(row) = crate::kotus::sanalista().by_word(base).next() {
            let pos = PartOfSpeech::parse_all(row.part_of_speech());
            if pos.contains(&PartOfSpeech::Noun) || pos.contains(&PartOfSpeech::Numeral) {
                return true;
            }
        }
    }
    if crate::expand::all_forms::is_known_inflected_form(bare) {
        return true;
    }
    if crate::expand::derived::is_modifier(bare) {
        return true;
    }
    is_bound_prefix(bare)
}

/// Bound derivational prefixes: the first member of a compound that is
/// not a word, an inflected form, or a stem on its own (`esi-merkki`,
/// `epä-selvä`, `pika-juna`). They are a small closed set; without them a
/// classless compound like `esimerkki` -- whose head `merkki` is a lemma
/// but whose modifier `esi` is nothing else -- cannot inflect by its
/// head, so `esimerkin`/`esimerkkiä` are missed.
///
/// Only entries that are *not* a lemma, a form or a stem belong here:
/// those three cases are handled above, so listing an ordinary word
/// (`maa`, `vesi`, `kivi`) or a genitive (`kansan`, `tumman`) would be
/// dead weight. The list is the frequent non-word, non-form, non-stem
/// modifiers of the classless sanalista compounds; a rarer one simply
/// leaves its word open, which is no regression.
///
/// A `matches!` over string literals, not a slice + `contains`: this is a
/// closed set of tokens, so the pattern-match reads as such and the
/// compiler turns it into a length/bytes decision tree.
fn is_bound_prefix(word: &str) -> bool {
    matches!(
        word,
        "epä" | "esi" | "pika" | "sisä" | "tasa" | "lähi" | "ylä" | "bio"
            | "kaksois" | "kauko" | "irto" | "etä" | "valko" | "nyky"
            | "raitio"
            | "yhdys" | "viher" | "kiinto" | "äkki" | "haja" | "uppo"
            | "uudis" | "oiko" | "alkeis" | "vähimmäis" | "enimmäis"
            | "digi" | "deka" | "giga" | "mega" | "tera" | "peta"
            | "supra" | "nano" | "trans" | "kolmois"
            | "kookos" | "jouto" | "etno" | "neo" | "psyko" | "super"
            | "hyper" | "kyber" | "maxi" | "pseudo" | "post" | "pre"
            | "inter" | "intra" | "extra"
            // Combining stems the sanalista uses in classless compounds:
            // `kyrmy` + `niska`, `housun` + `tasku`.
            | "kyrmy" | "housun"
            // Bound first members that are not standalone lemmas: `uima-`
            // (`uimapaikka`, `uimahalli`), the proper-name genitive
            // `kristin-` (`kristinusko`), and the `-nen` genitive stem
            // `vihollis-` (`vihollispuoli`, `vihollislaivasto`).
            | "uima" | "kristin" | "vihollis"
            // `toiviomatka`, `toivioretki`: the first member is not a
            // standalone lemma but the sanalista lists the compounds.
            | "toivio"
            // `synnyinseudulla` (`synnyin` + `seutu`) and `länkisääri`
            // (`länki` + `sääri`): bound first members. `hölynpöly` and
            // `suurkaani` use the bound `hölyn-` / `suur-` forms too.
            | "synnyin" | "länki" | "hölyn" | "suur"
            // `morsiussuudelma` (`morsius` + `suudelma`): the wedding
            // compounds in the sanalista all use this bound first member.
            | "morsius"
    )
}
/// The derived forms of a `-toista` numeral.
///
/// The teens (`kuusitoista` = 16) and their ordinals (`kuudestoista` =
/// 16th) are compounds whose *first* member inflects while `toista`
/// stays fixed: the genitive of `kuusitoista` is `kuuden` + `toista`.
/// The sanalista gives these words no class, so nothing else makes them;
/// the prefix is a numeral with a class of its own (`kuusi` : `kuuden`),
/// which supplies the paradigm.
pub(super) fn numeral_forms(
    lemma: &str,
    cat: &Catalogues,
    lexicon: Option<&LexiconIndex>,
) -> Vec<Generated> {
    let Some(prefix) = lemma.strip_suffix("toista").filter(|p| !p.is_empty()) else {
        return Vec::new();
    };
    // The prefix must be a *numeral*: `kuusi` is also the spruce
    // (substantiivi, class 24), whose paradigm would give `kuusentoista`.
    let Some(row) = crate::kotus::sanalista()
        .by_word(prefix)
        .find(|r| r.part_of_speech().contains("numeraali"))
    else {
        return Vec::new();
    };
    let inflections: Vec<Inflection> = row.inflections().collect();
    if inflections.is_empty() {
        return Vec::new();
    }
    let pos = crate::tags::PartOfSpeech::parse_all(row.part_of_speech())
        .first()
        .copied()
        .unwrap_or(PartOfSpeech::Noun);
    let mut out: Vec<Generated> = Vec::new();
    for form in inflect_all(prefix, &inflections, cat, lexicon, pos, false) {
        out.push(
            Generated::new(format!("{}toista", form.word), form.union()).with(Confidence::Repaired),
        );
    }
    out
}

/// The comparative (class 16) and superlative (class 36) paradigms an
/// adjective derives.
///
/// Finnish degrees are derivational, not inflectional: the sanalista
/// lists only 44 `-mpi` lemmas, so `suurempi`, `kauniimpi` and
/// `kaunein` belong to no lemma the engine can find. Omorfi derives them
/// (`tools/build_adj_degrees.py`); the engine emits the two paradigms
/// off the positive, so `find_bases` recognises `suuremman` as a form of
/// `suuri`.
///
/// The forms are `Repaired`: a degree is a derivation of the lemma, not
/// a plain attachment of a case ending, so it is accepted but never
/// offered as a correction.
pub(super) fn degree_forms(
    lemma: &str,
    cat: &Catalogues,
    lexicon: Option<&LexiconIndex>,
) -> Vec<Generated> {
    let mut values = degrees::ADJ_DEGREES.bases(lemma);
    // `ADJ_DEGREES` is Omorfi-derived and therefore missing for some
    // `-inen` adjectives (`salaperäinen` is absent entirely, and
    // `herttainen` has a comparative but no superlative). The productive
    // pattern is `nen` -> `sempi`/`sin` (`punainen` ->
    // `punaisempi`/`punaisin`), and the class engines verify the rest.
    // This fallback only runs for a sanalista adjective: a noun ending in
    // `-nen` (`nainen`) must not grow comparison forms.
    let mut comparative = values.next().map(str::to_string).filter(|s| !s.is_empty());
    let mut superlative = values.next().map(str::to_string).filter(|s| !s.is_empty());
    if (comparative.is_none() || superlative.is_none())
        && let Some(stem) = lemma.strip_suffix("nen").filter(|stem| !stem.is_empty())
    {
        let is_adjective = crate::kotus::sanalista()
            .by_word(lemma)
            .next()
            .is_some_and(|row| {
                crate::tags::PartOfSpeech::parse_all(row.part_of_speech())
                    .contains(&PartOfSpeech::Adjective)
            });
        if is_adjective {
            comparative.get_or_insert_with(|| format!("{stem}sempi"));
            superlative.get_or_insert_with(|| format!("{stem}sin"));
        }
    }
    // `-kas`/`-käs` is the other productive adjective suffix whose degrees
    // Omorfi's table does not always carry (`äänne` -> `äännekäs` ->
    // `äännekkäämpi`/`äännekkäin`). The suffix gradates kk, so the degree
    // nominatives are built from the noun stem, not from the adjective.
    if comparative.is_none() || superlative.is_none() {
        for (suffix, comparative_tail, superlative_tail) in
            [("kas", "kkaampi", "kkain"), ("käs", "kkäämpi", "kkäin")]
        {
            let Some(stem) = lemma.strip_suffix(suffix).filter(|stem| !stem.is_empty()) else {
                continue;
            };
            let stem_is_noun = crate::kotus::sanalista()
                .by_word(stem)
                .next()
                .is_some_and(|row| {
                    crate::tags::PartOfSpeech::parse_all(row.part_of_speech())
                        .contains(&PartOfSpeech::Noun)
                });
            if stem_is_noun {
                comparative.get_or_insert_with(|| format!("{stem}{comparative_tail}"));
                superlative.get_or_insert_with(|| format!("{stem}{superlative_tail}"));
            }
            break;
        }
    }
    let mut out: Vec<Generated> = Vec::new();
    // The comparative is class `16*H`: its `-mpi` gradates to `-mm-`
    // (`helpompi` -> `helpomman`), and without the `H` the weak cells
    // came out `helpomp-` (`helpompalta`), which the engine then could
    // not match. The superlative's `-in` -> `-imm-` is the class 36
    // model's own, so it needs no gradation flag.
    let comparative_gradation = crate::kotus::Gradation::from_letter('H');
    for (nominal, class, gradation) in [
        (comparative.as_deref(), 16u16, comparative_gradation),
        (superlative.as_deref(), 36u16, None),
    ] {
        let Some(nominal) = nominal else {
            continue;
        };
        // A degree is an adjective form, but the oracle does confirm the
        // possessive cells of a comparative (`alamaisempattaan`,
        // `olemattomampaaan`), so they are generated here as `Repaired` like
        // every other degree form: accepted, never suggested. The part of
        // speech is fixed so that `paha` inflected as a noun still derives
        // adjective degrees.
        for form in inflect_in(
            nominal,
            Inflection { class, gradation },
            cat,
            lexicon,
            PartOfSpeech::Adjective,
            true,
        ) {
            out.push(form.with(Confidence::Repaired));
        }
    }
    out
}
