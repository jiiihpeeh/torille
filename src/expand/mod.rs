//! Turning lemmas into their inflected forms.
//!
//! The catalogue gives, per class, a model word and its forms. To inflect an
//! arbitrary lemma of that class we have to find where the form's ending
//! attaches inside the lemma. Two adjustments are needed on top of a plain
//! longest-common-prefix match:
//!
//! * **Length compensation** — the lemma is usually longer than the model
//!   (`kaappi` for the `risti` paradigm), so the split point in the lemma has
//!   to be shifted by the length difference.
//! * **Epenthetic vowels** — the model's own stem sometimes ends where the
//!   lemma's does not, because a vowel was inserted: `koirien` is
//!   `koir` + `ien` although the model `koira` is five letters and
//!   `kalojen` is `kal` + `ojen`. Trying the split one or two characters
//!   earlier covers both.
//!
//! Generation is deliberately over-complete: a handful of non-words such as
//! `koiraien` slip in. For checking that is the right trade-off — a
//! mistakenly rejected word is a false alarm, a mistakenly accepted one is
//! merely a missed typo. The suggestion set is built separately and filtered
//! by phonotactics so that junk never surfaces to a user.

mod align;
mod build;
pub mod clitic;
mod compound;
pub mod connegative;
pub mod derived;
mod derived_forms;
pub mod fast_path;
mod find_bases;
mod inflect;
mod lexicon_index;
pub mod negation;
mod slot;
mod sound;
pub(super) mod split;

// The generated, per-class nominal rules. See the `generated-nominals`
// feature and tools/extract_class_rules.py --rust. `nominals::dispatch`
// routes a sanalista class to the module that implements its paradigm.
#[cfg(feature = "generated-nominals")]
#[allow(
    unused_imports,
    unused_mut,
    unused_variables,
    unused_attributes,
    duplicate_macro_attributes,
    non_snake_case,
    dropping_copy_types,
    clippy::all,
)]
mod nominals;

// The hand-written per-class verb engines. Off by default while a class is
// being built: a module joins the union (and later supersedes the type engine)
// only once it is clean against the verb lexicon.
#[cfg(feature = "generated-verbs")]
#[allow(
    unused_imports,
    unused_mut,
    unused_variables,
    unused_attributes,
    duplicate_macro_attributes,
    non_snake_case,
    dropping_copy_types,
    clippy::all,
)]
mod verbs;

pub use crate::tags::Form as Generated;

mod degrees {
    //! Adjective degrees: positive -> (comparative, superlative)
    //! nominatives, derived by Omorfi and embedded by `build.rs`. See
    //! [`degree_forms`].
    use crate::expand::static_table::StemMap;

    /// `positive -> [comparative, superlative]`. An absent degree is an
    /// empty string, so the two cells stay positional.
    pub static ADJ_DEGREES: StemMap = StemMap::new(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/data/precomputed/adj_degrees.bin",
    )));
}

use crate::kotus::Inflection;
use crate::spec::Catalogues;
use crate::tags::{Confidence, PartOfSpeech, Tags};

use compound::{compound_heads, head_class};
use inflect::{inflect_from, spellings};
pub use lexicon_index::LexiconIndex;
use split::dedup;
use std::collections::HashSet;

use derived_forms::{
    compound_head_forms, compound_verb_forms, degree_forms, derived_inen_forms, numeral_forms,
};
pub(crate) use derived_forms::modifier_is_known;
pub use derived_forms::head_inflections;
pub use derived_forms::{
    derived_agent_base, derived_inen_base, derived_kas_base, derived_lainen_base,
    derived_llinen_base, derived_tar_base, derived_ton_base, derived_uus_base,
};

// Whether the hand-written `nominals` engine has been verified for a class.
// A verified engine replaces the heuristic engine's forms; an unverified one
// is additive (its forms join the heuristic's without replacing them). The
// set is owned by the dispatch tree -- it is `nominals::AUTHORITATIVE_CLASSES`
// there, next to the dispatch table that qualifies -- and the heuristic side
// asks `nominals::is_authoritative` rather than carrying its own copy of the
// list. Adding a class to that set is the commit that supersedes the
// heuristic for it.
#[cfg(feature = "generated-nominals")]
fn nominals_authoritative_for(inflection: &Inflection) -> bool {
    nominals::is_authoritative(&inflection.to_string())
}

// The early-return branches of `inflect_in`. Each takes the same arguments
// the caller would have passed plus `out: &mut Vec<Generated>`, and
// returns `true` when the forms have been filled in and the caller
// should return. The shape keeps `inflect_in` readable as a top-down
// pipeline.

// A word the class system cannot derive is spelled out by hand, and
// its own paradigm supersedes the class engine: `minä` is suppletive
// and `aprilli` is a noun the sanalista marks indeclinable. Their
// classes (101, 99) have no model to work from, so nothing else could
// make them.
fn try_irregular(lemma: &str, out: &mut Vec<Generated>) -> bool {
    if crate::grammar::word_irregular(lemma).is_some() {
        out.extend(crate::grammar::word_cases(lemma));
        dedup(out);
        true
    } else {
        false
    }
}

// A class 50/51 compound is inflected part by part -- the head's bare
// case form joined to the tail's whole cell -- which reaches every
// case and puts the owner suffix on the right word. It is
// authoritative when a split is found; a word with no split falls
// through to the class model.
//
// The class-50-no-split fallback is documented where it lives below:
// running the word under its head class (named beside the marker in
// the sanalista) reaches forms the marker's whole-word paradigm would
// have missed. Class 51 deliberately has no fallback because its
// semantics are the opposite of 50's.
fn try_compound_marker(
    lemma: &str,
    inflection: Inflection,
    cat: &Catalogues,
    lexicon: Option<&LexiconIndex>,
    pos: PartOfSpeech,
    possessives: bool,
    out: &mut Vec<Generated>,
) -> bool {
    if !matches!(inflection.class, 50 | 51) {
        return false;
    }
    let mut compound =
        compound::compound_paradigm(lemma, inflection, cat, lexicon);
    if !compound.is_empty() {
        let mut seen = std::collections::HashSet::new();
        compound.retain(|f| seen.insert(f.word.clone()));
        out.extend(compound);
        return true;
    }
    // No split, so the compound engine has nothing to work from. For class
    // 50 that is not a reason to apply the marker as a whole-word paradigm,
    // which is what happens by falling through: the class-50 model
    // (`isoisä`) then aligns the compound against itself and lands the
    // gradation in the middle of it, so `ahkeraliisa` came back with
    // `ahkeraldin` and `vapaaherratar` with `vapaaheeilla`.
    //
    // What 50 asserts is that the compound's *initial* part does not
    // inflect (`data/kotus-tyypit.txt`: `isoäiti : isoäidin`, number 50).
    // With no split available, the whole word therefore behaves as the head's
    // own class, and the sanalista names that class beside the marker:
    // `ahkeraliisa` is `9, 50`, `vapaaherratar` is `32*C, 50`. Running it
    // recovers the paradigm -- class 9 covers 100% of the reference's 25
    // confirmed non-possessive forms for `ahkeraliisa`, against 52% for the
    // marker's 168.
    //
    // Class 51 is deliberately excluded. It asserts the opposite -- the
    // *initial* part inflects -- so the whole word is not its head's
    // paradigm and falling back would be wrong rather than merely useless.
    // No lemma needs it either: all 91 class-51 words in the sanalista split.
    if inflection.class == 50
        && let Some(head) = head_class(lemma, lexicon)
    {
        out.extend(inflect_in(lemma, head, cat, lexicon, pos, possessives));
        dedup(out);
        return true;
    }
    // Class 51 asserts that the *initial* part inflects, so the whole word is
    // not its head's paradigm. With no split there is nothing correct to
    // build; falling through to the catalogue model aligns the compound
    // against the model word and fabricates forms (`nuori-idenkuun`,
    // `mudtaraaakuita`). Treat the class as handled with no output instead.
    if inflection.class == 51 {
        dedup(out);
        return true;
    }
    false
}

/// Generates every form of `lemma` under `inflection`, tagged.
///
/// `pos` is the part of speech from the sanalista, and it is taken as given
/// rather than guessed from the class: an adjective and a noun share a class
/// here, and only the dictionary says which one this lemma is. A form of an
/// unknown part of speech keeps whatever the class implied, which is nothing.
pub fn inflect(
    lemma: &str,
    inflection: Inflection,
    cat: &Catalogues,
    pos: PartOfSpeech,
    possessives: bool,
) -> Vec<Generated> {
    // A class-50/51 compound can only be split when the lexicon is in hand,
    // and class 51 has no whole-word fallback. Constructing this index is
    // free (it is two precomputed orders over the embedded lexicon), so the
    // public wrapper supplies it rather than letting a compound fall through
    // to the catalogue model and fabricate forms such as `nuori-idenkuun`.
    let lexicon = crate::kotus::lemmas();
    let index = LexiconIndex::new(lexicon);
    inflect_in(lemma, inflection, cat, Some(&index), pos, possessives)
}

/// Generates every form of `lemma`, with the lexicon to hand for compound
/// heads. `possessives` says whether the word's class takes possessive
/// suffixes -- a noun does, an adjective or participle does not.
/// Dual-class nouns whose second reading the reference rejects.
///
/// `lymfa` is listed as `9, 10`; the nominal oracle confirms the class-9
/// o-plural (`lymfoitta`, `lymfoissa`) and marks the class-10 readings
/// (`lymfitta`) only `engine`. The two classes share a stem, so suppressing
/// the class-10 reading before either engine runs is the same fix the dual
/// 72/74 verbs use.
const CLASS_9_WINS: &[&str] = &["lymfa"];

pub fn inflect_in(
    lemma: &str,
    inflection: Inflection,
    cat: &Catalogues,
    lexicon: Option<&LexiconIndex>,
    pos: PartOfSpeech,
    possessives: bool,
) -> Vec<Generated> {
    let mut out: Vec<Generated> = Vec::with_capacity(64);
    // An empty lemma has no paradigm. Some class engines build `lemma + ending`
    // and would otherwise emit bare endings; a couple assumed a non-empty stem
    // outright. The public API must treat this as a no-op, not a panic.
    if lemma.is_empty() {
        return out;
    }
    if try_irregular(lemma, &mut out) {
        return out;
    }
    if try_compound_marker(lemma, inflection, cat, lexicon, pos, possessives, &mut out) {
        return out;
    }
    // A dual 72/74 lemma has one real reading and one the reference rejects.
    // Suppress the losing class before either the heuristic or the hand-written
    // module can emit it; the module alone cannot do this because an empty
    // module result leaves the heuristic's forms in place.
    if verbs::suppresses_reading(lemma, inflection.class) {
        return out;
    }
    if inflection.class == 10 && CLASS_9_WINS.contains(&lemma) {
        return out;
    }
    // The catalogue model drives the heuristic engine. Some classes have no
    // model at all -- 99 is the "indeclinable" marker (`Sana on taipumaton`)
    // -- so the heuristic contributes nothing; the hand-written `nominals`
    // engine below is then the only source. Where the catalogue has a model,
    // the class's stem may split on it (class 27's `käsi`), which is prefixed
    // to the fixed stem if that stem belongs to the model word and to nothing
    // else.
    if let Some(para) = cat.get(inflection.class) {
        let fixed = crate::grammar::fixed_stem(inflection.class);
        let model = fixed.unwrap_or_else(|| para.model());
        let heads = compound_heads(lemma, inflection, cat, lexicon);
        let fixed = fixed.filter(|_| lemma == para.model());
        for lemma in spellings(lemma, fixed) {
            inflect_from(lemma, inflection, &mut out, para, model, &heads, pos);
        }
    }
    // The hand-written per-class engines (`nominals`) are authoritative for the
// classes they have been built and verified for; their module alone inflects
// the class, so the heuristic cannot sneak wrong forms in beside them. For a
// class whose engine is not yet hand-written, the module is additive -- its
// forms join the heuristic engine's own, so nothing correct is lost while the
// grammar for that class is still being built.
    #[cfg(feature = "generated-nominals")]
    if let Some(generated) =
            nominals::dispatch(&inflection.to_string(), lemma, possessives)
                // The class engines build a *shape*, and every cell's reading
                // is `nominal_tags(PartOfSpeech::Noun, ...)`. The part of speech
                // is dictionary metadata, not shape, so it is applied here at
                // the one boundary both trees share: `Form::of_pos` re-tags
                // every reading, which is what makes `aallokas` an adjective
                // and `se` a pronoun. Doing it here rather than in the class
                // modules is also what lets the refactored tree be swapped in
                // with a one-line change without the defect returning.
                .map(|forms| forms.into_iter().map(|f| f.of_pos(pos)).collect::<Vec<_>>())
    {
        let authoritative = nominals_authoritative_for(&inflection);
        if authoritative && !generated.is_empty() {
            out.clear();
        }
        out.extend(generated);
    }
    // A hand-written verb class that covers its whole paradigm supersedes the
    // type engine, the way an authoritative nominal class does above. Classes
    // without a module yet keep the engine's forms.
    #[cfg(feature = "generated-verbs")]
    if crate::grammar::is_verb_class(inflection.class)
        && let Some(generated) = verbs::dispatch(&inflection.to_string(), lemma)
    {
        if !generated.is_empty() {
            out.clear();
        }
        out.extend(generated);
    }
    dedup(&mut out);
    out
}

