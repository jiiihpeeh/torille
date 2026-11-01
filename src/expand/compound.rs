//! Compound expansion: the full paradigm of a class 50/51 compound.
//!
//! This is the counterpart to [`crate::compound`], which *splits* an unknown
//! orthographic word into components. This module starts from the split and
//! inflects the head and tail cell by cell (the tail carries the case and the
//! possessive, the head only the case), so `aavameri` reaches every case its
//! parts have rather than only the eight the catalogue tabulates.

use crate::kotus::Inflection;
use crate::kotus::Lemma;
use crate::spec::Catalogues;

use super::Generated;
use super::inflect::{inflect_from, spellings};
use super::lexicon_index::LexiconIndex;
use crate::tags::PartOfSpeech;
use crate::tags::{Case, Form, Number};
use std::collections::HashMap;

/// The full paradigm of a class 50/51 compound, built from its parts.
///
/// A compound's cell is the head's *bare* case form followed by the tail's
/// whole cell: the tail carries the case and the owner suffix, the head only
/// the case -- `aavameri` is `aavan` + `meren`, `aavaa` + `mertani`. Building
/// it this way reaches every case the parts have, not just the eight the
/// catalogue tabulates, and gets the owner suffix on the right word.
pub(super) fn compound_paradigm(
    lemma: &str,
    inflection: Inflection,
    cat: &Catalogues,
    lexicon: Option<&LexiconIndex>,
) -> Vec<Generated> {
    let Some(lexicon) = lexicon else {
        return Vec::new();
    };
    let class = inflection.class;
    if !compound_class(class) {
        return Vec::new();
    }
    let mut out: Vec<Generated> = Vec::new();
    // A compound-like tail is accepted only for a lemma that is class 51
    // alone. `mustaviinimarja` (51) is `musta` + `viinimarja`, and the middle
    // `viinimarja` has no inflection of its own. A dual-class word
    // (`isomaksaruoho` is `50, 51`) already has a correct class-50 reading, and
    // letting the 51 reading recurse into the same tail would add a second
    // paradigm the oracle does not confirm.
    let dual_class = lexicon
        .tail_exactly(lemma)
        .any(|row| row.inflections.iter().any(|inf| inf.class == 50));
    for (prefix, head, tail, shape) in find_parts(lemma, class, lexicon, cat, !dual_class) {
        // The head's bare case forms (class 51 only; a class-50 prefix is fixed).
        let mut head_cells: HashMap<(Case, Number), Vec<String>> = HashMap::new();
        if class == 51 {
            for head_inf in &head.inflections {
                if compound_class(head_inf.class) {
                    continue;
                }
                for h in super::inflect_in(
                    head.word,
                    *head_inf,
                    cat,
                    Some(lexicon),
                    PartOfSpeech::Noun,
                    false,
                ) {
                    let tags = h.union();
                    if tags.possessive.is_some() {
                        continue;
                    }
                    if let (Some(c), Some(n)) = (tags.case, tags.number) {
                        head_cells.entry((c, n)).or_default().push(h.word);
                    }
                }
            }
        }
        // A compound-like tail has no inflection of its own. It is either a
        // plural-only lemma (`sisarukset` is the plural of `sisarus`) or a
        // compound of its own (`viinimarja` is `viini` + `marja`). The first
        // takes the base's plural cells; the second recurses as a class-50
        // compound so its head inflects and its modifiers stay fixed.
        let tail_forms: Vec<Generated> = if tail.inflections.is_empty() {
            if tail.word.ends_with('t')
                && let Some(base) = nominative_plural_base(tail.word, lexicon, cat)
            {
                plural_only_forms(base, cat, lexicon)
            } else {
                compound_paradigm(
                    tail.word,
                    Inflection {
                        class: 50,
                        gradation: None,
                    },
                    cat,
                    Some(lexicon),
                )
            }
        } else {
            tail.inflections
                .iter()
                .filter(|inf| !compound_class(inf.class))
                .flat_map(|inf| {
                    super::inflect_in(
                        tail.word,
                        *inf,
                        cat,
                        Some(lexicon),
                        PartOfSpeech::Noun,
                        true,
                    )
                })
                .collect()
        };
        for t in tail_forms {
            let tags = t.union();
            // A class-51 tail possessive that does not name its case is
            // ambiguous between the nominative and the genitive, and joining
            // it to either head gives one real and one unconfirmed word, so
            // it is left out rather than guessed. A class-50 prefix is fixed,
            // so the ambiguity does not arise there.
            if class == 51 && tags.case.is_none() {
                continue;
            }
            // A plural-only compound's tail is the nominative plural of its
            // base (`vanhemmat` against `vanhempi`), so the compound has no
            // singular at all: `isovanhemmat` is the whole lemma, and
            // `isovanhempi` is not a word.
            if shape == TailShape::PluralOnly && tags.number != Some(Number::Plural) {
                continue;
            }
            // A possessive tail form is often case-ambiguous (`amaryllikseni`
            // is both the nominative and the genitive singular), so match the
            // head on whatever the tail actually claims -- a missing field is
            // a wildcard, never a disqualification.
            let heads: Vec<String> = if class == 51 {
                let mut v: Vec<String> = Vec::new();
                for ((c, n), h) in &head_cells {
                    // The head's case must be the reading's own: an
                    // ambiguous tail possessive combined with the wrong head
                    // case is a different word (`aavanmereni` vs the
                    // unconfirmed `aavamerini`).
                    if tags.case.is_none_or(|cs| cs == *c) && tags.number.is_none_or(|nu| nu == *n)
                    {
                        v.extend(h.iter().cloned());
                    }
                }
                v.sort();
                v.dedup();
                v
            } else {
                // A class-50 prefix is fixed, so every tail cell joins it.
                vec![prefix.clone()]
            };
            if heads.is_empty() {
                continue;
            }
            for h in &heads {
                // A hyphenated class-51 lemma keeps its hyphen in every
                // cell: `nuori-isäntä` is an established expression, so the
                // table is `nuoren-isännän`, `nuorta-isäntää`,
                // `nuoret-isännät`, not the closed spelling a generic
                // compound would use. The separator is not part of either
                // part, so it is chosen here.
                let separator = if class == 51 && prefix == "-" {
                    "-"
                } else {
                    ""
                };
                out.push(Form {
                    word: format!("{h}{separator}{}", shape.surface(&t.word)),
                    readings: t.readings.clone(),
                    confidence: t.confidence,
                });
            }
        }
    }
    out
}

