//! Derived and classless forms: compound heads, numerals and adjective
//! degrees, plus the productive suffix recognisers re-exported below.
//!
//! These are the helpers `inflect_in` consults after the class engines have
//! had their say. The file was split by suffix family; the public paths are
//! re-exported from `expand`, so callers do not change.

use crate::kotus::{EmbeddedLemma, Inflection};
use crate::spec::Catalogues;
use crate::tags::{Case, Confidence, Number, PartOfSpeech, Tags};

use super::degrees;
use super::lexicon_index::LexiconIndex;
use super::{Generated, forms_of, inflect_all, inflect_in};

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
        lexicon
            .get(&bound)
            .or_else(|| lexicon.get(&format!("-{head}")))
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
    for head in classless_heads(lemma, cat, lexicon) {
        // The compound's reading is the lemma's, not the head's: `aamukylmä`
        // and `aamuvarhainen` are `adjektiivi, substantiivi` even though their
        // heads are adjective-first or adjective-only, and their possessive
        // forms must carry the noun reading. Taking the head's first reading
        // made `aamukylmienne` and `aamuvarhaiseemme` adjectives.
        let pos = if pos == PartOfSpeech::Unknown {
            head.pos
        } else {
            pos
        };
        let mut out: Vec<Generated> = Vec::new();
        // A noun compound does not take the head's adjective degrees:
        // `happiradikaali` is a noun, and `happiradikaaleimmittaan` is not a
        // word. An adjective compound does take them -- `tummanpunaisempi` is
        // `tumman` + `punaisempi` -- so only the nominal case drops the
        // derived layers.
        let head_forms = if pos == PartOfSpeech::Adjective {
            inflect_all(head.word, &head.inflections, cat, lexicon, pos, possessives)
        } else {
            super::inflect_paradigms(head.word, &head.inflections, cat, lexicon, pos, possessives)
        };
        for form in head_forms {
            // The compound is as trustworthy as its head: a plain head form
            // gives a plain compound form, and a repaired one (`väkein` for
            // `väkien`) stays repaired rather than being promoted by the
            // prefix. The modifier only adds the prefix; it does not repair
            // anything.
            out.push(
                Generated::new(format!("{}{}", head.modifier, form.word), form.union())
                    .with(form.confidence),
            );
        }
        if !out.is_empty() {
            return out;
        }
    }
    Vec::new()
}

/// The split of a classless compound: its modifier, its head and the head's
/// classes.
struct ClasslessHead<'a> {
    modifier: &'a str,
    word: &'a str,
    inflections: Vec<Inflection>,
    pos: PartOfSpeech,
    /// Whether the head is a bound form (`‑mielisyys`), not a free lemma.
    ///
    /// A bound form is inflected as a head but is a suffix, not a word, so it
    /// does not classify the compound; see [`head_inflections`].
    bound: bool,
}

/// The heads of a classless compound, longest first.
///
/// A split is a boundary whose right half is a classed nominal and whose left
/// half is a word, a form, a stem or a bound prefix. The head's class is what
/// the compound declines as: Kotus omits the type of a compound because it
/// declines like its final component. More than one split can pass (a word can
/// have a compound head that is itself a compound), and the caller takes the
/// first whose paradigm is not empty.
fn classless_heads<'a>(
    lemma: &'a str,
    cat: &Catalogues,
    lexicon: Option<&LexiconIndex>,
) -> Vec<ClasslessHead<'a>> {
    let mut heads = Vec::new();
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
        let top = crate::tags::PartOfSpeech::inflecting_reading(row.part_of_speech());
        if !matches!(
            top,
            PartOfSpeech::Noun | PartOfSpeech::Adjective | PartOfSpeech::Numeral
        ) {
            continue;
        }
        if !modifier_is_known(modifier)
            && !classless_modifier_is_known(modifier, cat, lexicon)
            && !verb_stem_is_a_modifier(modifier)
        {
            continue;
        }
        // `compound_head_row` also accepts the bound spelling (`‑head`); a
        // head found only that way is a suffix, not a word.
        let bound = crate::kotus::sanalista().get(head).is_none();
        heads.push(ClasslessHead {
            modifier,
            word: head,
            inflections,
            pos: top,
            bound,
        });
    }
    heads
}

