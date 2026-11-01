//! Class 9 (the aatra/kala type) inflection, written by hand as a logical
//! engine.
//!
//! Base class 9 is the non-gradating `-a`/`-ä` family (`aatra`, `aava`). The
//! singular is the whole lemma as its stem, like class 1. The plural changes
//! the stem's final `a`/`ä` to `o`/`ö` (`aatra -> aatro`, `lähde`-style) for
//! every oblique plural cell -- the genitive `aatrojen`, the partitive
//! `aatroja`, the local cases `aatroissa` -- while the nominative plural keeps
//! the plain stem (`aatrat`).
//!
//! The graded types `9*A`..`9*K` are this module too, with consonant gradation
//! on top: `inflect_graded_impl` is the same grid on a weak stem and the class
//! modules are one line each. The graded grid has three entry points rather
//! than two, because a graded `-a` word's confirmed plural is on the strong
//! `o`-stem for most of the class (`akka` -> `akkojen`), on the weak `o`-stem
//! for two loans (`estetiikka` -> `estetiikoita`), and on both for the class-14
//! family (`allakka` -> `allakkojen` and `allakoiden`).
//!
//! What is worth naming: the short genitive plural of the graded grid is
//! emitted *after* the comitative, which is where the old code put it, and a
//! form that comes after the comitative cannot be one `Decl` -- so it is a
//! second `decline` pass. Sorting it into the grid would change what the engine
//! emits, which is observable; two passes do not.

#![allow(dead_code)]

use crate::kotus::{Gradation, Grade};
use crate::tags::{Case, Form, Number, PartOfSpeech};

use super::utility::{Builder, Decl, Ending, cell, nominal_tags, plain, plural_only, rare};

pub const CLASS: &str = "9";

/// The plural stem: the final `a`/`ä` written `o`/`ö` (`aatra -> aatro`).
fn o_stem(stem: &str) -> String {
    let mut s = stem.to_string();
    let last = s.pop().unwrap();
    match last {
        'a' => s.push('o'),
        'ä' => s.push('ö'),
        _ => s.push(last),
    }
    s
}

/// Class-9 graded words whose whole plural is on the weak o-stem: loanwords
/// such as `estetiikka`, whose confirmed plural is `estetiikoita`,
/// `estetiikoihin`, `estetiikoiden` -- the i-plural throughout. Every other
/// gradating `-a` word (akka, aitta) has its confirmed plural on the strong
/// o-stem (akkojen, akkoina) and the weak-o-stem forms are a rare alternant.
const I_PLURAL_LOANS: &[&str] = &["estetiikka", "solakka"];

