//! Class 5 (the aari/argon type) inflection, written by hand as a logical
//! engine.
//!
//! Base class 5 is the plain, non-gradating loanword family: `-i` nouns
//! (`aari`, `aasi`, `aatami`) alongside consonant-final loans (`argon`,
//! `aerobic`, `ajatollah`). Every form grows from an `i`-stem -- the lemma when
//! it already ends in `i`, otherwise the lemma with `i` appended:
//! `aari -> aari`, `argon -> argoni`. The singular is then `i`-stem + ending
//! (aarin, argonin; illative `aariin`/`argoniin` doubles the `i`), with the
//! bare nominative keeping the lemma for a consonant-final word (`argon`).
//!
//! The plural grows from an `e`-stem, the `i`-stem with its final `i` written
//! `e` (`aare`, `argone`): every oblique plural cell is `e`-stem + ending
//! (aareja/argoneja, aareissa). Only the nominative (`aarit`) and the genitive
//! (`aarien`, `i`-stem + `en`) keep the `i`.
//!
//! The plain grid is not this module's: it is [`build_i_loans`], shared with
//! classes 6, 22 and 99, because the four classes are the same 26 cells with
//! the same three `-i`-plural alternants. What this module adds is the graded
//! grid (`5*A`..`5*J`) and the three word lists that decide policy -- which
//! words take both harmonies, which do not gradate at all, and which take the
//! `-i`-plural. All three are measured from the reference and say so in their
//! own doc comments.

#![allow(dead_code)]

use crate::kotus::{Gradation, Grade};
use crate::tags::{Case, Form, Number, PartOfSpeech};

use super::harmony::loanword_harmony;
use super::utility::{
    Builder, Decl, IPlural, build_i_loans, build_i_loans_with_nom_base, cell, e_stem, nominal_tags,
    plain, rare,
};

pub const CLASS: &str = "5";

/// Loanwords the reference gives with both a/ä spellings, so both are built.
///
/// Two of the four are the reference's own doing: `design` and `estrogeeni`
/// have an a/ä twin in every single case, and they are the only two class-5
/// lemmas the reference does that for -- the list is complete for the claim.
/// The other two, `analyysi` and `kitaristi`, have no such twin in the
/// catalogue but are genuinely ambiguous-harmony words in the language at
/// large, so both are built on purpose rather than on the reference's word.
const BOTH_HARMONY: &[&str] = &["analyysi", "design", "estrogeeni", "kitaristi"];

/// Loanwords in a graded class that do not actually gradate (anakoluutti keeps
/// its `tt`), the `kyty`-style irregular; they inflect through the plain
/// non-graded paradigm.
const NON_GRADATING: &[&str] = &["anakoluutti"];

/// Native `-ppi` words the sanalista also lists under 5*B beside their real
/// 7*B row. The loanword grid would give `apit`/`appini`; the native word takes
/// `apet`/`appeni`. The singular translative (`apikseen`) is still class 5's,
/// which is why the 5*B row cannot simply delegate the whole paradigm.
const NATIVE_SEVEN: &[&str] = &["soppi"];

/// The class-5 loans that confirm the `-i`-plural, so its three alternants
/// surface plainly (`glykoli -> glykoleiden`, not merely `glykolien`). These
/// are the only 26 of the class's 3375 lemmas that do; everyone else is plain
/// `aari`, takes only `-ien`/`-eja`, and their alternants stay repaired
/// guesses.
///
/// Measured, not curated: the list is every class-5 lemma whose reference gives
/// a genitive plural in `-iden` and a partitive plural in `-ita`, and each of
/// the 26 gives all three -- `-iden`, `-itten` and `-ita` alike.
const I_PLURAL: &[&str] = &[
    "epsilon",
    "fitness",
    "glykoli",
    "gooni",
    "grafeeni",
    "groteski",
    "halal",
    "hanami",
    "humaani",
    "humidori",
    "ibuprofeeni",
    "jargon",
    "koolon",
    "makroni",
    "marsipaani",
    "nokturni",
    "posteljooni",
    "refleksi",
    "shamaani",
    "spinning",
    "styyrpuuri",
    "teflon",
    "tsunami",
    "webinaari",
    "ypsilon",
    "šaali",
];

/// The `-i`-plural policy for a base-class-5 word.
fn i_plural(lemma: &str) -> IPlural {
    if I_PLURAL.contains(&lemma) {
        IPlural::All
    } else {
        IPlural::None
    }
}

