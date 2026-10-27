//! Class 49 (the askar type) inflection, written by hand as a logical engine.
//!
//! Class 49 is the short consonant family (askar, askel) whose oblique stem
//! doubles the final vowel (`askar -> askaree`, `askel -> askelee`) and takes
//! both a single-`e` and a double-`e` spelling (`askelen`/`askareen`) -- the
//! single carries the genitive/nominative-plural/genitive-plural, the double
//! the rest. The lemma keeps the partitive (`askarta`).
//!
//! The single-`e` half is a second `decline` pass on top of class 41's
//! [`a_stem_inflect`], because the class spells the genitive singular, the
//! nominative plural, the genitive plural and the singular obliques on the
//! short `e`-stem rather than the long `ee`-stem the first pass builds. Several
//! cells are emitted twice and the two passes are merged with a first-wins
//! dedup, which is the class's own way of saying "the double-`e` shape is the
//! default and these are the exceptions".
//!
//! [`inflect_with`] is the shared engine and class 49*J is its other user.

#![allow(dead_code)]

use crate::kotus::Gradation;
use crate::tags::{Case, Form, Number, PartOfSpeech};

use super::class_41::a_stem_inflect;
use super::utility::{Builder, Decl, Entry, cell, nominal_tags, plain};

pub const CLASS: &str = "49";

/// The `49*D` oblique base: the weak grade deletes the `k` (`säen`), so the
/// strong grade has to put it back before the final vowel (`säken`). The other
/// gradation types are reconstructible from their weak pair and use
/// [`Gradation::strongen`].
fn strong_d(lemma: &str) -> String {
    if lemma.contains('k') {
        return lemma.to_string();
    }
    let Some((at, _)) = lemma
        .char_indices()
        .rev()
        .find(|(_, c)| "aeiouyäö".contains(*c))
    else {
        return lemma.to_string();
    };
    format!("{}k{}", &lemma[..at], &lemma[at..])
}

/// Inflect one of the class's `*X` gradation variants on its strong stem.
///
/// The class's `*D`-`*J` subclasses are the same paradigm with a different
/// consonant alternation (`säen/säken`, `taival/taipal`, `udar/utar`,
/// `penger/penker`, `ommel/ompel`, `kannel/kantel`). The generated per-class
/// files each carried one guess per cell; this runs the one class-49 engine
/// instead, so every member gets the same two `e`-stem spellings
/// (`ompelen`/`ompeleen`) rather than one.
pub fn inflect_graded(lemma: &str, possessives: bool, grade: Gradation) -> Vec<Form> {
    let strong = if grade.0 == 4 {
        strong_d(lemma)
    } else {
        grade.strongen(lemma)
    };
    inflect_with(lemma, &strong, possessives)
}

/// A class 49*D noun (`säen`): `k`-deletion gradation.
pub fn inflect_d(lemma: &str, possessives: bool) -> Vec<Form> {
    inflect_graded(lemma, possessives, Gradation(4))
}

/// A class 49*E noun (`taival`): `p`/`v`.
pub fn inflect_e(lemma: &str, possessives: bool) -> Vec<Form> {
    inflect_graded(lemma, possessives, Gradation(5))
}

/// A class 49*F noun (`udar`): `t`/`d`.
pub fn inflect_f(lemma: &str, possessives: bool) -> Vec<Form> {
    inflect_graded(lemma, possessives, Gradation(6))
}

/// A class 49*G noun (`penger`): `nk`/`ng`.
pub fn inflect_g(lemma: &str, possessives: bool) -> Vec<Form> {
    inflect_graded(lemma, possessives, Gradation(7))
}

/// The class members whose oblique base is not the nominative plus an `e`.
/// `auer` is the one: its paradigm is the e-final `autere` with the `t` in
/// every oblique (`auteren`, `auteressa`), and only the nominative and
/// partitive keep the short `auer`/`auerta`. The old `inflect(lemma, lemma)`
/// invented `auere` and `aueren`, which live Voikko rejects.
const BASES: &[(&str, &str)] = &[("auer", "auter")];

