//! Class 63 (the `saada` type) inflection, written by hand.
//!
//! Class 63 is the `-ada` type, and it is class 62's `-ida` with the theme vowel
//! still doubled where class 62 has it single: `saada` is `saa` + `da` beside
//! `voida`'s `voi` + `da`, `jäädä` is `jää` + `dä` beside `dieselöidä`'s
//! `dieselöi` + `dä`. That doubled vowel is the whole difference between the
//! present and the past, and it is the only reason the type is a type of its
//! own. So the present stem is the lemma less its `da`/`dä` (`saa`, `jää`,
//! `myy`) and the past stem is that less its final vowel (`sa`, `jä`, `my`),
//! which is the lemma less its whole `ada`/`ädä`/`ydä`.
//!
//! Everything else about the type is class 62's, cell for cell, because the two
//! are the same type with one vowel apart and the reference says so:
//!
//! * **The passive deranks its `t` to `d`,** taking the `d` in the present and
//!   writing a single `t` everywhere else. `saadaan` and `saatiin` stand beside
//!   class 62's `voidaan` and `voitiin` and beside class 61's `sallitaan` and
//!   `sallitt`. That is [`PassiveCons::Deranked`], the same field class 62 sets,
//!   and it carries the past passive participle's oblique with it: `saatu` in
//!   the nominative and `saadu` in the oblique, giving `saadun` and `saadussa`
//!   where class 62 has `voitu`, `voidun` and `voidussa`.
//!
//! * **The passive theme is not rethemed to `e`.** Class 53 rethemes the weak
//!   grade's final `a`/`ä` to `e` -- `muistetaan` beside `sanoa`'s `sanotaan` --
//!   and because this type's stem *ends* in a doubled `a`/`ä` the rule would
//!   reach it and give the non-word `saetaan`. So `p_theme` is the weak stem as
//!   it stands. Class 61 and class 62 both get the same result by accident
//!   rather than by intent: their stems end in `i`, and `retheme` only fires on
//!   an `a`/`ä`.
//!
//! * **The third person present is the bare stem,** `saa`, `jää`, `myy`, so
//!   `pres3sg` is empty as it is for class 62. Here the reason is the plain one
//!   the rest of the type-1 classes have: the stem already ends in a vowel and
//!   there is no theme left to write again.
//!
//! * **The second infinitive runs on the `d`:** `saadessa` and `saaden` are
//!   `saad` + `essa` and `saad` + `en`, where class 53 has `muisteessa` and
//!   `muisteen`. So `inf2_stem` is the stem plus `de`, as in class 62.
//!
//! One cell is its own, and it is where the past stem shows outside the past:
//! the **conditional** runs on the past stem rather than the present one, so
//! `saisi` is `sa` + `isi` where class 53's `muistaisi` is `muist` + `isi` off
//! the present stem, and class 62's `voisi` is off its own (which is both
//! stems). Hence `cond_stem` is the past stem. The potential and the imperative
//! keep the present stem either way -- `saanee` and `saakoon` -- so the two do
//! not part company, as they do in classes 58 and 59.
//!
//! The class has three members and no gradation code: `saada`, `jäädä` and
//! `myydä` are all plain `63`, so the two grades are the same word throughout
//! and there is no `missing_code` table.

#![allow(dead_code)]

use crate::tags::Form;

use super::utility::{PassiveCons, Stems, paradigm};

pub const CLASS: &str = "63";

/// The past stem of a class-63 verb: the present stem less its final vowel.
///
/// This is `Stems::past_stem_of` widened to the `y` that harmony leaves behind
/// in the front vowel, which is all this type needs: a present stem is `aa`,
/// `ää` or `yy` (`myydä` gives `myy`), never `ai` or `i`, so the vowel to drop
/// is always the doubled one and never a theme a class 52 stem would keep.
fn past_stem_of(stem: &str) -> String {
    let mut chars: Vec<char> = stem.chars().collect();
    match chars.last() {
        Some('a') | Some('ä') | Some('y') => {
            chars.pop();
        }
        _ => {}
    }
    chars.into_iter().collect()
}

