//! Class 62 (the `voida` type) inflection, written by hand.
//!
//! Class 62 is the `-ida` type, and it is class 61's `sallia` with the `d` still
//! in the infinitive's ending. Both keep the theme vowel, so both have the stem
//! *and* the past stem the same string -- `voi` + `i` is `voi` by the same
//! `absorb_i` that gives class 61's `salli` + `i` is `salli` -- and both are the
//! six-cell type-1 paradigm otherwise. The stem is the lemma less its ending:
//! class 61's is less the final `a`/`ä` (`salli`), this one's is less the whole
//! `da`/`dä` (`voi` for `voida`, `aateloi` for `aateloida`, `dieselöi` for
//! `dieselöidä`). Every member of the class ends in `ida`/`idä`, and the `i`/`y`
//! of that is the stem's, so there is no member to tell apart.
//!
//! Four cells are where the type is not class 61. Three are about the `t` the
//! passive adds to the theme.
//!
//! **The passive deranks its `t` to `d`.** Every other type's theme ends in a
//! vowel, so the cluster is written out in full and geminated -- `sanot` beside
//! `sanott`, `sallit` beside `sallitt` -- which is what
//! [`PassiveCons`](super::utility::PassiveCons)'s `Geminated` variant is. Class
//! 62 writes one consonant each way instead, and the present is the one that
//! takes the `d`: `voidaan` where class 61 has `sallitaan`. Everywhere else the
//! passive has a single `t` where class 61 has `tt`, so `voitiin` beside
//! `sallittaisiin` and `voitava` beside `sallittava`. That is the `Deranked`
//! variant, and it is why the field is on `Stems` at all.
//!
//! **The past passive participle deranks in the oblique as well.** `voitu` is
//! the nominative and `voidu` the oblique stem, so `voidun` and `voidussa` sit
//! beside the `t` of the nominative -- where class 61's `sallittu` halves its
//! own doubled `t` to get `sallitun`. Both are the participle's `t` deranked
//! before its final vowel, and `tu_participle` takes the flag that says which
//! way.
//!
//! **The second infinitive runs on the `d` too.** `voidessa` and `voiden` are
//! the inessive and the instructive, which are `void` + `essa` and `void` +
//! `en`, where class 61 has the `e` of `salliessa` and `sallien`. So
//! `inf2_stem` is the stem plus `de` rather than plus `e`. That is the field's
//! own job and class 62 needs nothing new for it.
//!
//! The fourth is not about the `t`. Class 61's third person present repeats the
//! stem's last letter (`sallii`, `salli` + `i`) because its infinitive is `-ia`:
//! the `i` of `-ia` is the type's `i` and it is written again. Here the
//! infinitive's `i` and the stem's `i` are the same `i` -- `voida` is `voi` +
//! `da` -- so the third person is the bare stem, `voi` and `aateloi`, and there
//! is nothing to repeat. Hence [`Stems::pres3sg`](super::utility::Stems) being
//! empty where every other
//! type's is the stem's last letter. With that, *all six* of this type's
//! present and past cells are the same words: `voin`, `voit`, `voi`, `voimme`,
//! `voitte` and `voivat` are each both a present and a past form, and the
//! reference gives no other present or past cell for the type either. Class 61
//! had one cell that parted company (`sallii`/`salli`); class 62 has none.
//!
//! The class has no gradation code at all -- every member is plain `62` -- so
//! the two grades are the same word and the usual cells take either.
//!
//! [`Stems::ida`](super::utility::Stems) is where the four of these live, because
//! class 68's `tupakoida` is the same type cell for cell and takes them too. What
//! class 62 has over class 68 is the length of its lemma and the 670 `-oid-`/
//! `-oidä` words among its 735 members -- `voida` and `tupakoida` inflect by one
//! rule.

#![allow(dead_code)]

use crate::tags::Form;

use super::utility::{Stems, paradigm};

pub const CLASS: &str = "62";