/// Inflect a class-9 noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    // Class 9 carries plural-only lemmas (`perskat`, `tangat`, `kihlat`): the
    // citation form is the nominative plural of a base the sanalista does not
    // list (`perska`, `tanka`, `kihla`), so decline the dropped-`t` stem and
    // keep only its plural cells. Without this the engine builds `perskatn`,
    // `perskata` and `perskatt`, none of which is a word.
    if let Some(base) = lemma.strip_suffix('t')
        && !base.is_empty()
    {
        return plural_only(inflect(base, possessives));
    }
    let mut b = Builder::new(lemma, possessives);
    let stem = lemma;
    let ostem = o_stem(stem);
    let (sg, pl) = (Number::Singular, Number::Plural);
    // The singular illative doubles the stem's own last letter, which is not
    // known until the lemma is in hand: `aatraan`.
    let last = stem.chars().last().unwrap();

    b.decline(
        &Decl::new(vec![
            // ---- the singular (on the stem) ----
            cell(Case::Nominative, sg, stem, ["", ""], false).on_stem(),
            cell(Case::Genitive, sg, stem, ["n", "n"], false).on_stem(),
            cell(Case::Partitive, sg, stem, ["a", "ä"], true),
            cell(Case::Essive, sg, stem, ["na", "nä"], true),
            cell(Case::Translative, sg, stem, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            cell(Case::Inessive, sg, stem, ["ssa", "ssä"], true),
            cell(Case::Elative, sg, stem, ["sta", "stä"], true),
            cell(
                Case::Illative,
                sg,
                stem,
                Ending::Given(format!("{last}n")),
                false,
            )
            .on_ending(Ending::Given(format!("{last}"))),
            cell(Case::Adessive, sg, stem, ["lla", "llä"], true),
            cell(Case::Ablative, sg, stem, ["lta", "ltä"], true),
            cell(Case::Allative, sg, stem, ["lle", "lle"], true),
            cell(Case::Abessive, sg, stem, ["tta", "ttä"], true),
            // ---- the plural (the o-stem, except the nominative) ----
            // Nominative keeps the plain stem: aatra -> aatrat.
            cell(Case::Nominative, pl, stem, ["t", "t"], false).on_stem(),
            cell(Case::Genitive, pl, &ostem, ["jen", "jen"], false).on_ending(["je", "je"]),
            // The short genitive plural is a rare lexicalised reading: generated
            // for the catalogue model (kala -> kalain) but never offered plain.
            rare(Case::Genitive, pl, stem, ["in", "in"]),
            cell(Case::Partitive, pl, &ostem, ["ja", "jä"], true),
            cell(Case::Essive, pl, &ostem, ["ina", "inä"], true),
            cell(Case::Translative, pl, &ostem, ["iksi", "iksi"], true).on_ending(["ikse", "ikse"]),
            cell(Case::Inessive, pl, &ostem, ["issa", "issä"], true),
            cell(Case::Elative, pl, &ostem, ["ista", "istä"], true),
            cell(Case::Illative, pl, &ostem, ["ihin", "ihin"], false).on_ending(["ihi", "ihi"]),
            cell(Case::Adessive, pl, &ostem, ["illa", "illä"], true),
            cell(Case::Ablative, pl, &ostem, ["ilta", "iltä"], true),
            cell(Case::Allative, pl, &ostem, ["ille", "ille"], true),
            cell(Case::Abessive, pl, &ostem, ["itta", "ittä"], true),
            plain(Case::Instructive, pl, &ostem, ["in", "in"]),
        ]),
        |case, number, poss| nominal_tags(PartOfSpeech::Noun, case, number, poss),
    );
    b.comitative(&ostem);

    b.into_forms()
}

/// The graded class-9 subtypes (9*A..9*K). A graded class-9 noun is an `-a`
/// word with consonant gradation on top of the `a -> o` plural stem: the strong
/// stem is the lemma (`ilta`), the weak stem is `grad` applied (`illa`), and
/// their `o`-stems are `ilto`/`illo`.
///
/// Singular: strong feeds nominative/partitive/essive/illative; weak feeds
/// genitive (`illan`)/translative/locals, with the genitive's possessives on
/// the strong stem (`iltani`). Plural: the `o`-stems -- strong for genitive/
/// partitive/essive/illative (`iltojen`, `iltoja`), weak for the nominative
/// (`illat`), translative, locals and instructive.
pub fn inflect_graded(lemma: &str, possessives: bool, grad: Gradation) -> Vec<Form> {
    // A plural-only lemma (`menkat`, `hipat`, `harkat`) is the nominative
    // plural of a base the sanalista does not list. Dropping the `-t` leaves the
    // *weak* grade (`menka`, `hipa`), and this engine takes the strong stem
    // (`menkka`, `hippa`), so turn it back before declining.
    if let Some(base) = lemma.strip_suffix('t')
        && !base.is_empty()
    {
        let strong = grad.strongen(base);
        return plural_only(inflect_graded(&strong, possessives, grad));
    }
    let weak = grad.apply(lemma, Grade::Weak);
    inflect_graded_impl(lemma, possessives, &weak, false)
}

/// A graded class-9 noun confirmed with both the strong vowel-plural and the
/// weak `i`-plural (the class-14 `-a` family: `allakka` confirms both
/// `allakkojen`/`allakkoja` and `allakoiden`/`allakoita`). The weak `i`-plural
/// is offered plainly alongside the strong vowel-plural.
pub fn inflect_graded_both(lemma: &str, possessives: bool, grad: Gradation) -> Vec<Form> {
    // The class-14 plural-only lemmas (`rintsikat`) reach here too; see
    // [`inflect_graded`] for why the weak stem is strong-graded first.
    if let Some(base) = lemma.strip_suffix('t')
        && !base.is_empty()
    {
        let strong = grad.strongen(base);
        return plural_only(inflect_graded_both(&strong, possessives, grad));
    }
    let weak = grad.apply(lemma, Grade::Weak);
    inflect_graded_impl(lemma, possessives, &weak, true)
}

