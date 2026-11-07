//! Class 10 (the koira type) inflection, written by hand as a logical engine.
//!
//! Class 10 is the `-a`/`-ä` family (`koira`, `adenooma`) whose plural stem
//! changes the final `a`/`ä` to `i` (`koira -> koiri`) rather than the class-9
//! `a -> o`. Because the plural stem ends in `i`, the plural endings are the
//! truncated ones (as in class 7): the genitive `koirien`, the partitive
//! `koiria`, the illative `koiriin`, the inessive `koirissa`. The singular is
//! the whole lemma as its stem, and the nominative plural keeps the plain stem
//! (`koirat`).
//!
//! The graded types `10*A`..`10*L` are this module too, with consonant
//! gradation on top: `inflect_graded_with` is the same grid on a weak stem and
//! the class modules are one line each, which is the shape the verb side calls
//! `paradigm(&Stems)`. The two grids are separate `Decl`s rather than one with a
//! flag, because they disagree about the plural in a way that is not a
//! difference of stem: the plain grid's whole plural is the `i`-stem, while the
//! graded one splits it, strong for the genitive/partitive/essive/illative and
//! weak for the nominative, the translative and the locals.
//!
//! What is worth naming: the graded grid crosses over twice, and both crossovers
//! are the strong-genitive rule this class exists for. The singular genitive's
//! bare is weak (`pojan`) while its possessives sit on the strong stem
//! (`pojani`), and the plural nominative is the same (`pojat` beside `poikiani`
//! from the strong `poiki`). `Base::On` is what says so.

#![allow(dead_code)]

use std::sync::OnceLock;

use crate::expand::blob::{Blob, cell as blob_cell};
use crate::kotus::{Gradation, Grade};
use crate::tags::{Case, Form, Number, PartOfSpeech};

use super::utility::{Builder, Decl, Ending, cell, nominal_tags, plain, rare};

pub const CLASS: &str = "10";

/// Graded words that do not actually gradate (rukka keeps its kk), so they run
/// through the plain non-graded paradigm.
const NON_GRADATING: &[&str] = &["rukka"];

/// Words whose doubled third-person essive singular (`-aanaan`/`-äyänä`) is
/// not backed by the reference: Voikko's dictionary confirms only the `-nsa`
/// spelling (`sopanansa`), so the doubled `sopanaan` is an unconfirmed
/// alternate and must not be offered as plain.
const NO_ESSIVE_SG_DOUBLE: &[&str] = &["sopa", "lonka", "punka"];

/// Class-10 lemmas whose reference paradigm confirms the alternative
/// `-oi-`/`-öi-` plural (`ampuja` -> `ampujoita`, `ampujoiden`). The subset is
/// lexical -- `alkaja` has no such plural and is the same class -- so it is
/// read from the reference (`tools/oracle/build_oi_plural_lemmas.py`) rather than
/// derived.
static OI_PLURAL_LEMMAS: OnceLock<Blob> = OnceLock::new();

const OI_PLURAL_LEMMAS_BIN: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/oi_plural_lemmas.bin",
));

fn takes_oi_plural(lemma: &str) -> bool {
    blob_cell(&OI_PLURAL_LEMMAS, OI_PLURAL_LEMMAS_BIN)
        .find(lemma)
        .is_some()
}

