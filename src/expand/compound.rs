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
    for (prefix, head, tail) in find_parts(lemma, class, lexicon) {
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
        for tail_inf in &tail.inflections {
            if compound_class(tail_inf.class) {
                continue;
            }
            for t in super::inflect_in(
                tail.word,
                *tail_inf,
                cat,
                Some(lexicon),
                PartOfSpeech::Noun,
                true,
            ) {
                let tags = t.union();
                // A class-51 tail possessive that does not name its case is
                // ambiguous between the nominative and the genitive, and joining
                // it to either head gives one real and one unconfirmed word, so
                // it is left out rather than guessed. A class-50 prefix is fixed,
                // so the ambiguity does not arise there.
                if class == 51 && tags.case.is_none() {
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
                        if tags.case.is_none_or(|cs| cs == *c)
                            && tags.number.is_none_or(|nu| nu == *n)
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
                        word: format!("{h}{separator}{}", t.word),
                        readings: t.readings.clone(),
                        confidence: t.confidence,
                    });
                }
            }
        }
    }
    out
}

/// Whether a class is one of the compound markers.
fn compound_class(class: u16) -> bool {
    class == 50 || class == 51
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
) -> Vec<(String, &'a Lemma<'a>, &'a Lemma<'a>)> {
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
            if let Some(tail) = index.tail_exactly(rest).find(|e| carries_a_case(e)) {
                parts.push((String::new(), head, tail));
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
                parts.push(("-".to_string(), head, tail));
            }
        }
    } else {
        // Class 50: only the tail inflects; the prefix is whatever precedes the
        // longest suffix that is a word of its own.
        let mut best: Option<(String, &Lemma, &Lemma)> = None;
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
            let longer = best
                .as_ref()
                .is_none_or(|(_, _, t)| t.word.len() < tail.word.len());
            if longer {
                best = Some((prefix.to_string(), tail, tail));
            }
        }
        if let Some((p, h, t)) = best {
            parts.push((p, h, t));
        }
    }
    parts
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
