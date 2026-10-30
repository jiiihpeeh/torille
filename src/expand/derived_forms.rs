//! Derived and classless forms: compound heads, `-inen`, `-lainen`,
//! `-ja` agent nouns, numerals and adjective degrees.
//!
//! These are the helpers `inflect_in` consults after the class engines have
//! had their say. Split out of `expand/mod.rs`; the public entry points are
//! re-exported from `expand` so their paths do not change.

use crate::kotus::{EmbeddedLemma, Inflection};
use crate::spec::Catalogues;
use crate::tags::{Confidence, PartOfSpeech};

use super::degrees;
use super::lexicon_index::LexiconIndex;
use super::{Generated, inflect_all, inflect_in};

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
        for form in inflect_all(head, &inflections, cat, lexicon, pos, possessives) {
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
pub(super) fn derived_inen_forms(
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

/// The derived `-kas`/`-käs` adjective (and its degrees) a surface belongs to.
///
/// `-kas`/`-käs` is the productive NOUN -> ADJECTIVE rule (`voima` ->
/// `voimakas`, `äänne` -> `äännekäs`), but the derived adjective is usually
/// not a sanalista lemma, so it is in neither the stem maps nor
/// `find_bases`. Reconstruct the positive from the degree/normal surface,
/// require its stem to be a noun lemma, then inflect the whole class-41
/// adjective and verify the surface. This also covers the productive
/// comparative (`äännekkäämpi`) and superlative (`äännekkäin`).
pub fn derived_kas_base(
    word: &str,
    cat: &Catalogues,
    lexicon: Option<&LexiconIndex>,
) -> Option<String> {
    for candidate in kas_candidates(word) {
        let Some(stem) = candidate
            .strip_suffix("käs")
            .or_else(|| candidate.strip_suffix("kas"))
        else {
            continue;
        };
        if stem.is_empty() {
            continue;
        }
        let stem_is_noun = crate::kotus::sanalista()
            .by_word(stem)
            .next()
            .is_some_and(|row| {
                crate::tags::PartOfSpeech::parse_all(row.part_of_speech())
                    .contains(&PartOfSpeech::Noun)
            });
        if !stem_is_noun {
            continue;
        }
        let forms = inflect_all(
            &candidate,
            &[Inflection {
                class: 41,
                gradation: Some(crate::kotus::Gradation(1)),
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

/// Candidate `-kas`/`-käs` positives for a surface.
///
/// A surface can be the adjective itself, a case form of it, or a degree
/// (`-kkäämpi`/`-kkäin`). Strip the possible case ending, then undo the
/// degree or the kk gradation and put the positive suffix back.
fn kas_candidates(word: &str) -> Vec<String> {
    fn add(s: &str, out: &mut Vec<String>) {
        if s.ends_with("kas") || s.ends_with("käs") {
            out.push(s.to_string());
        }
        // Comparative and superlative stems after a case ending has been
        // stripped: `äännekkäämmä` -> `äännekäs`, `voimakkaimma` ->
        // `voimakas`.
        for (needle, replacement) in [
            ("kkäämm", "käs"),
            ("kkaamm", "kas"),
            ("kkäimm", "käs"),
            ("kkaimm", "kas"),
        ] {
            if let Some(at) = s.find(needle) {
                out.push(format!("{}{replacement}", &s[..at]));
            }
        }
        // A case form of the positive itself: `äännekkäässä` loses `ssä`
        // first, leaving `äännekkää`, whose kk belongs to the suffix.
        for (needle, replacement) in [("kkää", "käs"), ("kkaa", "kas")] {
            if let Some(prefix) = s.strip_suffix(needle) {
                out.push(format!("{prefix}{replacement}"));
            }
        }
    }

    let mut out = Vec::new();
    add(word, &mut out);
    for ending in crate::expand::check_normal::ENDINGS {
        if ending.is_empty() {
            continue;
        }
        if let Some(stem) = word.strip_suffix(ending) {
            add(stem, &mut out);
        }
    }
    out.sort();
    out.dedup();
    out
}

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
fn inen_candidates(word: &str) -> Vec<String> {
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
        for (suffix, comparative_tail, superlative_tail) in [
            ("kas", "kkaampi", "kkain"),
            ("käs", "kkäämpi", "kkäin"),
        ] {
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
        // A degree is an adjective form, and an adjective takes no
        // possessive suffix, so both are fixed here rather than taken
        // from the caller: `paha` inflected as a noun still derives
        // adjective degrees.
        for form in inflect_in(
            nominal,
            Inflection { class, gradation },
            cat,
            lexicon,
            PartOfSpeech::Adjective,
            false,
        ) {
            out.push(form.with(Confidence::Repaired));
        }
    }
    out
}