/// Generates every form of `lemma` under each of `inflections`, in that order.
///
/// This is what a caller holding a resolved class list should call instead of
/// looping over [`inflect_in`], for two reasons.
///
/// The sanalista can list one word under two classes whose grids coincide, and
/// then concatenating emits the same word twice with the same reading and the
/// same confidence. `ahtaus` is two homonym rows, `40` and `39`, and its 318
/// forms are 159 words each appearing twice -- 2 667 such copies over 50 lemmas
/// in the fingerprint, none carrying a reading the other copy lacked. Dropping
/// them loses nothing; keeping them costs every downstream consumer a row.
///
/// And the order forms come out in is observable, so [`split::dedup`] is not
/// available here: it sorts by word and then by confidence, which a caller
/// reading a stream cannot see past. This keeps first-occurrence order and drops
/// only copies identical in every field.
///
/// "Identical in every field" is doing real work in that sentence, and two kinds
/// of near-repeat survive it on purpose.
///
/// A word that two classes spell the same but *read* differently keeps both
/// readings. `appi` is `7*B` and `5*B`, and `apikseen` is the plural
/// translative under one and the singular under the other: one spelling, two
/// numbers, and either copy is a fact the other does not carry.
///
/// A word that two classes spell the same but derive at different *confidence*
/// keeps both. `kungfu` is `1,21`; class 1 reaches `kungfuiden` only with a
/// repair and class 21 derives it cleanly, and a `repaired` form beside a
/// `plain` one is the correction the caller is being offered, not a duplicate of
/// the form it corrects. Keying on the spelling alone would swallow it, which is
/// The forms of `lemma` under each inflection, deduplicated.
///
/// This is the core of [`inflect_all`] without the derived layers (degrees,
/// numerals, and the classless-compound and `-inen` fallbacks). A compound head
/// needs the core only: its degrees belong to the compound, and prefixing the
/// head's comparative onto a noun compound is how `happiradikaali` grew
/// `happiradikaaleimmittaan`.
pub(super) fn inflect_paradigms(
    lemma: &str,
    inflections: &[Inflection],
    cat: &Catalogues,
    lexicon: Option<&LexiconIndex>,
    pos: PartOfSpeech,
    possessives: bool,
) -> Vec<Generated> {
    let mut out: Vec<Generated> = Vec::new();
    let mut seen: HashSet<(String, Confidence, Vec<Tags>)> = HashSet::new();
    for inflection in inflections {
        for form in inflect_in(lemma, *inflection, cat, lexicon, pos, possessives) {
            let key = (form.word.clone(), form.confidence, form.readings.clone());
            if seen.insert(key) {
                out.push(form);
            }
        }
    }
    out
}

/// why the key carries the confidence.
pub fn inflect_all(
    lemma: &str,
    inflections: &[Inflection],
    cat: &Catalogues,
    lexicon: Option<&LexiconIndex>,
    pos: PartOfSpeech,
    possessives: bool,
) -> Vec<Generated> {
    let mut out = inflect_paradigms(lemma, inflections, cat, lexicon, pos, possessives);
    let mut seen: HashSet<(String, Confidence, Vec<Tags>)> = out
        .iter()
        .map(|f| (f.word.clone(), f.confidence, f.readings.clone()))
        .collect();
    // Degree forms are added for any lemma in the table, whatever part of
    // speech the caller asks under: `paha` is `adjektiivi, substantiivi`
    // and is inflected as a noun in some callers, but it still has
    // `pahempi`/`pahin`. The table is mostly adjectives, but Omorfi also
    // derives the quantifier `usea`'s degrees (`useampi`, `usein`), so a
    // lemma outside the adjective tag is not necessarily a no-op here.
    for form in degree_forms(lemma, cat, lexicon) {
        let key = (form.word.clone(), form.confidence, form.readings.clone());
        if seen.insert(key) {
            out.push(form);
        }
    }
    for form in numeral_forms(lemma, cat, lexicon) {
        let key = (form.word.clone(), form.confidence, form.readings.clone());
        if seen.insert(key) {
            out.push(form);
        }
    }
    // A classless lemma that is a compound with a classed head inflects as
    // the head (`periaate` = `peri` + `aate`, `täysi-ikäinen` = `täysi` +
    // `ikäinen`). The constraint in `compound_head_forms` is what keeps
    // this off the classless words whose right half only looks like a
    // lemma. Only a lemma with no class of its own gets this: a classed
    // lemma has its own paradigm.
    if inflections.is_empty() {
        for form in compound_head_forms(lemma, cat, lexicon, pos, possessives) {
            let key = (form.word.clone(), form.confidence, form.readings.clone());
            if seen.insert(key) {
                out.push(form);
            }
        }
        // A classless `-inen` adjective (`kovakorvainen`,
        // `yksinkertainen`): its head is a *derived* adjective
        // (`korva` + `-inen`), not a lemma, so `compound_head_forms`
        // cannot reach it, but the whole word inflects as the `-inen`
        // class.
        for form in derived_inen_forms(lemma, cat, lexicon, pos, possessives) {
            let key = (form.word.clone(), form.confidence, form.readings.clone());
            if seen.insert(key) {
                out.push(form);
            }
        }
        // A classless verb is a compound with a verb head (`aliarvioida` is
        // `ali` + `arvioida`). `compound_head_forms` refuses a verb head, so
        // the verb case has its own path.
        if matches!(pos, PartOfSpeech::Verb | PartOfSpeech::Unknown) {
            for form in compound_verb_forms(lemma, cat, lexicon) {
                let key = (form.word.clone(), form.confidence, form.readings.clone());
                if seen.insert(key) {
                    out.push(form);
                }
            }
        }
    }
    out
}

/// The key of [`emits_form`]'s paradigm cache: the lemma, its inflections,
/// the reading, the possessive flag, and whether a lexicon was supplied.
///
/// The inflections are part of the key: two callers may ask about the same
/// lemma under different subsets, and the cached set is the union of the
/// forms of the inflections it was built from.
type ParadigmKey = (String, Vec<(u16, u8)>, u8, bool, bool);

/// One lemma's verified word set.
type Paradigm = std::rc::Rc<HashSet<String>>;

/// Whether `lemma` under `inflections` produces `word`.
///
/// [`inflect_all`] exists to *list* a paradigm: it deduplicates with a
/// `HashSet<(String, Confidence, Vec<Tags>)>`, cloning every word and reading.
/// A caller that only asks "is this one form in the paradigm" pays that clone
/// for every other form, and the stem-map gates ask it thousands of times. This
/// walks the same engines and stops at the first match.
///
/// The derived layers are kept (`degree_forms`, `numeral_forms`), because a
/// surface such as `pahempi` is a form of `paha` only through them. A classless
/// lemma has nothing but those layers and the compound fallbacks, so it takes
/// the full [`inflect_all`] path.
pub fn emits_form(
    lemma: &str,
    inflections: &[Inflection],
    cat: &Catalogues,
    lexicon: Option<&LexiconIndex>,
    pos: PartOfSpeech,
    possessives: bool,
    word: &str,
) -> bool {
    if inflections.is_empty() {
        return inflect_all(lemma, inflections, cat, lexicon, pos, possessives)
            .iter()
            .any(|f| f.word == word);
    }
    // The stem-map gates verify the same base once per token, and a page has
    // many case forms of one lemma (`talo`, `talossa`, `talolle`, ...). Each
    // verification otherwise rebuilds the whole paradigm; the first builds the
    // word set and the rest are one hash probe.
    thread_local! {
        static CACHE: std::cell::RefCell<
            std::collections::HashMap<ParadigmKey, Paradigm>,
        > = std::cell::RefCell::new(std::collections::HashMap::new());
        static WORDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }
    const WORD_CAP: usize = 100_000;
    let key = (
        lemma.to_string(),
        inflections
            .iter()
            .map(|inflection| (inflection.class, inflection.gradation.map_or(0, |g| g.0)))
            .collect(),
        pos as u8,
        possessives,
        lexicon.is_some(),
    );
    if let Some(set) = CACHE.with_borrow(|cache| cache.get(&key).cloned()) {
        return set.contains(word);
    }
    let mut set: HashSet<String> = HashSet::new();
    for inflection in inflections {
        for form in inflect_in(lemma, *inflection, cat, lexicon, pos, possessives) {
            set.insert(form.word);
        }
    }
    for form in degree_forms(lemma, cat, lexicon) {
        set.insert(form.word);
    }
    for form in numeral_forms(lemma, cat, lexicon) {
        set.insert(form.word);
    }
    let hit = set.contains(word);
    let cached = std::rc::Rc::new(set);
    CACHE.with_borrow_mut(|cache| {
        if WORDS.with(std::cell::Cell::get) + cached.len() > WORD_CAP {
            cache.clear();
            WORDS.with(|w| w.set(0));
        }
        WORDS.with(|w| w.set(w.get() + cached.len()));
        cache.insert(key, cached);
    });
    hit
}

/// Generates every form of `lemma` under `inflection` whose readings
/// fit `want`, tagged.
///
/// A `None` on a dimension of `want` is a wildcard -- the same rule
/// [`crate::tags::Tags::matches`] follows -- so asking with `Tags::default()`
/// is the same as [`inflect`]. Asking with only a `case` set narrows to
/// the words in that case; asking with `case` *and* `number` narrows to
/// the singular or plural of it.
///
/// A form is named once however many of its readings fit `want` -- the
/// question is which words a lemma has in this shape, not how many ways
/// they read. A form whose every reading carries the wanted tags is
/// preferred; a form that fits only via one of several readings comes
/// back too, because it might be what was asked for and the caller can
/// say no.
pub fn form_of(
    lemma: &str,
    inflection: Inflection,
    cat: &Catalogues,
    want: &Tags,
    pos: PartOfSpeech,
    possessives: bool,
) -> Vec<Generated> {
    form_of_in(lemma, inflection, cat, None, want, pos, possessives)
}

/// The [`form_of`] variant that takes a [`LexiconIndex`] for compound
/// heads. Useful when a lemma of class 50/51 might be in play, otherwise
/// [`form_of`] is enough.
pub fn form_of_in(
    lemma: &str,
    inflection: Inflection,
    cat: &Catalogues,
    lexicon: Option<&LexiconIndex>,
    want: &Tags,
    pos: PartOfSpeech,
    possessives: bool,
) -> Vec<Generated> {
    let mut seen: HashSet<String> = HashSet::new();
    inflect_in(lemma, inflection, cat, lexicon, pos, possessives)
        .into_iter()
        .filter(|f| f.matches(want))
        .filter(|f| seen.insert(f.word.clone()))
        .collect()
}

/// Generates every form of `lemma` whose readings fit `want`, across all
/// of `inflections`. The loop here does what [`form_of_in`] does for
/// one inflection, then keeps the first occurrence of each word.
pub fn forms_of(
    lemma: &str,
    inflections: &[Inflection],
    cat: &Catalogues,
    lexicon: Option<&LexiconIndex>,
    want: &Tags,
    pos: PartOfSpeech,
    possessives: bool,
) -> Vec<Generated> {
    let mut seen: HashSet<(String, Confidence, Vec<Tags>)> = HashSet::new();
    let mut out: Vec<Generated> = Vec::new();
    for inflection in inflections {
        for form in form_of_in(lemma, *inflection, cat, lexicon, want, pos, possessives) {
            let key = (form.word.clone(), form.confidence, form.readings.clone());
            if seen.insert(key) {
                out.push(form);
            }
        }
    }
    out
}

pub use build::{ClassPrior, FormBuilder, build};
pub use clitic::{CLITICS, all_clitics, cliticize, cliticize_all, strip_clitic, strip_all_clitics};
pub use find_bases::{BasesReport, find_any_base_with, find_bases_with, set_single_thread};
pub(crate) use find_bases::{find_bases_for_check, find_bases_with_form_filter};
pub use sound::is_phonotactic;

pub mod all_forms;
mod blob;
pub mod check_normal;
mod huffman_set;
pub mod prefix_index;
pub mod proper_names;
mod static_table;
pub mod stem_index;

#[cfg(test)]
mod tests {
    use super::*;

    use crate::kotus::Gradation;
    use crate::tags::{Case, Number, Possessive};

    fn cat() -> Catalogues {
        Catalogues::embedded().clone()
    }

