//! Class 72 (the `vanheta` type) inflection, written by hand.
//!
//! **The type is a `-ne-` present on the strong grade, everywhere else on the
//! weak one.** `vanheta` is the class of intransitive change-of-state verbs
//! (`vanheta`, `paeta`, `edetä`, `huonota`), and its one signature is the `n`
//! that the present, the past and the conditional put in front of the theme:
//!
//! ```text
//! vanheta / vanhenen / vanheni / vanhenisi / vanhennee / vanhetkoon / vanhennut / vanhettiin
//! ```
//!
//! The weak base is the lemma less its `ta`/`tä` (`vanhe`, `pae`, `ede`,
//! `huono`, `lämme`) and the strong base is that base's own gradation (`pake`,
//! `ete`, `lämpe`). The present is the strong base with `ne` on it
//! (`vanhenen`, `pakenen`, `etenen`) and the past is the strong base with `n`
//! (`vanheni`, `pakeni`), so the two are one letter apart and the conditional
//! follows the past's (`vanhenisi`, `pakenisi`). That single `n` is the whole
//! of the type, and it is why `vanheta` is not class 53's `a`-theme with a
//! gradation code: the vowel the ending attaches to has been replaced by a
//! consonant, not merely weakened.
//!
//! Everything else runs on the **weak** base, and its `n` is the class's own
//! leading consonant, exactly as class 67's `l` and class 69's `n` are:
//!
//! * the potential and the past active participle put a doubled `n` in front of
//!   the ending (`pae` + `n` + `n` + `ee` is `paennee`, `vanhe` + `n` + `n` +
//!   `ut` is `vanhennut`), which is the [`Alt`] shape classes 66, 67 and 69
//!   share;
//! * the imperative's plurals are the weak base with the type's own `t`
//!   (`paetkaa`, `vanhetkoon`), the same `t` the passive fronts, while its
//!   second person is the present stem (`vanhene`);
//! * the passive adds the ordinary geminated `t` to the weak base
//!   (`vanhetaan` beside `vanhettiin`, `paetaan` beside `paettiin`) -- the
//!   [`PassiveCons::Geminated`] every type-1 class has;
//! * and the second infinitive is the weak base's `te` (`vanhetessa`,
//!   `vanheten`).
//!
//! The third and fourth infinitives, the present participle, the agent and the
//! negative participle take the *present* stem, as in every type.
//!
//! **The strong base is read two ways, and only one of them is a `pair`.** The
//! sanalista's own code carries almost every gradating member -- `A` `kk <-> k`,
//! `B` `pp <-> p`, `C` `tt <-> t`, `E` `p <-> v`, `F` `t <-> d`, `H`
//! `mp <-> mm`, `J` `nt <-> nn`, `L` `k <-> j` -- and [`Gradation::strongen`]
//! does all of them, because in each the last weak consonant of the base stands
//! for the strong one. `D` (`k <-> nothing`) is the exception and one of the
//! class's most common codes: the base shows the *dropped* grade and there is no
//! weak consonant to find, so the `k` is put back by hand in front of the base's
//! final vowel (`pae` -> `pake`, `kye` -> `kyke`, `ulo` -> `ulko`). That is the
//! one rule no other class needed, precisely because every other class's strong
//! grade adds to a consonant that is already written.
//!
//! ## What the reference says
//!
//! **133 of the class's 146 lemmas audit at 0 invented and 0 missing against
//! `both`**, over 103 058 reference rows — every cell the type gives. Each of
//! them leaves exactly one `union` row uncovered, the `...imatontaan` row every
//! verb class in this project has (`vanhenematontaan`, `levenemätöntään`):
//! omorfi-only, mis-tagged, and not a Finnish word — the `-mätön` participle in
//! the illative with the genitive's `t` in it.
//!
//! Of the other 13, six are absent from the lexicon altogether (`erkanee`,
//! `karkenee`, `korkenee`, `mustenee`, `paranee`, `ulkonee` — the `-nee` members
//! the sanalista files under `72`, which have no rows in either source), and
//! seven are the class's two-reading members, whose reference form set is the
//! union of both readings' and so cannot be matched by one class's rule. Six of
//! those seven carry class 74 as their second reading (`aueta`, `oieta`,
//! `raueta` as `72*D, 74*D` in one row; `isota`, `sietä`, `tyvetä` as two
//! homonyms), and class 74 has no module yet, so what they leave over is the old
//! type engine's class-74 attempt built on the *weak* base — `oie`, `raue`,
//! `iso`, `sie`, `aue` — every form of which libvoikko rejects.
//!
//! The seventh, `seljetä`, is plain `72*L` and needs no second reading to
//! explain it. Its reference entry is half empty (456 `both` rows against the
//! class median of 982 — Kotus-Verbit holds only the passive, the second
//! infinitive, the imperative's third person and the perfect), and all 456 are
//! generated. What is left over is omorfi's: it reads the strong stem `selke`
//! and then gives it a type-1 present (`selkeän`, `selkeämme`) *and* a class-74
//! past (`selkesi`) *and* two conditionals (`selkeisin` and `selkeäisin`).
//! `selkesi` is not a word, and libvoikko rejects 635 of its 663 single-word
//! forms.

#![allow(dead_code)]

use crate::kotus::Gradation;
use crate::tags::Form;

use super::utility::{Alt, PassiveCons, Stems, paradigm};

pub const CLASS: &str = "72";

