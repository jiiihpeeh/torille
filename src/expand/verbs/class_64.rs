//! Class 64 (the `juoda` type) inflection, written by hand.
//!
//! Class 64 is the `-oda`/`-ödä` type, and it is class 63's `saada` with a
//! connecting vowel standing between the root and the ending's final vowel:
//! `juoda` is `jo` + `u` + `o` + `da` beside `saada`'s `saa` + `da`, and `lyödä`
//! is `lö` + `y` + `ö` + `dä` beside `jäädä`'s `jää` + `dä`. The present stem is
//! the lemma less its `da`/`dä` (`juo`, `luo`, `lyö`, `suo`, `syö`, `tuo`,
//! `vie`) and its tail is always two vowels; the past stem is the root plus that
//! tail's second vowel alone, which is the whole difference between the two
//! tenses: `joi` beside `juon`, `löi` beside `lyön`, `vei` beside `vien`.
//!
//! The connector is the penultimate letter, and it varies with the harmony: a
//! `u` where the stem is back (`juo`, `luo`, `suo`, `tuo`) and a `y` or an `i`
//! where it is front (`lyö`, `syö`, and `viedä`, whose tail is `ie` rather than
//! `yö`). So the rule drops a penultimate vowel, and a stem with none would stand
//! as it is; every member has one, and that is what makes the type a type of its
//! own.
//!
//! Everything else about the type is class 63's cell for cell, and the reference
//! says so:
//!
//! * **The passive deranks its `t` to `d`,** taking the `d` in the present and
//!   writing a single `t` everywhere else: `juodaan` and `juotiin`, `juotava`
//!   and `juotu` with its oblique `juodun`, all beside class 63's `saadaan` and
//!   `saatiin`. That is [`PassiveCons::Deranked`], and it brings the past passive
//!   participle's oblique deranking with it as it does for classes 62 and 63.
//!
//! * **The passive theme needs no retheming,** and that is worth spelling out
//!   because class 63 has to say it explicitly. Class 53 reads a stem-final
//!   `a`/`ä` as the passive's `e` (`muistetaan` beside `sanoa`'s `sanotaan`),
//!   which is what class 63's doubled `a`/`ä` stem has to switch off to avoid
//!   `saetaan`. This type's stems end in `o`, `ö` or `e` instead, so `retheme`
//!   never fires and the weak grade is the passive theme as it stands -- the same
//!   result class 61 and class 62 reach by the same accident.
//!
//! * **The third person present is the bare stem,** `juo`, `lyö`, `vie`, so
//!   `pres3sg` is empty as it is for classes 62 and 63: the stem ends in a vowel
//!   and there is no theme left to write again.
//!
//! * **The second infinitive runs on the `d`:** `juodessa` and `juoden` are
//!   `juod` + `essa` and `juod` + `en`, where class 53 has `muisteessa` and
//!   `muisteen`. So `inf2_stem` is the stem plus `de`, as in classes 62 and 63.
//!
//! * **The conditional follows the past stem,** so `joisi` is `jo` + `isi` where
//!   class 53's `muistaisi` is `muist` + `isi` off the present stem, and
//!   `lyödä`'s `löisi` and `viedä`'s `veisi` are the same shape. Hence
//!   `cond_stem` is the past stem, as in class 63. The potential and the
//!   imperative keep the present stem (`juonee`, `juokoon`), so those two do not
//!   part company, as they do in classes 58 and 59.
//!
//! Seven members and no gradation code -- `juoda`, `luoda`, `lyödä`, `suoda`,
//! `syödä`, `tuoda` and `viedä` are all plain `64` -- so the two grades are the
//! same word throughout and there is no `missing_code` table.

#![allow(dead_code)]

use crate::tags::Form;

use super::utility::{PassiveCons, Stems, paradigm};

pub const CLASS: &str = "64";

