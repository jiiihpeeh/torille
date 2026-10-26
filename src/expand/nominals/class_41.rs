//! Class 41 (the ahnas type) inflection, written by hand as a logical engine.
//!
//! Class 41 is the gradated `-as`/`-is`/`-us` family (ahnas, hammas, allas):
//! the `-as` reads a long `-aa` a-stem (`ahnas -> ahnaa`) whose `aa` carries
//! the singular obliques and the nominative plural, whose short `a` carries
//! the plural obliques, and whose bare lemma keeps the partitive (`ahnasta`).
//! [`a_stem_inflect`] is shared by the 41\* gradated subtypes, so porting it
//! ports the family.
//!
//! What is worth naming: the plural carries two genitive alternants (`-iden`,
//! `-itten`) and two illatives (`-ihin`, `-isiin`), which is more than the
//! base 24-cell grid, and the nominative possessives ride the long a-stem
//! (`armas` -> `armaani`), not the short oblique stem.

#![allow(dead_code)]

use crate::tags::{Case, Form, Number, PartOfSpeech};

use super::utility::{Builder, Decl, cell, nominal_tags, plain};

pub const CLASS: &str = "41";

/// The shared long-`a`-stem engine (`paltaa`/`palta`, `ahnaa`/`ahna`): the
/// singular obliques and nominative plural sit on `long`, the plural obliques
/// on `short`, the nominative and partitive on the bare lemma.
pub fn a_stem_inflect(
    lemma: &str,
    possessives: bool,
    long: &str,
    short: &str,
    sten_plural: bool,
) -> Vec<Form> {
    // A plural-only lemma (`tikkaat`, `valjaat`) arrives through both the
    // plain class and the graded 41* subtypes. Derive the long/short stems
    // from the dropped `-t` (`tikkaa` / `tikka`) and keep only the plural
    // cells; the callers' stems are built for a singular `-as` lemma.
    let plural_only = sten_plural && lemma.ends_with('t');
    let long_owned = plural_only.then(|| lemma.strip_suffix('t').unwrap_or(lemma).to_string());
    let short_owned = long_owned.as_ref().map(|long| match long.chars().last() {
        Some(c) if "aäeiouyö".contains(c) => long[..long.len() - c.len_utf8()].to_string(),
        _ => long.clone(),
    });
    let long = long_owned.as_deref().unwrap_or(long);
    let short = short_owned.as_deref().unwrap_or(short);
    // The `-as`/`-is` family includes adjectives and nouns; keep possessives.
    let mut b = Builder::new(lemma, possessives);
    let (sg, pl) = (Number::Singular, Number::Plural);

    let mut cells = vec![
        // ---- the singular, on the long a-stem ----
        // The nominative possessives ride the long a-stem (`armas` ->
        // `armaani`), not the short oblique stem.
        cell(Case::Nominative, sg, lemma, ["", ""], false).base_on(long.to_string(), ["", ""]),
        cell(Case::Genitive, sg, long, ["n", "n"], false).on_stem(),
        cell(Case::Partitive, sg, lemma, ["ta", "tä"], true),
        cell(Case::Essive, sg, long, ["na", "nä"], true),
        cell(Case::Translative, sg, long, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
        cell(Case::Illative, sg, long, ["seen", "seen"], false).on_ending(["see", "see"]),
        cell(Case::Inessive, sg, long, ["ssa", "ssä"], true),
        cell(Case::Elative, sg, long, ["sta", "stä"], true),
        cell(Case::Adessive, sg, long, ["lla", "llä"], true),
        cell(Case::Ablative, sg, long, ["lta", "ltä"], true),
        cell(Case::Allative, sg, long, ["lle", "lle"], true),
        cell(Case::Abessive, sg, long, ["tta", "ttä"], true),
        // ---- the plural, on the short a-stem ----
        cell(Case::Nominative, pl, long, ["t", "t"], false).on_stem(),
        cell(Case::Genitive, pl, short, ["iden", "iden"], false).on_ending(["ide", "ide"]),
        cell(Case::Genitive, pl, short, ["itten", "itten"], false).on_ending(["itte", "itte"]),
        cell(Case::Partitive, pl, short, ["ita", "itä"], true),
        cell(Case::Essive, pl, short, ["ina", "inä"], true),
        cell(Case::Translative, pl, short, ["iksi", "iksi"], true).on_ending(["ikse", "ikse"]),
        cell(Case::Illative, pl, short, ["ihin", "ihin"], false).on_ending(["ihi", "ihi"]),
        cell(Case::Illative, pl, short, ["isiin", "isiin"], false).on_ending(["isii", "isii"]),
        cell(Case::Inessive, pl, short, ["issa", "issä"], true),
        cell(Case::Elative, pl, short, ["ista", "istä"], true),
        cell(Case::Adessive, pl, short, ["illa", "illä"], true),
        cell(Case::Ablative, pl, short, ["ilta", "iltä"], true),
        cell(Case::Allative, pl, short, ["ille", "ille"], true),
        cell(Case::Abessive, pl, short, ["itta", "ittä"], true),
        plain(Case::Instructive, pl, short, ["in", "in"]),
    ];
    if sten_plural {
        // The third confirmed genitive plural of the `-as` family:
        // `patsas` -> `patsasten`, `ahdas` -> `ahdasten`, `vieras` ->
        // `vierasten`. Without it a compound such as `savupatsasten` cannot
        // find its head. The classes that merely reuse this grid (`kevät`,
        // `mennyt`, `askar`) do not have it.
        cells.push(
            cell(Case::Genitive, pl, short, ["sten", "sten"], false).on_ending(["ste", "ste"]),
        );
    }
    b.decline(&Decl::new(cells), |case, number, poss| {
        nominal_tags(PartOfSpeech::Noun, case, number, poss)
    });
    b.comitative(short);

    let mut forms = b.into_forms();
    if plural_only {
        forms.retain(|form| {
            form.readings
                .iter()
                .any(|reading| reading.number == Some(Number::Plural))
        });
    }
    forms
}

/// The gradated short stem: replace the trailing consonant cluster of the body
/// (the lemma minus its final `s` and last vowel) with `to` when it is `from`.
pub fn grad_stem(lemma: &str, from: &str, to: &str) -> String {
    let mut base = lemma.to_string();
    base.pop(); // drop the final s
    let last = base.chars().last().unwrap();
    let body = &base[..base.len() - base.chars().last().unwrap().len_utf8()];
    let short = if body.ends_with(from) {
        format!("{}{}", &body[..body.len() - from.len()], to)
    } else {
        body.to_string()
    };
    format!("{short}{last}")
}

/// The ungradated short stem: the lemma minus its final `s`.
pub fn base_stem(lemma: &str) -> String {
    let mut s = lemma.to_string();
    s.pop();
    s
}

/// The long stem from a short stem: double the final vowel (hampa -> hampaa).
pub fn long_stem(short: &str) -> String {
    let last = short.chars().last().unwrap();
    format!("{short}{last}")
}

/// Inflect a plain class-41 noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    // Class 41 also carries plural-only lemmas whose citation form is the
    // nominative plural (`valjaat`, `rattaat`, `tikkaat`). Drop the `-t`, use
    // the remaining long vowel as the plural stem (`valjaa`), shorten it by
    // one vowel for the oblique stem (`valja`), and keep only the plural
    // cells: `valjaisiin`, not `valjaa` or `valjaan`.
    if let Some(long) = lemma.strip_suffix('t') {
        let short = match long.chars().last() {
            Some(c) if "aäeiouyö".contains(c) => &long[..long.len() - c.len_utf8()],
            _ => long,
        };
        let mut forms = a_stem_inflect(lemma, possessives, long, short, true);
        forms.retain(|form| {
            form.readings
                .iter()
                .any(|reading| reading.number == Some(Number::Plural))
        });
        return forms;
    }
    let short = base_stem(lemma);
    let long = long_stem(&short);
    a_stem_inflect(lemma, possessives, &long, &short, true)
}