/// The classes a classless compound declines as: its head's.
///
/// Kotus omits the type of a compound because it declines like its final
/// component, so the head's class is not a guess: `aamuhartaus` is `aamu` +
/// `hartaus`, and its paradigm is class 40's with the modifier in front.
///
/// A **bound** head does not classify: `‑mielisyys` in `leikkimielisyys` is a
/// listed head the engine inflects, but it is a suffix, not a word, and a class
/// taken from it would be a class taken from an ending. The first split is the
/// longest head, so the answer is `None` rather than the class of a shorter
/// head further in: `leikkimielisyys` must not become class 99 through `syys`.
///
/// `None` also when the head is not a classed nominal, or no split passes the
/// modifier check (a derived head, a verb stem, an unresolved modifier).
pub fn head_inflections(
    lemma: &str,
    cat: &Catalogues,
    lexicon: Option<&LexiconIndex>,
) -> Option<Vec<Inflection>> {
    let head = classless_heads(lemma, cat, lexicon).into_iter().next()?;
    if head.bound {
        return None;
    }
    Some(head.inflections)
}

/// The forms of a classless lemma that is a compound with a verb head.
///
/// `aliarvioida` is `ali` + `arvioida` and `aivopestä` is `aivo` + `pestä`:
/// Kotus gives the compound no type, and the head is a verb with a verb class,
/// so the whole inflects as the head with the modifier in front. This is the
/// verb counterpart of [`compound_head_forms`], which refuses a verb head
/// because a nominal compound's head is nominal.
///
/// Only the head's verb classes are run: a head that is also a noun does not
/// give the compound a noun paradigm.
pub(super) fn compound_verb_forms(
    lemma: &str,
    cat: &Catalogues,
    lexicon: Option<&LexiconIndex>,
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
        let inflections: Vec<Inflection> = row
            .inflections()
            .filter(|i| crate::grammar::is_verb_class(i.class))
            .collect();
        if inflections.is_empty() {
            continue;
        }
        if !verb_modifier_is_known(modifier, cat, lexicon) {
            continue;
        }
        let mut out: Vec<Generated> = Vec::new();
        for form in super::inflect_paradigms(
            head,
            &inflections,
            cat,
            lexicon,
            PartOfSpeech::Verb,
            false,
        ) {
            // The negative is two words, and the modifier belongs on the main
            // verb, not on the auxiliary: `ei aliarvioi`, never `aliei arvioi`.
            let word = match form.word.split_once(' ') {
                Some((auxiliary, verb)) => format!("{auxiliary} {modifier}{verb}"),
                None => format!("{modifier}{}", form.word),
            };
            out.push(Generated::new(word, form.union()).with(form.confidence));
        }
        if !out.is_empty() {
            return out;
        }
    }
    Vec::new()
}