/// Whether a class is one of the compound markers.
fn compound_class(class: u16) -> bool {
    class == 50 || class == 51
}

/// How a compound's tail surface relates to the lemma that inflects it.
///
/// The sanalista spells some compound heads as bound combining forms with a
/// leading U+2011 NON-BREAKING HYPHEN (`‑kulmio`, `‑mielisyys`), and a class-50
/// lemma that ends in one writes it without the hyphen (`suorakulmio`). The
/// hyphen is not part of the compound, so it has to come off every generated
/// form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TailShape {
    /// The tail surface is the lemma itself: `vety` in `raskasvety`.
    Exact,
    /// The tail surface is the lemma without its leading U+2011:
    /// `kulmio` in `suorakulmio` against the bound lemma `‑kulmio`.
    Bound,
    /// The tail surface is the nominative plural of the lemma: `vanhemmat` in
    /// `isovanhemmat` against `vanhempi`. Only plural cells are emitted, so the
    /// compound stays plural-only the way its lemma is.
    PluralOnly,
}

impl TailShape {
    /// The tail form as it appears in the compound.
    fn surface(self, tail: &str) -> &str {
        match self {
            Self::Exact | Self::PluralOnly => tail,
            Self::Bound => tail.strip_prefix('\u{2011}').unwrap_or(tail),
        }
    }
}