/// The alternative `-oi-`/`-öi-` plural, when the lemma has one.
///
/// Unlike the `i`-stem grid this is a full second plural: the genitive keeps
/// both `-den` and `-tten`, and the possessives hang off the `-oi-` stem
/// (`ampujoideni`, `ampujoitteni`, `ampujoihini`).
fn add_oi_plural(b: &mut Builder, stem: &str) {
    let mut letters = stem.chars();
    let Some(last) = letters.next_back() else {
        return;
    };
    let mut oi = letters.as_str().to_string();
    oi.push_str(if last == 'ä' { "öi" } else { "oi" });
    let pl = Number::Plural;
    b.decline(
        &Decl::new(vec![
            cell(Case::Genitive, pl, &oi, ["den", "den"], false).on_ending(["de", "de"]),
            cell(Case::Genitive, pl, &oi, ["tten", "tten"], false).on_ending(["tte", "tte"]),
            cell(Case::Partitive, pl, &oi, ["ta", "tä"], true),
            cell(Case::Essive, pl, &oi, ["na", "nä"], true),
            cell(Case::Translative, pl, &oi, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            cell(Case::Illative, pl, &oi, ["hin", "hin"], false).on_ending(["hi", "hi"]),
            cell(Case::Inessive, pl, &oi, ["ssa", "ssä"], true),
            cell(Case::Elative, pl, &oi, ["sta", "stä"], true),
            cell(Case::Adessive, pl, &oi, ["lla", "llä"], true),
            cell(Case::Ablative, pl, &oi, ["lta", "ltä"], true),
            cell(Case::Allative, pl, &oi, ["lle", "lle"], true),
            cell(Case::Abessive, pl, &oi, ["tta", "ttä"], true),
            plain(Case::Instructive, pl, &oi, ["n", "n"]),
        ]),
        |case, number, poss| nominal_tags(PartOfSpeech::Noun, case, number, poss),
    );
}

/// The plural stem: the final `a`/`ä` written `i` (`koira -> koiri`).
fn i_stem(stem: &str) -> String {
    let mut s = stem.to_string();
    let last = s.pop().unwrap();
    match last {
        'a' | 'ä' => s.push('i'),
        _ => s.push(last),
    }
    s
}

/// Inflect a class-10 noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    // Class 10 also carries plural-only lemmas (`käräjät`, `bänät`): the
    // citation form is the nominative plural, so decline the dropped-`t`
    // base and keep only its plural cells. `käräjä` gives `käräjät`,
    // `käräjien`, `käräjiä`, `käräjiin`; the singular cells (`käräjä`,
    // `käräjän`) are not forms of the plural-only word and must go.
    if let Some(base) = lemma.strip_suffix('t')
        && !base.is_empty()
    {
        let mut forms = inflect(base, possessives);
        forms.retain(|form| {
            form.readings
                .iter()
                .any(|reading| reading.number == Some(Number::Plural))
        });
        return forms;
    }
    let mut b = Builder::new(lemma, possessives);
    // A class-10 stem ends in `a`/`ä`, since `i_stem` maps exactly that vowel to
    // `i`. A lemma ending in `n` is therefore not a stem: the `-n` is a case
    // ending already written onto it, and the stem is the lemma without it.
    // `kahdeksan` is `kahdeksa` + `n`, and the reference wants `kahdeksaa`,
    // `kahdeksaan` and `kahdeksiin` off that stem, not `kahdeksanen` off this
    // one. Requiring what is left to end in `a`/`ä` is what holds the rule to
    // the case it was measured on: three lemmas of 2 659, all three numerals.
    let bare = lemma.strip_suffix('n').filter(|s| s.ends_with(['a', 'ä']));
    let stem = bare.unwrap_or(lemma);
    let istem = i_stem(stem);
    let (sg, pl) = (Number::Singular, Number::Plural);
    // Both illatives double the stem's own last letter, which is not known until
    // the lemma is in hand: `koiraan` and `koiriin`.
    let last = stem.chars().last().unwrap();
    let ilast = istem.chars().last().unwrap();

    let mut cells = vec![
        // ---- the singular (on the stem) ----
        cell(Case::Genitive, sg, stem, ["n", "n"], false).on_stem(),
        cell(Case::Partitive, sg, stem, ["a", "ä"], false),
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
        // ---- the plural, on the i-stem (except the nominative) ----
        // Nominative keeps the plain stem: koira -> koirat.
        cell(Case::Nominative, pl, stem, ["t", "t"], false).on_stem(),
        // The i-stem takes the truncated endings (it already ends in i).
        cell(Case::Genitive, pl, &istem, ["en", "en"], false).on_ending(["e", "e"]),
        // The short genitive plural is a guess, never offered: koirain.
        rare(Case::Genitive, pl, stem, ["in", "in"]),
        cell(Case::Partitive, pl, &istem, ["a", "ä"], true),
        cell(Case::Essive, pl, &istem, ["na", "nä"], true),
        cell(Case::Translative, pl, &istem, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
        cell(
            Case::Illative,
            pl,
            &istem,
            Ending::Given(format!("{ilast}n")),
            false,
        )
        .on_ending(Ending::Given(format!("{ilast}"))),
        cell(Case::Inessive, pl, &istem, ["ssa", "ssä"], true),
        cell(Case::Elative, pl, &istem, ["sta", "stä"], true),
        cell(Case::Adessive, pl, &istem, ["lla", "llä"], true),
        cell(Case::Ablative, pl, &istem, ["lta", "ltä"], true),
        cell(Case::Allative, pl, &istem, ["lle", "lle"], true),
        cell(Case::Abessive, pl, &istem, ["tta", "ttä"], true),
        plain(Case::Instructive, pl, &istem, ["n", "n"]),
    ];
    // The bare stem is no lemma's citation form -- `kahdeksa`, `seitsemä` and
    // `yhdeksä` appear in the reference neither as words nor as forms of
    // anything -- so where the `-n` was stripped there is no nominative to
    // offer. Nothing is lost by leaving it out: the lemma is the genitive cell.
    // The insert keeps it first, which is where it was.
    if bare.is_none() {
        cells.insert(
            0,
            cell(Case::Nominative, sg, stem, ["", ""], false).on_stem(),
        );
    }
    b.decline(&Decl::new(cells), |case, number, poss| {
        nominal_tags(PartOfSpeech::Noun, case, number, poss)
    });
    b.comitative(&istem);
    if takes_oi_plural(stem) {
        add_oi_plural(&mut b, stem);
    }

    b.into_forms()
}

/// The graded class-10 subtypes (10*A..10*L). A graded class-10 noun is an
/// `-a` word with consonant gradation on top of the `a -> i` plural stem:
/// strong = the lemma (`poika`), weak = `grad` applied (`poja`), and their
/// `i`-stems (`poiki`, `poji`).
///
/// Singular: strong feeds nominative/partitive/essive/illative; weak feeds
/// genitive/translative/locals (possessives on the strong stem). Plural: the
/// `i`-stems -- strong for genitive/partitive/essive/illative (`poikien`,
/// `poikia`), weak for nominative (`pojat`), translative, locals and
/// instructive.
pub fn inflect_graded(lemma: &str, possessives: bool, grad: Gradation) -> Vec<Form> {
    // A rare graded `-a` word may not actually gradate (rukka keeps its kk and
    // the reference lists only the strong rukkasta); route it through the plain
    // non-graded class-10 paradigm.
    if NON_GRADATING.contains(&lemma) {
        return inflect(lemma, possessives);
    }
    let weak = grad.apply(lemma, Grade::Weak);
    inflect_graded_with(lemma, possessives, &weak)
}

/// The graded class-10 paradigm for an explicit weak stem.
pub fn inflect_graded_with(lemma: &str, possessives: bool, weak: &str) -> Vec<Form> {
    let mut b = Builder::new(lemma, possessives);
    let strong = lemma;
    let i_strong = i_stem(strong);
    let i_weak = i_stem(weak);
    let (sg, pl) = (Number::Singular, Number::Plural);
    // The two illatives that double their own stem's last letter, which is not
    // known until the stems are in hand: `poikaan` and `poikiin`.
    let sit = strong.chars().last().unwrap();
    let iit = i_strong.chars().last().unwrap();
    let essive_sg_double = !NO_ESSIVE_SG_DOUBLE.contains(&lemma);

    b.decline(
        &Decl::new(vec![
            // ---- singular, strong grade ----
            cell(Case::Nominative, sg, strong, ["", ""], false).on_stem(),
            cell(Case::Partitive, sg, strong, ["a", "ä"], false),
            cell(Case::Essive, sg, strong, ["na", "nä"], essive_sg_double),
            cell(
                Case::Illative,
                sg,
                strong,
                Ending::Given(format!("{sit}n")),
                false,
            )
            .on_ending(Ending::Given(format!("{sit}"))),
            // ---- singular, weak grade ----
            // The genitive's bare is weak and its possessives are strong.
            cell(Case::Genitive, sg, weak, ["n", "n"], false).base_on(strong.to_string(), ["", ""]),
            cell(Case::Translative, sg, weak, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            cell(Case::Inessive, sg, weak, ["ssa", "ssä"], true),
            cell(Case::Elative, sg, weak, ["sta", "stä"], true),
            cell(Case::Adessive, sg, weak, ["lla", "llä"], true),
            cell(Case::Ablative, sg, weak, ["lta", "ltä"], true),
            cell(Case::Allative, sg, weak, ["lle", "lle"], true),
            cell(Case::Abessive, sg, weak, ["tta", "ttä"], true),
            // ---- plural, strong i-stem ----
            cell(Case::Genitive, pl, &i_strong, ["en", "en"], false).on_ending(["e", "e"]),
            // The archaic short genitive plural (`poikain`, `koirain`)
            // is confirmed by Omorfi/Voikko but marked archaic; accept it
            // without putting it in the suggestion set.
            rare(Case::Genitive, pl, strong, ["in", "in"]),
            cell(Case::Partitive, pl, &i_strong, ["a", "ä"], true),
            cell(Case::Essive, pl, &i_strong, ["na", "nä"], true),
            cell(
                Case::Illative,
                pl,
                &i_strong,
                Ending::Given(format!("{iit}n")),
                false,
            )
            .on_ending(Ending::Given(format!("{iit}"))),
            // ---- plural, weak i-stem ----
            // The nominative is the class's second crossover: the bare is weak
            // (`pojat`) and the possessives are strong (`poikiani`).
            cell(Case::Nominative, pl, weak, ["t", "t"], false)
                .base_on(strong.to_string(), ["", ""]),
            cell(Case::Translative, pl, &i_weak, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            cell(Case::Inessive, pl, &i_weak, ["ssa", "ssä"], true),
            cell(Case::Elative, pl, &i_weak, ["sta", "stä"], true),
            cell(Case::Adessive, pl, &i_weak, ["lla", "llä"], true),
            cell(Case::Ablative, pl, &i_weak, ["lta", "ltä"], true),
            cell(Case::Allative, pl, &i_weak, ["lle", "lle"], true),
            cell(Case::Abessive, pl, &i_weak, ["tta", "ttä"], true),
            plain(Case::Instructive, pl, &i_weak, ["n", "n"]),
        ]),
        |case, number, poss| nominal_tags(PartOfSpeech::Noun, case, number, poss),
    );
    // The comitative rides the strong i-stem, like the rest of the plural's
    // genitive end.
    b.comitative(&i_strong);

    b.into_forms()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tags::{Confidence, Possessive};

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }

    /// Every form the reference double-confirms for a `-n` numeral, in the
    /// reference's own spellings. The population is exactly these three lemmas:
    /// `parse_inflections` puts 2 659 lemmas in class 10 and three of them end in
    /// `n`, and the reference tabulates all 23 forms of each. These lists are
    /// generated from that table rather than typed, because a list of spellings
    /// typed by hand is a list that can be wrong without anything noticing:
    ///
    ///     SELECT form FROM forms
    ///      WHERE lemma = 'kahdeksan' AND ok = 'both' AND possessive = '' AND lemma <> form
    const KAHDEKSAN: &[&str] = &[
        // the plural, on the i-stem
        "kahdeksat",
        "kahdeksia",
        "kahdeksien",
        "kahdeksiin",
        "kahdeksiksi",
        "kahdeksilla",
        "kahdeksille",
        "kahdeksilta",
        "kahdeksin",
        "kahdeksina",
        "kahdeksissa",
        "kahdeksista",
        "kahdeksitta",
        // the singular, on the stem; the lemma is the genitive it already is
        "kahdeksaa",
        "kahdeksaan",
        "kahdeksaksi",
        "kahdeksalla",
        "kahdeksalle",
        "kahdeksalta",
        "kahdeksana",
        "kahdeksassa",
        "kahdeksasta",
        "kahdeksatta",
    ];

    const SEITSEMAN: &[&str] = &[
        // the plural, on the i-stem
        "seitsemät",
        "seitsemiä",
        "seitsemien",
        "seitsemiin",
        "seitsemiksi",
        "seitsemille",
        "seitsemillä",
        "seitsemiltä",
        "seitsemin",
        "seitseminä",
        "seitsemissä",
        "seitsemistä",
        "seitsemittä",
        // the singular, on the stem; the lemma is the genitive it already is
        "seitsemää",
        "seitsemään",
        "seitsemäksi",
        "seitsemälle",
        "seitsemällä",
        "seitsemältä",
        "seitsemänä",
        "seitsemässä",
        "seitsemästä",
        "seitsemättä",
    ];

    const YHDEKSAN: &[&str] = &[
        // the plural, on the i-stem
        "yhdeksät",
        "yhdeksiä",
        "yhdeksien",
        "yhdeksiin",
        "yhdeksiksi",
        "yhdeksille",
        "yhdeksillä",
        "yhdeksiltä",
        "yhdeksin",
        "yhdeksinä",
        "yhdeksissä",
        "yhdeksistä",
        "yhdeksittä",
        // the singular, on the stem; the lemma is the genitive it already is
        "yhdeksää",
        "yhdeksään",
        "yhdeksäksi",
        "yhdeksälle",
        "yhdeksällä",
        "yhdeksältä",
        "yhdeksänä",
        "yhdeksässä",
        "yhdeksästä",
        "yhdeksättä",
    ];

    /// Plural-only lemmas such as `käräjät` decline from the dropped-`t`
    /// stem, and no singular is invented.
    #[test]
    fn a_plural_only_lemma_declines_from_the_dropped_stem() {
        let w = words("käräjät");
        for want in ["käräjät", "käräjien", "käräjiä", "käräjiin", "käräjissä"] {
            assert!(w.contains(&want.to_string()), "missing {want}");
        }
        for bad in ["käräjä", "käräjän", "käräjää"] {
            assert!(!w.contains(&bad.to_string()), "invented {bad}");
        }
    }

    #[test]
    fn singular_on_stem() {
        let w = words("koira");
        assert!(w.contains(&"koiran".to_string()));
        assert!(w.contains(&"koiraa".to_string()));
        assert!(w.contains(&"koiraan".to_string()));
    }

    #[test]
    fn plural_i_stem() {
        // koira -> koiri: the oblique plural uses the a->i stem.
        let w = words("koira");
        assert!(w.contains(&"koirat".to_string())); // nom pl keeps a
        assert!(w.contains(&"koirien".to_string())); // gen pl
        assert!(w.contains(&"koiria".to_string())); // part pl
        assert!(w.contains(&"koiriin".to_string())); // illat pl
        assert!(w.contains(&"koirissa".to_string())); // inessive pl
    }

    /// The three `-n` numerals are the whole of the branch that strips a case
    /// ending off a lemma, and every form the reference lists for them comes off
    /// the stem the lemma is built from rather than off the lemma itself. This is
    /// the half that says the forms are there.
    #[test]
    fn a_numeral_whose_lemma_carries_its_genitive_declines_off_the_stem() {
        for (lemma, forms) in [
            ("kahdeksan", KAHDEKSAN),
            ("seitsemän", SEITSEMAN),
            ("yhdeksän", YHDEKSAN),
        ] {
            let w = words(lemma);
            for form in forms {
                assert!(w.contains(&form.to_string()), "{lemma} lost {form}");
            }
            // `kahdeksanen` is what the unstripped stem produces, and it is not a
            // word. This assertion is the half that has teeth.
            assert!(
                !w.iter().any(|f| f.ends_with("nen")),
                "{lemma} still treats the -n as part of the stem: {w:?}"
            );
        }
    }

    /// And the other half: the stem the `-n` was stripped from is not itself a
    /// citation form. `kahdeksa`, `seitsemä` and `yhdeksä` appear in the
    /// reference neither as lemmas nor as forms of anything, so offering one as a
    /// nominative would add non-words to the output to fix a defect about them.
    #[test]
    fn the_stem_under_a_stripped_ending_is_never_offered_as_a_word() {
        for (lemma, bare) in [
            ("kahdeksan", "kahdeksa"),
            ("seitsemän", "seitsemä"),
            ("yhdeksän", "yhdeksä"),
        ] {
            let w = words(lemma);
            assert!(
                w.contains(&lemma.to_string()),
                "{lemma} must still be a form of itself"
            );
            assert!(
                !w.contains(&bare.to_string()),
                "{bare} is not a word, and {lemma} offered it"
            );
        }
    }

    /// The graded grid is the same shape on a weak stem, and both crossovers
    /// are visible in one word: `poika`'s genitive is `pojan` but `poikani`, and
    /// its nominative plural is `pojat` but `poikiani`. The possessives ride
    /// the strong stem in both, which is the whole of what the class does.
    #[test]
    fn the_graded_grid_crosses_over_twice() {
        let w: Vec<String> = inflect_graded_with("poika", true, "poja")
            .into_iter()
            .map(|f| f.word)
            .collect();
        assert!(w.contains(&"pojan".to_string())); // gen sg bare: weak
        assert!(w.contains(&"poikani".to_string())); // gen sg 1sg: strong
        assert!(w.contains(&"pojat".to_string())); // nom pl bare: weak
        assert!(w.contains(&"poikiani".to_string())); // nom pl 1sg: strong
        assert!(w.contains(&"poikien".to_string())); // gen pl: strong i-stem
        assert!(w.contains(&"pojissa".to_string())); // iness pl: weak i-stem
    }

    /// `rukka` is filed as graded but keeps its `kk`, so it runs the plain
    /// paradigm: no weak `ruk` anywhere, and the whole plural on the `i`-stem.
    #[test]
    fn a_word_that_does_not_actually_gradate_runs_the_plain_grid() {
        let w: Vec<String> = inflect_graded("rukka", true, Gradation(1))
            .into_iter()
            .map(|f| f.word)
            .collect();
        assert!(
            w.contains(&"rukkat".to_string()),
            "the nominative plural is missing"
        );
        assert!(w.contains(&"rukkien".to_string()));
        assert!(!w.iter().any(|f| f.starts_with("ruj")), "{:?}", w);
    }

    /// The alternative `-oi-` plural is lexical, not derivable.
    ///
    /// `ampuja` has it, `alkaja` does not, and both are class 10; `suoja` is
    /// confirmed only on Omorfi's side, so it is not in the `both` list. The
    /// whole second plural must be emitted, including the possessives.
    #[test]
    fn the_oi_plural_is_read_from_the_reference() {
        let ampuja = words("ampuja");
        for want in [
            "ampujoita",
            "ampujoiden",
            "ampujoitten",
            "ampujoilla",
            "ampujoihin",
            "ampujoinaan",
            "ampujoideni",
            "ampujoitteni",
            "ampujoihini",
        ] {
            assert!(ampuja.contains(&want.to_string()), "{want} missing");
        }
        assert!(ampuja.contains(&"ampujia".to_string()), "the i-stem remains");
        assert!(
            !words("alkaja").iter().any(|f| f.starts_with("alkajoi")),
            "alkaja has no -oi- plural in the reference"
        );
        assert!(
            !words("suoja").iter().any(|f| f.starts_with("suojoi")),
            "suoja is omorfi-only, not a both row"
        );
    }

    /// The doubled third-person essive singular is the one cell of the graded
    /// grid that is per word: the reference confirms `sopanansa` and not
    /// `sopanaan` for `sopa`, `lonka` and `punka`, so those three get only the
    /// `-nsa` spelling and every other word of the class gets both.
    #[test]
    fn the_doubled_essive_singular_is_per_word() {
        let doubled = |lemma: &str| {
            inflect_graded_with(lemma, true, lemma)
                .into_iter()
                .filter(|f| {
                    f.readings.iter().any(|t| {
                        t.case == Some(Case::Essive)
                            && t.number == Some(Number::Singular)
                            && t.possessive == Some(Possessive::Third)
                    })
                })
                .map(|f| f.word)
                .collect::<Vec<String>>()
        };
        for lemma in NO_ESSIVE_SG_DOUBLE {
            let forms = doubled(lemma);
            // The essive singular's own `-na`, so the third person's `-nsa`
            // lands on it: `sopana` -> `sopanansa`.
            let expected = format!("{lemma}nansa");
            assert_eq!(forms, vec![expected], "{lemma}: only the -nsa spelling");
        }
        assert_eq!(
            doubled("koira").len(),
            2,
            "a word the reference confirms gets both"
        );
    }
}