/// Whether `modifier` can precede a verb head.
///
/// The compile-time form subset holds bare lemmas and genitive singulars only,
/// so it has no `aikaan` (the illative of `aika`) or `hyväksi` (the translative
/// of `hyvä`), and `aikaansaada`, `hyväksikäyttää` and `maahantuoda` had no
/// paradigm. The fallback inflects a lemma that is a prefix of the modifier and
/// matches the surface, which is the bounded reverse lookup `modifier_is_known`
/// does not do. It is used only for a verb head, where the head constraint is
/// tight enough to keep the widening off nominal compounds.
fn verb_modifier_is_known(
    modifier: &str,
    cat: &Catalogues,
    lexicon: Option<&LexiconIndex>,
) -> bool {
    if modifier_is_known(modifier) {
        return true;
    }
    for (at, _) in modifier.char_indices().skip(1) {
        let base = &modifier[..at];
        for row in crate::kotus::sanalista().by_word(base) {
            let inflections: Vec<Inflection> = row.inflections().collect();
            if inflections.is_empty() {
                continue;
            }
            let pos = crate::tags::PartOfSpeech::inflecting_reading(row.part_of_speech());
            for form in super::inflect_paradigms(base, &inflections, cat, lexicon, pos, true) {
                if form.word == modifier {
                    return true;
                }
            }
        }
    }
    // A verb stem is a modifier in its own right: `riippu` of `riippua` in
    // `riippuliitää` (`riippumatto`, `riippuliidin`).
    if verb_stem_is_a_modifier(modifier) {
        return true;
    }
    // A gradation inside the modifier means no lemma is a prefix of the
    // surface: `puhtaaksi` is the translative of `puhdas`, and `puhdas` is not
    // a prefix of `puhtaaksi`. The engine's reverse lookup handles gradation,
    // so it is the last resort.
    if let Some(index) = lexicon {
        let report = crate::expand::find_bases_with(index.lemmas(), cat, modifier);
        if !report.bases.is_empty() {
            return true;
        }
    }
    false
}

/// Whether `modifier` is the stem of a verb lemma.
///
/// `riippu` of `riippua` (`riippumatto`, `riippuliitää`), `leiju` of `leijua`
/// (`leijupakastus`, `leijukerros`). [`modifier_is_known`] deliberately
/// accepts only nominal stems, so this is a separate, narrow probe: the stem
/// map must name a verb lemma, not merely a noun that shares the spelling.
fn verb_stem_is_a_modifier(modifier: &str) -> bool {
    let bare = modifier.strip_suffix('-').unwrap_or(modifier);
    for base in crate::expand::check_normal::STEM_BUCKETS_VERB.bases(bare) {
        if let Some(row) = crate::kotus::sanalista().by_word(base).next()
            && PartOfSpeech::parse_all(row.part_of_speech()).contains(&PartOfSpeech::Verb)
        {
            return true;
        }
    }
    false
}