/// Whether a lemma can carry a case ending, and so can be a compound's tail.
///
/// This is what separates the two-letter words the sanalista lists into ones
/// that belong at the end of a compound and ones that do not, and it is the
/// reason the tail's length floor could be lowered at all. Of the 48 lemmas the
/// sanalista spells with two letters, 23 carry a real declension and 25 do not:
/// `yö` is class 19 and belongs there, while the ten pronouns (`sa`, `ma`, `te`,
/// `se`, `ne`, ...) are 99 or 101 and the fourteen interjections (`ai`, `ha`,
/// `in`, `no`, ...) carry no class at all. Without this test, `ahkeraliisa`
/// splits as `ahkeralii` + `sa`, `sa` being an interjection, and the word comes
/// back with one form where it had twenty-seven.
///
/// It reads the sanalista's own two columns and adds no list of its own.
fn carries_a_case(row: &Lemma<'_>) -> bool {
    !row.is_indeclinable() && !row.is_compound_like()
}

/// The class a class-50 compound's sanalista row declares beside the marker.
///
/// A class-50 word is one whose initial part stays uninflected, so the
/// inflection sits on the head. [`compound_paradigm`] finds the head by looking
/// for a tail that is a lemma of its own, which fails whenever the head is not
/// one the sanalista lists -- `ahkeraliisa` is `ahkera` + `liisa`, and `liisa`
/// is not in the sanalista at all. This reads the head's class off the same row
/// instead, which is where the sanalista states it.
///
/// Returns `None` where the row names nothing but the marker: `puoliyö` and
/// `suorakulmio` are in that position, and there is no principled fallback for
/// them.
pub(super) fn head_class(lemma: &str, lexicon: Option<&LexiconIndex<'_>>) -> Option<Inflection> {
    lexicon?
        .tail_exactly(lemma)
        .flat_map(|row| row.inflections.iter().copied())
        .find(|inf| !compound_class(inf.class))
}

/// The shortest a compound's second part may be.
///
/// Nothing in the sanalista is one letter long, so this is in effect "any part
/// at all". Two-letter parts are ordinary words -- `yö`, `ma`, `te`, `in`, `no`,
/// `ha` -- and the floor this replaces read as three, which threw away every
/// compound ending in one. `puoliyö` is `puoli` + `yö`: the sanalista row is
/// `50, 51`, the 51 branch wants the tail to be a lemma of its own, `yö` is one,
/// and the length test discarded it. Of the 23 forms the reference confirms,
/// `puoliyö` produced 1 -- and `puolenyön`, `puolenayönä`, `puoletyöt`,
/// `puoleenyöhön` and `puoliinöihin` all need the tail, and not merely as a
/// piece of stem: in several of them the case ending lands on the tail itself.
///
/// The *head* keeps a floor of three and is not covered by this constant: a
/// short prefix is a coincidence of spelling rather than a compound, which is a
/// different risk from a short tail.
pub(super) const SHORTEST_TAIL: usize = 2;

/// The shortest a compound's first part may be.
pub(super) const SHORTEST_HEAD: usize = 3;