/// Inflect a class-63 verb into its whole paradigm.
pub fn inflect(lemma: &str) -> Vec<Form> {
    let stem = lemma
        .strip_suffix("da")
        .or_else(|| lemma.strip_suffix("dä"))
        .unwrap_or(lemma);
    let past = past_stem_of(stem);
    let mut stems = Stems::with_past(lemma, stem, &past, None);
    // The passive writes one consonant each way rather than geminating, which
    // is class 62's rule and this type's.
    stems.p_cons = PassiveCons::Deranked;
    // The theme is already a vowel, so there is nothing to repeat in the third
    // person and nothing to retheme for the passive.
    stems.pres3sg = String::new();
    stems.p_theme = stems.weak();
    // The conditional follows the past stem; the second infinitive the `d`.
    stems.cond_stem = past;
    stems.inf2_stem = format!("{stem}de");
    paradigm(&stems)
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

    /// The present stem is the lemma less its `da`/`dä` and the past stem is
    /// that less its final vowel, which is the whole difference between the two
    /// tenses in this type.
    #[test]
    fn the_past_stem_is_the_present_less_its_final_vowel() {
        for (lemma, stem, past) in [
            ("saada", "saa", "sa"),
            ("jäädä", "jää", "jä"),
            ("myydä", "myy", "my"),
        ] {
            assert_eq!(past_stem_of(stem), past);
            // The third person present is the bare present stem and the third
            // person past the bare past stem plus `i`, so each pins its own.
            assert!(has(lemma, stem), "{lemma} should have {stem}");
            assert!(
                has(lemma, &format!("{past}i")),
                "{lemma} should have {past}i"
            );
        }
        // `myy` is the case worth spelling out: harmony leaves a `y` where the
        // other two members have `a`/`ä`, and it goes with them.
        assert!(has("myydä", "myy"));
        assert!(has("myydä", "myi"));
        assert!(has("myydä", "myyn"));
        assert!(has("myydä", "myyt"));
        assert!(has("myydä", "myyvät"));
    }

    /// Six present cells on the present stem and six past on the past stem,
    /// which is the one place the type parts from class 62 -- there the two
    /// stems are one string and all twelve cells are the same six words.
    #[test]
    fn the_present_and_the_past_run_on_different_stems() {
        for want in ["saan", "saat", "saa", "saamme", "saatte", "saavat"] {
            assert!(has("saada", want), "saada should have {want}");
        }
        for want in ["sain", "sait", "sai", "saimme", "saitte", "saivat"] {
            assert!(has("saada", want), "saada should have {want}");
        }
        // Front harmony gives the `ä` twins throughout.
        for want in [
            "jään",
            "jäät",
            "jää",
            "jään",
            "jäit",
            "jäi",
            "jäävät",
            "jäivät",
        ] {
            assert!(has("jäädä", want), "jäädä should have {want}");
        }
    }

    /// The conditional is the one mood that follows the past stem, which is what
    /// makes `saisi` rather than `saaisi`. The potential and the imperative keep
    /// the present stem.
    #[test]
    fn the_conditional_follows_the_past_stem_and_the_rest_the_present() {
        for want in [
            "saisin", "saisit", "saisi", "saisimme", "saisitte", "saisivat",
        ] {
            assert!(has("saada", want), "saada should have {want}");
        }
        for want in ["saanen", "saanet", "saanee", "saanemme", "saanevat"] {
            assert!(has("saada", want), "saada should have {want}");
        }
        for want in ["saa", "saakaa", "saakaamme", "saakoon", "saakoot"] {
            assert!(has("saada", want), "saada should have {want}");
        }
        // `saaisi` would be the present stem's own conditional, and it is not a
        // word.
        assert!(hasnt("saada", "saaisi"));
        assert!(hasnt("saada", "saaisin"));
        assert!(has("jäädä", "jäisi"));
        assert!(has("myydä", "myisi"));
    }

    /// The passive takes the `d` in the present and a single `t` everywhere
    /// else, and its theme is the present stem as it stands rather than a
    /// retheming of it, which for a stem ending in `a`/`ä` is the whole point:
    /// `saetaan` is not a word.
    #[test]
    fn the_passive_deranks_its_t_and_leaves_the_theme_alone() {
        for want in [
            "saadaan",
            "saatiin",
            "saataisiin",
            "saataneen",
            "saatakoon",
            "saatava",
            "saataessa",
            "saataen",
            "saataman",
            "saatusi",
        ] {
            assert!(has("saada", want), "saada should have {want}");
        }
        for absent in [
            "saetaan",
            "saattiiin",
            "saattaisiin",
            "saattava",
            "saattu",
            "saattaessa",
        ] {
            assert!(hasnt("saada", absent), "saada should not have {absent}");
        }
        // Front harmony runs through the same cells.
        for want in ["jäädään", "jäätiin", "jäätäisiin", "jäätävä", "jäätäessä"] {
            assert!(has("jäädä", want), "jäädä should have {want}");
        }
        for want in ["myydään", "myytiin", "myytäisiin", "myytävä", "myytäessä"] {
            assert!(has("myydä", want), "myydä should have {want}");
        }
    }

    /// `saatu` is the nominative and `saadu` the oblique stem, so the participle
    /// deranks its `t` to a `d` in the oblique exactly as class 62's `voitu` does.
    #[test]
    fn the_past_passive_participle_deranks_in_the_oblique() {
        for want in [
            "saatu",
            "saadun",
            "saadussa",
            "saadutta",
            "saadulta",
            "saaduin",
            "saadut",
            "saatua",
            "saaduissa",
            "saadussaan",
        ] {
            assert!(has("saada", want), "saada should have {want}");
        }
        // The geminated spellings of class 61 are not words here.
        for absent in ["saattu", "saattun", "saatuttu"] {
            assert!(hasnt("saada", absent), "saada should not have {absent}");
        }
        for want in ["jääty", "jäädyn", "jäädyssä", "jäädyin", "myyty", "myydyn"] {
            let lemma = if want.starts_with('j') {
                "jäädä"
            } else {
                "myydä"
            };
            assert!(has(lemma, want), "{lemma} should have {want}");
        }
    }

    /// The second infinitive runs on the `d`, where class 53 runs on the `e`.
    #[test]
    fn the_second_infinitive_takes_the_d() {
        for want in ["saadessa", "saadessaan", "saadessani", "saaden"] {
            assert!(has("saada", want), "saada should have {want}");
        }
        for absent in ["saaessa", "saaeessa", "saaen"] {
            assert!(hasnt("saada", absent), "saada should not have {absent}");
        }
        for want in ["jäädessä", "jääden", "myydessä", "myyden"] {
            let lemma = if want.starts_with('j') {
                "jäädä"
            } else {
                "myydä"
            };
            assert!(has(lemma, want), "{lemma} should have {want}");
        }
    }

    /// The remaining infinitives and the active participles all run on the
    /// present stem, as in every other type. The fifth infinitive takes no bare
    /// form, only its possessives, `saamaisillani`, as everywhere.
    #[test]
    fn the_infinitives_and_the_active_participles_run_on_the_present_stem() {
        for want in [
            "saada",
            "saama",
            "saaminen",
            "saamaisillani",
            "saavan",
            "saava",
            "saanut",
            "saamaton",
            "jääminen",
            "jäävän",
            "jäänyt",
            "jäämätön",
            "jäämäisilläni",
            "myyminen",
            "myyvän",
            "myynyt",
            "myymätön",
        ] {
            let lemma = if want.starts_with('j') {
                "jäädä"
            } else if want.starts_with('m') {
                "myydä"
            } else {
                "saada"
            };
            assert!(has(lemma, want), "{lemma} should have {want}");
        }
        assert!(hasnt("saada", "saamaisilla"));
    }
}
