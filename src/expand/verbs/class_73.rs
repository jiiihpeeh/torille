//! Class 73 (the `salata` type) inflection, written by hand.
//!
//! **The type is class 72's neighbour with an `-a-` present instead of an
//! `-ne-` one.** Both are `-ta/-tä` intransitives of change of state, both put a
//! consonant in front of the theme rather than weakening it, and both run the
//! potential and the past active participle on the weak base with a doubled
//! `n` (`salannee`, `salannut` beside `vanhennee`, `vanhennut`). What is
//! different is only what the finite present does with that consonant, and so
//! which grade each cell takes:
//!
//! ```text
//! salata / salaan / salasi / salaisi / salannee / salatkoon / salannut / salattiin
//! ```
//!
//! The weak base is the lemma less its `ta`/`tä` (`sala`, `paka`, `levä`) and
//! the strong base is that base's own gradation (`pakka`, `lepä`). The sanalista
//! writes every member in the weak grade, so the strong one is the module's
//! work, and **the whole finite mood runs on it** -- present, past and
//! conditional alike. That is the one place class 73 parts company with the
//! `-ta/-tä` types around it: classes 52, 53 and 56 put the weak grade in front
//! of the vowel in the first and second persons and keep the strong one only
//! for the third (`haukko`/`haukkoa`, `auttoi`/`autoin`), because their gradation
//! sits in the middle of the stem and the weak form is the unmarked one. Here
//! the gradation is at the very end of the base, so the weak form is not a
//! shorter or lighter variant of anything -- `pak` + `kaan` is not a Finnish
//! word -- and the paradigm has no weak present at all.
//!
//! The present is the strong base with the infinitive's own theme vowel on it
//! (`salaa`, `pakkaa`, `lepää`), which is the whole of the type's shape: the
//! third person is that stem unchanged, since the theme `a` is already the
//! letter the third person would repeat (`salaa`, not `salaaa` as type 1's
//! `sanao` beside `sanon`), and every other person hangs its ending on it
//! (`salaan`, `salaat`, `salaamme`, `salaatte`, `salaavat`). The past is the
//! same strong base with `s` in front of the endings' own `i` -- `salasi` is
//! `salas` + `i` where type 1 drops the theme instead (`sanoi` is `sano` +
//! `i`) -- and the conditional is that base bare with the ordinary `-isi`
//! (`salaisi`), so the past and the conditional are one letter apart exactly as
//! in class 72 (`vanheni`/`vanhenisi`), and both are the strong base.
//!
//! Everything else runs on the weak base with the type's own `t` in front of
//! it, which is class 72's own arrangement and for the same reason: the `-tä` of
//! the infinitive is a stem consonant rather than part of a theme, so the
//! second infinitive is `salate` + `ssa` in the inessive and `salate` + `n` in
//! the instructive (`salatessa`, `salaten`), the imperative's plurals are
//! `salat` + `-kaa`/`-koon` (`salatkaa`, `salatkoon`), and the passive theme is
//! the base with `t` added to that stem in the present and `tt` added to it
//! everywhere else (`salataan` beside `salattiin`). The potential and the past
//! active participle are the base with
//! the class's doubled `n` (`salan` + `n` + `ee`, `salan` + `n` + `ut`), on the
//! ordinary [`Alt`] shape class 72 shares. The imperative's second person
//! singular and the negated present are the present stem (`salaa`: `salaa!`,
//! `en salaa`), and the third and fourth infinitives, the present participle,
//! the agent and the negative participle take it too (`salaama`,
//! `salaaminen`, `salaava`, `salaamaton` -- the last of which is the present
//! stem and not the base, `salemamätön` being a form Finnish does not have).
//!
//! ## What the reference says
//!
//! **947 of the class's 962 lemmas audit at 0 invented and 0 missing against
//! `both`**, over 655 560 reference rows out of the class's 660 912 — every cell
//! the type gives. Each of them leaves exactly one `union` row uncovered, the
//! `...amatontaan` row every verb class in this project has (`salaamatontaan`,
//! `lepäämätöntään`): omorfi-only, mis-tagged, and not a Finnish word — the
//! `-mätön` participle in the illative with the genitive's `t` in it. Before the
//! module the class generated 101 forms a lemma against a 1 289-form reference;
//! the missing rows over the class were 596 803, and are now 1 093.
//!
//! The other 15 are the data's rather than the module's, and they fall into four
//! groups:
//!
//! * **seven have no Kotus-Verbit rows at all** (`digata`, `feikata`,
//!   `krampata`, `shakata`, `tiltata`, `torpata`, `trendata`), so their 668
//!   invented are our paradigm against an empty reference;
//! * **four have a half-empty entry** (`blogata` and `hyljätä`/`peljätä` at 499
//!   rows, `pokata` and `reitata` at 456, against the class median of 981) —
//!   holding only the weak-base cells. Three of the four are covered in full;
//!   `blogata` is the exception and is the next item;
//! * **`taata` is two words** (verb `73*D` and noun class 9), so its reference
//!   set is the noun's and the verb's union; the union is covered to within the
//!   one `union` row above, and the 152 invented are the noun's own;
//! * **two have a gradation the sanalista does not record.** `lobata` and
//!   `blogata` are filed as plain `73`, but Kotus-Verbit gradates them `bb <-> b`
//!   and `gg <-> g` (`lobbaa`, `lobbasi`, `bloggaa`, `bloggasi` against
//!   `lobatkaa` and `blogannut` on the weak base), and libvoikko accepts all
//!   four and rejects `lobaa` and `blogaa`. Neither consonant has a code — the
//!   sanalista's thirteen codes run `kk`, `pp`, `tt`, `k`, `p`, `t`, `nk`, `mp`,
//!   `lt`, `nt`, `rt`, `k`, `k` and there is no `b` or `g` among them — so this
//!   is a gap in the data and not something the module can read. It is the same
//!   trade class 68 declined for its `-oid-` members: reproducing the grade means
//!   a hard-coded list of two words.
//!
//! `tavata` is the fifteenth and the one worth reading twice, because it is the
//! sanalista that is wrong rather than thin. It is filed `73*E`, `73` and
//! `73*E` across three homonym rows, but **none of the three reference entries
//! gradates**: all 981 `both` rows of each are the ungradated `tavaa`, `tavasi`,
//! `tavannee`, `tavannut`, and the 525 missing are all of them. The module
//! follows the code and gives `tapaa`, which is the right thing to do with a code
//! it has been handed, and the binding's `dedup_graded` then drops the plain `73`
//! row that would have supplied the ungradated half — by design, since a code and
//! its bare class are meant to be one declension rather than two. `tavata` does
//! not gradate, so here the code and the bare class are two words and that
//! assumption costs a reading.