/// The weak base of a class-72 verb: the lemma less its `ta`/`tä` (`vanhe`,
/// `pae`, `ede`, `huono`).
///
/// The sanalista writes every member in this grade -- `paeta` has no `k`,
/// `edetä` a `d`, `lämmetä` an `mm` -- so the base is the lemma's own stem and
/// the strong grade is the one the module has to work out.
/// The stripping itself is [`ta_base`], which classes 72 to 76 all use, so
/// what each of them has to say is which grade the sanalista wrote it in.
use super::utility::ta_base as base_of;

// The strong base (`pake` for `pae`, `ete` for `ede`) is `utility`'s shared
// helper, since classes 73 and 74 want the same one; the type's `D` code (`k`
// against nothing) is the reason it is shared rather than a bare call to
// `Gradation::strongen`.
use super::utility::strong_base_of;

/// Inflect a class-72 verb into its whole paradigm.
pub fn inflect(lemma: &str, gradation: Option<Gradation>) -> Vec<Form> {
    if lemma == "erkanee" {
        return inflect_erkanee();
    }
    let base = base_of(lemma);
    let strong = strong_base_of(base, gradation);
    // The present and the past are the strong base with the type's own `ne`
    // and `n`, and they are the only cells on that grade.
    let present = format!("{strong}ne");
    let past = format!("{strong}n");
    let mut s = Stems::with_past(lemma, &present, &past, None);
    // The conditional follows the *past* stem, which is this type's whole
    // point: `vanhenisi` is `vanhen` + `isi`, not the present's `vanhene` +
    // `isi`.
    s.cond_stem = past.clone();
    // The potential and the past active participle run on the weak base with
    // the class's doubled `n` in front of the ending: `paennee`, `vanhennut`.
    let plain_n = format!("{base}n");
    s.pot = Alt {
        stem: plain_n.clone(),
        lead: "n",
    };
    s.past_part = Alt {
        stem: plain_n,
        lead: "n",
    };
    // The imperative's plurals and the passive take the type's own `t` on the
    // weak base, and the second infinitive the same base's `te`.
    s.imp_stem = format!("{base}t");
    s.p_theme = base.to_string();
    s.p_cons = PassiveCons::Geminated;
    s.inf2_stem = format!("{base}te");
    paradigm(&s)
}

/// `erkanee` is the *Dictionary of Contemporary Finnish*'s headword for the
/// defective `erata` ("to diverge"): the weak `era-` forms are nonstandard,
/// so Kotus files the verb under the strong 3rd person. The erkane- stem
/// supplies the finite active present, past, conditional, the imperative's
/// second person, the 3rd-5th infinitives and the active participles; the
/// missing cells are supplanted from `erkaantua`.
///
/// The whole paradigm is therefore one `Stems` with a hybrid set of stems:
/// the present/past/conditional and the nominal non-finites on `erkane`/
/// `erkan`, while the potential, imperative plurals, passive and 1st-2nd
/// infinitives run on `erkaantu`/`erkaannu`. See the Wiktionary `erata`
/// entry's manual conjugation table.
fn inflect_erkanee() -> Vec<Form> {
    let mut s = Stems::with_past("erkaantua", "erkane", "erkan", None);
    // The conditional follows the past's `erkan` (`erkanisin`), not the
    // present theme.
    s.cond_stem = "erkan".to_string();
    // The supplanted cells come from `erkaantua`: potential `erkaantunen`,
    // past participle `erkaantunut`, imperative plurals `erkaantukaa`.
    s.pot = Alt::n("erkaantu");
    s.past_part = Alt::n("erkaantu");
    s.imp_stem = "erkaantu".to_string();
    s.p_theme = "erkaannu".to_string();
    s.inf2_stem = "erkaantue".to_string();
    paradigm(&s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has(lemma: &str, gradation: Option<Gradation>, word: &str) -> bool {
        inflect(lemma, gradation)
            .into_iter()
            .any(|form| form.word == word)
    }

    #[test]
    fn erkanee_is_the_defective_erata_paradigm() {
        // Present/past/conditional on erkane-.
        for want in [
            "erkanen",
            "erkanet",
            "erkanee",
            "erkanemme",
            "erkanette",
            "erkanevat",
            "erkanin",
            "erkanit",
            "erkani",
            "erkanimme",
            "erkanitte",
            "erkanivat",
            "erkanisin",
            "erkanisi",
            "erkanisivat",
            "erkane",
            "erkanemassa",
            "erkanemasta",
            "erkanemaan",
            "erkanemalla",
            "erkanematta",
            "erkaneman",
            "erkaneminen",
            "erkanemaisillaan",
            "erkaneva",
            "erkanema",
            "erkanematon",
        ] {
            assert!(has("erkanee", None, want), "erkanee should have {want}");
        }
        // The missing cells are supplanted from erkaantua.
        for want in [
            "erkaantua",
            "erkaantuessa",
            "erkaantuen",
            "erkaantunen",
            "erkaantunee",
            "erkaantukaamme",
            "erkaantukaa",
            "erkaantukoot",
            "erkaannutaan",
            "erkaannuttiin",
            "erkaannuttaisiin",
            "erkaannuttaneen",
            "erkaannuttakoon",
            "erkaantunut",
            "erkaantuneet",
            "erkaannuttu",
            "erkaannuttava",
        ] {
            assert!(has("erkanee", None, want), "erkanee should have {want}");
        }
        // The class's ordinary `-ne-` present must not leak in.
        assert!(!has("erkanee", None, "erkaneene"));
        assert!(!has("erkanee", None, "erkaneenut"));
    }
}