    fn forms(word: &str, cls: u16, grad: Option<u8>) -> Vec<String> {
        let cat = cat();
        let inf = Inflection {
            class: cls,
            gradation: grad.map(Gradation),
        };
        inflect(word, inf, &cat, PartOfSpeech::Noun, true)
            .into_iter()
            .map(|g| g.word)
            .collect()
    }

    /// The plural stem the catalogue's partitive plural gives a class, at the
    /// stem-generation boundary rather than after case attachment.
    fn plural_stem_of_word(word: &str, cls: u16, grad: Option<u8>) -> String {
        let cat = cat();
        let para = cat.get(cls).expect("class");
        super::inflect::plural_stems(para, word, cls, grad.map(Gradation))
            .first()
            .map_or_else(String::new, |(weak, _)| weak.clone())
    }

    #[test]
    fn a_fixed_stem_class_offers_its_oblique_from_the_genitive_stem() {
        // `käsi` is `27`, a fixed-stem class inflected from two spellings (the
        // lemma and the fixed `käs`). Its singular oblique is on the genitive
        // `käde`, and a demotion for one spelling must not reach back and demote
        // the other spelling's forms: `kädessä`, `kädellä` and `kätenä` are the
        // real paradigm and are offered.
        for want in ["kädessä", "kädellä", "kädettä", "kätenä"] {
            assert!(
                has_plain("käsi", 27, None, want),
                "käsi does not offer {want}"
            );
        }
    }

    #[test]
    fn a_class_that_cuts_its_lemma_does_not_offer_whole_lemma_oblique_forms() {
        // `aakkonen` is `38`, a class whose model `nainen` cuts the lemma
        // (`naisiin`, never the whole `nainen`). A member's whole lemma is not
        // an oblique stem, so `aakkonensen` and `aakkonenseen` are slid
        // readings and are not offered, while the cut `aakkoseen` is. A
        // class that keeps its lemma (class 1, `talo`) keeps the whole-lemma
        // oblique forms (`talon`).
        assert!(
            has_plain("aakkonen", 38, None, "aakkoseen"),
            "aakkoseen is not offered"
        );
        for wrong in ["aakkonensen", "aakkonenseen", "aakkonenset"] {
            assert!(
                !has_plain("aakkonen", 38, None, wrong),
                "aakkonen offers the slid {wrong}"
            );
        }
        assert!(
            has_plain("talo", 1, None, "talon"),
            "talo does not offer talon"
        );
    }

    #[test]
    fn a_word_with_two_stem_readings_offers_both() {
        // `joki` is `7*D`: the singular reads its stem `joe` and the plural
        // reads `jo`. Both stems are live readings of the class, so a form
        // matching either is offered rather than demoted to a guess just
        // because the class does not settle on one reading.
        assert!(
            has_plain("joki", 7, Some(4), "joissa"),
            "joissa is not offered"
        );
        assert!(
            has_plain("joki", 7, Some(4), "joista"),
            "joista is not offered"
        );
    }

    #[test]
    fn the_illative_of_a_consonant_final_lemma_is_not_doubled() {
        // The illative `correct` form doubles the lemma's final vowel (`aamu`
        // is `aamuun`). A consonant-final lemma has no doubled vowel to build,
        // so it must not get the broken doubled-consonant form: `aakkoset`
        // ends in `t`, so `aakkosettn` is not a word and `aakkoseen` is.
        assert!(
            has("aakkoset", 38, None, "aakkoseen"),
            "aakkoset is missing the illative aakkoseen"
        );
        assert!(
            !has("aakkoset", 38, None, "aakkosettn"),
            "aakkoset offers the broken aakkosettn"
        );
    }

    #[test]
    fn the_plural_oblique_is_built_on_the_class_plural_stem() {
        // The plural stem is a stem the class shares rather than a cut of the
        // model word: `nainen` is `naise` in the singular and `nais` in the
        // plural, and the edit between them is what the members make to their
        // own stems. So `aakkonen` is `aakkos` in the plural and takes
        // its plural oblique there. This is the case the lcp-derived stem got
        // wrong: reading the plural stem out of the model and cutting the
        // lemma to what the two share pluralised this on a shortened stem.
        for want in [
            "aakkosia",
            "aakkosissa",
            "aakkosilla",
            "aakkosina",
            "aakkosiksi",
        ] {
            assert!(
                has_plain("aakkonen", 38, None, want),
                "aakkonen does not offer {want}"
            );
        }
    }

    #[test]
    fn a_consonant_final_member_takes_the_same_plural_stem() {
        // The class reaches `aakkoset` with the same cut as `aakkonen`,
        // `aakkose`, and the class's stem rule needs a substitution only
        // because the model's own stem has an `n` to change. There is nothing
        // to substitute in a member that ends in `t`, which used to throw the
        // reading away and leave the word with no plural oblique at all: the
        // whole lemma was then the plural stem, and every plural case came out
        // on `aakkoset-` instead of `aakkos-`.
        for want in [
            "aakkosia",
            "aakkosissa",
            "aakkosilla",
            "aakkosina",
            "aakkosiksi",
        ] {
            assert!(
                has_plain("aakkoset", 38, None, want),
                "aakkoset does not offer {want}"
            );
        }
        // The singular oblique is on the same stem, and is unaffected. The
        // illative is a lexical case, so it is a reading rather than a plain
        // form, which is how the test above already takes it.
        assert!(
            has("aakkoset", 38, None, "aakkoseen"),
            "aakkoset does not offer aakkoseen"
        );
    }

    #[test]
    fn the_plural_oblique_does_not_take_the_singular_i() {
        // The plural oblique *adds* an `i` to the plural stem, so a stem already
        // ending in one would double it. The plural stem is the consonant stem
        // `nais`, and the local cases gradate it to `jo`/`jok`, but neither
        // makes `naisiissa`; that is the form an earlier marker-length fix
        // produced for `nainen`.
        for want in ["naisia", "naisissa", "naisilla", "naisina", "naisiksi"] {
            assert!(
                has_plain("nainen", 38, None, want),
                "nainen does not offer {want}"
            );
        }
        for wrong in ["naisiissa", "naisiilla", "naisiina", "naisiiksi"] {
            assert!(
                !has_plain("nainen", 38, None, wrong),
                "nainen offers the doubled {wrong}"
            );
        }
    }

    #[test]
    fn reverse_gradation_lexical_cases_are_offered() {
        // `hammas` is `41*H`: the nominative carries the weak `mm`, so the
        // genitive `hampaan` and the plural `hampaat` are the strong grade.
        // Those are the reverse word's own lexical paradigm, owned by the
        // alignment, so they are offered as plain rather than demoted.
        for want in [
            "hammas",
            "hampaan",
            "hammasta",
            "hampaat",
            "hampaiden",
            "hampaissa",
        ] {
            assert!(
                has_plain("hammas", 41, Some(8), want),
                "hammas does not offer {want}"
            );
        }
    }

    #[test]
    fn the_plural_stem_of_the_fixed_stem_classes_is_the_weak_stem() {
        // `sisin` is `sisimm` and `vasen` is `vasemm` in the plural, the weak
        // grade of their fixed stems, not the strong `sisimp`/`vasemp` that the
        // plural partitive (`sisimpiä`, `vasempia`) alone would give.
        assert_eq!(plural_stem_of_word("sisin", 36, None), "sisimm");
        assert_eq!(plural_stem_of_word("vasen", 37, None), "vasemm");
    }

    #[test]
    fn the_plural_stem_of_omena_drops_the_theme_to_o() {
        // `omena` is `omeno` in the plural (the a-to-o the class-9 kala also
        // makes), not the `omeni` of the partitive `omenia`.
        assert_eq!(plural_stem_of_word("omena", 11, None), "omeno");
    }

    fn has(word: &str, cls: u16, grad: Option<u8>, want: &str) -> bool {
        forms(word, cls, grad).iter().any(|f| f == want)
    }

    fn has_plain(word: &str, cls: u16, grad: Option<u8>, want: &str) -> bool {
        let cat = cat();
        let inf = Inflection {
            class: cls,
            gradation: grad.map(Gradation),
        };
        inflect(word, inf, &cat, PartOfSpeech::Noun, true)
            .into_iter()
            .any(|f| f.plain() && f.word == want)
    }

    #[test]
    fn simple_noun_all_cases() {
        // class 1, valo: valo valon valoa valoon valot valojen valoja valoihin
        for want in [
            "valo", "valon", "valoa", "valoon", "valot", "valojen", "valoja", "valoihin",
        ] {
            assert!(has("valo", 1, None, want), "missing {want}");
        }
    }

    #[test]
    fn epenthesis_before_j() {
        // Class 1's plural genitive is -jen: valo -> talojen.
        assert!(has("talo", 1, None, "talojen"), "missing talojen");
    }

    #[test]
    fn i_family_plural_stem() {
        // Class 5 is the -i family (aari); its plural stem writes the final
        // i as e: genitive -en (aarien), partitive -eja (aareja).
        assert!(has("aari", 5, None, "aarien"), "missing aarien");
        assert!(has("aari", 5, None, "aareja"), "missing aareja");
        assert!(has("aari", 5, None, "aarin"), "missing aarin");
    }

    #[test]
    fn the_plural_genitive_keeps_the_strong_grade() {
        // A gradating noun keeps the strong grade in the plural genitive:
        // `joki` is `jokien`, `umpi` is `umpien`, `pankki` is `pankkien`, and
        // not the weakened `joien`, `ummien`, `pankien`.
        assert!(has("joki", 7, Some(4), "jokien"), "missing jokien");
        assert!(
            !has_plain("joki", 7, Some(4), "joien"),
            "unexpected weak joien"
        );
        assert!(has("umpi", 7, Some(8), "umpien"), "missing umpien");
        assert!(
            !has_plain("umpi", 7, Some(8), "ummien"),
            "unexpected weak ummien"
        );
        assert!(has("pankki", 5, Some(1), "pankkien"), "missing pankkien");
    }

    #[test]
    fn the_possessive_uses_the_strong_genitive_stem() {
        // A noun whose genitive stem is not its lemma attaches the possessive
        // to that stem: `joki` is `jokeni`, `ovi` is `oveni` and `lapsi` is
        // `lapseni`, not the `jokini`, `ovini` and `lapsini` of the nominative.
        assert!(has_plain("joki", 7, Some(4), "jokeni"), "missing jokeni");
        assert!(
            !has_plain("joki", 7, Some(4), "jokini"),
            "unexpected jokini"
        );
        assert!(has_plain("ovi", 7, None, "oveni"), "missing oveni");
        assert!(!has_plain("ovi", 7, None, "ovini"), "unexpected ovini");
        assert!(has_plain("lapsi", 29, None, "lapseni"), "missing lapseni");
    }

    #[test]
    fn gradation_applied() {
        // kaappi : kaapin, type B
        assert!(has("kaappi", 5, Some(2), "kaapin"));
        // laatikko : laatikko, type A -- the genitive is laatikon
        assert!(has("laatikko", 4, Some(1), "laatikon"), "missing laatikon");
        // kauppa : kaupan, type B
        assert!(has("kauppa", 9, Some(2), "kaupan"), "missing kaupan");
    }

    #[test]
    fn i_epenthesis_nouns() {
        assert!(has("koira", 10, None, "koirien"), "missing koirien");
        assert!(has("kala", 9, None, "kalojen"), "missing kalojen");
        assert!(has("omena", 11, None, "omenien"), "missing omenien");
    }

    #[test]
    fn vowel_final_stem_loses_last_vowel() {
        // kylmä is the askel paradigm (49); kylmien needs the split pulled back.
        assert!(has("kylmä", 49, None, "kylmien"), "missing kylmien");
    }

    #[test]
    fn possessive_suffixes() {
        assert!(has("talo", 1, None, "taloni"), "missing taloni");
        assert!(has("talo", 1, None, "talomme"), "missing talomme");
    }

    #[test]
    fn every_catalogue_slot_takes_a_possessive_but_the_plural_nominative() {
        // The possessive hangs off the nominative and the genitive singular,
        // and the two spellings coincide there, so `talot` has no possessive of
        // its own. Every other slot has one.
        assert!(has_plain("talo", 1, None, "taloani"), "missing taloani");
        assert!(has_plain("talo", 1, None, "talojani"), "missing talojani");
        assert!(has_plain("talo", 1, None, "talojeni"), "missing talojeni");
        assert!(has_plain("talo", 1, None, "taloihini"), "missing taloihini");
        for want in ["talotani", "talotasi", "talotamme", "talotansa"] {
            assert!(!has_plain("talo", 1, None, want), "unexpected {want}");
        }
    }