#![allow(dead_code)]

use crate::kotus::Gradation;
use crate::tags::Form;

use super::utility::{Alt, PassiveCons, Stems, front, paradigm, strong_base_of};

pub const CLASS: &str = "73";

/// The weak base of a class-73 verb: the lemma less its `ta`/`tä` (`sala`,
/// `paka`, `levä`, `huipa`).
///
/// The sanalista writes every member in this grade -- `pakata` has no `kk`,
/// `levätä` a `v` where its strong grade has a `p` -- so the base is the lemma's
/// own stem and the strong grade is what the module has to work out.
/// The stripping itself is [`ta_base`], which classes 72 to 76 all use, so
/// what each of them has to say is which grade the sanalista wrote it in.
use super::utility::ta_base as base_of;

/// Inflect a class-73 verb into its whole paradigm.
pub fn inflect(lemma: &str, gradation: Option<Gradation>) -> Vec<Form> {
    let base = base_of(lemma);
    let strong = strong_base_of(base, gradation);
    // The infinitive's own theme vowel, which is also the type's: `-ata` gives
    // `salaa` and `pakkaa`, `-ätä` gives `lepää` and `levännee`.
    let theme = if front(lemma) { 'ä' } else { 'a' };
    // The present is the strong base with the theme on it, and every cell of
    // the mood runs on that same strong base: this type has no weak present
    // and no weak past, which is what `gradation: None` below says -- the two
    // grades have already been worked out by hand, so `Stems::weak` has
    // nothing left to weaken.
    let present = format!("{strong}{theme}");
    // The past is that base with the ending's `s` in front of its own `i`, so
    // that the third person's bare `i` gives `salasi`.
    let past = format!("{strong}s");
    let mut s = Stems::with_past(lemma, &present, &past, None);
    // The third person has nothing to repeat: the stem already ends in the
    // doubled theme, so `pres3sg` is empty where type 1 has the stem's last
    // letter (`salaa`, not `salaaa`).
    s.pres3sg = String::new();
    // The conditional is the strong base bare with the ordinary `-isi`
    // (`salaisi`), not the past's `s` and not the present's theme.
    s.cond_stem = strong.clone();
    // The potential and the past active participle run on the weak base with the
    // class's doubled `n` in front of the ending: `salannee`, `salannut`.
    let plain_n = format!("{base}n");
    s.pot = Alt {
        stem: plain_n.clone(),
        lead: "n",
    };
    s.past_part = Alt {
        stem: plain_n,
        lead: "n",
    };
    // The imperative's plurals and the second infinitive take the type's own `t`
    // on the weak base, and the passive's theme is the base itself so that the
    // passive's `t` lands on the stem's own: `salatkaa`, `salatessa`, `salataan`
    // beside `salattiin`.
    s.imp_stem = format!("{base}t");
    s.inf2_stem = format!("{base}te");
    s.p_theme = base.to_string();
    s.p_cons = PassiveCons::Geminated;
    paradigm(&s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tags::{Mood, Polarity};

    fn forms(lemma: &str, code: Option<Gradation>) -> Vec<String> {
        inflect(lemma, code)
            .into_iter()
            .map(|f| f.word)
            .collect()
    }

    fn has(lemma: &str, code: Option<Gradation>, word: &str) -> bool {
        forms(lemma, code).iter().any(|w| w == word)
    }

    #[test]
    fn base_is_the_lemma_less_its_ta() {
        assert_eq!(base_of("salata"), "sala");
        assert_eq!(base_of("levätä"), "levä");
    }

    #[test]
    fn strong_base_gradates_every_code_the_class_names() {
        let d = Gradation(4);
        // `D` deletes a consonant rather than changing it, so the `k` it dropped
        // has to be written in before the base's final vowel.
        assert_eq!(strong_base_of("huoa", Some(d)), "huoka");
        assert_eq!(strong_base_of("hylä", Some(d)), "hylkä");
        assert_eq!(strong_base_of("iä", Some(d)), "ikä");
        assert_eq!(strong_base_of("pae", Some(d)), "pake");
        // The site is the last vowel, not the last character: class 67's
        // consonant-final `jael` gradates the same way.
        assert_eq!(strong_base_of("jael", Some(d)), "jakel");
        assert_eq!(strong_base_of("paka", Some(Gradation(1))), "pakka");
        assert_eq!(strong_base_of("levä", Some(Gradation(5))), "lepä");
        assert_eq!(strong_base_of("hyljä", Some(Gradation(12))), "hylkä");
        assert_eq!(strong_base_of("sala", None), "sala");
    }

    #[test]
    fn the_finite_mood_runs_on_the_strong_base_throughout() {
        // `pakata` is `73*A`, so its strong base is `pakka` and no cell may show
        // the weak `paka`.
        for word in [
            "pakkaan", "pakkaat", "pakkaa", "pakkaamme", "pakkaatte", "pakkaavat",
            "pakkasi", "pakkasimme", "pakkasin", "pakkasit", "pakkaisi", "pakkaisimme",
        ] {
            assert!(has("pakata", Some(Gradation(1)), word), "missing {word}");
        }
        // ... while the weak base is what the potential, the participle, the
        // imperative's plurals, the second infinitive and the passive take.
        for word in [
            "pakannee", "pakanneemme", "pakannut", "pakatkaa", "pakatkoon",
            "pakatessa", "pakaten", "pakataan", "pakattiin", "pakattava", "pakattu",
            "pakattaisiin", "pakattaneen", "pakattakoon", "pakattaessa",
        ] {
            assert!(has("pakata", Some(Gradation(1)), word), "missing {word}");
        }
        // The imperative's second person singular and the negated present are the
        // present stem, the only cells the mood and the imperative share.
        assert!(has("pakata", Some(Gradation(1)), "pakkaa"));
        assert!(
            inflect("pakata", Some(Gradation(1))).iter().any(|f| {
                f.word == "pakkaa"
                    && f.readings.iter().any(|t| {
                        t.polarity == Polarity::Negative && t.mood == Some(Mood::Indicative)
                    })
            }),
            "the negated present is the present stem"
        );
    }

    #[test]
    fn the_present_stem_is_the_theme_on_the_strong_base() {
        // The third person does not repeat the theme: `salaa`, not `salaaa`.
        assert!(has("salata", None, "salaa"));
        assert!(!has("salata", None, "salaaa"));
        // The infinitives and the `a`-participles take the same stem.
        for word in [
            "salaama", "salaamaan", "salaaminen", "salaava", "salaamaton",
            // The fifth infinitive is possessive-only in every class, and the
            // reference lists none of them bare.
            "salaamaisillani",
        ] {
            assert!(has("salata", None, word), "missing {word}");
        }
    }

    #[test]
    fn the_front_vowel_type_fronts_the_way_the_past_participle_does() {
        // `levätä` is `73*E`: base `levä`, strong `lepä`.
        for word in [
            "lepään", "lepää", "lepäämme", "lepäsi", "lepäisin", "lepäisi",
            "levännee", "levännyt", "levänneet", "levätkäämme", "levätköön",
            "levätessä", "leväten", "levätään", "levättiin", "levättävä", "levätty",
            "lepääminen", "lepäämätön", "levättäisiin",
        ] {
            assert!(has("levätä", Some(Gradation(5)), word), "missing {word}");
        }
    }

    #[test]
    fn an_ungradated_member_is_one_paradigm() {
        let got = forms("salata", None);
        for word in [
            "salata", "salaan", "salaat", "salaa", "salaamme", "salaatte", "salaavat",
            "salasi", "salasimme", "salasin", "salasit", "salasitte", "salasivat",
            "salaisi", "salaisimme", "salaisin", "salaisit", "salaisitte", "salaisivat",
            "salannee", "salannemme", "salannen", "salannet", "salannette",
            "salannevat",
            "salannut", "salanneet", "salatkaa", "salatkaamme", "salatkoon", "salatkoot",
            "salaten", "salatessa", "salattaessa", "salataan", "salattiin",
            "salattava", "salattu", "salattaisiin", "salattaneen", "salattakoon",
            "salattaen", "salattaman", "salaama", "salaaminen", "salaava", "salaamaton",
        ] {
            assert!(got.iter().any(|w| w == word), "missing {word}");
        }
    }
}