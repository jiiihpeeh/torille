//! Class 24 (the `kuusi` type) inflection, written by hand as a logical engine.
//!
//! Class 24 is class 23 with the consonant stem given a use. Both have the
//! `e`-stem, the `i`-stem and the consonant stem (`kuusi` -> `kuuse`, `kuusi`,
//! `kuus`), and both put their singular partitive on the consonant stem
//! (`kuusta`); what 24 adds is the `-ten` genitive plural beside the `-ien`
//! one (`kuusten` beside `kuusien`).
//!
//! Two words of the class take a partitive that is not their own harmony. `meri`
//! is front by its only vowel (`i`, neutral) but its partitive is `merta`, and a
//! `t`-ending takes back harmony whatever the word's own is -- so the whole
//! column follows it into the back (`mertani`, `mertansa`). `veri` is the same
//! word by another class (26, which delegates here). That column is the one
//! place this module does not go through the [`Decl`], and the comment at it
//! says why: it is two lemmas of twenty-eight, and the third person's `-nsa`
//! there is not the harmonic one.

#![allow(dead_code)]

use crate::tags::{Case, Form, Number, PartOfSpeech, Possessive};

use super::utility::{Builder, Decl, PERSONAL, cell, e_stem, nominal_tags, plain};

pub const CLASS: &str = "24";

/// The consonant stem: the lemma with its final `i` dropped (`kuusi -> kuus`),
/// which the singular partitive and the `-ten` genitive plural are built on.
fn c_stem(lemma: &str) -> String {
    let mut s = lemma.to_string();
    s.pop(); // drop the final -i
    s
}

/// The words whose partitive is back-harmonised (`meri`, and the class-26
/// `veri` that reaches this module). `t`-endings take back harmony, so the
/// whole possessive column is back too: `mertani`, `mertansa`.
fn back_partitive(lemma: &str) -> bool {
    matches!(lemma, "meri" | "veri")
}