/// The parts of a class 50/51 compound: `(prefix, head, tail)`.
fn find_parts<'a>(
    lemma: &str,
    class: u16,
    index: &'a LexiconIndex<'a>,
    cat: &Catalogues,
    accept_compound_tail: bool,
) -> Vec<(String, &'a Lemma<'a>, &'a Lemma<'a>, TailShape)> {
    let mut parts = Vec::new();
    if class == 51 {
        // The head is a prefix of the compound, so the bucket index narrows the
        // candidates from every lemma in the sanalista to the handful sharing
        // the compound's leading characters. See `lexicon_index` for the
        // measurement; this used to scan all 104 743 lemmas per compound.
        for head in index.heads_of(lemma) {
            if head.word.chars().count() < SHORTEST_HEAD
                || head.word.len() + SHORTEST_TAIL > lemma.len()
            {
                continue;
            }
            if !lemma.starts_with(head.word) {
                continue;
            }
            let rest = &lemma[head.word.len()..];
            if rest.chars().count() < SHORTEST_TAIL {
                continue;
            }
            // A compound-like tail (`viinimarja` in `mustaviinimarja`) has no
            // inflection of its own but is still the right part: the class-51
            // branch recurses into it as a class-50 compound.
            if let Some(tail) = index
                .tail_exactly(rest)
                .find(|e| carries_a_case(e) || (accept_compound_tail && e.is_compound_like()))
            {
                parts.push((String::new(), head, tail, TailShape::Exact));
            }
        }
        // `nuori-isäntä` is the one class-51 lemma the sanalista writes with a
        // hyphen. The hyphen belongs to the established expression and is kept
        // in every cell (`nuoren-isännän`, `nuorta-isäntää`), so the parts are
        // joined with it. The ordinary head scan cannot see the word because
        // the tail it looks for is `-isäntä`, hyphen included.
        if parts.is_empty()
            && let Some((head_word, tail_word)) = lemma.split_once('-')
            && head_word.chars().count() >= SHORTEST_HEAD
            && tail_word.chars().count() >= SHORTEST_TAIL
        {
            let head = index.tail_exactly(head_word).find(|e| carries_a_case(e));
            let tail = index.tail_exactly(tail_word).find(|e| carries_a_case(e));
            if let (Some(head), Some(tail)) = (head, tail) {
                parts.push(("-".to_string(), head, tail, TailShape::Exact));
            }
        }
    } else {
        // Class 50: only the tail inflects; the prefix is whatever precedes the
        // longest suffix that is a word of its own.
        //
        // The fourth field is how many characters of `lemma` the tail surface
        // occupies. It is not always the lemma's own length: a bound form is one
        // character longer than the surface (`‑kulmio` against `kulmio`), and a
        // plural surface is not the lemma at all (`vanhemmat` against
        // `vanhempi`). Comparing surface widths is what makes the longest tail
        // win.
        let mut best: Option<(String, &Lemma, &Lemma, TailShape, usize)> = None;
        for tail in index.tails_of(lemma) {
            if tail.word.chars().count() < SHORTEST_TAIL
                || tail.word.len() + 1 > lemma.len()
                || !carries_a_case(tail)
            {
                continue;
            }
            if !lemma.ends_with(&tail.word) {
                continue;
            }
            let prefix = &lemma[..lemma.len() - tail.word.len()];
            if prefix.is_empty() {
                continue;
            }
            let width = tail.word.chars().count();
            let longer = best.as_ref().is_none_or(|&(_, _, _, _, w)| w < width);
            if longer {
                best = Some((prefix.to_string(), tail, tail, TailShape::Exact, width));
            }
        }
        // A bound combining form is written with a leading U+2011 in the
        // sanalista but without it inside a compound: `suorakulmio` is `suora` +
        // `kulmio`, and the lemma the sanalista lists is `‑kulmio`. The tail
        // index cannot see this because the stored word carries the hyphen, so
        // each suffix is looked up with one prepended.
        for (at, _) in lemma.char_indices().skip(1) {
            let surface = &lemma[at..];
            if surface.chars().count() < SHORTEST_TAIL {
                continue;
            }
            let bound = format!("\u{2011}{surface}");
            let Some(tail) = index.tail_exactly(&bound).find(|e| carries_a_case(e)) else {
                continue;
            };
            let width = surface.chars().count();
            let longer = best.as_ref().is_none_or(|&(_, _, _, _, w)| w < width);
            if longer {
                best = Some((lemma[..at].to_string(), tail, tail, TailShape::Bound, width));
            }
        }
        // A plural-only compound's tail is the nominative plural of a lemma
        // that the tail index cannot find by spelling: `isovanhemmat` is `iso` +
        // `vanhemmat`, and the sanalista lists `vanhempi`; `omakustannukset` is
        // `oma` + `kustannukset` against `kustannus`. [`plural_tail`] resolves
        // the surface, and the compound then takes the base's plural cells.
        if best.is_none()
            && let Some((prefix, base, width)) = plural_tail(lemma, index, cat)
        {
            best = Some((prefix, base, base, TailShape::PluralOnly, width));
        }
        if let Some((p, h, t, shape, _)) = best {
            parts.push((p, h, t, shape));
        }
    }
    parts
}