/// The graded class-9 paradigm for an explicit weak stem, so a caller with an
/// irregular weak form (aika -> aja, contracting the `-aika`) can supply it.
pub fn inflect_graded_with(lemma: &str, possessives: bool, weak: &str) -> Vec<Form> {
    inflect_graded_impl(lemma, possessives, weak, false)
}

fn inflect_graded_impl(lemma: &str, possessives: bool, weak: &str, both: bool) -> Vec<Form> {
    let mut b = Builder::new(lemma, possessives);
    let strong = lemma;
    let o_strong = o_stem(strong);
    let o_weak = o_stem(weak);
    let (sg, pl) = (Number::Singular, Number::Plural);
    // The singular illative doubles the strong stem's own last letter: iltaan.
    let last = strong.chars().last().unwrap();

    // The `i`-plural alternant on the weak o-stem. For the i-plural loanwords
    // (estetiikka) and the both-mode class-14 words it is the word's real
    // plural -- estetiikoiden, allakoiden, allakoita -- offered plain. For
    // every ordinary gradating `-a` word (akka, aitta) the confirmed plural is
    // on the strong o-stem (akkojen, akkoina), so the weak-o-stem spelling is a
    // rare alternant, never a plain correction the reference does not vouch
    // for. Note the two branches are not the same shapes: the plain branch is
    // five full cells and the rare one three bare forms, with no possessive
    // column on them at all.
    let i_plain = both || I_PLURAL_LOANS.contains(&lemma);

    let mut entries = vec![
        // ---- singular, strong grade ----
        cell(Case::Nominative, sg, strong, ["", ""], false).on_stem(),
        // The partitive singular takes only the `-nsa` third person: the doubled
        // `-Vn` (`allakkaaa+n`) is not confirmed for any graded `-a` word.
        cell(Case::Partitive, sg, strong, ["a", "ä"], false),
        cell(Case::Essive, sg, strong, ["na", "nä"], true),
        cell(
            Case::Illative,
            sg,
            strong,
            Ending::Given(format!("{last}n")),
            false,
        )
        .on_ending(Ending::Given(format!("{last}"))),
        // ---- singular, weak grade ----
        // Genitive bare is weak (illan); its possessives sit on the strong stem.
        cell(Case::Genitive, sg, weak, ["n", "n"], false).base_on(strong.to_string(), ["", ""]),
        cell(Case::Translative, sg, weak, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
        cell(Case::Inessive, sg, weak, ["ssa", "ssä"], true),
        cell(Case::Elative, sg, weak, ["sta", "stä"], true),
        cell(Case::Adessive, sg, weak, ["lla", "llä"], true),
        cell(Case::Ablative, sg, weak, ["lta", "ltä"], true),
        cell(Case::Allative, sg, weak, ["lle", "lle"], true),
        cell(Case::Abessive, sg, weak, ["tta", "ttä"], true),
        // ---- plural, strong o-stem ----
        cell(Case::Genitive, pl, &o_strong, ["jen", "jen"], false).on_ending(["je", "je"]),
        cell(Case::Partitive, pl, &o_strong, ["ja", "jä"], true),
        cell(Case::Essive, pl, &o_strong, ["ina", "inä"], true),
        cell(Case::Illative, pl, &o_strong, ["ihin", "ihin"], false).on_ending(["ihi", "ihi"]),
    ];
    entries.extend(if i_plain {
        vec![
            cell(Case::Genitive, pl, &o_weak, ["iden", "iden"], false).on_ending(["ide", "ide"]),
            cell(Case::Genitive, pl, &o_weak, ["itten", "itten"], false)
                .on_ending(["itte", "itte"]),
            cell(Case::Partitive, pl, &o_weak, ["ita", "itä"], true),
            cell(Case::Essive, pl, &o_weak, ["ina", "inä"], true),
            cell(Case::Illative, pl, &o_weak, ["ihin", "ihin"], false).on_ending(["ihi", "ihi"]),
        ]
    } else {
        vec![
            rare(Case::Genitive, pl, &o_weak, ["iden", "iden"]),
            rare(Case::Genitive, pl, &o_weak, ["itten", "itten"]),
            rare(Case::Partitive, pl, &o_weak, ["ita", "itä"]),
        ]
    });
    entries.extend(vec![
        // ---- plural, weak o-stem ----
        // Nominative is the plain weak stem + t: ilta -> illat, and its
        // possessives sit on the strong stem.
        cell(Case::Nominative, pl, weak, ["t", "t"], false).base_on(strong.to_string(), ["", ""]),
        cell(Case::Translative, pl, &o_weak, ["iksi", "iksi"], true).on_ending(["ikse", "ikse"]),
        cell(Case::Inessive, pl, &o_weak, ["issa", "issä"], true),
        cell(Case::Elative, pl, &o_weak, ["ista", "istä"], true),
        cell(Case::Adessive, pl, &o_weak, ["illa", "illä"], true),
        cell(Case::Ablative, pl, &o_weak, ["ilta", "iltä"], true),
        cell(Case::Allative, pl, &o_weak, ["ille", "ille"], true),
        cell(Case::Abessive, pl, &o_weak, ["itta", "ittä"], true),
        plain(Case::Instructive, pl, &o_weak, ["in", "in"]),
    ]);

    b.decline(&Decl::new(entries), |case, number, poss| {
        nominal_tags(PartOfSpeech::Noun, case, number, poss)
    });
    b.comitative(&o_strong);
    // The short genitive plural comes after the comitative, so it is a second
    // pass rather than an entry in the first: it is rare, never offered
    // (solakkain).
    b.decline(
        &Decl::new(vec![rare(Case::Genitive, pl, strong, ["in", "in"])]),
        |case, number, poss| nominal_tags(PartOfSpeech::Noun, case, number, poss),
    );

    b.into_forms()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tags::Confidence;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }

    /// A plural-only lemma declines from the dropped-`t` base and invents no
    /// singular.
    ///
    /// `perskat` is the nominative plural of `perska`; the base is not a
    /// sanalista headword, so the engine used to treat the plural as the
    /// nominative singular and build `perskatn`, `perskata`, `perskatt`. The
    /// graded subtypes need the strong stem back (`hipat` -> `hippa`,
    /// `menkat` -> `menkka`), which `Gradation::strongen` supplies.
    #[test]
    fn a_plural_only_lemma_declines_from_the_dropped_stem() {
        // Class 9 (ungraded): `perskat` is the plural of `perska`.
        let perskat = words("perskat");
        for want in ["perskat", "perskojen", "perskoja", "perskoissa"] {
            assert!(
                perskat.contains(&want.to_string()),
                "perskat missing {want}"
            );
        }
        for bad in ["perskatn", "perskata", "perskatt"] {
            assert!(
                !perskat.contains(&bad.to_string()),
                "perskat invented {bad}"
            );
        }

        // The graded subtypes take the strong stem back: `hipat` (9*B) is the
        // plural of `hippa`, and `menkat` (9*A) of `menkka`.
        for (lemma, grad, wants) in [
            (
                "hipat",
                Gradation(2),
                ["hipat", "hippojen", "hippoja", "hipoissa"],
            ),
            (
                "menkat",
                Gradation(1),
                ["menkat", "menkkojen", "menkkoja", "menkkoihin"],
            ),
        ] {
            let w: Vec<String> = inflect_graded(lemma, true, grad)
                .into_iter()
                .map(|f| f.word)
                .collect();
            for want in wants {
                assert!(w.contains(&want.to_string()), "{lemma} missing {want}");
            }
            let base = lemma.strip_suffix('t').unwrap();
            for bad in [
                format!("{lemma}n"),
                format!("{base}ta"),
                format!("{lemma}t"),
            ] {
                assert!(!w.contains(&bad), "{lemma} invented {bad}");
            }
        }
    }

    #[test]
    fn singular_on_stem() {
        let w = words("aatra");
        assert!(w.contains(&"aatran".to_string()));
        assert!(w.contains(&"aatraa".to_string()));
        assert!(w.contains(&"aatraan".to_string()));
    }

    #[test]
    fn plural_o_stem() {
        // aatra -> aatro: the oblique plural uses the a->o stem.
        let w = words("aatra");
        assert!(w.contains(&"aatrat".to_string())); // nom pl keeps a
        assert!(w.contains(&"aatrojen".to_string())); // gen pl, o-stem
        assert!(w.contains(&"aatroja".to_string())); // part pl
        assert!(w.contains(&"aatroissa".to_string())); // iness pl
    }

    #[test]
    fn graded_strong_weak() {
        // ilta is 9*I (lt -> ll): weak feeds genitive (illan), strong the
        // partitive/illative; the o-stems carry the plural.
        let w = inflect_graded("ilta", true, Gradation(9));
        let ws: Vec<&str> = w.iter().map(|f| f.word.as_str()).collect();
        assert!(ws.contains(&"illan")); // gen, weak
        assert!(ws.contains(&"iltaa")); // part, strong
        assert!(ws.contains(&"iltaan")); // illat, strong
        assert!(ws.contains(&"illat")); // nom pl, weak
        assert!(ws.contains(&"iltojen")); // gen pl, strong o-stem
        assert!(ws.contains(&"iltoja")); // part pl
        assert!(ws.contains(&"illoissa")); // inessive pl, weak o-stem
    }

    #[test]
    fn aika_contracts_weak() {
        // aika -> aja (the -aika head contracts ai+j to aj): 9*D feeds this weak.
        let w = inflect_graded_with("aika", true, "aja");
        let ws: Vec<&str> = w.iter().map(|f| f.word.as_str()).collect();
        assert!(ws.contains(&"ajan"));
        assert!(ws.contains(&"ajaksi"));
    }

    /// Which of the two plural stems carries the word's confirmed plural, and
    /// that is the class's own three-way split: `akka` is on the strong o-stem,
    /// the two `I_PLURAL_LOANS` on the weak one, and the class-14 family on
    /// both. The rare branch is three bare forms with no possessive column, so
    /// the difference is visible in the count as well as the confidence.
    #[test]
    fn the_confirmed_plural_is_the_stem_the_reference_gives() {
        let plain = |forms: Vec<Form>, ending: &str| {
            forms
                .into_iter()
                .filter(|f| f.confidence == Confidence::Plain && f.word.ends_with(ending))
                .count()
        };
        let akka = inflect_graded("akka", true, Gradation(2));
        assert_eq!(
            plain(akka.clone(), "oiden"),
            0,
            "akka's plural is on the strong stem"
        );
        assert_eq!(plain(akka, "ojen"), 1);
        for lemma in I_PLURAL_LOANS {
            let forms = inflect_graded_with(lemma, true, lemma);
            assert_eq!(
                plain(forms.clone(), "oiden"),
                1,
                "{lemma}: the loan's plural"
            );
            assert_eq!(plain(forms, "ojen"), 1, "{lemma}: and the strong one too");
        }
        let allakka = inflect_graded_both("allakka", true, Gradation(2));
        assert_eq!(plain(allakka.clone(), "oiden"), 1);
        assert_eq!(plain(allakka, "ojen"), 1);
    }

    /// The short genitive plural of the graded grid is a bare rare guess, and it
    /// is emitted last -- after the comitative, which is where the engine has
    /// always put it and which is why it is a second `Decl` rather than an entry
    /// in the first.
    #[test]
    fn the_short_genitive_plural_comes_after_the_comitative() {
        let forms = inflect_graded("ilta", true, Gradation(9));
        let short = forms
            .iter()
            .position(|f| f.word == "iltan" || f.word == "iltain")
            .expect("the short genitive plural");
        let comitative = forms
            .iter()
            .position(|f| f.readings.iter().any(|t| t.case == Some(Case::Comitative)))
            .expect("a comitative");
        assert!(
            short > comitative,
            "short at {short}, comitative at {comitative}"
        );
        let short = &forms[short];
        assert_eq!(short.confidence, Confidence::Repaired);
        assert!(short.readings.iter().all(|t| t.possessive.is_none()));
    }
}