/// The `5*(F)` loanword that genuinely gradates (kuti -> kudin), even though it
/// is filed under plain `5`. `huti`, the class's model, does not gradate, so it
/// is not here.
const GRADATED_F: &[&str] = &["kuti", "peti"];

/// Graded class-5 words whose reference confirms the *strong*-grade instructive
/// (`prinssinakkein`, `fasilitointein`) rather than the weak one (`aerobikein`)
/// that is the norm. They are few, so they are listed by name.
const STRONG_INSTRUCTIVE: &[&str] = &[
    "prinssinakki",
    "valepukki",
    "pikavippi",
    "diversiteetti",
    "fasilitointi",
    "kalkylointi",
    "konservointi",
    "valmiuslaki",
];

/// Native `-i`/`-ri` words that also carry a class-5 row take the native
/// `e`-stem in the nominative possessive (`kuoreni`, `vyyhteni`), while the
/// rest of the class-5 grid stays on its i-stem.
fn native_nom_base(lemma: &str) -> Option<String> {
    let mut native = false;
    for row in crate::kotus::sanalista().by_word(lemma) {
        for inflection in row.inflections() {
            if matches!(inflection.class, 7 | 24 | 26) {
                native = true;
            }
        }
    }
    native.then(|| e_stem(lemma))
}

/// Inflect a base-class-5 noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    // `peitsi` has a 30 row beside its spurious 5 row. The class-5 i-plural
    // (`peitseittä`, `peitseistä`) is not a form of the word; live Voikko
    // accepts the class-30 `peitsittä`/`peitsistä`.
    if lemma == "peitsi" {
        return super::class_30::inflect(lemma, possessives);
    }
    // The 5*(F) loanword that gradates (kuti t->d) is not a plain `-i` noun;
    // route it through the graded engine on its own gradation type.
    if GRADATED_F.contains(&lemma) {
        return inflect_graded(lemma, possessives, crate::kotus::Gradation(6));
    }
    // A few ambiguous loanwords (analyysi) are genuinely a/ä both, so both
    // spellings are built; otherwise the shared loanword harmony decides.
    let nom_base = native_nom_base(lemma);
    if BOTH_HARMONY.contains(&lemma) {
        let mut out = build_i_loans_with_nom_base(
            lemma,
            possessives,
            true,
            i_plural(lemma),
            nom_base.as_deref(),
        );
        out.extend(build_i_loans_with_nom_base(
            lemma,
            possessives,
            false,
            i_plural(lemma),
            nom_base.as_deref(),
        ));
        return out;
    }
    build_i_loans_with_nom_base(
        lemma,
        possessives,
        loanword_harmony(lemma),
        i_plural(lemma),
        nom_base.as_deref(),
    )
}