/// Inflect a class-24 noun into every cell.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    let mut b = Builder::new(lemma, possessives);
    let estem = e_stem(lemma); // kuuse
    let istem = lemma; // kuusi
    let cstem = c_stem(lemma); // kuus
    let (sg, pl) = (Number::Singular, Number::Plural);

    let mut entries = vec![
        // ---- the singular, on the e-stem ----
        // The nominative is the bare lemma, but its possessives sit on the
        // genitive `e`-stem (`kuuseni`, not the `i`-stem `kuusini`).
        cell(Case::Nominative, sg, lemma, ["", ""], false).base_on(estem.clone(), ["", ""]),
        cell(Case::Genitive, sg, &estem, ["n", "n"], false).on_stem(),
    ];

    if back_partitive(lemma) {
        // `meri` and `veri` keep a back partitive (`merta`, `verta`) while every
        // other ending of theirs stays front. The column is spelled out rather
        // than declined, because `decline` would harmonise the third person to
        // `nsä` and the word's is `nsa` (`mertansa`).
        //
        // It is two lemmas of the eighteen the sanalista lists for this class,
        // and it is the only cell in the whole tree whose third-person `-nsa`
        // is not the harmonic one -- measured over every lemma of every class
        // the fixture covers. So it is spelled here rather than made a variant
        // of [`Decl`]: the alternative would be a flag on every cell to serve
        // two words.
        let base = format!("{cstem}ta");
        b.push(Case::Partitive, sg, None, base.clone());
        if b.has_possessives() {
            for (suf, p) in PERSONAL {
                b.push(Case::Partitive, sg, Some(*p), format!("{base}{suf}"));
            }
            // The third person is spelled two ways here and both are back: the
            // `-nsa` and the doubled `a` (`mertaan`).
            b.push(
                Case::Partitive,
                sg,
                Some(Possessive::Third),
                format!("{base}nsa"),
            );
            b.push(
                Case::Partitive,
                sg,
                Some(Possessive::Third),
                format!("{base}an"),
            );
        }
    } else {
        // Partitive sits on the consonant stem: kuus + ta.
        entries.push(cell(Case::Partitive, sg, &cstem, ["ta", "tä"], true));
    }

    entries.extend([
        cell(Case::Essive, sg, &estem, ["na", "nä"], true),
        // The translative's possessives stay on the e-stem: kuusekseen,
        // merekseen. The i-stem spelling is not part of the paradigm.
        cell(Case::Translative, sg, &estem, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
        cell(Case::Illative, sg, &estem, ["en", "en"], false).on_ending(["e", "e"]),
        cell(Case::Inessive, sg, &estem, ["ssa", "ssä"], true),
        cell(Case::Elative, sg, &estem, ["sta", "stä"], true),
        cell(Case::Adessive, sg, &estem, ["lla", "llä"], true),
        cell(Case::Ablative, sg, &estem, ["lta", "ltä"], true),
        cell(Case::Allative, sg, &estem, ["lle", "lle"], true),
        cell(Case::Abessive, sg, &estem, ["tta", "ttä"], true),
        // ---- the plural ----
        cell(Case::Nominative, pl, &estem, ["t", "t"], false).base_on(estem.clone(), ["", ""]),
        cell(Case::Genitive, pl, istem, ["en", "en"], false).on_ending(["e", "e"]),
    ]);

    // The consonant-stem `-ten` genitive plural. `veri` confirms only the
    // `-ien` form, so its `-ten` is a restored guess rather than a plain offer.
    let ten = cell(Case::Genitive, pl, &cstem, ["ten", "ten"], false).on_ending(["te", "te"]);
    entries.push(if lemma == "veri" { ten.as_rare() } else { ten });

    entries.extend([
        cell(Case::Partitive, pl, istem, ["a", "ä"], true),
        cell(Case::Essive, pl, istem, ["na", "nä"], true),
        // The `i`-plural sits on the `i`-final stem; the endings are truncated
        // (kuusissa, kuusilla, kuusiksi).
        cell(Case::Translative, pl, istem, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
        cell(Case::Illative, pl, istem, ["in", "in"], false).on_ending("i"),
        cell(Case::Inessive, pl, istem, ["ssa", "ssä"], true),
        cell(Case::Elative, pl, istem, ["sta", "stä"], true),
        cell(Case::Adessive, pl, istem, ["lla", "llä"], true),
        cell(Case::Ablative, pl, istem, ["lta", "ltä"], true),
        cell(Case::Allative, pl, istem, ["lle", "lle"], true),
        cell(Case::Abessive, pl, istem, ["tta", "ttä"], true),
        plain(Case::Instructive, pl, istem, ["n", "n"]),
    ]);

    b.decline(&Decl::new(entries), |case, number, poss| {
        nominal_tags(PartOfSpeech::Noun, case, number, poss)
    });
    b.comitative(istem);

    b.into_forms()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tags::Confidence;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }

    /// The class's own difference from 23: the consonant stem is used, for the
    /// singular partitive and for a second genitive plural. The sanalista
    /// confirms both halves for `kusi`, and confirms both numbers of the `-ten`
    /// plural for every word of the class.
    ///
    /// `kuusi` is *not* the test word here: it is class 27 (the `käsi` type),
    /// which gives it the doubled `kuutta`, and its compounds with this head
    /// come out both ways in the sanalista anyway -- `jalokuusta` but
    /// `hopeakuutta` -- so the class's own rule is the undivided `tta` that
    /// nine of the ten confirm.
    #[test]
    fn consonant_stem() {
        let w = words("kusi");
        assert!(w.contains(&"kusen".to_string())); // gen sg
        assert!(w.contains(&"kusta".to_string())); // part sg, consonant stem
        assert!(w.contains(&"kusien".to_string())); // gen pl, i-stem
        assert!(w.contains(&"kusten".to_string())); // and the consonant-stem twin
        assert!(w.contains(&"kusten".to_string())); // spelled the same both numbers
        for (lemma, ten) in [
            ("hiiri", "hiirten"),
            ("hiili", "hiilten"),
            ("huuli", "huulten"),
            ("meri", "merten"),
        ] {
            assert!(
                words(lemma).contains(&ten.to_string()),
                "{lemma}: {ten} missing"
            );
        }
    }

    /// `veri` reaches this module through class 26 and is the one word whose
    /// `-ten` genitive plural is not confirmed, so it is a guess and not an
    /// offer. It is the only `-t` word of the class with that shape.
    #[test]
    fn a_word_that_confirms_only_the_ien_plural_gets_ten_as_a_guess() {
        let forms = inflect("veri", true);
        let gen_pl: Vec<(&str, Confidence)> = forms
            .iter()
            .filter(|f| {
                f.readings
                    .iter()
                    .any(|t| t.case == Some(Case::Genitive) && t.number == Some(Number::Plural))
            })
            .filter(|f| f.word.ends_with("en"))
            .map(|f| (f.word.as_str(), f.confidence))
            .collect();
        assert_eq!(
            gen_pl,
            vec![
                ("verien", Confidence::Plain),
                ("verten", Confidence::Repaired)
            ],
            "one is confirmed and one is restored"
        );
        // and `kuusi` confirms both
        let kuusi = words("kuusi");
        assert!(kuusi.contains(&"kuusten".to_string()));
    }

    /// The back partitive and its whole column. `meri` is front by its only
    /// vowel but its partitive is back (`merta`), and the `t`-ending takes the
    /// third person with it: `mertansa`, not `mertänsä`.
    #[test]
    fn a_t_ending_takes_the_whole_possessive_column_into_the_back() {
        let w = words("meri");
        assert!(w.contains(&"merta".to_string()));
        assert!(w.contains(&"mertani".to_string()));
        assert!(w.contains(&"mertansa".to_string()));
        assert!(
            w.contains(&"mertaan".to_string()),
            "the doubled third person"
        );
        assert!(!w.contains(&"mertä".to_string()), "not front");
        // The rest of the paradigm stays front: the `i`-plural and the cases on
        // the `e`-stem are unaffected.
        assert!(w.contains(&"merien".to_string()));
        assert!(w.contains(&"meressä".to_string()));
        assert!(w.contains(&"mereksi".to_string()));
    }

    /// The singular translative and its possessives both ride the `e`-stem.
    /// The plural keeps the `i`-stem (`merikseen`), so the check is number-
    /// specific.
    #[test]
    fn the_singular_translative_uses_the_e_stem() {
        let singular: Vec<String> = inflect("meri", true)
            .into_iter()
            .filter(|f| {
                f.readings.iter().any(|r| {
                    r.case == Some(Case::Translative) && r.number == Some(Number::Singular)
                })
            })
            .map(|f| f.word)
            .collect();
        assert!(singular.contains(&"mereksi".to_string()));
        assert!(singular.contains(&"merekseen".to_string()));
        assert!(!singular.contains(&"merikseen".to_string()));
    }
}