/// The non-plural-only `-as`/`-ät` grid. Class 44 (`kevät`) is a singular
/// t-final lemma that shares this shape but must not take the plural-only
/// branch above, so it calls this entry point directly.
pub fn inflect_singular(lemma: &str, possessives: bool) -> Vec<Form> {
    let short = base_stem(lemma);
    let long = long_stem(&short);
    a_stem_inflect(lemma, possessives, &long, &short, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }

    /// Plural-only lemmas (`valjaat`) decline from the dropped-`t` plural
    /// stem and must not invent a singular.
    #[test]
    fn a_plural_only_lemma_declines_from_the_plural_stem() {
        let w = words("valjaat");
        for want in [
            "valjaat",
            "valjaiden",
            "valjaitten",
            "valjaita",
            "valjaisiin",
            "valjaissa",
        ] {
            assert!(w.contains(&want.to_string()), "missing {want}");
        }
        for bad in ["valjaa", "valjaan", "valjaana", "valjas"] {
            assert!(!w.contains(&bad.to_string()), "invented {bad}");
        }
    }

    /// The graded 41* subtypes also carry plural-only lemmas (`tikkaat`).
    #[test]
    fn a_graded_plural_only_lemma_declines_from_the_plural_stem() {
        let w: Vec<String> = super::super::class_41A::inflect("tikkaat", true)
            .into_iter()
            .map(|f| f.word)
            .collect();
        for want in [
            "tikkaat",
            "tikkaiden",
            "tikkaitten",
            "tikkaita",
            "tikkailla",
            "tikkaille",
            "tikkaisiin",
        ] {
            assert!(w.contains(&want.to_string()), "missing {want}");
        }
        for bad in ["tikkaa", "tikkaan", "tikkaana"] {
            assert!(!w.contains(&bad.to_string()), "invented {bad}");
        }
    }

    /// The `-sten` genitive plural is a confirmed alternant of the `-as`
    /// family: `patsas` -> `patsasten`.
    #[test]
    fn the_as_family_has_a_sten_genitive_plural() {
        assert!(words("patsas").contains(&"patsasten".to_string()));
        assert!(words("ahdas").contains(&"ahdasten".to_string()));
    }

    #[test]
    fn as_type() {
        let w = words("ahnas");
        assert!(w.contains(&"ahnaan".to_string()));
        assert!(w.contains(&"ahnasta".to_string()));
        assert!(w.contains(&"ahnaana".to_string()));
        assert!(w.contains(&"ahnaat".to_string()));
        assert!(w.contains(&"ahnaiden".to_string()));
        assert!(w.contains(&"ahnaita".to_string()));
    }

    /// The plural carries two genitive alternants and two illatives, and the
    /// nominative possessives ride the long a-stem.
    #[test]
    fn the_plural_alternants_and_the_nominative_base() {
        let w = words("ahnas");
        assert!(w.contains(&"ahnaitten".to_string()));
        assert!(w.contains(&"ahnaihin".to_string()) || w.contains(&"ahnaisiin".to_string()));
        assert!(
            w.contains(&"ahnaani".to_string()),
            "long-stem possessive missing"
        );
        assert!(
            !w.contains(&"ahnani".to_string()),
            "short-stem possessive emitted"
        );
    }
}