/// Inflect a class-62 verb into its whole paradigm.
pub fn inflect(lemma: &str) -> Vec<Form> {
    paradigm(&Stems::ida(lemma))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has(lemma: &str, form: &str) -> bool {
        inflect(lemma).iter().any(|f| f.word == form)
    }

    fn hasnt(lemma: &str, form: &str) -> bool {
        !has(lemma, form)
    }

    /// Every member ends in `ida`/`idä`, so the `i`/`y` of the infinitive's
    /// ending is the stem's own last letter and the stem is the lemma less the
    /// whole ending. The third person present is the bare stem, so it is what
    /// pins the stem down.
    #[test]
    fn the_stem_is_the_lemma_less_the_whole_ending() {
        for (lemma, stem, inessive) in [
            ("voida", "voi", "voidessa"),
            ("soida", "soi", "soidessa"),
            ("naida", "nai", "naidessa"),
            ("aateloida", "aateloi", "aateloidessa"),
            ("dieselöidä", "dieselöi", "dieselöidessä"),
        ] {
            assert!(
                stem.ends_with('i') || stem.ends_with('y'),
                "{stem} is not i-final"
            );
            assert!(has(lemma, stem), "{lemma} should have the bare stem {stem}");
            // And the second infinitive runs on the `d` of that ending, which is
            // the same fact seen from the other side.
            assert!(has(lemma, inessive), "{lemma} should have {inessive}");
        }
    }

    /// Class 61's third person repeats the stem's last letter (`sallii`) because
    /// its `-ia` infinitive's `i` is a second `i`. Here the infinitive's `i` and
    /// the stem's `i` are one `i`, so there is nothing to repeat and the present
    /// and past agree in all six cells.
    #[test]
    fn the_third_person_is_the_bare_stem_and_the_tenses_agree_thoroughly() {
        for want in [
            "voi", "voin", "voit", "voimme", "voitte", "voivat", "voin", "voit", "voi", "voimme",
            "voitte", "voivat",
        ] {
            assert!(has("voida", want), "voida should have {want}");
        }
        // `nai` shows it just as well on a stem two letters long.
        assert!(has("naida", "nai"));
        // The doubled `i` class 61 writes is not a word here.
        for absent in ["voii", "naii"] {
            assert!(hasnt("voida", absent), "voida should not have {absent}");
            assert!(hasnt("naida", absent), "naida should not have {absent}");
        }
    }

    /// The other four moods, which run on the stem alone: `voisi`, `voinee`,
    /// `voinen`, `voikaa` and the `-nut`/`-neet` participles.
    #[test]
    fn the_moods_and_the_active_participles_run_on_the_stem() {
        for want in [
            "voisi",
            "voisin",
            "voinee",
            "voinen",
            "voinet",
            "voinemme",
            "voinette",
            "voinevat",
            "voikaa",
            "voikaamme",
            "voikoon",
            "voikoot",
            "voinut",
            "voineet",
            "voiva",
            "voima",
        ] {
            assert!(has("voida", want), "voida should have {want}");
        }
    }

    /// The passive writes one consonant each way instead of geminating: the
    /// `d` of the `-da` ending in the present and a single `t` everywhere else,
    /// where class 61 writes `sallitt` throughout.
    #[test]
    fn the_passive_deranks_its_t_to_a_d() {
        for want in [
            "voidaan",
            "voitiin",
            "voitaisiin",
            "voitaneen",
            "voitakoon",
            "voitava",
            "voitu",
            "voitaessa",
        ] {
            assert!(has("voida", want), "voida should have {want}");
        }
        // The geminated spellings of class 61 are not words here.
        for absent in [
            "voittaan",
            "voittiin",
            "voittaisiin",
            "voittava",
            "voittu",
            "voittaessa",
        ] {
            assert!(hasnt("voida", absent), "voida should not have {absent}");
        }
        // Front harmony gives the `ä` twins: `dieselöidään`, `dieselöityyn`,
        // `dieselöidyssä` and the `dieselöity` participle.
        for want in [
            "dieselöidään",
            "dieselöityyn",
            "dieselöidyssä",
            "dieselöity",
        ] {
            assert!(has("dieselöidä", want), "dieselöidä should have {want}");
        }
        for absent in ["dieselöidattaan", "dieselöittu"] {
            assert!(
                hasnt("dieselöidä", absent),
                "dieselöidä should not have {absent}"
            );
        }
    }

    /// `voitu` is the nominative and `voidu` the oblique stem, where class 61
    /// halves its own doubled `t` instead (`sallittu` beside `sallitun`). Both are
    /// the participle's `t` deranked in front of its final vowel.
    #[test]
    fn the_past_passive_participle_deranks_in_the_oblique() {
        for want in [
            "voitu",
            "voidun",
            "voidussa",
            "voidutta",
            "voidut",
            "voiduin",
            "voiduista",
            "voidussani",
            "voituja",
            "voituun",
            "voituna",
        ] {
            assert!(has("voida", want), "voida should have {want}");
        }
        // `voitun` is class 61's oblique, not this type's.
        for absent in ["voitun", "voittu", "voittun", "voittua", "voittuun"] {
            assert!(hasnt("voida", absent), "voida should not have {absent}");
        }
        // A front-harmony member keeps the participle's `y`: `dieselöidyn` and
        // `dieselöidyin` sit on the same `d`.
        for want in ["dieselöity", "dieselöidyn", "dieselöidyin"] {
            assert!(has("dieselöidä", want), "dieselöidä should have {want}");
        }
    }

    /// The second infinitive runs on the `d`-stem, where class 61 runs on the
    /// `e`-stem: `voidessa` and `voiden` beside `salliessa` and `sallien`.
    #[test]
    fn the_second_infinitive_takes_the_d() {
        for want in ["voidessa", "voidessaan", "voidessani", "voiden"] {
            assert!(has("voida", want), "voida should have {want}");
        }
        for absent in ["voieessa", "voiaessa", "voiessa", "voiiden", "voiien"] {
            assert!(hasnt("voida", absent), "voida should not have {absent}");
        }
    }

    /// The negative participle takes no part in any of this: it is the stem plus
    /// `-maton` like every other type (`voimaton`, `aateloimaton`).
    #[test]
    fn the_negative_participle_is_the_stem_plus_maton() {
        for want in ["voimaton", "voimattoman", "aateloimaton", "dieselöimätön"] {
            let (lemma, want) = match want {
                "aateloimaton" => ("aateloida", want),
                "dieselöimätön" => ("dieselöidä", want),
                _ => ("voida", want),
            };
            assert!(has(lemma, want), "{lemma} should have {want}");
        }
    }
}