/// Resolves a class-50 tail that is the nominative plural of another lemma.
///
/// `isovanhemmat` is `iso` + `vanhemmat`, and `vanhemmat` is the nominative
/// plural of `vanhempi`; `omakustannukset` is `oma` + `kustannukset` against
/// `kustannus`, and `normaaliolot` is `normaali` + `olot` against `olo`. The
/// tail index matches spellings, so it never finds a base whose plural differs
/// from its lemma. The candidates are the lemmas sharing the surface's leading
/// characters, and each is inflected once to see whether it owns the surface.
///
/// Returns the prefix, the base lemma and the surface width, or `None` when no
/// suffix of `lemma` resolves to a nominative plural.
fn plural_tail<'a>(
    lemma: &str,
    index: &'a LexiconIndex<'a>,
    cat: &Catalogues,
) -> Option<(String, &'a Lemma<'a>, usize)> {
    // Only a `-t` surface can be a nominative plural.
    if !lemma.ends_with('t') {
        return None;
    }
    let mut best: Option<(String, &Lemma, usize)> = None;
    for (at, _) in lemma.char_indices().skip(1) {
        let surface = &lemma[at..];
        if surface.chars().count() < SHORTEST_TAIL || !surface.ends_with('t') {
            continue;
        }
        let Some(base) = nominative_plural_base(surface, index, cat) else {
            continue;
        };
        let width = surface.chars().count();
        if best.as_ref().is_none_or(|&(_, _, w)| w < width) {
            best = Some((lemma[..at].to_string(), base, width));
        }
    }
    best
}

/// The lemma whose nominative plural is `surface`, if there is one.
///
/// Only the lemmas sharing `surface`'s leading characters are considered, so
/// this is the bounded neighbourhood [`crate::expand::find_bases_with`] starts
/// from, narrowed to one question: does this lemma inflect to the surface in the
/// plural? A compound lemma is skipped outright -- its own expansion is what
/// called this, and re-entering it is what the old reverse-lookup version did.
fn nominative_plural_base<'a>(
    surface: &str,
    index: &'a LexiconIndex<'a>,
    cat: &Catalogues,
) -> Option<&'a Lemma<'a>> {
    for base in index.heads_of(surface) {
        if !carries_a_case(base) || base.word.contains('\u{2011}') {
            continue;
        }
        if base.inflections.iter().any(|i| compound_class(i.class)) {
            continue;
        }
        for inf in &base.inflections {
            if compound_class(inf.class) {
                continue;
            }
            let forms =
                super::inflect_in(base.word, *inf, cat, Some(index), PartOfSpeech::Noun, false);
            if forms
                .iter()
                .any(|f| f.word == surface && f.union().number == Some(Number::Plural))
            {
                return Some(base);
            }
        }
    }
    None
}

/// The plural cells of `base`, for a compound-like tail that is itself a
/// plural-only lemma.
///
/// `sisarukset` in `täysisisarukset` is the nominative plural of `sisarus`, and
/// the sanalista lists it as a compound-like row of its own (no inflection), so
/// the tail is resolved back to `sisarus` and only its plural cells are kept.
fn plural_only_forms(
    base: &Lemma<'_>,
    cat: &Catalogues,
    lexicon: &LexiconIndex<'_>,
) -> Vec<Generated> {
    let mut out = Vec::new();
    for inf in &base.inflections {
        if compound_class(inf.class) {
            continue;
        }
        for form in super::inflect_in(
            base.word,
            *inf,
            cat,
            Some(lexicon),
            PartOfSpeech::Noun,
            true,
        ) {
            if form.union().number == Some(Number::Plural) {
                out.push(form);
            }
        }
    }
    out
}

/// A class 51 compound, split into the forms of its two parts.
pub(super) struct Heads {
    /// The head's forms and the tail's forms for each case, in the order the
    /// catalogue gives the cases. A case with two spellings on either side has
    /// all four combinations, which is what the catalogue itself lists.
    pub(super) forms: Vec<Vec<(String, String)>>,
}