    #[test]
    fn the_partitive_takes_the_possessive_suffix_as_it_stands() {
        // Nothing is dropped and nothing is moved: the partitive ending sits in
        // front of the suffix rather than being displaced by it.
        assert!(has_plain("talo", 1, None, "taloani"), "missing taloani");
        assert!(has_plain("talo", 1, None, "talojani"), "missing talojani");
        assert!(
            has_plain("lutkutus", 39, None, "lutkutustani"),
            "missing lutkutustani"
        );
        assert!(has_plain("kissa", 9, None, "kissaani"), "missing kissaani");
        assert!(has_plain("maa", 18, None, "maatani"), "missing maatani");
        // The third person is spelled both ways and both are offered: the bare
        // `-nsa` and a doubling of the last vowel.
        for want in ["taloansa", "taloaan", "talojansa", "talojaan"] {
            assert!(has_plain("talo", 1, None, want), "missing {want}");
        }
    }

    #[test]
    fn a_possessive_that_only_drops_a_case_ending_is_a_plain_attachment() {
        // The genitive and the illative drop their `n` before the suffix, and
        // the plural genitive and illative drop it too. The stem is untouched,
        // so these are plain forms a correction may be drawn from rather than
        // repairs of the lemma.
        for want in [
            "taloni",
            "talooni",
            "taloihini",
            "talojeni",
            "taloihinsa",
            "talojensa",
        ] {
            assert!(has_plain("talo", 1, None, want), "missing {want}");
        }
    }

    #[test]
    fn phonotactics_rejects_impossible_clusters() {
        assert!(!is_phonotactic("koiraiaa")); // four vowels
        assert!(!is_phonotactic("taloaaa")); // three of the same consonant
        assert!(is_phonotactic("taloihin"));
        assert!(is_phonotactic("jäätyä"));
        // Three vowels in a row is ordinary Finnish, not a broken generation.
        assert!(is_phonotactic("valtioon"));
        assert!(is_phonotactic("korkeaa"));
    }

    /// An adjective's comparative and superlative forms are derived from
    /// the positive, so `find_bases` recognises them as forms of it.
    #[test]
    fn an_adjective_derives_its_degree_paradigms() {
        let cat = Catalogues::embedded();
        let lemma = crate::kotus::sanalista()
            .by_word("suuri")
            .next()
            .expect("suuri is in the sanalista");
        let inflections: Vec<Inflection> = lemma.inflections().collect();
        let got = inflect_all(
            lemma.word(),
            &inflections,
            cat,
            None,
            PartOfSpeech::Adjective,
            false,
        );
        for want in ["suurempi", "suuremman", "suurin", "suurimman"] {
            assert!(
                got.iter().any(|f| f.word == want),
                "the degree {want} is not derived"
            );
        }
    }

    /// `usea` is a pronomini in the sanalista, but Omorfi derives its
    /// degree paradigm and the table carries the row. The superlative
    /// plural `useimmat` (the reported word) has to come from the same
    /// derivation the adjectives use, not a special case.
    #[test]
    fn a_pronominal_quantifier_derives_its_degree_paradigm() {
        let cat = Catalogues::embedded();
        let lemma = crate::kotus::sanalista()
            .by_word("usea")
            .next()
            .expect("usea is in the sanalista");
        let inflections: Vec<Inflection> = lemma.inflections().collect();
        let got = inflect_all(
            lemma.word(),
            &inflections,
            cat,
            None,
            PartOfSpeech::Pronoun,
            false,
        );
        for want in ["useampi", "useamman", "useimmat", "useimpia"] {
            assert!(
                got.iter().any(|f| f.word == want),
                "the degree {want} is not derived"
            );
        }
    }

    /// A `-toista` numeral's paradigm is derived from its first member:
    /// `kuusitoista` inflects `kuusi` and keeps `toista` fixed.
    #[test]
    fn a_toista_numeral_derives_its_paradigm() {
        let cat = Catalogues::embedded();
        let lemma = crate::kotus::sanalista()
            .by_word("kuusitoista")
            .next()
            .expect("kuusitoista is in the sanalista");
        let inflections: Vec<Inflection> = lemma.inflections().collect();
        let got = inflect_all(
            lemma.word(),
            &inflections,
            cat,
            None,
            PartOfSpeech::Numeral,
            false,
        );
        for want in ["kuusitoista", "kuudentoista", "kuuttatoista"] {
            assert!(
                got.iter().any(|f| f.word == want),
                "the numeral form {want} is not derived"
            );
        }
    }

    #[test]
    fn every_catalogue_form_is_reproduced() {
        // The catalogue lists the cases it covers, in the order the Finnish
        // cases are given. Running the inflector on each class model and
        // checking the results contain all of them is what keeps the stem
        // splitting, the length compensation and the gradation honest.
        let cat = Catalogues::embedded().clone();
        // The class 51 compounds need a lexicon to find their head in, which is
        // what the dictionary build passes. It comes from the embedded text, so
        // this test cannot quietly stop covering the compound classes by losing
        // a data file -- which is what the old read-and-skip fallback allowed.
        let lexicon = crate::kotus::parse_sanalista(crate::SANALISTA);
        // The gradation each class model really carries, read out of the
        // sanalista. The template is built under exactly this gradation
        // (`tools/src/bin/build_catalogue.rs`), so the runs below use the
        // model's own flag and not an invented one: a grade the model does
        // not carry is a paradigm it does not have.
        let mut real: std::collections::BTreeMap<&str, Vec<Option<Gradation>>> =
            std::collections::BTreeMap::new();
        for lem in &lexicon {
            let entry = real.entry(lem.word).or_default();
            for inf in &lem.inflections {
                if !entry.contains(&inf.gradation) {
                    entry.push(inf.gradation);
                }
            }
        }
        let mut checked = 0;
        let mut failed = Vec::new();
        for (class, para) in cat.iter() {
            let model = para.model();
            let grads = real.get(model).cloned().unwrap_or_else(|| vec![None]);
            for grad in grads {
                let inflection = Inflection {
                    class: *class,
                    gradation: grad,
                };
                let generated =
                    inflect_in(model, inflection, &cat, Some(&LexiconIndex::new(&lexicon)), PartOfSpeech::Noun, true);
                let got: Vec<&str> = generated.iter().map(|g| g.word.as_str()).collect();
                for (i, slot) in para.slots.iter().enumerate() {
                    for want in slot.variants() {
                        checked += 1;
                        if !got.contains(&want) {
                            failed.push(format!("{class}:{model} slot {i} {want}"));
                        }
                    }
                }
            }
        }
        assert!(checked > 400, "only {checked} forms checked");
        assert!(
            failed.is_empty(),
            "{} of {checked} missing: {:?}",
            failed.len(),
            &failed[..failed.len().min(12)]
        );
    }

    /// A class-50 compound whose head is not a lemma of its own still inflects
    /// in the class the sanalista declares beside the marker.
    ///
    /// `find_parts` splits a compound by looking for a tail the sanalista lists,
    /// which fails when the head is not listed. `ahkeraliisa` is `ahkera` +
    /// `liisa` and `liisa` is not in the sanalista at all, so there was no
    /// split, and falling through applied the class-50 model to the whole word.
    /// The model aligns the compound against itself and lands the gradation in
    /// the middle of it: 168 forms including `ahkeraldin` and `ahkeraleilla`,
    /// none of which is a Finnish word.
    ///
    /// Class 50 asserts the *initial* part stays uninflected, so with no split
    /// the whole word behaves as the head's class. Asserted on the forms rather
    /// than on the mechanism, since the mechanism is what changed.
    #[cfg(feature = "generated-nominals")]
    #[test]
    fn a_class_50_compound_with_no_split_inflects_as_its_head() {
        let cat = cat();
        let lexicon = crate::kotus::sanalista().to_lemmas();
        // `Gradation` counts `A..Z` as `1..=26`, so a letter has to go through
        // `from_letter` rather than being cast.
        let plain = |word: &str, class: u16, grad: Option<char>| -> Vec<String> {
            inflect_in(
                word,
                Inflection {
                    class,
                    gradation: grad.and_then(Gradation::from_letter),
                },
                &cat,
                Some(&LexiconIndex::new(&lexicon)),
                PartOfSpeech::Noun,
                true,
            )
            .into_iter()
            .filter(|f| f.readings.iter().all(|r| r.possessive.is_none()))
            .map(|g| g.word)
            .collect()
        };

        for (word, class, grad) in [
            ("ahkeraliisa", 9, None),
            ("vapaaherratar", 32, Some('C')),
            ("laiskajaakko", 1, Some('A')),
            ("helppoheikki", 5, Some('A')),
        ] {
            let via_marker = plain(word, 50, None);
            let via_head = plain(word, class, grad);
            assert_eq!(
                via_marker, via_head,
                "{word}: the class-50 fallback did not reach class {class} \
                 (marker gave {} forms, head gave {})",
                via_marker.len(),
                via_head.len()
            );
        }
        // And the point of it, as a fact about words: the genitive inflects the
        // tail and leaves the head alone. Before, class 50 gave 168 forms of
        // which these were among the plainest -- `ahkeraldin`, `ahkeraleilla`.
        assert!(plain("ahkeraliisa", 9, None).contains(&"ahkeraliisan".to_string()));
        assert!(plain("vapaaherratar", 32, Some('C')).contains(&"vapaaherrattaren".to_string()));

        // A compound that *does* split keeps the part-by-part engine, which is
        // what puts the owner suffix on the right word. `nuoripari` is 51 and
        // inflects its initial part, so it must not fall back to class 5.
        let nuoripari = plain("nuoripari", 51, None);
        assert!(
            nuoripari.contains(&"nuoreenpariin".to_string()),
            "nuoripari lost its class-51 split: {:?}",
            &nuoripari[..nuoripari.len().min(8)]
        );
    }

    /// A hyphenated class-51 compound keeps its hyphen in every cell.
    ///
    /// `nuori-isäntä` is the one class-51 lemma the sanalista writes with a
    /// hyphen, and the hyphen is part of the established expression rather than
    /// a nominative-only boundary marker: the table is `nuoren-isännän`,
    /// `nuorta-isäntää`, `nuoret-isännät`. `find_parts` could not see it before
    /// because it looks for `isäntä` after the head and the row spells the tail
    /// `-isäntä`.
    #[cfg(feature = "generated-nominals")]
    #[test]
    fn a_hyphenated_class_51_compound_keeps_its_hyphen() {
        let cat = cat();
        let lexicon = crate::kotus::sanalista().to_lemmas();
        let forms: Vec<String> = inflect_in(
            "nuori-isäntä",
            Inflection {
                class: 51,
                gradation: None,
            },
            &cat,
            Some(&LexiconIndex::new(&lexicon)),
            PartOfSpeech::Noun,
            false,
        )
        .into_iter()
        .map(|g| g.word)
        .collect();
        for expected in [
            "nuori-isäntä",
            "nuoren-isännän",
            "nuorta-isäntää",
            "nuoressa-isännässä",
            "nuoresta-isännästä",
            "nuoreen-isäntään",
            "nuorella-isännällä",
            "nuorelta-isännältä",
            "nuorelle-isännälle",
            "nuorena-isäntänä",
            "nuoreksi-isännäksi",
            "nuoretta-isännättä",
            "nuoret-isännät",
            "nuorten-isäntien",
            "nuoria-isäntiä",
            "nuorissa-isännissä",
            "nuorista-isännistä",
            "nuoriin-isäntiin",
            "nuorilla-isännillä",
            "nuorilta-isänniltä",
            "nuorille-isännille",
            "nuorina-isäntinä",
            "nuoriksi-isänniksi",
            "nuoritta-isännittä",
            "nuorin-isännin",
        ] {
            assert!(
                forms.iter().any(|f| f == expected),
                "{expected} is missing from nuori-isäntä"
            );
        }
    }

    /// The adjective prefixes in class-50 compounds stay uninflected.
    ///
    /// `kevyt` is a prefix in `kevytsora`, not a word that agrees with `sora`,
    /// so the genitive is `kevytsoran`, not `kevyensoran`; the same holds for
    /// `iso`, `hapan`, `ruskea` and `laiska`.
    #[cfg(feature = "generated-nominals")]
    #[test]
    fn an_adjective_prefix_in_a_class_50_compound_stays_fixed() {
        let cat = cat();
        let lexicon = crate::kotus::sanalista().to_lemmas();
        let forms = |word: &str| -> Vec<String> {
            inflect_in(
                word,
                Inflection {
                    class: 50,
                    gradation: None,
                },
                &cat,
                Some(&LexiconIndex::new(&lexicon)),
                PartOfSpeech::Noun,
                false,
            )
            .into_iter()
            .map(|g| g.word)
            .collect()
        };
        for (word, genitive, wrong) in [
            ("kevytsora", "kevytsoran", "kevyensoran"),
            ("isojalka", "isojalan", "isonjalan"),
            ("hapannaama", "hapannaaman", "happamannaaman"),
            ("ruskeakieli", "ruskeakielen", "ruskeankielen"),
            ("laiskamato", "laiskamadon", "laiskanmadon"),
        ] {
            let rows = forms(word);
            assert!(
                rows.iter().any(|f| f == genitive),
                "{word}: expected {genitive}"
            );
            assert!(
                !rows.iter().any(|f| f == wrong),
                "{word}: the prefix inflected to {wrong}"
            );
        }
    }

