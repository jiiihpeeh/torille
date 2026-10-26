//! The shared front/back harmony for the loanword `-i` family (classes 5/6).
//!
//! Back/front for these loanwords is decided by *head*, not by the vowels
//! visible in the whole word. `loanword_harmony` composes three stages -- an
//! explicit lexical override, then the rightmost compound head, then ordinary
//! vowel harmony. `ends_with` below is a practically good *proxy* for the head
//! (a full compound separator/lexicon is out of scope); it is exact for the
//! productive heads and the exceptions fence off the lexical outliers.

use super::utility::front;

/// Productive loanword compound heads that set front harmony. A word whose
/// rightmost head is one of these is front-harmonic *by its head*, whatever
/// back vowel a modifier carries (`hehkuviini`, `intiaaniriisi`, `omeenakeksi`,
/// `paperiliivi`, `ikkunakitti`, `sumusireeni`). A new `*-viini`/`*-riisi`/
/// `*-keksi`/… word -- listed or derived -- is handled with no code change. Add
/// a head here, not the words built from it.
///
/// `-geeni`, `-metri`, `-ismi` are absent: they mix in the reference
/// (`antigeeni` vs `allergeeni`, `dynamometri` vs `kronometri`), so they are not
/// productive heads and fall back to ordinary vowel harmony, which keeps their
/// back members right; the few genuinely front ones are in `FRONT_EXCEPTIONS`.
const FRONT_HEADS: &[&str] = &[
    "viini", "riisi", "riski", "keksi", "liivi", "kitti", "ihti", "visiiri",
    "sireeni", "sini",
];

/// Genuinely irregular loanwords: back though a productive front head would
/// read them front (`riboflaviini` ends in the wine head but is a chemical
/// back-harmonic term; `oliivi` ends in `-liivi` but is back), or front though
/// ordinary harmony would read them back (the exceptional `-geeni`/`-metri`/
/// `-ismi`/`-iitti` front members).
const BACK_EXCEPTIONS: &[&str] = &[
    "riboflaviini", "beaujolaisviini", "avorioriisi", "basmatiriisi",
    "kapriisi", "matriisi", "patriisi", "oliivi", "kosini",
];
const FRONT_EXCEPTIONS: &[&str] = &[
    "antigeeni", "antisemitismi", "antisemiitti", "autotermiitti", "dynamometri",
    "heksametri", "isotermi", "kalorimetri", "marksismi-leninismi",
    "marxismi-leninismi", "mutageeni", "patogeeni", "pikkurilli",
];

/// Class-38 `-inen`-family loanwords that are front-harmonic though their own
/// spelling reads back.
///
/// `front` scans the whole lemma right to left for the last a/o/u/ä/ö/y, with
/// `e` and `i` neutral, and the `-inen`/`-llinen`/`-ttinen` suffixes are nothing
/// but neutral vowels -- so the scan walks straight through the suffix and into
/// the foreign element in front of it and reads that instead. `biogeeninen` has
/// no Finnish back vowel at all except the `o` of `geo`, and that is what
/// ordinary Finnish harmony reads.
///
/// This is the same situation [`FRONT_EXCEPTIONS`] above describes for the `-i`
/// family, and it gets the same answer, for the same reason: the words are
/// lexical. Measured against the reference over all 3 342 class-38 lemmas that
/// have a confirmed inessive singular, the suffix predicts nothing -- `-llinen`
/// is 442 back and 1 front, `-ttinen` 110 and 1, `-meinen` 4 and 5, `-trinen`
/// 6 and 5 -- and all 23 of these are the front ones. There is no sanalista
/// column for harmony to be wrong in, and 22 of the 23 have no sanalista base to
/// inherit it from either (`biogeeni` is not a lemma; `mutageeni`, which *is*,
/// is already in `FRONT_EXCEPTIONS` above). So this is a list, and it is the
/// same kind of list the `-i` family already needed; it is not a rule that could
/// be found by looking harder.
const FRONT_CLASS_38: &[&str] = &[
    "ajanvietteellinen", "antisemiittinen", "antiseptinen", "biogeeninen",
    "biometrinen", "egosentrinen", "erogeeninen", "heterogeeninen",
    "homogeeninen", "isometrinen", "laajaspektrinen", "moni-ilmeinen",
    "monoteistinen", "mutageeninen", "muuntogeeninen", "omailmeinen",
    "polikliininen", "psykogeeninen", "psykometrinen", "rikasilmeinen",
    "siirtogeeninen", "tiukkailmeinen", "vakavailmeinen",
];