/// Finds the parts of a class 51 compound.
///
/// Kotus marks a compound noun with class 51 when its *first part* is the part
/// that inflects, and the catalogue shows what that means: `nuoripari` is
/// `nuori` + `pari`, and its genitive is `nuoren` + `parin`. Both parts inflect
/// and the case ending lands on the last one, so the form for a case is the
/// head's form for that case followed by the tail's form for the same one. That
/// also gets the length compensation right, because it is the tail that
/// lengthens: `pareja` is the partitive plural of `pari` on its own.
///
/// The parts are found by looking for lemmas in the lexicon, which keeps the
/// rule free of a compound lexicon, and they are inflected by the same code as
/// any other word, so a part brings its own stem alternation with it.
pub(super) fn compound_heads(
    lemma: &str,
    inflection: Inflection,
    cat: &Catalogues,
    lexicon: Option<&LexiconIndex>,
) -> Vec<Heads> {
    if inflection.class != COMPOUND_CLASS {
        return Vec::new();
    }
    let Some(lexicon) = lexicon else {
        return Vec::new();
    };
    let mut heads = Vec::new();
    for head in lexicon.heads_of(lemma) {
        // A one or two letter head is a coincidence of spelling rather than a
        // compound. The tail has no floor worth the name: nothing in the
        // sanalista is one letter long, and two-letter words are ordinary --
        // `puoli` + `yö` is a compound and `yö` is in the sanalista.
        if head.word.len() < SHORTEST_HEAD
            || head.word.len() + SHORTEST_TAIL > lemma.len()
            || !lemma.starts_with(head.word)
        {
            continue;
        }
        let Some(tail) = lexicon
            .tail_exactly(&lemma[head.word.len()..])
            .find(|e| carries_a_case(e))
        else {
            continue;
        };
        for head_inf in &head.inflections {
            for tail_inf in &tail.inflections {
                if head_inf.class == COMPOUND_CLASS || tail_inf.class == COMPOUND_CLASS {
                    continue;
                }
                let head_forms = plain_forms_by_slot(head.word, *head_inf, cat);
                let tail_forms = plain_forms_by_slot(tail.word, *tail_inf, cat);
                let forms: Vec<Vec<(String, String)>> = head_forms
                    .iter()
                    .zip(&tail_forms)
                    .map(|(h, t)| {
                        h.iter()
                            .flat_map(|h| t.iter().map(move |t| (h.clone(), t.clone())))
                            .collect()
                    })
                    .collect();
                if forms.iter().all(Vec::is_empty) {
                    continue;
                }
                heads.push(Heads { forms });
            }
        }
    }
    heads
}

/// The forms of one word that the catalogue's own split produced, grouped by
/// case, which are the forms a compound part contributes.
///
/// A class with a table of its own is asked for them directly. The catalogue
/// alignment cannot be used there at all: it splits the lemma against the class
/// model, and for these classes the model is a different kind of word, so
/// aligning `nuori` against `pieni` gives a set of forms that shares almost
/// nothing with the compound it is supposed to head.
///
/// Otherwise `inflect` sorts what it returns, and the grouping needs the cases
/// in the order the catalogue gives them, so this asks the splitting code
/// directly and then cuts the flat list where each case ends.
fn plain_forms_by_slot(word: &str, inflection: Inflection, cat: &Catalogues) -> Vec<Vec<String>> {
    if crate::grammar::irregular(inflection.class).is_some() {
        return crate::grammar::cases_by_slot(word, inflection.class)
            .into_iter()
            .map(|slot| slot.into_iter().map(|f| f.word).collect())
            .collect();
    }
    let Some(para) = cat.get(inflection.class) else {
        return Vec::new();
    };
    let fixed = crate::grammar::fixed_stem(inflection.class);
    let model = fixed.unwrap_or_else(|| para.model());
    let mut out: Vec<Generated> = Vec::new();
    for lemma in spellings(word, fixed) {
        inflect_from(
            lemma,
            inflection,
            &mut out,
            para,
            model,
            &[],
            PartOfSpeech::Noun,
        );
    }
    let mut forms: Vec<String> = out
        .into_iter()
        .filter(|f| f.plain())
        .map(|f| f.word.to_string())
        .collect();
    let mut slots = Vec::with_capacity(para.slots.len());
    for slot in &para.slots {
        let n = slot.variants().count();
        slots.push(forms.drain(..n.min(forms.len())).collect());
    }
    slots
}

/// The Kotus class for a compound whose first part inflects.
const COMPOUND_CLASS: u16 = 51;