    /// A compound whose tail is a two-letter word still splits.
    ///
    /// `find_parts` required *both* parts of a compound to be three letters or
    /// more, which threw away every compound ending in an ordinary two-letter
    /// word. `puoliyö` is `puoli` + `yö`: the sanalista row is `50, 51`, both
    /// halves are lemmas of their own, and the tail's length was the only thing
    /// in the way. The reference confirms 23 forms for the word -- `puolenyön`,
    /// `puolenayönä`, `puoletyöt`, `puoleenyöhön`, `puoliinöihin`,
    /// `puolettayöttä` -- and the nominative was the one torille produced.
    ///
    /// Lowering the floor is not enough on its own, and this test is where that
    /// shows. The sanalista lists 48 two-letter lemmas and only 23 of them can
    /// carry a case: the ten pronouns (`sa`, `ma`, `te`) are 99 or 101 and the
    /// fourteen interjections (`ai`, `ha`, `in`, `no`) carry no class at all.
    /// `ahkeraliisa` ends in `sa`, so a floor with nothing behind it splits the
    /// word there and hands back a single form -- which is precisely the failure
    /// the test above exists to rule out. The tail therefore has to be short
    /// *and* inflectable, and neither half of that is sufficient on its own.
    #[cfg(feature = "generated-nominals")]
    #[test]
    fn a_compound_whose_tail_is_two_letters_still_splits() {
        let cat = cat();
        let lexicon = crate::kotus::sanalista().to_lemmas();
        let plain = |word: &str, class: u16| -> Vec<String> {
            inflect_in(
                word,
                Inflection {
                    class,
                    gradation: None,
                },
                &cat,
                Some(&LexiconIndex::new(&lexicon)),
                PartOfSpeech::Noun,
                true,
            )
            .into_iter()
            .filter(|f| f.readings.iter().all(|r| r.possessive.is_none()))
            .map(|g| g.word)
            .collect()
        };

        // The 51 branch is the one that inflects the initial part, which is what
        // the reference shows: `puolen` + `yön` is neither part left alone.
        let forms = plain("puoliyö", 51);
        for expected in [
            "puolenyön",     // head in the genitive, tail in the genitive
            "puolenayönä",   // both in the inessive
            "puoletyöt",     // head translative, tail partitive
            "puolettayöttä", // and the tail takes the case ending itself
            "puoleenyöhön",  // head illative, tail illative
            "puoliinöihin",  // ... which the inessive would get wrong
        ] {
            assert!(
                forms.iter().any(|w| w == expected),
                "puoliyö lost {expected}: {} forms, none of them the split",
                forms.len()
            );
        }

        // The two-letter tail is not the only shape the floor was hiding:
        // `normaalilämpö`, `vinoköysisilta` and `syväkivilaji` were all short of
        // their reference before, and `tuore-elintarvike` had a tail the sanalista
        // does not mark as inflecting. Each reaches the reference whole now.
        for (word, class, expected) in [
            ("normaalilämpö", 50, "normaalilämpöön"),
            ("vinoköysisilta", 50, "vinoköysisillalle"),
            ("syväkivilaji", 50, "syväkivilajille"),
            ("tuore-elintarvike", 50, "tuore-elintarvikkeelle"),
        ] {
            let forms = plain(word, class);
            assert!(
                forms.iter().any(|w| w == expected),
                "{word} lost {expected}: {} forms",
                forms.len()
            );
        }

        // And the filter that lets a short tail through is not a word list, so
        // it has to reject a short tail that is not a noun. `sa` is two letters
        // and is in the sanalista, and the word ends in it.
        let liisa = plain("ahkeraliisa", 50);
        assert_eq!(
            liisa.len(),
            plain("ahkeraliisa", 9).len(),
            "ahkeraliisa stopped reaching its head's class, so `sa` was taken for a tail"
        );
    }

    /// A class-50 tail the sanalista spells as a bound form still inflects.
    ///
    /// `suorakulmio` is `suora` + `kulmio`, and the sanalista lists the head as
    /// `‑kulmio` with a leading U+2011. The tail index matches spellings, so it
    /// could not find the head and the word fell through to the class-50 model,
    /// which aligned it against `isoisä` and produced `suorakulmdin`. The
    /// bound form is looked up with the hyphen prepended, and the hyphen comes
    /// off every generated form.
    #[cfg(feature = "generated-nominals")]
    #[test]
    fn a_bound_combining_form_can_be_a_compound_tail() {
        let cat = cat();
        let lexicon = crate::kotus::sanalista().to_lemmas();
        let forms: Vec<String> = inflect_in(
            "suorakulmio",
            Inflection {
                class: 50,
                gradation: None,
            },
            &cat,
            Some(&LexiconIndex::new(&lexicon)),
            PartOfSpeech::Noun,
            true,
        )
        .into_iter()
        .map(|g| g.word)
        .collect();

        for expected in [
            "suorakulmion",  // genitive singular
            "suorakulmiota", // partitive singular
            "suorakulmiossa",
            "suorakulmioiden",
        ] {
            assert!(
                forms.iter().any(|w| w == expected),
                "suorakulmio lost {expected}: {} forms",
                forms.len()
            );
        }
        assert!(
            !forms.iter().any(|w| w.starts_with("suorakulmd")),
            "suorakulmio still falls through to the class-50 model"
        );
        assert!(
            !forms.iter().any(|w| w.contains('\u{2011}')),
            "the bound form's hyphen leaked into a compound"
        );
    }

    /// A plural-only compound's tail is the nominative plural of its head.
    ///
    /// `isovanhemmat` is `iso` + `vanhemmat`, and the sanalista lists only
    /// `vanhempi`; `omakustannukset` and `normaaliolot` are the same shape
    /// against `kustannus` and `olo`. The tail index cannot find a base whose
    /// plural differs from its lemma, so these fell through to the class-50
    /// model and produced stems like `isovanhemmaeihin` and
    /// `omakusdannukseikseen`. The bounded neighbourhood probe resolves the
    /// base, and the compound keeps only the base's plural cells.
    #[cfg(feature = "generated-nominals")]
    #[test]
    fn a_plural_only_compound_tail_inflects_from_its_base() {
        let cat = cat();
        let lexicon = crate::kotus::sanalista().to_lemmas();
        let plain = |word: &str| -> Vec<String> {
            inflect_in(
                word,
                Inflection {
                    class: 50,
                    gradation: None,
                },
                &cat,
                Some(&LexiconIndex::new(&lexicon)),
                PartOfSpeech::Noun,
                true,
            )
            .into_iter()
            .filter(|f| f.readings.iter().all(|r| r.possessive.is_none()))
            .map(|g| g.word)
            .collect()
        };

        for (word, expected) in [
            ("isovanhemmat", "isovanhempien"),
            ("isovanhemmat", "isovanhemmitta"),
            ("omakustannukset", "omakustannusten"),
            ("normaaliolot", "normaalioloja"),
            ("täysisisarukset", "täysisisarusten"),
        ] {
            let forms = plain(word);
            assert!(
                forms.iter().any(|w| w == expected),
                "{word} lost {expected}: {} forms",
                forms.len()
            );
        }

        // The compound is plural-only, so it has no singular at all.
        let forms = plain("isovanhemmat");
        assert!(
            forms.iter().all(|w| w != "isovanhempi"),
            "isovanhemmat gained a singular"
        );
        assert!(
            !forms.iter().any(|w| w.starts_with("isovanhemmaei")),
            "isovanhemmat still falls through to the class-50 model"
        );
    }

    /// A compound-like tail is a compound of its own.
    ///
    /// `mustaviinimarja` is `musta` + `viini` + `marja`, and the sanalista lists
    /// the middle tail `viinimarja` as a compound-like row with no inflection of
    /// its own. The class-51 split therefore found no tail and the word produced
    /// nothing at all. The tail is now resolved as its own class-50 compound
    /// (`viini` fixed, `marja` inflecting), and the class-51 first part inflects
    /// as well: `mustanviinimarjan`, `mustassaviinimarjassa`.
    #[cfg(feature = "generated-nominals")]
    #[test]
    fn a_compound_like_tail_is_split_recursively() {
        let cat = cat();
        let lexicon = crate::kotus::sanalista().to_lemmas();
        let plain = |word: &str| -> Vec<String> {
            inflect_in(
                word,
                Inflection {
                    class: 51,
                    gradation: None,
                },
                &cat,
                Some(&LexiconIndex::new(&lexicon)),
                PartOfSpeech::Noun,
                true,
            )
            .into_iter()
            .filter(|f| f.readings.iter().all(|r| r.possessive.is_none()))
            .map(|g| g.word)
            .collect()
        };

        let forms = plain("mustaviinimarja");
        for expected in [
            "mustaviinimarja",       // nominative singular
            "mustanviinimarjan",     // first part and head in the genitive
            "mustaaviinimarjaa",     // ... and the partitive
            "mustassaviinimarjassa", // ... and the inessive
            "mustienviinimarjojen",  // plural genitive
            "mustiaviinimarjoja",    // plural partitive
        ] {
            assert!(
                forms.iter().any(|w| w == expected),
                "mustaviinimarja lost {expected}: {} forms",
                forms.len()
            );
        }
        // The middle `viini` stays in the nominative in every cell.
        assert!(
            forms.iter().all(|w| w.contains("viini")),
            "the middle part was inflected"
        );
    }

    /// A noun compound does not take the head's adjective degrees.
    ///
    /// `happiradikaali` is a noun, so `happiradikaaleimmittaan` is not a word,
    /// even though its head `radikaali` has degrees. An adjective compound does
    /// take them: `tummanpunaisempi` is `tumman` + `punaisempi`.
    #[test]
    fn a_noun_compound_does_not_take_the_heads_degrees() {
        let cat = cat();
        let lexicon = crate::kotus::sanalista().to_lemmas();
        let index = LexiconIndex::new(&lexicon);
        let words = |word: &str, pos: PartOfSpeech| -> Vec<String> {
            inflect_all(word, &[], &cat, Some(&index), pos, pos == PartOfSpeech::Noun)
                .into_iter()
                .map(|g| g.word)
                .collect()
        };

        let noun = words("happiradikaali", PartOfSpeech::Noun);
        assert!(
            noun.iter().any(|w| w == "happiradikaalin"),
            "happiradikaali lost its own paradigm"
        );
        assert!(
            !noun.iter().any(|w| w.contains("radikaaleimmi")),
            "a noun compound took the head's degrees"
        );

        let adjective = words("tummanpunainen", PartOfSpeech::Adjective);
        assert!(
            adjective.iter().any(|w| w == "tummanpunaisempi"),
            "an adjective compound lost its comparative"
        );
    }

    /// A class-51 head that appears as its nominative plural.
    ///
    /// `isotaivot` is `isot` + `aivot`, not `iso` + `taivot`: the first part is
    /// the nominative plural of `iso`, so the ordinary head scan finds no tail.
    /// The compound is plural-only, like `isovanhemmat`, and both parts carry
    /// the plural case: `isojenaivojen`, `isojaaivoja`, `isoissaaivoissa`.
    #[test]
    fn a_plural_head_inflects_both_parts_in_the_plural() {
        let cat = cat();
        let lexicon = crate::kotus::sanalista().to_lemmas();
        let plain = |word: &str| -> Vec<String> {
            inflect_in(
                word,
                Inflection {
                    class: 51,
                    gradation: None,
                },
                &cat,
                Some(&LexiconIndex::new(&lexicon)),
                PartOfSpeech::Noun,
                true,
            )
            .into_iter()
            .filter(|f| f.readings.iter().all(|r| r.possessive.is_none()))
            .map(|g| g.word)
            .collect()
        };

        for (word, expected) in [
            ("isotaivot", "isojenaivojen"),
            ("isotaivot", "isojaaivoja"),
            ("isotaivot", "isoissaaivoissa"),
            ("pitkäthousut", "pitkienhousujen"),
            ("pitkäthousut", "pitkiähousuja"),
            ("pitkäthousut", "pitkissähousuissa"),
        ] {
            let forms = plain(word);
            assert!(
                forms.iter().any(|w| w == expected),
                "{word} lost {expected}: {} forms",
                forms.len()
            );
        }
        // A plural head makes a plural-only compound: no singular cell.
        let forms = plain("isotaivot");
        assert!(
            !forms.iter().any(|w| w == "isonaivo"),
            "a plural-head compound produced a singular: {forms:?}"
        );
    }