/// Inflect a class-49 noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    let base = BASES
        .iter()
        .find_map(|(word, base)| (*word == lemma).then_some(*base))
        .unwrap_or(lemma);
    inflect_with(lemma, base, possessives)
}

/// The class-49 paradigm with an explicit oblique base. For class 49 the base
/// is the lemma (`askar`); for class 49*J it is the gradated stem
/// (`kannel` -> `kantel`), which every oblique uses while the nominative and
/// partitive singular keep the lemma (`kannel`, `kannelta`).
pub fn inflect_with(lemma: &str, base: &str, possessives: bool) -> Vec<Form> {
    let last = base.chars().last().unwrap();
    let (long, short): (String, String) = if "aeiouyäö".contains(last) {
        (format!("{base}{last}"), base.to_string()) // askare -> askaree
    } else {
        (format!("{base}ee"), format!("{base}e")) // askar -> askaree / askare
    };
    // The double-`e` oblique paradigm ...
    let mut out = a_stem_inflect(lemma, possessives, &long, &short, false);
    // ... plus the single-`e` cells, which the catalogue spells on the base
    // (askelen, askelet, askelien) and which the dedup below lets the first
    // pass's forms win where the two agree.
    let gp = if "aeiouyäö".contains(last) {
        format!("{}ien", &base[..base.len() - last.len_utf8()])
    } else {
        format!("{base}ien")
    };
    let gp_stem = &gp[..gp.len() - 3]; // drop `ien`, keep the possessive base `ie`
    let (sg, pl) = (Number::Singular, Number::Plural);
    let mut b = Builder::new(lemma, possessives);

    let mut decl = Decl::new(vec![
            cell(Case::Nominative, sg, &short, ["", ""], false).on_stem(),
            cell(Case::Genitive, sg, base, ["en", "en"], false).base_on(short.clone(), ["", ""]),
            cell(Case::Nominative, pl, base, ["et", "et"], false).base_on(short.clone(), ["", ""]),
            // The genitive plural's `-n` gives way to the owner suffix
            // (`askarien` -> `askariensa`), so its base drops the `n`.
            cell(Case::Genitive, pl, gp_stem, ["ien", "ien"], false).on_ending(["ie", "ie"]),
            // The lemma-based `-ten` genitive plural (auerten, kannelten).
            cell(Case::Genitive, pl, lemma, ["ten", "ten"], false).on_ending(["te", "te"]),
            // The base-based i-plural obliques of the askel catalogue model.
            cell(Case::Partitive, pl, base, ["ia", "iä"], true),
            cell(Case::Illative, pl, base, ["iin", "iin"], false).on_ending(["ii", "ii"]),
            cell(Case::Inessive, pl, base, ["issa", "issä"], true),
            cell(Case::Elative, pl, base, ["ista", "istä"], true),
            cell(Case::Adessive, pl, base, ["illa", "illä"], true),
            cell(Case::Ablative, pl, base, ["ilta", "iltä"], true),
            cell(Case::Allative, pl, base, ["ille", "ille"], true),
            cell(Case::Essive, pl, base, ["ina", "inä"], true),
            // The translative's owner suffix sits on the `-kse` stem
            // (`kanteliksi` -> `kantelikseni`).
            cell(Case::Translative, pl, base, ["iksi", "iksi"], true).on_ending(["ikse", "ikse"]),
            cell(Case::Abessive, pl, base, ["itta", "ittä"], true),
            plain(Case::Instructive, pl, base, ["in", "in"]),
            // The short-stem singular cells (askareksi, askarella) and the
            // short partitive.
            cell(Case::Partitive, sg, &short, ["tta", "ttä"], true).on_stem(),
            cell(Case::Translative, sg, &short, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            cell(Case::Inessive, sg, &short, ["ssa", "ssä"], true),
            cell(Case::Essive, sg, &short, ["na", "nä"], true),
            cell(Case::Elative, sg, &short, ["sta", "stä"], true),
            cell(Case::Adessive, sg, &short, ["lla", "llä"], true),
            cell(Case::Ablative, sg, &short, ["lta", "ltä"], true),
            cell(Case::Allative, sg, &short, ["lle", "lle"], true),
            cell(Case::Abessive, sg, &short, ["tta", "ttä"], true),
            // Repeated on purpose: the class emits the `-ten` genitive plural
            // once here as well, and the dedup keeps the first spelling.
            cell(Case::Genitive, pl, lemma, ["ten", "ten"], false).on_ending(["te", "te"]),
            plain(Case::Instructive, pl, &short, ["in", "in"]),
        ]);
    // A vowel-final base (`hepene`, `autere`) takes the i-plural genitive
    // (`hepeneiden`/`hepeneitten`); `lemma + ten` would build `hepeneten`.
    // The consonant-final `askel`/`kannel`/`auer` keep their `-ten` spelling
    // (`askelten`, `kannelten`, `auerten`).
    if "aeiouyäö".contains(last) {
        decl.entries.retain(|entry| {
            !matches!(entry, Entry::Cell(c)
                if (c.case == Case::Genitive && c.number == pl && c.stem == lemma)
                    || (c.case == Case::Partitive && c.number == pl && c.stem == base)
                    || (c.case == Case::Illative && c.number == pl && c.stem == base))
        });
    }
    b.decline(
        &decl,
        |case, number, poss| nominal_tags(PartOfSpeech::Noun, case, number, poss),
    );
    out.extend(b.into_forms());
    let mut seen = std::collections::HashSet::new();
    out.retain(|f| seen.insert(f.word.clone()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }

    #[test]
    fn askar() {
        let w = words("askar");
        assert!(w.contains(&"askareen".to_string()));
        assert!(w.contains(&"askarta".to_string()));
        assert!(w.contains(&"askareiden".to_string()));
    }

    /// `auer` is the one member whose oblique base is `auter`, not `auer`:
    /// live Voikko accepts `auteren`/`auteressa` and rejects `aueren`/`auere`.
    #[test]
    fn auer_takes_the_t_stem() {
        let w = words("auer");
        for word in ["auer", "auerta", "auteren", "autere", "autereen"] {
            assert!(w.contains(&word.to_string()), "auer is missing {word}");
        }
        for wrong in ["aueren", "auere", "auereen"] {
            assert!(
                !w.contains(&wrong.to_string()),
                "auer must not offer *{wrong}"
            );
        }
    }

    /// An e-final class-49 word takes the i-plural genitive, not `lemma + ten`.
    #[test]
    fn vowel_final_base_has_no_ten_genitive() {
        let w = words("hepene");
        for good in ["hepeneiden", "hepeneitten", "hepenien"] {
            assert!(w.contains(&good.to_string()), "hepene is missing {good}");
        }
        assert!(
            !w.contains(&"hepeneten".to_string()),
            "hepene must not offer *hepeneten"
        );
        for good in ["hepeneisiin", "hepeneihin", "hepeneitä"] {
            assert!(w.contains(&good.to_string()), "hepene is missing {good}");
        }
        for bad in ["hepeneiin", "hepeneiä"] {
            assert!(!w.contains(&bad.to_string()), "hepene must not offer *{bad}");
        }
    }

    #[test]
    fn single_e() {
        let w = words("askel");
        assert!(w.contains(&"askelen".to_string()));
        assert!(w.contains(&"askelet".to_string()));
        assert!(w.contains(&"askelien".to_string()));
    }

    /// The two passes overlap and the first wins: the double-`e` partitive
    /// (`askaretta`) is emitted by the first pass and survives the dedup, while
    /// the single-`e` genitive (`askelen`) is only in the second.
    #[test]
    fn the_two_passes_dedup() {
        let w = words("askel");
        let count = w.iter().filter(|x| *x == "askelen").count();
        assert_eq!(count, 1, "askelen appears {count} times");
        assert!(w.contains(&"askeleeseen".to_string()) || w.contains(&"askeleesen".to_string()));
    }
}