/// The graded class-5 subtypes (5*A..5*J). A graded class-5 word is a base-5
/// `-i`/loanword noun with consonant gradation: the strong stem is the lemma
/// (`aerobikki`), the weak stem is `grad` applied (`aerobiki`). As in class 1*A,
/// strong feeds the nominative/partitive/essive/illative and the genitive
/// plural; weak feeds the genitive, translative, the local cases and the
/// nominative plural. The `-i` plural keeps the base shape: the plural genitive
/// is the strong `i`-stem with `-en`, the rest live on the `e`-stem split
/// strong (partitive, essive, illative) vs weak (translative, local,
/// instructive). There is no `i`-plural (`aerobikkien`, not `aerobikkoiden`).
pub fn inflect_graded(lemma: &str, possessives: bool, grad: Gradation) -> Vec<Form> {
    // The native `-ppi` words take the class-7 grid (apet, apissa, apilla).
    // Their 5*B row still confirms the i-stem translative possessive column
    // (`apikseen`, `apikseni`) that class 7 spells on the e-stem, so that one
    // column is added on top rather than dropped with the rest of the loanword
    // grid.
    if NATIVE_SEVEN.contains(&lemma) {
        return super::class_7::inflect_graded(lemma, possessives, grad);
    }
    let mut b = Builder::new_harmony(lemma, possessives, loanword_harmony(lemma));
    // A rare loanword in a graded class may be non-gradating (anakoluutti keeps
    // its tt while instituutti/absoluutti weaken): route it through the
    // ordinary non-graded paradigm.
    if NON_GRADATING.contains(&lemma) {
        return build_i_loans(lemma, possessives, b.is_front(), i_plural(lemma));
    }
    let strong = lemma;
    let nom_base = native_nom_base(lemma);
    let weak = grad.apply(strong, Grade::Weak);
    let estrong = e_stem(strong);
    let eweak = e_stem(&weak);
    let (sg, pl) = (Number::Singular, Number::Plural);
    // The instructive is confirmed on the weak grade for most words
    // (`aerobikein`), but on the strong grade for a handful (`prinssinakkein`,
    // `fasilitointein`); offer the confirmed grade plainly and the other as a
    // repaired guess. Two cells of one case, so the order is the confirmed one
    // first.
    let (instr, instr_alt) = if STRONG_INSTRUCTIVE.contains(&lemma) {
        (
            plain(Case::Instructive, pl, &estrong, ["in", "in"]),
            rare(Case::Instructive, pl, &eweak, ["in", "in"]),
        )
    } else {
        (
            plain(Case::Instructive, pl, &eweak, ["in", "in"]),
            rare(Case::Instructive, pl, &estrong, ["in", "in"]),
        )
    };

    b.decline(
        &Decl::new(vec![
            // ---- singular, strong grade ----
            cell(Case::Nominative, sg, strong, ["", ""], false).base_on(
                nom_base.clone().unwrap_or_else(|| strong.to_string()),
                ["", ""],
            ),
            cell(Case::Partitive, sg, strong, ["a", "ä"], true),
            cell(Case::Essive, sg, strong, ["na", "nä"], true),
            // The illative doubles the i: aerobikki -> aerobikkiin.
            cell(Case::Illative, sg, strong, ["in", "in"], false).on_ending(["i", "i"]),
            // ---- singular, weak grade ----
            // Genitive is weak but its possessives sit on the strong stem.
            cell(Case::Genitive, sg, &weak, ["n", "n"], false).base_on(
                nom_base.clone().unwrap_or_else(|| strong.to_string()),
                ["", ""],
            ),
            cell(Case::Translative, sg, &weak, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            cell(Case::Inessive, sg, &weak, ["ssa", "ssä"], true),
            cell(Case::Elative, sg, &weak, ["sta", "stä"], true),
            cell(Case::Adessive, sg, &weak, ["lla", "llä"], true),
            cell(Case::Ablative, sg, &weak, ["lta", "ltä"], true),
            cell(Case::Allative, sg, &weak, ["lle", "lle"], true),
            cell(Case::Abessive, sg, &weak, ["tta", "ttä"], true),
            // ---- plural, strong grade ----
            // Genitive plural is the strong i-stem with -en: aerobikki ->
            // aerobikkien, and the rest of the strong plural lives on its
            // e-stem: aerobikkeja, aerobikkeina, aerobikkeihin.
            cell(Case::Genitive, pl, strong, ["en", "en"], false).on_ending(["e", "e"]),
            cell(Case::Partitive, pl, &estrong, ["ja", "jä"], true),
            cell(Case::Essive, pl, &estrong, ["ina", "inä"], true),
            cell(Case::Illative, pl, &estrong, ["ihin", "ihin"], false).on_ending(["ihi", "ihi"]),
            // ---- plural, weak grade ----
            // Nominative is weak but its possessives sit on the strong stem,
            // or the native e-stem when this word also has a native row.
            cell(Case::Nominative, pl, &weak, ["t", "t"], false).base_on(
                nom_base.clone().unwrap_or_else(|| strong.to_string()),
                ["", ""],
            ),
            cell(Case::Translative, pl, &eweak, ["iksi", "iksi"], true).on_ending(["ikse", "ikse"]),
            cell(Case::Inessive, pl, &eweak, ["issa", "issä"], true),
            cell(Case::Elative, pl, &eweak, ["ista", "istä"], true),
            cell(Case::Adessive, pl, &eweak, ["illa", "illä"], true),
            cell(Case::Ablative, pl, &eweak, ["ilta", "iltä"], true),
            cell(Case::Allative, pl, &eweak, ["ille", "ille"], true),
            cell(Case::Abessive, pl, &eweak, ["itta", "ittä"], true),
            instr,
            instr_alt,
        ]),
        |case, number, poss| nominal_tags(PartOfSpeech::Noun, case, number, poss),
    );
    b.comitative(&estrong);

    b.into_forms()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kotus::Gradation;

    /// A native word that also carries a class-5 row takes the native e-stem
    /// in the nominative possessive, not the loanword i-stem.
    #[test]
    fn a_native_twin_uses_the_e_stem_for_nominative_possessives() {
        fn nominative_singular(forms: Vec<Form>) -> Vec<String> {
            forms
                .into_iter()
                .filter(|form| {
                    form.readings.iter().any(|reading| {
                        reading.case == Some(Case::Nominative)
                            && reading.number == Some(Number::Singular)
                    })
                })
                .map(|form| form.word)
                .collect()
        }

        let kuori = nominative_singular(inflect("kuori", true));
        assert!(kuori.contains(&"kuoreni".to_string()));
        assert!(!kuori.contains(&"kuorini".to_string()));

        fn genitive_singular(forms: Vec<Form>) -> Vec<String> {
            forms
                .into_iter()
                .filter(|form| {
                    form.readings.iter().any(|reading| {
                        reading.case == Some(Case::Genitive)
                            && reading.number == Some(Number::Singular)
                    })
                })
                .map(|form| form.word)
                .collect()
        }

        let vyyhti = nominative_singular(inflect_graded("vyyhti", true, Gradation(6)));
        assert!(vyyhti.contains(&"vyyhteni".to_string()));
        assert!(!vyyhti.contains(&"vyyhtini".to_string()));
        let vyyhti_gen = genitive_singular(inflect_graded("vyyhti", true, Gradation(6)));
        assert!(vyyhti_gen.contains(&"vyyhteni".to_string()));
        assert!(!vyyhti_gen.contains(&"vyyhtini".to_string()));

        let appi_gen = genitive_singular(inflect_graded("appi", true, Gradation(2)));
        assert!(appi_gen.contains(&"appeni".to_string()));
        assert!(!appi_gen.contains(&"appini".to_string()));

        let viti = nominative_singular(inflect_graded("viti", true, Gradation(6)));
        assert!(viti.contains(&"vitini".to_string()));
        assert!(!viti.contains(&"viteni".to_string()));
    }

    use super::*;
    use crate::tags::Confidence;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }

    #[test]
    fn i_final_stem() {
        let w = words("aari");
        assert!(w.contains(&"aarin".to_string())); // gen
        assert!(w.contains(&"aaria".to_string())); // part
        assert!(w.contains(&"aariin".to_string())); // illat (double i)
        assert!(w.contains(&"aarit".to_string())); // nom pl
        assert!(w.contains(&"aarien".to_string())); // gen pl, keeps i
        assert!(w.contains(&"aareja".to_string())); // part pl, e-stem
    }

    #[test]
    fn consonant_final_stem() {
        // argon inserts i: argoni; nominative keeps the lemma.
        let w = words("argon");
        assert!(w.contains(&"argon".to_string())); // nom
        assert!(w.contains(&"argonin".to_string())); // gen
        assert!(w.contains(&"argonia".to_string())); // part
        assert!(w.contains(&"argoniin".to_string())); // illat
        assert!(w.contains(&"argonit".to_string())); // nom pl
        assert!(w.contains(&"argonien".to_string())); // gen pl
        assert!(w.contains(&"argoneja".to_string())); // part pl
    }

    #[test]
    fn front_harmony() {
        let w = words("brändi");
        assert!(w.contains(&"brändiä".to_string())); // part, front
        assert!(w.contains(&"brändejä".to_string())); // part pl, front
    }

    #[test]
    fn compound_head_sets_harmony() {
        // A `*-viini` word is front on its head, ignoring a modifier back vowel.
        let w = words("hehkuviini");
        assert!(w.contains(&"hehkuviiniä".to_string()));
        assert!(!w.contains(&"hehkuviinia".to_string()));
        // A word whose head is absent from FRONT_HEADS falls to ordinary harmony.
        assert!(words("kronometri").contains(&"kronometria".to_string()));
    }

    #[test]
    fn exception_tables_override() {
        // back though the wine head, and front though ordinary harmony reads back.
        assert!(words("riboflaviini").contains(&"riboflaviinia".to_string()));
        assert!(words("antigeeni").contains(&"antigeeniä".to_string()));
    }

    #[test]
    fn open_ended_for_new_compounds() {
        // A brand-new, not-in-lexicon compound resolves harmony from its head:
        // front for a front neutral head, back for a back-vowel head.
        assert!(words("omeenaviini").contains(&"omeenaviiniä".to_string()));
        assert!(words("luumukeksi").contains(&"luumukeksiä".to_string()));
        assert!(!loanword_harmony("omenapullo")); // back head pullo
        assert!(!loanword_harmony("kaupanhalli")); // back head halli
    }

    /// A loanword the reference gives in all three `-i`-plural alternants takes
    /// them plainly; everyone else takes them as repairs. `glykoli` is one of
    /// the 26 that do, `aari` is not one.
    #[test]
    fn the_i_plural_is_plain_only_where_the_reference_gives_it() {
        let confirmed = inflect("glykoli", true);
        let plain: Vec<&str> = confirmed
            .iter()
            .filter(|f| f.confidence == Confidence::Plain)
            .map(|f| f.word.as_str())
            .collect();
        for form in ["glykoleiden", "glykoleitten", "glykoleita"] {
            assert!(plain.contains(&form), "{form} not plain in {plain:?}");
        }
        let ordinary = inflect("aari", true);
        let plain: Vec<&str> = ordinary
            .iter()
            .filter(|f| f.confidence == Confidence::Plain)
            .map(|f| f.word.as_str())
            .collect();
        for form in ["aareiden", "aareitten", "aareita"] {
            assert!(!plain.contains(&form), "{form} wrongly plain in {plain:?}");
        }
        // The alternants are offered either way -- they are repairs, not
        // absentees -- and `aarien`/`aareja` stay plain throughout.
        let all: Vec<&str> = ordinary.iter().map(|f| f.word.as_str()).collect();
        assert!(all.contains(&"aareiden") && all.contains(&"aarien"));
        assert!(all.contains(&"aareja"));
    }

    /// The list is the reference's, not a curation: 26 words, and the whole
    /// `-iden`/`-itten`/`-ita` group comes as a set. `fitness` is the word the
    /// old code called `-itten`-less; the reference has `fitnessitten`.
    #[test]
    fn the_i_plural_list_is_the_whole_group_of_twenty_six() {
        assert_eq!(I_PLURAL.len(), 26);
        assert!(I_PLURAL.contains(&"fitness"));
        assert!(I_PLURAL.contains(&"epsilon") && I_PLURAL.contains(&"šaali"));
        assert!(!I_PLURAL.contains(&"aari") && !I_PLURAL.contains(&"argon"));
    }

    #[test]
    fn graded_strong_weak_split() {
        // aerobikki is 5*A (kk -> k): weak feeds genitive, strong feeds
        // partitive/illative and the strong e-stem plural.
        let w = inflect_graded("aerobikki", true, crate::kotus::Gradation(1));
        let ws: Vec<&str> = w.iter().map(|f| f.word.as_str()).collect();
        assert!(ws.contains(&"aerobikin")); // genitive, weak
        assert!(ws.contains(&"aerobikkia")); // partitive, strong
        assert!(ws.contains(&"aerobikkiin")); // illative, strong
        assert!(ws.contains(&"aerobikit")); // nom plural, weak
        assert!(ws.contains(&"aerobikkien")); // gen plural, strong i-stem
        assert!(ws.contains(&"aerobikkeja")); // part plural, strong e-stem
        assert!(ws.contains(&"aerobikeissa")); // inessive plural, weak e-stem
    }

    /// `appi` and `soppi` carry both a 7*B row and a spurious 5*B row. The
    /// 5*B engine must not add its loanword `-i` plural (`apit`, `sopit`) to
    /// the native `-e` plural (`apet`, `sopet`); live Voikko rejects the
    /// former and accepts the latter.
    /// `peitsi`'s 5 row must not add the i-plural `peitseittä` to the class-30
    /// `peitsittä`.
    #[test]
    fn peitsi_uses_the_class_30_paradigm() {
        let ws: Vec<String> = inflect("peitsi", true)
            .into_iter()
            .map(|f| f.word)
            .collect();
        assert!(ws.contains(&"peitsittä".to_string()));
        assert!(!ws.contains(&"peitseittä".to_string()));
    }

    #[test]
    fn soppi_uses_the_class_7_plural() {
        let ws: Vec<String> = inflect_graded("soppi", true, crate::kotus::Gradation(2))
            .into_iter()
            .map(|f| f.word)
            .collect();
        assert!(ws.contains(&"sopet".to_string()));
        assert!(!ws.contains(&"sopit".to_string()));
    }

    /// `appi` is two homonyms: the 7*B father-in-law (`apet`) and the 5*B
    /// colloquial "app" (`apit`). The 5*B engine keeps its own `-i` plural.
    #[test]
    fn appi_keeps_the_5b_app_plural() {
        let ws: Vec<String> = inflect_graded("appi", true, crate::kotus::Gradation(2))
            .into_iter()
            .map(|f| f.word)
            .collect();
        assert!(ws.contains(&"apit".to_string()));
        assert!(ws.contains(&"appia".to_string()));
    }

    /// The 5*B "app" homonym has the loanword i-stem paradigm: `apin`,
    /// `apeissa`, `apeilla`, `appeja`, `apein`. It must not borrow the 7*B
    /// father-in-law forms (`apen`, `apissa`, `appia`, `apin`).
    #[test]
    fn appi_has_the_5b_app_paradigm() {
        let ws: Vec<String> = inflect_graded("appi", false, crate::kotus::Gradation(2))
            .into_iter()
            .map(|f| f.word)
            .collect();
        for want in [
            "appi", "apin", "appia", "appiin", "apissa", "apista", "apilla", "apilta", "apille",
            "appina", "apiksi", "apitta", "apit", "appien", "appein", "appeja", "appeihin",
            "apeissa", "apeista", "apeilla", "apeilta", "apeille", "appeina", "apeiksi", "apeitta",
            "apein",
        ] {
            assert!(ws.contains(&want.to_string()), "missing {want}");
        }
        for wrong in ["apen", "appea", "appeen", "apessa", "apesta", "apet"] {
            assert!(!ws.contains(&wrong.to_string()), "invented {wrong}");
        }
    }

    /// The graded grid's one place where both grades appear for the same case:
    /// the instructive, which is confirmed on the weak grade for most of the
    /// class (`aerobikein`) and on the strong one for eight named words
    /// (`prinssinakkein`). The confirmed grade comes first and the other is a
    /// repair -- so the order of the two cells is a fact about the word, not a
    /// sort.
    #[test]
    fn the_instructive_follows_the_grade_the_reference_gives() {
        fn instructive(lemma: &str) -> (Vec<String>, Vec<String>) {
            let forms = inflect_graded(lemma, true, crate::kotus::Gradation(1));
            let words: Vec<String> = forms.iter().map(|f| f.word.clone()).collect();
            let plain: Vec<String> = forms
                .iter()
                .filter(|f| {
                    f.confidence == Confidence::Plain
                        && f.readings.iter().any(|t| t.case == Some(Case::Instructive))
                })
                .map(|f| f.word.clone())
                .collect();
            (words, plain)
        }
        // aerobikki: weak is confirmed, strong is the repair.
        let (words, plain) = instructive("aerobikki");
        assert!(
            words.iter().any(|w| w == "aerobikein") && words.iter().any(|w| w == "aerobikkein")
        );
        assert_eq!(plain, vec!["aerobikein".to_string()], "{plain:?}");
        // prinssinakki: the other way round, and the strong one is first.
        let (words, plain) = instructive("prinssinakki");
        let strong = words.iter().position(|w| w == "prinssinakkein").unwrap();
        let weak = words.iter().position(|w| w == "prinssinakein").unwrap();
        assert!(strong < weak, "confirmed grade must come first");
        assert_eq!(plain, vec!["prinssinakkein".to_string()], "{plain:?}");
    }

    /// `anakoluutti` is filed under the gradating `5*C` (`tt -> t`) but the
    /// reference gives it 25 forms and not one of them weakens, so it inflects
    /// through the plain paradigm and `anakoluutin`/`anakoluutissa` are never
    /// offered -- unlike `instituutti`, whose the reference does weaken.
    #[test]
    fn a_word_that_does_not_gradate_in_a_gradating_class() {
        let forms = inflect("anakoluutti", true);
        let ws: Vec<&str> = forms.iter().map(|f| f.word.as_str()).collect();
        assert!(ws.contains(&"anakoluuttia"), "{ws:?}");
        assert!(ws.contains(&"anakoluuttin"), "{ws:?}");
        assert!(ws.contains(&"anakoluuttissa"), "{ws:?}");
        assert!(!ws.contains(&"anakoluutissa"), "weak form offered: {ws:?}");
        assert!(NON_GRADATING.contains(&"anakoluutti"));
        // Its class-5*C sibling does weaken, so the entry is word-specific.
        assert!(!NON_GRADATING.contains(&"instituutti"));
    }
}