    /// A class-51 head that is a participle.
    ///
    /// `särkynytsydän` is `särkynyt` + `sydän`, and `särkynyt` is not a
    /// sanalista lemma: it is the `-nyt` participle of `särkyä`. The ordinary
    /// head scan cannot see it, so the class-47 engine inflects the participle
    /// and the class-51 join does the rest.
    #[test]
    fn a_participle_head_inflects_with_the_class_47_paradigm() {
        let cat = cat();
        let lexicon = crate::kotus::sanalista().to_lemmas();
        let forms: Vec<String> = inflect_in(
            "särkynytsydän",
            Inflection {
                class: 51,
                gradation: None,
            },
            &cat,
            Some(&LexiconIndex::new(&lexicon)),
            PartOfSpeech::Noun,
            false,
        )
        .into_iter()
        .map(|g| g.word)
        .collect();

        for expected in [
            "särkyneensydämen",     // genitive singular
            "särkyneetsydämet",     // nominative plural
            "särkyneittäsydämittä", // abessive plural
        ] {
            assert!(
                forms.iter().any(|w| w == expected),
                "särkynytsydän lost {expected}: {} forms",
                forms.len()
            );
        }
    }

    /// Two classes that spell a form the same way say it once.
///
/// `ahtaus` is two homonym rows, `40` and `39`, and the two grids coincide
/// exactly: 318 forms over 159 words, each word twice, each copy with the same
/// reading and the same confidence. Emitting the pair is 2 667 redundant rows
/// over 50 fingerprint lemmas.
///
/// `appi` is `7*B` and `5*B`, and it is the case the other side of this: its
/// grids also overlap, but `apikseen` is the plural translative under one and
/// the singular under the other. Same spelling, two readings, neither redundant
/// -- so a dedup keyed on the spelling alone would delete a fact about the word.
#[test]
    fn two_classes_that_spell_a_form_the_same_way_say_it_once() {
        let cat = cat();
        let lexicon = crate::kotus::sanalista().to_lemmas();
        let all = |word: &str, classes: &[(u16, Option<char>)]| -> Vec<Generated> {
            let inflections: Vec<Inflection> = classes
                .iter()
                .map(|(class, grad)| Inflection {
                    class: *class,
                    gradation: grad.and_then(Gradation::from_letter),
                })
                .collect();
            inflect_all(
                word,
                &inflections,
                &cat,
                Some(&LexiconIndex::new(&lexicon)),
                PartOfSpeech::Noun,
                true,
            )
        };
        let concatenated = |word: &str, classes: &[(u16, Option<char>)]| -> Vec<Generated> {
            classes
                .iter()
                .flat_map(|c| all(word, std::slice::from_ref(c)))
                .collect()
        };

        // Every word once, and the words themselves unchanged.
        let deduped = all("ahtaus", &[(40, None), (39, None)]);
        let naive = concatenated("ahtaus", &[(40, None), (39, None)]);
        assert_eq!(deduped.len(), 160, "ahtaus should keep 160 words");
        assert_eq!(naive.len(), 320, "concatenating should give 320 rows");
        let spelled: Vec<&str> = deduped.iter().map(|f| f.word.as_str()).collect();
        let mut sorted = spelled.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), spelled.len(), "a word still appears twice");

        // The output is the concatenation with the second copy of each identical row
        // removed -- not re-sorted and not re-grouped. `split::dedup` sorts by word
        // and then by confidence, which changes the order a caller sees, so the order
        // here has to be the order the two classes were run in.
        let mut seen: HashSet<(String, Confidence, Vec<Tags>)> = HashSet::new();
        let expected: Vec<&Generated> = naive
            .iter()
            .filter(|f| seen.insert((f.word.clone(), f.confidence, f.readings.clone())))
            .collect();
        assert_eq!(deduped.len(), expected.len(), "the wrong rows survived");
        for (got, want) in deduped.iter().zip(expected) {
            assert_eq!((&got.word, got.confidence), (&want.word, want.confidence));
            assert_eq!(got.readings, want.readings);
        }

        // And the set of words is untouched: only the repeat count moved.
        fn words(forms: &[Generated]) -> std::collections::HashSet<&str> {
            forms.iter().map(|f| f.word.as_str()).collect()
        }
        assert_eq!(words(&deduped), words(&naive));
        assert_eq!(words(&deduped).len(), 160);

        // A word two classes spell the same but read differently keeps both
        // readings. This is the half of the defect that must not be swept up.
        //
        // `appi` is `7*B` and `5*B`, and `apikseen` is the plural translative in one
        // and the singular in the other: one spelling, two numbers, and dropping
        // either copy loses a fact. A dedup keyed on the spelling would take both
        // out.
        let appi = all("appi", &[(7, Some('B')), (5, Some('B'))]);
        let readings_of = |forms: &[Generated],
                           word: &str|
         -> Vec<(Option<Case>, Option<Number>, Option<Possessive>)> {
            forms
                .iter()
                .filter(|f| f.word == word)
                .flat_map(|f| {
                    f.readings
                        .iter()
                        .map(|r| (r.case, r.number, r.possessive))
                        .collect::<Vec<_>>()
                })
                .collect()
        };
        assert_eq!(
            readings_of(&appi, "apikseen"),
            vec![
                (
                    Some(Case::Translative),
                    Some(Number::Plural),
                    Some(Possessive::Third)
                ),
                (
                    Some(Case::Translative),
                    Some(Number::Singular),
                    Some(Possessive::Third)
                ),
            ],
            "apikseen is the plural translative under 7*B and the singular under 5*B, \
         and both readings are real"
        );

        // And the third way a repeat is not redundant: same spelling, same reading,
        // different confidence. `kungfu` is `1,21`; class 1 reaches `kungfuiden`
        // only with a repair and class 21 derives it cleanly. A `repaired` row beside
        // a `plain` one is the correction being offered, so both stay -- and they
        // come out in the sanalista's order, class 1's copy first.
        let kungfu = all("kungfu", &[(1, None), (21, None)]);
        let confidences_of = |forms: &[Generated], word: &str| -> Vec<Confidence> {
            forms
                .iter()
                .filter(|f| f.word == word)
                .map(|f| f.confidence)
                .collect()
        };
        assert_eq!(
            confidences_of(&kungfu, "kungfuiden"),
            vec![Confidence::Repaired, Confidence::Plain],
            "kungfuiden is repaired under class 1 and plain under class 21, and a \
         correction candidate is not a duplicate of the form it corrects"
        );
    }

    /// A word that does not take possessive suffixes gets no possessive column.
    ///
    /// [`inflect_in`] takes `possessives` as a decision already made from the
    /// part of speech: a noun does take them, an adjective or a participle does
    /// not. This is the test for that contract, and it exists because seven
    /// class modules took the argument and then overrode it with a literal
    /// `true` -- `Builder::new(lemma, true)`, or a delegating call such as
    /// `class_45`'s `class_39::inflect(lemma, true)` -- so the flag was dead
    /// code and every adjective in those classes was handed the whole column
    /// anyway. Class 38 was the bulk of it: 84% of its 4 491 lemmas are
    /// adjectives.
    ///
    /// The two-tree port is complete, so the check is stated against the
    /// contract itself rather than against a second implementation: a class
    /// that ignores the flag has no other tree to disagree with, and the
    /// fixture covers every dispatch key.
    ///
    /// The comitative used to be an exception named here on purpose, so that
    /// this test would not fail for a reason unrelated to what it is testing:
    /// [`Builder::comitative`] pushed `stem + ine` and its five owners with no
    /// `possessives` guard at all, so an adjective of *any* class with a
    /// comitative was handed `taloineni` and its four siblings. That is fixed,
    /// and the assertion below is what now holds it fixed -- it covers every
    /// class in the fixture rather than the eight that were named by hand.
    #[cfg(feature = "generated-nominals")]
    #[test]
    fn a_word_without_possessives_gets_no_possessive_column() {
        const FIXTURE: &str = include_str!("../../tests/fixtures/noun_classes.txt");

        type Dispatch = fn(&str, &str, bool) -> Option<Vec<Generated>>;
        let dispatch: Dispatch = super::nominals::dispatch;

        // Every owned reading a dispatch produced, whichever case it hangs off.
        let owners = |forms: &[Generated]| -> Vec<String> {
            forms
                .iter()
                .filter(|f| f.readings.iter().any(|r| r.possessive.is_some()))
                .map(|f| f.word.clone())
                .collect()
        };

        let mut checked = 0usize;
        let mut owned_when_on = 0usize;
        let mut failed: Vec<String> = Vec::new();
        for line in FIXTURE.lines().filter(|l| !l.starts_with('#')) {
            let Some((class, lemma)) = line.split_once('\t') else {
                continue;
            };
            let Some(forms) = dispatch(class, lemma, false) else {
                failed.push(format!("{class} {lemma}: not dispatched"));
                continue;
            };
            checked += 1;
            let stray = owners(&forms);
            if !stray.is_empty() {
                failed.push(format!(
                    "{class} {lemma}: {} owned form(s) with possessives=false: {:?}",
                    stray.len(),
                    &stray[..stray.len().min(4)]
                ));
            }
            // Non-vacuity: with the flag on, the same lemma must actually
            // produce owned forms, or "no owned forms when off" would pass
            // for a class whose possessive column had quietly gone missing.
            if let Some(on) = dispatch(class, lemma, true) {
                owned_when_on += owners(&on).len();
            }
        }

        assert!(checked > 700, "only {checked} dispatches checked");
        assert!(
            owned_when_on > 25_000,
            "only {owned_when_on} owned forms with possessives=true, so the \
             assertion below is not testing much"
        );
        assert!(
            failed.is_empty(),
            "{} of {checked} dispatches still emit a possessive column with \
             possessives=false:\n  {}",
            failed.len(),
            failed[..failed.len().min(10)].join("\n  ")
        );
    }

    // ---- form_of / forms_of: ask for a specific shape --------------------

    use crate::tags::{Mood, Person, Voice};

    fn looking_for(
        word: &str,
        cls: u16,
        grad: Option<u8>,
        want: Tags,
    ) -> Vec<String> {
        let cat = cat();
        let inf = Inflection {
            class: cls,
            gradation: grad.map(Gradation),
        };
        form_of_in(word, inf, &cat, None, &want, PartOfSpeech::Noun, true)
            .into_iter()
            .map(|f| f.word)
            .collect()
    }

    #[test]
    fn form_of_with_just_a_case_returns_its_singular_and_plural() {
        // The case dimension is set; the number is wildcard. Class 1's
        // inessive forms are `talossa` (singular) and `taloissa` (plural),
        // and both come back.
        let have = looking_for(
            "talo",
            1,
            None,
            Tags { case: Some(Case::Inessive), ..Tags::default() },
        );
        assert!(have.contains(&"talossa".to_string()));
        assert!(have.contains(&"taloissa".to_string()));
    }

    #[test]
    fn form_of_with_case_and_number_narrows_to_the_right_number() {
        // Singular inessive is `talossa`; plural inessive is `taloissa`.
        // With both dimensions set, only the singular forms come back -- and
        // since the possessive dimension is left as a wildcard, the possessives
        // (`talossaan`, ...) come with it, as the superset rule says.
        let have = looking_for(
            "talo",
            1,
            None,
            Tags {
                case: Some(Case::Inessive),
                number: Some(Number::Singular),
                ..Tags::default()
            },
        );
        assert!(have.contains(&"talossa".to_string()));
        assert!(!have.iter().any(|w| w == "taloissa"), "plural leaked through");
        // The possessive variants ride along on the singular stem:
        assert!(have.contains(&"talossaan".to_string()));
    }

    #[test]
    fn form_of_with_an_empty_tags_matches_inflect() {
        // A `None` on every dimension is a wildcard, so the answer equals
        // `inflect` -- this is the round trip the wildcard rule promises.
        let cat = cat();
        let inf = Inflection { class: 1, gradation: None };
        let from_inflect: Vec<String> =
            inflect("talo", inf, &cat, PartOfSpeech::Noun, true)
                .into_iter()
                .map(|f| f.word)
                .collect();
        let from_form_of: Vec<String> =
            inflect("talo", inf, &cat, PartOfSpeech::Noun, true)
                .into_iter()
                .filter(|f| f.matches(&Tags::default()))
                .map(|f| f.word)
                .collect();
        assert_eq!(from_inflect, from_form_of);
    }

    #[test]
    fn form_of_with_possessive_includes_both_bare_and_owned_forms() {
        // The matcher's wildcard rule: a form whose reading has no
        // possessive (a bare `talossa`) matches a `want` with any
        // possessive, because it could be that shape. So adding
        // `possessive: Some(Third)` brings back both `talossa` and
        // `talossaan` -- the bare form (open on possessive) and the
        // explicitly-owned one.
        let want = Tags {
            case: Some(Case::Inessive),
            number: Some(Number::Singular),
            possessive: Some(Possessive::Third),
            ..Tags::default()
        };
        let have = looking_for("talo", 1, None, want);
        assert!(have.contains(&"talossaan".to_string()),
            "owned form missing: {have:?}");
        assert!(have.contains(&"talossa".to_string()),
            "bare form missing per wildcard: {have:?}");
    }

    #[test]
    fn form_of_returns_only_owned_forms_when_bare_are_excluded() {
        // To exclude bare forms, the caller has to ask for the
        // possessive and *not* leave the form-side wildcard room.
        // The matcher's rule says "a None on the form side is open" --
        // so the way to say "no bare forms" is to inspect the
        // returned Forms and reject any whose readings carry no
        // possessive tag. The matcher's `matches()` is intentionally
        // a superset, because that's the question the *engine* knows
        // how to answer; whether a bare form is correct is the
        // *caller's* job once the sentence is in view.
        let cat = cat();
        let inf = Inflection { class: 1, gradation: None };
        let want = Tags {
            case: Some(Case::Inessive),
            number: Some(Number::Singular),
            possessive: Some(Possessive::Third),
            ..Tags::default()
        };
        let owned_only: Vec<String> = form_of_in(
            "talo",
            inf,
            &cat,
            None,
            &want,
            PartOfSpeech::Noun,
            true,
        )
        .into_iter()
        .filter(|f| f
            .readings
            .iter()
            .any(|r| r.possessive == Some(Possessive::Third)))
        .map(|f| f.word)
        .collect();
        assert!(owned_only.contains(&"talossaan".to_string()));
        assert!(!owned_only.contains(&"talossa".to_string()));
    }

    #[test]
    fn forms_of_loops_over_inflections() {
        // `forms_of` walks multiple inflections (a word might be tagged with
        // several classes in the sanalista) and joins the matches, keeping
        // first-occurrence order.
        let cat = cat();
        let inflections = vec![
            Inflection { class: 1, gradation: None },
            Inflection { class: 9, gradation: None },
        ];
        let want = Tags {
            case: Some(Case::Nominative),
            number: Some(Number::Singular),
            ..Tags::default()
        };
        let have: Vec<String> = forms_of(
            "talo",
            &inflections,
            &cat,
            None,
            &want,
            PartOfSpeech::Noun,
            true,
        )
        .into_iter()
        .map(|f| f.word)
        .collect();
        assert!(have.contains(&"talo".to_string()));
        assert_eq!(
            have.iter().filter(|w| *w == "talo").count(),
            1,
            "talo is offered twice across the two inflections"
        );
    }

    #[test]
    fn form_of_filters_by_part_of_speech() {
        // A form whose every reading is the wrong POS must not come back
        // when the caller asks for one POS.
        let cat = cat();
        let inf = Inflection { class: 1, gradation: None };
        let have = form_of(
            "talo",
            inf,
            &cat,
            &Tags {
                part_of_speech: Some(PartOfSpeech::Verb),
                case: Some(Case::Inessive),
                ..Tags::default()
            },
            PartOfSpeech::Noun,
            true,
        );
        for form in &have {
            assert!(
                form.readings
                    .iter()
                    .any(|r| r.part_of_speech == Some(PartOfSpeech::Noun)),
                "form {form:?} has no noun reading"
            );
        }
    }

    #[test]
    fn form_of_match_is_dense_on_the_form_side() {
        // The matcher is intentionally a wildcard on every dimension: a form
        // whose reading has no `person` tag (a noun abessive, say) still
        // matches a `want` with `person: Some(First)`, because the bare
        // form *could* be that shape. This is the superset rule; it's the
        // answer the engine knows how to give, and tightening it belongs to
        // the caller once the surrounding sentence is in view.
        let have = looking_for(
            "talo",
            1,
            None,
            Tags {
                case: Some(Case::Abessive),
                person: Some(Person::First),
                ..Tags::default()
            },
        );
        assert!(
            !have.is_empty(),
            "the matcher should still return abessive readings"
        );
    }

    #[test]
    fn form_of_filters_by_tense_and_mood_for_verbs() {
        // The same trick on the verb side. Class 52's present indicative of
        // `lukea` at first-person singular is `luen`; matching on tense,
        // mood, person, number lands the right reading.
        let cat = cat();
        let inf = Inflection { class: 52, gradation: None };
        let want = Tags {
            tense: Some(crate::tags::Tense::Present),
            mood: Some(Mood::Indicative),
            person: Some(Person::First),
            number: Some(Number::Singular),
            ..Tags::default()
        };
        let forms = form_of(
            "lukea",
            inf,
            &cat,
            &want,
            PartOfSpeech::Verb,
            true,
        );
        assert!(
            !forms.is_empty(),
            "lukea + present indicative + First found nothing"
        );
        // Voice is None in `want`, so the active and passive readings both
        // pass -- that's the superset rule the matcher's wildcard implements.
        assert!(
            forms.iter().all(|f| f.matches(&want)),
            "every form must match the want, got {forms:?}"
        );
    }

    #[test]
    fn form_of_voice_filter_narrows_passive_to_passive_only() {
        // Adding `voice = Passive` to the want narrows the answer so only
        // forms whose passive reading fits remain.
        let cat = cat();
        let inf = Inflection { class: 52, gradation: None };
        let want = Tags {
            tense: Some(crate::tags::Tense::Present),
            mood: Some(Mood::Indicative),
            person: Some(Person::First),
            number: Some(Number::Singular),
            voice: Some(Voice::Passive),
            ..Tags::default()
        };
        let forms = form_of(
            "lukea",
            inf,
            &cat,
            &want,
            PartOfSpeech::Verb,
            true,
        );
        assert!(
            forms.iter().all(|f| f
                .readings
                .iter()
                .any(|r| r.voice == Some(Voice::Passive))),
            "every form must have a passive reading, got {forms:?}"
        );
    }

    #[test]
    fn a_classless_compound_inflects_by_a_bound_prefix_head() {
        // `esimerkki` = `esi` + `merkki`: the head `merkki` is class 5 and
        // the modifier `esi` is a bound prefix -- not a word, form or
        // stem. With no class of its own the whole inflects as its head;
        // before `modifier_is_known` accepted a bound prefix (and a known
        // inflected form), `compound_head_forms` produced nothing and
        // every oblique of `esimerkki` was missed.
        let cat = cat();
        let forms = inflect_all("esimerkki", &[], &cat, None, PartOfSpeech::Noun, true);
        let words: Vec<&str> = forms.iter().map(|f| f.word.as_str()).collect();
        for want in ["esimerkin", "esimerkkiä", "esimerkkiään"] {
            assert!(
                words.contains(&want),
                "esimerkki does not inflect to {want}"
            );
        }
    }

    #[test]
    fn a_classless_compound_inflects_by_a_genitive_modifier_head() {
        // The modifier of a compound is very often the genitive singular
        // (`luonnon-` of `luonto`), which the compile-time form set
        // carries. `luonnonkaasu` = `luonnon` + `kaasu` (class 1).
        let cat = cat();
        let forms = inflect_all("luonnonkaasu", &[], &cat, None, PartOfSpeech::Noun, true);
        let words: Vec<&str> = forms.iter().map(|f| f.word.as_str()).collect();
        assert!(
            words.contains(&"luonnonkaasun"),
            "luonnonkaasu does not inflect to luonnonkaasun"
        );
    }

    /// `emits_form` answers exactly what listing the paradigm would.
    ///
    /// It exists to skip the dedup clone in `inflect_all`, not to change the
    /// answer: every word the full list holds must be `true`, and a word it
    /// does not hold `false`, including the degree forms (`pahempi`) and the
    /// verb forms.
    #[test]
    fn emits_form_agrees_with_inflect_all() {
        let cat = cat();
        let lexicon = crate::kotus::sanalista().to_lemmas();
        let index = LexiconIndex::new(&lexicon);
        let cases: &[(&str, &[&str])] = &[
            ("puhdas", &["puhtaan", "puhtaassa", "puhtaat", "puhtaampi", "puhtain"]),
            ("huomata", &["huomasi", "huomaan", "huomannut", "huomattiin"]),
            ("talo", &["talossa", "talon", "taloja", "taloton"]),
            ("paha", &["pahempi", "pahin", "pahaa"]),
            ("kissa", &["kissalle", "kissatta", "kissoja"]),
        ];
        for &(lemma_word, words) in cases {
            let lemma = lexicon
                .iter()
                .find(|l| l.word == lemma_word)
                .unwrap_or_else(|| panic!("{lemma_word} is not in the lexicon"));
            let pos = PartOfSpeech::inflecting_reading(lemma.part_of_speech);
            for &word in words {
                let listed = inflect_all(
                    lemma.word,
                    &lemma.inflections,
                    &cat,
                    Some(&index),
                    pos,
                    false,
                )
                .iter()
                .any(|f| f.word == word);
                let emitted = emits_form(
                    lemma.word,
                    &lemma.inflections,
                    &cat,
                    Some(&index),
                    pos,
                    false,
                    word,
                );
                assert_eq!(listed, emitted, "{lemma_word} / {word}");
            }
        }
    }

    /// The paradigm cache keys on the inflection list, not just the lemma.
    ///
    /// A lemma can carry more than one class (`kungfu` is `1,21`). A caller
    /// may ask about one class and a later caller about the other; if the
    /// cache keyed only on the lemma, the second would get the first's set.
    #[test]
    fn the_paradigm_cache_keys_on_the_inflections() {
        let cat = cat();
        let lexicon = crate::kotus::sanalista().to_lemmas();
        let index = LexiconIndex::new(&lexicon);
        let found = lexicon.iter().find_map(|lemma| {
            if lemma.inflections.len() < 2 {
                return None;
            }
            let pos = PartOfSpeech::inflecting_reading(lemma.part_of_speech);
            let forms_of = |inflection| {
                inflect_all(lemma.word, &[inflection], &cat, Some(&index), pos, false)
                    .into_iter()
                    .map(|form| form.word)
                    .collect::<HashSet<_>>()
            };
            let first = forms_of(lemma.inflections[0]);
            let second = forms_of(lemma.inflections[1]);
            let word = second.difference(&first).next()?.clone();
            Some((lemma, pos, word))
        });
        let Some((lemma, pos, word)) = found else {
            panic!("no lemma has two classes with differing forms");
        };
        let [first, second, ..] = lemma.inflections[..] else {
            unreachable!("the lemma was filtered on two inflections");
        };
        assert!(
            !emits_form(lemma.word, &[first], &cat, Some(&index), pos, false, &word),
            "{word} is not a form of {} under {first}",
            lemma.word
        );
        assert!(
            emits_form(lemma.word, &[second], &cat, Some(&index), pos, false, &word),
            "{word} is a form of {} under {second}",
            lemma.word
        );
        assert!(
            emits_form(
                lemma.word,
                &lemma.inflections,
                &cat,
                Some(&index),
                pos,
                false,
                &word
            ),
            "{word} is a form of {} under its full reading",
            lemma.word
        );
    }

    /// A classless compound is classified by its head, not by its ending.
    ///
    /// Kotus omits the type of a compound because it declines like its final
    /// component, so the head's class is the compound's class. The head must be
    /// a listed **free** nominal; a bound head is a suffix, a non-nominal
    /// suffix is not a head, and a word whose modifier cannot be resolved is
    /// left unclassified rather than guessed at.
    #[test]
    fn a_classless_compound_takes_its_heads_class() {
        let cat = cat();
        let lexicon = crate::kotus::sanalista().to_lemmas();
        let index = LexiconIndex::new(&lexicon);
        for (word, class) in [
            ("aamuhartaus", 40),         // hartaus
            ("kirjakauppa", 9),          // kauppa (9*B)
            ("pikkuaivot", 1),           // aivot
            ("kaskinauris", 41),         // nauris
            ("illallistanssiaiset", 38), // tanssiaiset
        ] {
            let inflections = head_inflections(word, &cat, Some(&index))
                .unwrap_or_else(|| panic!("{word} was not classified"));
            assert_eq!(inflections[0].class, class, "{word}");
        }
        // A bound head is a suffix, not a word: `leikkimielisyys` is `leikki` +
        // the listed `‑mielisyys`, and taking that class would be taking a class
        // from an ending. It must not fall through to the shorter head `syys`
        // (class 99) either.
        assert!(head_inflections("leikkimielisyys", &cat, Some(&index)).is_none());
        // `kuin` is a conjunction, not a nominal head, so `ennenkuin` keeps no
        // class rather than taking a suffix for one.
        assert!(head_inflections("ennenkuin", &cat, Some(&index)).is_none());
        // An unresolved modifier is not guessed at either.
        assert!(head_inflections("kylmäverihevonen", &cat, Some(&index)).is_none());
    }

    /// `oieta` and `aueta` are both listed under classes 72 and 74, but only
    /// one reading is real. `oieta` is the class-72 `vanheta` type (`oikenee`),
    /// `aueta` the class-74 `katketa` type (`aukeaa`); the other paradigm's
    /// forms are not words.
    /// `lymfa` is `9, 10`; only the class-9 o-plural is confirmed. The
    /// class-10 `lymfitta` is the engine's own output and must not be a plain
    /// form.
    #[test]
    fn a_dual_9_10_noun_uses_only_its_real_reading() {
        let cat = cat();
        let mut inflections: Vec<Inflection> = Vec::new();
        for row in crate::kotus::sanalista().by_word("lymfa") {
            inflections.extend(row.inflections());
        }
        let got = inflect_all(
            "lymfa",
            &inflections,
            &cat,
            None,
            PartOfSpeech::Noun,
            true,
        );
        let words: Vec<&str> = got.iter().map(|f| f.word.as_str()).collect();
        assert!(words.contains(&"lymfoitta"), "class-9 abessive missing");
        assert!(!words.contains(&"lymfitta"), "class-10 abessive still emitted");
    }

    #[test]
    fn a_dual_72_74_lemma_uses_only_its_real_reading() {
        let cat = cat();
        for (lemma, wanted, unwanted) in [
            ("oieta", "oikenee", "oikeaa"),
            ("aueta", "aukeaa", "aukenee"),
            ("tyvetä", "tyvennee", "tyvenee"),
        ] {
            let mut inflections: Vec<Inflection> = Vec::new();
            for row in crate::kotus::sanalista().by_word(lemma) {
                inflections.extend(row.inflections());
            }
            assert!(!inflections.is_empty(), "{lemma} is not in the sanalista");
            let got = inflect_all(
                lemma,
                &inflections,
                &cat,
                None,
                PartOfSpeech::Verb,
                false,
            );
            let words: Vec<&str> = got.iter().map(|f| f.word.as_str()).collect();
            assert!(words.contains(&wanted), "{lemma}: {wanted} missing");
            assert!(
                !words.contains(&unwanted),
                "{lemma}: the rejected reading {unwanted} is still emitted"
            );
        }
    }

    #[test]
    fn a_classless_verb_inflects_by_its_verb_head() {
        // `aliarvioida` is `ali` + `arvioida`. `compound_head_forms` refuses a
        // verb head, so classless verbs have their own path; the negative is
        // two words and the modifier belongs on the main verb.
        let cat = cat();
        let forms = inflect_all("aliarvioida", &[], &cat, None, PartOfSpeech::Verb, false);
        let words: Vec<&str> = forms.iter().map(|f| f.word.as_str()).collect();
        for want in ["aliarvioin", "aliarvioi", "aliarvioidaan", "ei aliarvioi"] {
            assert!(
                words.contains(&want),
                "aliarvioida does not inflect to {want}"
            );
        }
        assert!(
            !words.iter().any(|w| w.starts_with("aliei")),
            "the modifier went on the negative auxiliary"
        );

        // The modifier can be a case form the compile-time subset does not
        // hold: `aikaan` is the illative of `aika`, and `aikaansaada` is
        // `aikaan` + `saada`.
        let forms = inflect_all("aikaansaada", &[], &cat, None, PartOfSpeech::Verb, false);
        let words: Vec<&str> = forms.iter().map(|f| f.word.as_str()).collect();
        for want in ["aikaansaan", "aikaansaa", "ei aikaansaa"] {
            assert!(
                words.contains(&want),
                "aikaansaada does not inflect to {want}"
            );
        }

        // A gradation in the modifier (`puhdas` -> `puhtaaksi`) needs the
        // reverse lookup; a verb stem (`riippu` of `riippua`) needs the verb
        // stem map.
        let lexicon = crate::kotus::sanalista().to_lemmas();
        let index = LexiconIndex::new(&lexicon);
        for (lemma, want) in [
            ("puhtaaksikirjoittaa", "puhtaaksikirjoitan"),
            ("riippuliitää", "riippuliidän"),
        ] {
            let forms = inflect_all(lemma, &[], &cat, Some(&index), PartOfSpeech::Verb, false);
            let words: Vec<&str> = forms.iter().map(|f| f.word.as_str()).collect();
            assert!(words.contains(&want), "{lemma} does not inflect to {want}");
        }
    }

    #[test]
    fn a_classless_modifier_resolves_through_its_own_compound() {
        // `kotimaanmatka` = `kotimaan` + `matka`. The modifier is the genitive
        // of `kotimaa`, which is classless, so the compile-time form set does
        // not hold `kotimaan`; the modifier is resolved by inflecting the
        // classless base (`koti` + `maa`) and matching the surface.
        let cat = cat();
        let forms = inflect_all("kotimaanmatka", &[], &cat, None, PartOfSpeech::Noun, true);
        let words: Vec<&str> = forms.iter().map(|f| f.word.as_str()).collect();
        for want in ["kotimaanmatkan", "kotimaanmatkassa", "kotimaanmatkoja"] {
            assert!(
                words.contains(&want),
                "kotimaanmatka does not inflect to {want}"
            );
        }
    }

    /// A tens numeral declines both members.
    ///
    /// `kaksikymmentä` is a classless sanalista row, so nothing made its
    /// paradigm and `inflect` returned none. Both members decline
    /// (`kahdenkymmenen`, `kahtakymmentä`, `kahdellakymmenellä`); the
    /// nominative keeps the first member as the citation and puts `kymmen` in
    /// the partitive singular. `seitsemän`/`kahdeksan`/`yhdeksän` are class 10
    /// and have no nominative reading in the class output, so the citation is
    /// taken from the sanalista row.
    #[test]
    fn a_tens_numeral_declines_both_members() {
        let cat = cat();
        for (lemma, wants) in [
            (
                "kaksikymmentä",
                &[
                    "kaksikymmentä",
                    "kahdenkymmenen",
                    "kahtakymmentä",
                    "kahteenkymmeneen",
                    "kahdellakymmenellä",
                ][..],
            ),
            (
                "seitsemänkymmentä",
                &[
                    "seitsemänkymmentä",
                    "seitsemänkymmenen",
                    "seitsemääkymmentä",
                ][..],
            ),
            (
                "kahdeksankymmentä",
                &[
                    "kahdeksankymmentä",
                    "kahdeksankymmenen",
                    "kahdeksaakymmentä",
                ][..],
            ),
        ] {
            let forms = inflect_all(lemma, &[], &cat, None, PartOfSpeech::Numeral, false);
            let words: Vec<&str> = forms.iter().map(|f| f.word.as_str()).collect();
            for want in wants {
                assert!(
                    words.contains(want),
                    "{lemma} does not emit {want}"
                );
            }
        }
    }

    /// A numeral's combining stem is a compound modifier.
    ///
    /// `kaksikymmenvuotias` is `kaksikymmen` (the combining stem of
    /// `kaksikymmentä`) + the bound head `‑vuotias` (class 41). The
    /// modifier check did not know the numeral shape, so the head was never
    /// reached and the row produced no forms at all -- which is why
    /// `kaksikymmenvuotiaalla` was unknown. `kolmi`/`neli`/`seitsen` are
    /// the same shape through the bound rows `kolmi‑`, `neli‑`,
    /// `seitsen‑`.
    #[test]
    fn a_numeral_combining_modifier_reaches_the_head() {
        let cat = cat();
        for (lemma, pos, want) in [
            (
                "kaksikymmenvuotias",
                PartOfSpeech::Adjective,
                "kaksikymmenvuotiaalla",
            ),
            (
                "kolmikymmenvuotias",
                PartOfSpeech::Adjective,
                "kolmikymmenvuotiaana",
            ),
            (
                "nelitoistavuotias",
                PartOfSpeech::Adjective,
                "nelitoistavuotiaaksi",
            ),
            (
                "seitsentoistavuotias",
                PartOfSpeech::Adjective,
                "seitsentoistavuotiaan",
            ),
            // The `-s` stem of the same adjective: `viisikymmenvuotis-`
            // in `viisikymmenvuotispäivä`.
            (
                "viisikymmenvuotispäivä",
                PartOfSpeech::Noun,
                "viisikymmenvuotispäivänä",
            ),
        ] {
            let forms = inflect_all(lemma, &[], &cat, None, pos, false);
            assert!(
                forms.iter().any(|f| f.word == want),
                "{lemma} does not emit {want}"
            );
        }
    }

    #[test]
    fn a_classless_inen_adjective_inflects_as_class_38() {
        // `kovakorvainen` = `kova` + `korvainen`, `yksinkertainen` =
        // `yksin`/`yksi` + `kertainen`: the head is a *derived* `-inen`
        // adjective, not a sanalista lemma, so `compound_head_forms`
        // cannot inflect it. The whole word still inflects as the `-inen`
        // class (`kovakorvaisia`, `yksinkertaisia` are partitive plurals).
        let cat = cat();
        for (lemma, want) in [
            ("kovakorvainen", "kovakorvaisia"),
            ("yksinkertainen", "yksinkertaisia"),
            ("yksinkertainen", "yksinkertaiset"),
        ] {
            let forms = inflect_all(lemma, &[], &cat, None, PartOfSpeech::Adjective, true);
            let words: Vec<&str> = forms.iter().map(|f| f.word.as_str()).collect();
            assert!(words.contains(&want), "{lemma} does not inflect to {want}");
        }
    }

    #[test]
    fn a_compound_inen_adjective_takes_the_heads_harmony() {
        // `vastenmielinen` = `vasten` + `mielinen`; `mielinen` is
        // front-harmony, so the partitive plural is `vastenmielisiä`.
        // Inflecting the whole word would take back harmony from the `a`
        // in `vasten` and emit `vastenmielisia`.
        let cat = cat();
        let forms = inflect_all(
            "vastenmielinen",
            &[],
            &cat,
            None,
            PartOfSpeech::Adjective,
            true,
        );
        let words: Vec<&str> = forms.iter().map(|f| f.word.as_str()).collect();
        assert!(
            words.contains(&"vastenmielisiä"),
            "missing front-harmony vastenmielisiä; forms: {words:?}"
        );
        assert!(
            !words.contains(&"vastenmielisia"),
            "emitted the wrong back-harmony vastenmielisia"
        );
    }

    #[test]
    fn derived_inen_base_reconstructs_tahkoinen() {
        let cat = cat();
        assert_eq!(
            derived_inen_base("tahkoisella", &cat, None).as_deref(),
            Some("tahkoinen")
        );
        assert_eq!(
            derived_inen_base("tahkoinen", &cat, None).as_deref(),
            Some("tahkoinen")
        );
        // A compound head (`henkiruumiinen` = `henki` + `ruumiinen`).
        assert_eq!(
            derived_inen_base("henkiruumiisen", &cat, None).as_deref(),
            Some("henkiruumiinen")
        );
    }

    /// Class 51 has no whole-word fallback: its contract is that the *initial*
    /// part inflects, so the catalogue model cannot stand in for a split.
    /// Before the guard this returned a full paradigm of invented forms
    /// (`mudtaraaakuita`, `nuori-idenkuun`).
    #[test]
    fn a_class_51_compound_without_a_lexicon_fabricates_nothing() {
        let cat = cat();
        let got = inflect_in(
            "mustaraamattu",
            Inflection {
                class: 51,
                gradation: None,
            },
            &cat,
            None,
            PartOfSpeech::Noun,
            false,
        );
        assert!(
            got.is_empty(),
            "class 51 without a lexicon must produce no forms, got {:?}",
            got.iter().map(|g| &g.word).collect::<Vec<_>>()
        );
    }

    /// The public wrapper supplies the embedded lexicon, so the same word gets
    /// the real split instead of the catalogue model's invented forms.
    #[test]
    fn class_51_inflect_uses_the_embedded_lexicon() {
        let ws = forms("nuori-isäntä", 51, None);
        for want in [
            "nuori-isäntä",
            "nuoren-isännän",
            "nuorta-isäntää",
            "nuoret-isännät",
        ] {
            assert!(ws.iter().any(|w| w == want), "missing {want}");
        }
        assert!(
            !ws.iter().any(|w| w == "nuori-idenkuun"),
            "invented nuori-idenkuun"
        );
        let ws = forms("mustaraamattu", 51, None);
        assert!(
            !ws.iter().any(|w| w.starts_with("mudta")),
            "invented a gradation inside the first part: {ws:?}"
        );
    }

    /// Every class is reachable from the public API; an empty lemma must be a
    /// no-op rather than a panic (class 77's stem subtraction and the verb
    /// builder's `pres3sg_of` both used to assume a non-empty stem).
    #[test]
    fn an_empty_lemma_does_not_panic_in_any_class() {
        let cat = cat();
        for class in 1..=101u16 {
            for possessives in [false, true] {
                let got = inflect(
                    "",
                    Inflection {
                        class,
                        gradation: None,
                    },
                    &cat,
                    PartOfSpeech::Noun,
                    possessives,
                );
                assert!(
                    got.iter().all(|g| !g.word.is_empty()),
                    "class {class} emitted an empty form"
                );
            }
        }
    }
}