/// Stage 1: an explicit whole-word exception. The genuinely irregular loans.
fn lexical_override(word: &str) -> Option<bool> {
    if BACK_EXCEPTIONS.contains(&word) {
        return Some(false);
    }
    if FRONT_EXCEPTIONS.contains(&word) {
        return Some(true);
    }
    None
}

/// Stage 2: the rightmost productive head. `ends_with` is the head proxy; the
/// longest matching head wins so a future `foo-riisi` rule can't be shadowed by
/// a shorter one.
fn compound_head_harmony(word: &str) -> Option<bool> {
    FRONT_HEADS
        .iter()
        .filter(|head| word.ends_with(**head))
        .max_by_key(|head| head.len())
        .map(|_| true)
}

/// Stage 3: ordinary Finnish vowel harmony (the last non-neutral vowel).
fn ordinary_harmony(word: &str) -> bool {
    front(word)
}

/// The harmony a loanword `-i` word takes: lexical override -> head -> ordinary.
pub fn loanword_harmony(lemma: &str) -> bool {
    lexical_override(lemma)
        .or_else(|| compound_head_harmony(lemma))
        .unwrap_or_else(|| ordinary_harmony(lemma))
}

/// The harmony a class-38 `-inen`-family word takes.
///
/// Ordinary Finnish harmony with the measured loanword exceptions layered over
/// it, and nothing else: unlike [`loanword_harmony`] there is no head stage,
/// because no productive head governs these words -- `ajanvietteellinen` and
/// `laajaspektrinen` have Finnish-looking first parts (`aja`, `laaja`) that are
/// both back, and the reference still makes the whole word front.
pub fn class_38_harmony(lemma: &str) -> bool {
    FRONT_CLASS_38.contains(&lemma) || ordinary_harmony(lemma)
}

#[cfg(test)]
mod tests {
    use super::{FRONT_CLASS_38, class_38_harmony, front};

    /// Each listed word is front, and that is the whole claim the list makes.
    #[test]
    fn every_listed_class_38_word_is_front() {
        for lemma in FRONT_CLASS_38 {
            assert!(class_38_harmony(lemma), "{lemma} is listed as front");
        }
    }

    /// A list that only ever adds front harmony would pass the test above while
    /// quietly turning the whole class front, so this holds the other side: the
    /// back-harmony `-inen` words stay back, and a front one stays front with no
    /// list at all.
    #[test]
    fn an_unlisted_class_38_word_keeps_its_own_harmony() {
        for (lemma, is_front) in [
            // back: the last decisive vowel is an a/o/u
            ("nainen", false), // naise-n: the a of nai-
            ("tapahtuman", false),
            ("punainen", false), // punai-nen: the u of punai-
            // front: a decisive ä/ö/y ...
            ("kylminen", true), // ... the y of kylm-
            ("kätilö", true),
            // ... and front by the no-decisive-vowel default
            ("pienin", true),
        ] {
            assert_eq!(class_38_harmony(lemma), is_front, "{lemma}");
            assert_eq!(class_38_harmony(lemma), front(lemma), "{lemma} unlisted");
        }
    }

    /// The list is a list, so it should say how big it is: a word added to it
    /// must have been measured, and this is where the count is stated.
    #[test]
    fn the_class_38_exception_list_is_the_measured_one() {
        assert_eq!(FRONT_CLASS_38.len(), 23);
    }
}