/// Whether `modifier` is an inflected form of a classless compound.
///
/// The compile-time form set is built from the classed lemmas, so it has no
/// `kotimaan` even though `kotimaa` is in the sanalista: a classless row is a
/// compound, and its paradigm is built by this module. `kotimaanmatka` is
/// `kotimaan` (the genitive of `kotimaa`) + `matka`, and without this the split
/// is rejected and the whole word has no paradigm.
///
/// Only classless candidates are tried: a classed lemma's forms are already in
/// the form set, so `modifier_is_known` has seen them.
fn classless_modifier_is_known(
    modifier: &str,
    cat: &Catalogues,
    lexicon: Option<&LexiconIndex>,
) -> bool {
    for (at, _) in modifier.char_indices().skip(1) {
        let base = &modifier[..at];
        let mut rows = crate::kotus::sanalista().by_word(base).peekable();
        if rows.peek().is_none() || !rows.all(|row| row.inflections().next().is_none()) {
            continue;
        }
        for form in compound_head_forms(base, cat, lexicon, PartOfSpeech::Noun, true) {
            if form.word == modifier {
                return true;
            }
        }
    }
    false
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
    // Opaque bound modifiers that occur only inside a listed classless
    // compound: `ventovieras` is `vento` + `vieras`, but `vento` has no row
    // of its own. Keep the list lexical and short.
    const OPAQUE_MODIFIERS: &[&str] = &["vento"];
    if OPAQUE_MODIFIERS.contains(&bare) {
        return true;
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
    // The `-as`/`-ias` adjective has the same `-s` combining stem:
    // `viisikymmenvuotias` -> `viisikymmenvuotis-`
    // (`viisikymmenvuotispäivä`). Reconstruct the citation from the stem so
    // only a listed `-as` row licenses it.
    if let Some(stem) = bare.strip_suffix('s')
        && !stem.is_empty()
        && crate::kotus::sanalista()
            .by_word(&format!("{stem}as"))
            .next()
            .is_some()
    {
        return true;
    }
    // A `-nen` adjective's combining form drops the ending: `agraarinen` ->
    // `agraari-` (`agraaripolitiikka`), `digitaalinen` -> `digitaali-`.
    // Only when the modifier is not itself a known word or stem: `irstaisuu`
    // must not split as `irstai` + `suu` through the derived adjective
    // `irstainen`, and `kylmäveri` is a compound, not a combining form.
    // A stem that is also the stem of a non-`-inen` adjective is that
    // adjective's case stem, not a combining form: `irstai` belongs to
    // `irstas` as well as to `irstainen`, so `irstaisuu` must not split as
    // `irstai` + `suu`. A pure `-inen` stem (`agraari` -> `agraarinen`) is
    // the lexical combining form we want.
    let also_other_adjective = crate::expand::check_normal::STEM_BUCKETS_NOUN
        .bases(bare)
        .any(|base| {
            !base.ends_with("nen")
                && crate::kotus::sanalista()
                    .by_word(base)
                    .next()
                    .is_some_and(|row| {
                        PartOfSpeech::parse_all(row.part_of_speech())
                            .contains(&PartOfSpeech::Adjective)
                    })
        });
    if !also_other_adjective
        && crate::suggest::is_known_word_fast(bare).is_none()
        && crate::kotus::sanalista()
            .by_word(&format!("{bare}nen"))
            .next()
            .is_some_and(|row| {
                PartOfSpeech::parse_all(row.part_of_speech())
                    .contains(&PartOfSpeech::Adjective)
            })
    {
        return true;
    }
    // An `-eä`/`-ea` adjective's vowel stem drops the case vowel:
    // `jähmeä` -> `jähme-` (`jähmepiste`). Reconstruct the citation and
    // require a listed adjective, so an ordinary noun + `a` does not pass.
    if bare.ends_with(['a', 'e', 'i', 'o', 'u', 'y', 'ä', 'ö']) {
        for tail in ["ä", "a"] {
            if crate::kotus::sanalista()
                .by_word(&format!("{bare}{tail}"))
            .next()
                .is_some_and(|row| {
                    PartOfSpeech::parse_all(row.part_of_speech())
                        .contains(&PartOfSpeech::Adjective)
                })
            {
                return true;
            }
        }
    }
    // A `-kko`/`-kkö` noun derived from a `-kas`/`-käs` adjective is a
    // modifier: `kuivakas` -> `kuivakko-` (`kuivakkokasvi`).
    for (suffix, adj) in [("kko", "kas"), ("kkö", "käs")] {
        if let Some(stem) = bare.strip_suffix(suffix)
            && !stem.is_empty()
            && crate::kotus::sanalista()
                .by_word(&format!("{stem}{adj}"))
                .next()
                .is_some_and(|row| {
                    PartOfSpeech::parse_all(row.part_of_speech())
                        .contains(&PartOfSpeech::Adjective)
                })
        {
            return true;
        }
    }
    // An agent participle's `-ma`/`-mä` stem is the first member of a
    // compound (`ampuma-ase` from `ampua`, `hiomakone` from `hioa`,
    // `hautomakone` from `hautoa`). The agent participle nominative is not
    // in the verb stem map, but its genitive (`ampuman`) is, so probe that
    // and let [`verb_stem_is_a_modifier`] check the base is a verb.
    if (bare.ends_with("ma") || bare.ends_with("mä"))
        && verb_stem_is_a_modifier(&format!("{bare}n"))
    {
        return true;
    }
    // A compound modifier may itself be a numeral phrase whose tail is
    // fixed:
    //   * the tens -- `kaksikymmen` of `kaksikymmentä`, `kolmikymmen` of
    //     `kolmekymmentä`, `seitsenkymmen` of `seitsemänkymmentä`. The first
    //     member keeps its combining stem and `kymmen` is the class-32 noun;
    //   * the teens -- `nelitoista` of `neljätoista`, `seitsentoista` of
    //     `seitsemäntoista`, where `toista` stays fixed.
    // `kaksikymmenvuotias` then inflects by its bound head `‑vuotias`
    // (class 41) instead of producing no forms at all. The tail is accepted
    // only after a known modifier, so an arbitrary fragment does not become
    // one.
    for tail in ["kymmen", "toista"] {
        if let Some(prefix) = bare.strip_suffix(tail)
            && !prefix.is_empty()
            && modifier_is_known(prefix)
        {
            return true;
        }
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
    // A genitive modifier can itself be an inflected classless compound:
    // `avomeren` (genitive of `avomeri`) in `avomerenliikenne`, `ruumiin`
    // (genitive of `ruumis`) in `ruumiinavaus`, `työväen` in `työväenliike`.
    // Its lemma is not a literal prefix of the modifier, so the probes above
    // miss it; the bounded recogniser resolves it through the modifier's own
    // head. Restricted to a final `n` so an ordinary stem cannot take this
    // path, and to a nominal (not adjective) base.
    if bare.ends_with('n')
        && let Some(base) = crate::suggest::is_known_word_fast(bare)
        && crate::kotus::sanalista()
            .by_word(&base)
            .next()
            .is_some_and(|row| {
                matches!(
                    PartOfSpeech::inflecting_reading(row.part_of_speech()),
                    PartOfSpeech::Noun | PartOfSpeech::Numeral | PartOfSpeech::Pronoun
                )
            })
    {
        return true;
    }
    // A combining `-s` stem of a known modifier: `pari` -> `paris-`
    // (`pariskunta`). The `-nen`/`-as` rules above cover their own families;
    // this is the general shortened form the sanalista uses in a few
    // classless compounds.
    if let Some(stem) = bare.strip_suffix('s')
        && !stem.is_empty()
        && modifier_is_known(stem)
    {
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
/// The derived forms of a compound numeral: the `-toista` teens and the
/// `-kymmentä` tens.
///
/// The teens (`kuusitoista` = 16) and their ordinals (`kuudestoista` =
/// 16th) are compounds whose *first* member inflects while `toista`
/// stays fixed: the genitive of `kuusitoista` is `kuuden` + `toista`.
/// The sanalista gives these words no class, so nothing else makes them;
/// the prefix is a numeral with a class of its own (`kuusi` : `kuuden`),
/// which supplies the paradigm. The tens are [`tens_forms`]: both members
/// decline.
pub(super) fn numeral_forms(
    lemma: &str,
    cat: &Catalogues,
    lexicon: Option<&LexiconIndex>,
) -> Vec<Generated> {
    let Some(prefix) = lemma.strip_suffix("toista").filter(|p| !p.is_empty()) else {
        return hundreds_forms(lemma, cat, lexicon)
            .or_else(|| tens_forms(lemma, cat, lexicon))
            .unwrap_or_default();
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

/// The hundreds (`kaksisataa` = 200), or `None` when `lemma` is not one.
///
/// Same composition as [`tens_forms`]: both members inflect together
/// (`kahdensadan`, `kahtasataa`), except in the nominative, where the first
/// member is nominative and `sata` is partitive singular (`kaksi` +
/// `sataa`). The prefix must be a numeral lemma; a classless numeral prefix
/// (`kahdeksantoista`) is inflected through [`numeral_forms`] itself.
fn hundreds_forms(
    lemma: &str,
    cat: &Catalogues,
    lexicon: Option<&LexiconIndex>,
) -> Option<Vec<Generated>> {
    let prefix = lemma.strip_suffix("sataa").filter(|p| !p.is_empty())?;
    let prefix_row = crate::kotus::sanalista()
        .by_word(prefix)
        .find(|row| row.part_of_speech().contains("numeraali"))?;
    let sata_row = crate::kotus::sanalista().by_word("sata").next()?;
    let prefix_inflections: Vec<Inflection> = prefix_row.inflections().collect();
    let sata_inflections: Vec<Inflection> = sata_row.inflections().collect();
    if sata_inflections.is_empty() {
        return None;
    }
    let mut out = Vec::new();
    for case in Case::ALL {
        let head_case = if case == Case::Nominative {
            Case::Partitive
        } else {
            case
        };
        let head_want = Tags {
            case: Some(head_case),
            number: Some(Number::Singular),
            ..Tags::UNKNOWN
        };
        let heads = forms_of(
            "sata",
            &sata_inflections,
            cat,
            lexicon,
            &head_want,
            PartOfSpeech::Numeral,
            false,
        );
        let firsts: Vec<Generated> = if case == Case::Nominative {
            vec![Generated::new(
                prefix_row.word().to_string(),
                Tags {
                    part_of_speech: Some(PartOfSpeech::Numeral),
                    case: Some(Case::Nominative),
                    number: Some(Number::Singular),
                    ..Tags::UNKNOWN
                },
            )]
        } else if !prefix_inflections.is_empty() {
            let want = Tags {
                case: Some(case),
                number: Some(Number::Singular),
                ..Tags::UNKNOWN
            };
            forms_of(
                prefix,
                &prefix_inflections,
                cat,
                lexicon,
                &want,
                PartOfSpeech::Numeral,
                false,
            )
        } else {
            // A classless numeral prefix (`kahdeksantoista`): its forms come
            // from the numeral layer, not a catalogue.
            inflect_all(prefix, &[], cat, lexicon, PartOfSpeech::Numeral, false)
                .into_iter()
                .filter(|f| {
                    let t = f.union();
                    t.case == Some(case) && t.number == Some(Number::Singular)
                })
                .collect()
        };
        for head in &heads {
            for first in &firsts {
                out.push(
                    Generated::new(format!("{}{}", first.word, head.word), first.union())
                        .with(Confidence::Repaired),
                );
            }
        }
    }
    Some(out)
}

/// The tens (`kaksikymmentä` = 20), or `None` when `lemma` is not one.
///
/// Both members decline together -- `kahdenkymmenen`, `kahtakymmentä`,
/// `kahdellakymmenellä` -- except in the nominative, where the first member is
/// nominative and `kymmen` is partitive singular (`kaksi` + `kymmentä`). The
/// first member must be a numeral lemma; `kymmen` supplies the head forms, so
/// the class-32 noun's own spelling is used (`kymmenen`, not the numeral
/// `kymmenen`'s `kymmeneneen`).
fn tens_forms(
    lemma: &str,
    cat: &Catalogues,
    lexicon: Option<&LexiconIndex>,
) -> Option<Vec<Generated>> {
    let prefix = lemma.strip_suffix("kymmentä").filter(|p| !p.is_empty())?;
    let prefix_row = crate::kotus::sanalista()
        .by_word(prefix)
        .find(|row| row.part_of_speech().contains("numeraali"))?;
    let kymmen_row = crate::kotus::sanalista().by_word("kymmen").next()?;
    let prefix_inflections: Vec<Inflection> = prefix_row.inflections().collect();
    let kymmen_inflections: Vec<Inflection> = kymmen_row.inflections().collect();
    if prefix_inflections.is_empty() || kymmen_inflections.is_empty() {
        return None;
    }
    let mut out = Vec::new();
    for case in Case::ALL {
        let first_want = Tags {
            case: Some(case),
            number: Some(Number::Singular),
            ..Tags::UNKNOWN
        };
        let head_case = if case == Case::Nominative {
            Case::Partitive
        } else {
            case
        };
        let head_want = Tags {
            case: Some(head_case),
            number: Some(Number::Singular),
            ..Tags::UNKNOWN
        };
        let heads = forms_of(
            "kymmen",
            &kymmen_inflections,
            cat,
            lexicon,
            &head_want,
            PartOfSpeech::Noun,
            false,
        );
        // The citation is the first member's nominative whatever the numeral's
        // own class does with that cell: `seitsemän` is class 10 and its
        // nominative reading is missing from the class output, but the
        // sanalista word is the form.
        let firsts = if case == Case::Nominative {
            vec![Generated::new(
                prefix_row.word().to_string(),
                Tags {
                    part_of_speech: Some(PartOfSpeech::Numeral),
                    case: Some(Case::Nominative),
                    number: Some(Number::Singular),
                    ..Tags::UNKNOWN
                },
            )]
        } else {
            forms_of(
                prefix,
                &prefix_inflections,
                cat,
                lexicon,
                &first_want,
                PartOfSpeech::Numeral,
                false,
            )
        };
        for head in &heads {
            for first in &firsts {
                out.push(
                    Generated::new(format!("{}{}", first.word, head.word), first.union())
                        .with(Confidence::Repaired),
                );
            }
        }
    }
    Some(out)
}

/// The comparative (class 16) and superlative (class 36) paradigms an
/// adjective derives.
///
/// Finnish degrees are derivational, not inflectional: the sanalista
/// lists only 44 `-mpi` lemmas, so `suurempi`, `kauniimpi` and
/// `kaunein` belong to no lemma the engine can find. Omorfi derives them
/// (`gen_adj_degrees`); the engine emits the two paradigms
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
    // A `-nut`/`-nyt` participle used as an adjective compares on its `-e`
    // stem: `edistynyt` -> `edistyneempi`/`edistynein`, exactly like
    // `väsynyt` and `kehittynyt`. Class 47 is the `-nut`/`-nyt` type, and
    // the adjective gate keeps the rule off the non-comparing participles
    // and off other `-ut`/`-yt` adjectives (`ohut` compares as `ohuempi`).
    if (comparative.is_none() || superlative.is_none())
        && (lemma.ends_with("nut") || lemma.ends_with("nyt"))
        && let Some(base) = lemma.get(..lemma.len() - 2)
        && !base.is_empty()
        && crate::kotus::sanalista().by_word(lemma).next().is_some_and(|row| {
            crate::tags::PartOfSpeech::parse_all(row.part_of_speech())
                .contains(&PartOfSpeech::Adjective)
                && row
                    .inflections()
                    .any(|inflection| inflection.class == 47)
        })
    {
        let stem = format!("{base}e");
        comparative.get_or_insert_with(|| format!("{stem}empi"));
        superlative.get_or_insert_with(|| format!("{stem}in"));
    }
    // A `-ttu`/`-tty` passive participle used as an adjective compares on
    // the `-tu` stem: `siunattu` -> `siunatumpi`/`siunatuin`, like
    // `kirottu` and `haluttu`. Class 1 is the `-ttu` type; the adjective
    // gate keeps the rule off the participles with no established
    // comparison.
    if (comparative.is_none() || superlative.is_none())
        && (lemma.ends_with("ttu") || lemma.ends_with("tty"))
        && let Some(base) = lemma.get(..lemma.len() - 2)
        && !base.is_empty()
        && crate::kotus::sanalista().by_word(lemma).next().is_some_and(|row| {
            crate::tags::PartOfSpeech::parse_all(row.part_of_speech())
                .contains(&PartOfSpeech::Adjective)
                && row
                    .inflections()
                    .any(|inflection| inflection.class == 1)
        })
    {
        comparative.get_or_insert_with(|| format!("{base}umpi"));
        superlative.get_or_insert_with(|| format!("{base}uin"));
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
    // Adverb degrees are curated rather than derived: `hiljaa` has no
    // positive paradigm to compare, but `hiljempaa` is its comparative.
    for form in degrees::ADVERB_DEGREES.bases(lemma) {
        if form.is_empty() {
            continue;
        }
        let tags = Tags {
            part_of_speech: Some(PartOfSpeech::Adverb),
            ..Default::default()
        };
        out.push(Generated::new(form.to_string(), tags).with(Confidence::Repaired));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `vento` has no lemma row, but it is the bound modifier of the listed
    /// classless compound `ventovieras`.
    #[test]
    fn an_opaque_compound_modifier_is_known() {
        assert!(modifier_is_known("vento"));
        assert!(modifier_is_known("vieras"));
    }
}
