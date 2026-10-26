//! Class 45 (the kymmenes type) inflection, written by hand as a logical
//! engine.
//!
//! The ordinal `-es` family (kahdeksas, kahdes, kolmas, viides, sadas,
//! kymmenes, tuhannes). Most of it is the ordinal shape: `kahdeksas ->
//! kahdeksannen`, the tail `-s` becoming `-nnen`/`-nne`/`-nte`, with the
//! partitive on the lemma (`kahdeksatta`) and the plural on the `-ns` stem
//! (`kahdeksansien`, `kahdeksansia`). Two words, `kymmenes` and `tuhannes`,
//! instead keep class 39's `-kse` shape (`kymmeneksen`, `kymmenesten`) and
//! delegate to it.
//!
//! [`ordinal`] is the shared engine and class 46 (`-s` ordinals) is its other
//! user.
//!
//! One measured gap, left as a gap. `inflect` decides the `-kse` route by
//! `ends_with("kymmenes")`, but the class also holds eight compound ordinals --
//! `kahdeksaskymmenes`, `kahdeskymmenes`, `kolmaskymmenes`, `kuudeskymmenes`,
//! `neljäskymmenes`, `seitsemäskymmenes`, `viideskymmenes`,
//! `yhdeksäskymmenes` -- whose reference genitive is a true compound
//! (`kahdeksankymmenen`, both parts inflected), not the `-kse` shape the
//! `ends_with` sends them to (`kahdeksaskymmeneksen`) nor the ordinal shape
//! `ordinal` would build (`kahdeksaskymmennen`). That is the class-50/51
//! compound model's job, and it is not reachable from here; the port mirrors
//! the live routing and records the gap rather than silently swapping one
//! wrong spelling for another.

#![allow(dead_code)]

use crate::tags::{Case, Form, Number, PartOfSpeech};

use super::utility::{Builder, Decl, cell, nominal_tags, plain};

pub const CLASS: &str = "45";

/// Inflect a class-45 noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    // Almost the whole class is the ordinal family (kahdeksas, kahdes, kolmas,
    // viides, sadas), and every one takes the `-ns` stem (`kolmansien`,
    // `viidensien`). Only `kymmenes` and `tuhannes` keep class 39's `-kse`
    // stem (`kymmeneksen`, `kymmenesten`). (The `ends_with` here also catches
    // the compound `*kymmenes` ordinals; see the module comment.)
    if lemma.ends_with("kymmenes") || lemma.ends_with("tuhannes") {
        return super::class_39::inflect(lemma, possessives);
    }
    ordinal(lemma, possessives)
}

/// The ordinal `kahdeksas -> kahdeksannen` gradation.
pub fn ordinal(lemma: &str, possessives: bool) -> Vec<Form> {
    let mut b = Builder::new(lemma, possessives);
    let stem = &lemma[..lemma.len() - 1]; // kahdeksa / tuha
    let ns = format!("{stem}ns"); // kahdeksans (plural)
    let nn = format!("{stem}nne"); // kahdeksanne (gen/locs)
    let nt = format!("{stem}nte"); // kahdeksante (ess/transl/illat)
    let (sg, pl) = (Number::Singular, Number::Plural);

    b.decline(
        &Decl::new(vec![
            // ---- the singular ----
            // The bare nominative and its possessives ride the `-nte` stem
            // (kahdeksas -> kahdeksanteni).
            cell(Case::Nominative, sg, lemma, ["", ""], false).base_on(nt.clone(), ["", ""]),
            cell(Case::Genitive, sg, stem, ["nnen", "nnen"], false).base_on(nt.clone(), ["", ""]),
            cell(Case::Partitive, sg, stem, ["tta", "ttä"], true),
            cell(Case::Essive, sg, &nt, ["na", "nä"], true),
            cell(Case::Translative, sg, &nn, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            // kahdeksa + nteen, possessives on kahdeksante.
            cell(Case::Illative, sg, &nt, ["en", "en"], false).on_stem(),
            cell(Case::Inessive, sg, &nn, ["ssa", "ssä"], true),
            cell(Case::Elative, sg, &nn, ["sta", "stä"], true),
            cell(Case::Adessive, sg, &nn, ["lla", "llä"], true),
            cell(Case::Ablative, sg, &nn, ["lta", "ltä"], true),
            cell(Case::Allative, sg, &nn, ["lle", "lle"], true),
            cell(Case::Abessive, sg, &nn, ["tta", "ttä"], true),
            // ---- the plural ----
            cell(Case::Nominative, pl, stem, ["nnet", "nnet"], false).base_on(nt, ["", ""]),
            cell(Case::Genitive, pl, &ns, ["ien", "ien"], false).on_ending(["ie", "ie"]),
            // The `-ten` genitive plural is real for some ordinals but the
            // reference confirms only `-sien` (`kahdeksansien`), so it is
            // offered repaired.
            cell(Case::Genitive, pl, stem, ["nten", "nten"], false)
                .on_ending(["nte", "nte"])
                .as_rare(),
            cell(Case::Partitive, pl, &ns, ["ia", "iä"], true),
            cell(Case::Essive, pl, &ns, ["ina", "inä"], true),
            cell(Case::Translative, pl, &ns, ["iksi", "iksi"], true).on_ending(["ikse", "ikse"]),
            cell(Case::Illative, pl, &ns, ["iin", "iin"], false).on_ending(["ii", "ii"]),
            cell(Case::Inessive, pl, &ns, ["issa", "issä"], true),
            cell(Case::Elative, pl, &ns, ["ista", "istä"], true),
            cell(Case::Adessive, pl, &ns, ["illa", "illä"], true),
            cell(Case::Ablative, pl, &ns, ["ilta", "iltä"], true),
            cell(Case::Allative, pl, &ns, ["ille", "ille"], true),
            cell(Case::Abessive, pl, &ns, ["itta", "ittä"], true),
            plain(Case::Instructive, pl, &ns, ["in", "in"]),
        ]),
        |case, number, poss| nominal_tags(PartOfSpeech::Noun, case, number, poss),
    );
    b.comitative(&ns);

    b.into_forms()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }

    #[test]
    fn kymmenes() {
        let w = words("kymmenes");
        assert!(w.contains(&"kymmeneksen".to_string()));
        assert!(w.contains(&"kymmenestä".to_string()));
    }

    /// The ordinal shape: the tail `-s` reads `-nnen`, the plural `-ns`, and
    /// the partitive stays on the lemma.
    #[test]
    fn kahdeksas_is_the_ordinal_shape() {
        let w = words("kahdeksas");
        assert!(w.contains(&"kahdeksannen".to_string())); // gen sg
        assert!(w.contains(&"kahdeksatta".to_string())); // part sg
        assert!(w.contains(&"kahdeksansien".to_string())); // gen pl
        assert!(w.contains(&"kahdeksansia".to_string())); // part pl
        assert!(w.contains(&"kahdeksanteen".to_string())); // illat sg
    }
}