/// The past stem of a class-64 verb: the present stem less the vowel that
/// separates the root from the ending's final `o`/`ö`/`e`.
///
/// The present stem's tail is always two vowels -- `juo` is `jo` + `uo`,
/// `lyö` is `lö` + `yö`, `vie` is `ve` + `ie` -- and the past keeps only the
/// second of them, so `jo`, `lö` and `ve`. The connector is a `u` where the stem
/// is back and a `y` or an `i` where it is front.
fn past_stem_of(stem: &str) -> String {
    let mut chars: Vec<char> = stem.chars().collect();
    if chars.len() >= 2 {
        let connector = chars.len() - 2;
        if "aeiouyäö".contains(chars[connector]) {
            chars.remove(connector);
        }
    }
    chars.into_iter().collect()
}

/// Inflect a class-64 verb into its whole paradigm.
pub fn inflect(lemma: &str) -> Vec<Form> {
    let stem = lemma
        .strip_suffix("da")
        .or_else(|| lemma.strip_suffix("dä"))
        .unwrap_or(lemma);
    let past = past_stem_of(stem);
    let mut stems = Stems::with_past(lemma, stem, &past, None);
    // The passive writes one consonant each way rather than geminating, which is
    // class 62's rule and classes 63's and 64's.
    stems.p_cons = PassiveCons::Deranked;
    // The theme is already a vowel, so there is nothing to repeat in the third
    // person.
    stems.pres3sg = String::new();
    // `p_theme` is left as `with_past` set it: a stem ending in `o`, `ö` or `e`
    // is not one `retheme` fires on, so the passive theme is the stem itself.
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

    /// The present stem is the lemma less its `da`/`dä` and the past stem drops
    /// the `u` or `y` that stands before the ending's `o`/`ö`.
    #[test]
    fn the_past_stem_drops_the_connecting_vowel() {
        for (lemma, stem, past) in [
            ("juoda", "juo", "jo"),
            ("luoda", "luo", "lo"),
            ("suoda", "suo", "so"),
            ("tuoda", "tuo", "to"),
            ("lyödä", "lyö", "lö"),
            ("syödä", "syö", "sö"),
        ] {
            assert_eq!(past_stem_of(stem), past);
            assert!(has(lemma, stem), "{lemma} should have {stem}");
            assert!(
                has(lemma, &format!("{past}i")),
                "{lemma} should have {past}i"
            );
        }
        // `viedä` is the member whose stem tail is `ie` rather than `uo` or
        // `yö`, so its past stem is `ve` and its 3sg past `vei`.
        assert_eq!(past_stem_of("vie"), "ve");
        assert!(has("viedä", "vei"));
        assert!(has("viedä", "vein"));
        assert!(has("viedä", "veit"));
        assert!(has("viedä", "veivät"));
    }

    /// Six present cells on the present stem and six past on the past stem,
    /// which is where this type parts from classes 62 and 63: class 62's two
    /// stems are one string and all twelve cells are the same six words.
    #[test]
    fn the_present_and_the_past_run_on_different_stems() {
        for want in ["juon", "juot", "juo", "juomme", "juotte", "juovat"] {
            assert!(has("juoda", want), "juoda should have {want}");
        }
        for want in ["join", "joit", "joi", "joimme", "joitte", "joivat"] {
            assert!(has("juoda", want), "juoda should have {want}");
        }
        // Front harmony gives the `ö` twins, and the present's 2sg on the `ö`
        // is where it geminates: `lyöt`.
        for want in ["lyön", "lyöt", "lyö", "lyömme", "lyötte", "lyövät"] {
            assert!(has("lyödä", want), "lyödä should have {want}");
        }
        for want in ["löin", "löit", "löi", "löimme", "löitte", "löivät"] {
            assert!(has("lyödä", want), "lyödä should have {want}");
        }
    }

    /// The conditional follows the past stem; the potential and the imperative
    /// keep the present one, so those two do not part company as they do in
    /// classes 58 and 59.
    #[test]
    fn the_conditional_follows_the_past_stem_and_the_rest_the_present() {
        for want in [
            "joisin", "joisit", "joisi", "joisimme", "joisitte", "joisivat",
        ] {
            assert!(has("juoda", want), "juoda should have {want}");
        }
        for want in ["juonen", "juonet", "juonee", "juonemme", "juonevat"] {
            assert!(has("juoda", want), "juoda should have {want}");
        }
        for want in ["juo", "juokaa", "juokaamme", "juokoon", "juokoot"] {
            assert!(has("juoda", want), "juoda should have {want}");
        }
        // `juoisi` would be the present stem's own conditional, and it is not a
        // word.
        assert!(hasnt("juoda", "juoisi"));
        assert!(hasnt("juoda", "juoisin"));
        assert!(has("lyödä", "löisi"));
        assert!(has("viedä", "veisi"));
    }

    /// The passive takes the `d` in the present and a single `t` everywhere
    /// else, and its theme is the stem as it stands.
    #[test]
    fn the_passive_deranks_its_t() {
        for want in [
            "juodaan",
            "juotiin",
            "juotaisiin",
            "juotaneen",
            "juotakoon",
            "juotava",
            "juotu",
            "juotaessa",
            "juotaen",
            "juotaman",
        ] {
            assert!(has("juoda", want), "juoda should have {want}");
        }
        for absent in ["juottiiin", "juottava", "juottu", "juottaessa"] {
            assert!(hasnt("juoda", absent), "juoda should not have {absent}");
        }
        // `viedä` shows the same cells on a stem that needs no retheming.
        for want in ["viedään", "vietiin", "vietävä", "viety"] {
            assert!(has("viedä", want), "viedä should have {want}");
        }
    }

    /// `juotu` is the nominative and `juodu` the oblique stem, so the participle
    /// deranks its `t` to a `d` in the oblique exactly as classes 62 and 63 do.
    #[test]
    fn the_past_passive_participle_deranks_in_the_oblique() {
        for want in [
            "juotu",
            "juodun",
            "juodussa",
            "juodutta",
            "juoduilta",
            "juoduin",
            "juodut",
            "juotua",
            "juoduissa",
        ] {
            assert!(has("juoda", want), "juoda should have {want}");
        }
        // The geminated spellings of class 61 are not words here.
        for absent in ["juottu", "juottun", "juottua"] {
            assert!(hasnt("juoda", absent), "juoda should not have {absent}");
        }
    }

    /// The second infinitive runs on the `d`, where class 53 runs on the `e`.
    #[test]
    fn the_second_infinitive_takes_the_d() {
        for want in ["juodessa", "juodessaan", "juodessani", "juoden"] {
            assert!(has("juoda", want), "juoda should have {want}");
        }
        for absent in ["juoessa", "juoen"] {
            assert!(hasnt("juoda", absent), "juoda should not have {absent}");
        }
        for want in ["lyödessä", "lyöden", "viedessä", "vieden"] {
            let lemma = if want.starts_with('l') {
                "lyödä"
            } else {
                "viedä"
            };
            assert!(has(lemma, want), "{lemma} should have {want}");
        }
    }

    /// The remaining infinitives and the active participles all run on the
    /// present stem, as in every other type. The fifth infinitive takes no bare
    /// form, only its possessives, as everywhere.
    #[test]
    fn the_infinitives_and_the_active_participles_run_on_the_present_stem() {
        for want in [
            "juoda",
            "juoma",
            "juominen",
            "juomaisillani",
            "juovan",
            "juova",
            "juonut",
            "juomaton",
            "lyöminen",
            "lyövän",
            "lyönyt",
            "lyömätön",
            "vieminen",
            "vievän",
            "vienyt",
            "viemätön",
        ] {
            let lemma = if want.starts_with('l') {
                "lyödä"
            } else if want.starts_with('v') {
                "viedä"
            } else {
                "juoda"
            };
            assert!(has(lemma, want), "{lemma} should have {want}");
        }
        assert!(hasnt("juoda", "juomaisilla"));
    }
}
