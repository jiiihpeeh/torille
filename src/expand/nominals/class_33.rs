//! Class 33 (the ahrain type) inflection, written by hand as a logical engine.
//!
//! Class 33 is the `-in`/`-an` instrument family (ahrain, aistin, puhelin,
//! hapan). From the lemma (e.g. `ahrain`) the oblique `e`-stem grows by
//! reading the final `n` as `m` and geminating where a subtype does
//! (`ahrain -> ahraime`, `hapan -> happama`). That oblique stem carries the
//! singular obliques (`ahraimen`, `ahraimessa`); the partitive and the `-ten`
//! genitive plural stay on the bare lemma (`ahrainta`, `ahrainten`), and the
//! plural sits on the bare graded stem with the `i`-plural (`ahraimien`,
//! `ahraimia`).
//!
//! This module is the family's shared engine, not just class 33:
//! [`inflect_graded`] is the grid for every `33*` subtype (which differ only in
//! how the oblique stem is spelled) and for class 42, and
//! [`inflect_graded_nom_base`] is the same grid with the two nominative
//! possessive bases named, for classes 39 and 40 whose nominatives ride the
//! full oblique stem. So porting this one `Decl` ports the `-in`/`-an` family.
//!
//! What is worth naming: the illative doubles the oblique stem's own last
//! letter, which is not known until the stem is built (`ahraime ->
//! ahraimeen`, `happama -> happamaan`), so it is an `Ending::Given` rather than
//! a harmonic pair.

#![allow(dead_code)]

use crate::tags::{Case, Form, Number, PartOfSpeech};

use super::utility::{Builder, Decl, Ending, cell, nominal_tags, plain};

pub const CLASS: &str = "33";

/// The base oblique stem: the lemma's final `n` read `m` (`ahrain -> ahraim`).
fn weak_stem(lemma: &str) -> String {
    let mut s = lemma.to_string();
    s.pop(); // drop the final `n`
    s.push('m');
    s
}

/// Inflect a plain class-33 noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    inflect_graded(lemma, possessives, &format!("{}e", weak_stem(lemma)))
}

/// The shared class-33 engine, given the oblique (`e`/`a`-stem) spelling of the
/// lemma (`ahraime`, `asettime`, `happama`). The nominative singular possessive
/// suffixes attach to the bare lemma (`ahraimeni`) and the nominative plural to
/// the bare graded stem (`ahraimeni`).
pub fn inflect_graded(lemma: &str, possessives: bool, estem: &str) -> Vec<Form> {
    // The nominative possessives in both numbers ride the oblique `e`-stem:
    // `ahdin` is `ahtimeni`/`ahtimensa`, `ahrain` is `ahraimeni`. The bare
    // lemma and the bare graded stem give `ahdinni`/`ahraimnsa`, which are
    // not words.
    inflect_graded_nom_base(lemma, possessives, estem, estem, estem)
}

/// The shared class-33 engine with caller-chosen nominative possessive bases.
/// Class 39's `-us` set needs the full oblique stem for *both* nominatives
/// (`aakkostus -> aakkostukse`, so `aakkostukseni` in both), unlike the other
/// 33\* families which use the bare lemma / bare graded stem.
pub fn inflect_graded_nom_base(
    lemma: &str,
    possessives: bool,
    estem: &str,
    nom_sg_base: &str,
    nom_pl_base: &str,
) -> Vec<Form> {
    // Class 33 carries the plural-only instrument nouns (`antimet`,
    // `aterimet`, `kuolaimet`). Their citation form is the nominative
    // plural, and the plural stem is the lemma without `-et`
    // (`antim`, `aterim`). The singular grid would read the whole lemma as
    // an `-in` stem and invent `antimetta`; build the plural directly.
    if let Some(pl_stem) = lemma.strip_suffix("et")
        && !pl_stem.is_empty()
    {
        // The plural-only `-imet` nouns decline from the stem without `-et`:
        // `antimet` -> `antim`, giving `antimien`, `antimia`, `antimissa` and
        // `antimitta`. That is the Wiktionary/Kielitoimisto paradigm, and it is
        // the one the old pre-oracle stem already produced. The extra `m` some
        // oracle rows carry (`antimemissa`) is not part of the word.
        return plural_only(lemma, possessives, pl_stem);
    }
    let mut b = Builder::new(lemma, possessives);
    let graded = estem[..estem.len() - estem.chars().last().unwrap().len_utf8()].to_string();
    let lastc = estem.chars().last().unwrap();
    let (sg, pl) = (Number::Singular, Number::Plural);

    b.decline(
        &Decl::new(vec![
            // ---- the singular, on the oblique stem ----
            // The nominative's own possessives ride the caller's base (the
            // oblique stem for class 33, where the bare lemma gives
            // `ahraimnsa`, which is not a word).
            cell(Case::Nominative, sg, lemma, ["", ""], false)
                .base_on(nom_sg_base.to_string(), ["", ""]),
            cell(Case::Genitive, sg, estem, ["n", "n"], false).on_stem(),
            cell(Case::Partitive, sg, lemma, ["ta", "tä"], true),
            cell(Case::Essive, sg, estem, ["na", "nä"], true),
            cell(Case::Translative, sg, estem, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            // The illative doubles the oblique stem's last letter.
            cell(
                Case::Illative,
                sg,
                estem,
                Ending::Given(format!("{lastc}n")),
                false,
            )
            .on_ending(Ending::Given(format!("{lastc}"))),
            cell(Case::Inessive, sg, estem, ["ssa", "ssä"], true),
            cell(Case::Elative, sg, estem, ["sta", "stä"], true),
            cell(Case::Adessive, sg, estem, ["lla", "llä"], true),
            cell(Case::Ablative, sg, estem, ["lta", "ltä"], true),
            cell(Case::Allative, sg, estem, ["lle", "lle"], true),
            cell(Case::Abessive, sg, estem, ["tta", "ttä"], true),
            // ---- the plural, the i-plural on the bare graded stem ----
            cell(Case::Nominative, pl, estem, ["t", "t"], false)
                .base_on(nom_pl_base.to_string(), ["", ""]),
            cell(Case::Genitive, pl, &graded, ["ien", "ien"], false).on_ending(["ie", "ie"]),
            // The `-ten` genitive plural goes back to the bare lemma.
            cell(Case::Genitive, pl, lemma, ["ten", "ten"], false).on_ending(["te", "te"]),
            cell(Case::Partitive, pl, &graded, ["ia", "iä"], true),
            cell(Case::Essive, pl, &graded, ["ina", "inä"], true),
            cell(Case::Translative, pl, &graded, ["iksi", "iksi"], true)
                .on_ending(["ikse", "ikse"]),
            cell(Case::Illative, pl, &graded, ["iin", "iin"], false).on_ending(["ii", "ii"]),
            cell(Case::Inessive, pl, &graded, ["issa", "issä"], true),
            cell(Case::Elative, pl, &graded, ["ista", "istä"], true),
            cell(Case::Adessive, pl, &graded, ["illa", "illä"], true),
            cell(Case::Ablative, pl, &graded, ["ilta", "iltä"], true),
            cell(Case::Allative, pl, &graded, ["ille", "ille"], true),
            cell(Case::Abessive, pl, &graded, ["itta", "ittä"], true),
            plain(Case::Instructive, pl, &graded, ["in", "in"]),
        ]),
        |case, number, poss| nominal_tags(PartOfSpeech::Noun, case, number, poss),
    );
    b.comitative(&graded);

    b.into_forms()
}

/// The plural-only `-imet` paradigm (`antimet`, `aterimet`): the citation
/// form is the nominative plural and every other cell is built on the stem
/// without `-et` (`antim` -> `antimia`, `antimiin`).
fn plural_only(lemma: &str, possessives: bool, pl_stem: &str) -> Vec<Form> {
    let mut b = Builder::new(lemma, possessives);
    let pl = Number::Plural;
    b.decline(
        &Decl::new(vec![
            cell(Case::Nominative, pl, lemma, ["", ""], false).on_stem(),
            cell(Case::Genitive, pl, pl_stem, ["ien", "ien"], false).on_ending(["ie", "ie"]),
            cell(Case::Partitive, pl, pl_stem, ["ia", "iä"], true),
            cell(Case::Essive, pl, pl_stem, ["ina", "inä"], true),
            cell(Case::Translative, pl, pl_stem, ["iksi", "iksi"], true)
                .on_ending(["ikse", "ikse"]),
            cell(Case::Illative, pl, pl_stem, ["iin", "iin"], false).on_ending(["ii", "ii"]),
            cell(Case::Inessive, pl, pl_stem, ["issa", "issä"], true),
            cell(Case::Elative, pl, pl_stem, ["ista", "istä"], true),
            cell(Case::Adessive, pl, pl_stem, ["illa", "illä"], true),
            cell(Case::Ablative, pl, pl_stem, ["ilta", "iltä"], true),
            cell(Case::Allative, pl, pl_stem, ["ille", "ille"], true),
            cell(Case::Abessive, pl, pl_stem, ["itta", "ittä"], true),
            plain(Case::Instructive, pl, pl_stem, ["in", "in"]),
        ]),
        |case, number, poss| nominal_tags(PartOfSpeech::Noun, case, number, poss),
    );
    b.comitative(pl_stem);
    b.into_forms()
}

/// The oblique stem of a geminating `-in`/`-an` word: drop the final `n`,
/// double the consonant before the last vowel, then append `m` and the stem
/// vowel (`asetin -> asettime`, `hapan -> happama`).
pub fn geminate_estem(lemma: &str) -> String {
    let mut base = lemma.to_string();
    base.pop(); // drop the final n
    let chars: Vec<char> = base.chars().collect();
    // the char index (not byte) of the last vowel
    let mut li = 0usize;
    for (i, c) in chars.iter().enumerate() {
        if "aeiouyäö".contains(*c) {
            li = i;
        }
    }
    let mut out = String::new();
    for (i, c) in chars.iter().enumerate() {
        if i + 1 == li {
            out.push(*c);
            out.push(*c); // geminate the consonant before the last vowel
        } else {
            out.push(*c);
        }
    }
    let fv = if lemma.ends_with("an") || lemma.ends_with("än") {
        'a'
    } else {
        'e'
    };
    out.push('m');
    out.push(fv);
    out
}

/// The oblique stem of an `-in` word whose consonant before the last vowel
/// changes per the pair (`kaavin` v->p, `ahdin` d->t, `askellin` ll->lt).
pub fn pair_estem(lemma: &str, from: &str, to: &str) -> String {
    let mut base = lemma.to_string();
    base.pop(); // drop the final n
    let bs = base.clone();
    let chars: Vec<char> = bs.chars().collect();
    let mut li = 0usize;
    for (i, c) in chars.iter().enumerate() {
        if "aeiouyäö".contains(*c) {
            li = i;
        }
    }
    let from_chars: Vec<char> = from.chars().collect();
    let start = li.saturating_sub(from_chars.len());
    let matches = start + from_chars.len() <= chars.len()
        && chars[start..start + from_chars.len()] == from_chars[..];
    let mut out = String::new();
    for (i, c) in chars.iter().enumerate() {
        if matches && i == start {
            out.push_str(to);
        } else if matches && i >= start && i < start + from_chars.len() {
            // the replaced segment
        } else {
            out.push(*c);
        }
    }
    let fv = if lemma.ends_with("an") || lemma.ends_with("än") {
        'a'
    } else {
        'e'
    };
    out.push('m');
    out.push(fv);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }

    #[test]
    fn in_type() {
        let w = words("ahrain");
        assert!(w.contains(&"ahraimen".to_string()));
        assert!(w.contains(&"ahrainta".to_string()));
        assert!(w.contains(&"ahraimien".to_string()));
        assert!(w.contains(&"ahrainten".to_string()));
        assert!(w.contains(&"ahraimia".to_string()));
    }

    /// Plural-only `-imet` instrument nouns (`antimet`) build their cells on
    /// the stem without `-et`, not on the whole citation form.
    #[test]
    fn a_plural_only_instrument_noun_uses_the_plural_stem() {
        let w = words("antimet");
        for want in ["antimet", "antimien", "antimia", "antimiin", "antimissa"] {
            assert!(w.contains(&want.to_string()), "missing {want}");
        }
        for bad in ["antimetta", "antimetten", "antimettaan", "antimemissa"] {
            assert!(!w.contains(&bad.to_string()), "invented {bad}");
        }
    }

    #[test]
    fn helpers() {
        assert_eq!(geminate_estem("asetin"), "asettime");
        assert_eq!(geminate_estem("hapan"), "happama");
        assert_eq!(pair_estem("kaavin", "v", "p"), "kaapime");
    }

    /// The nominative possessives ride the oblique stem, not the bare lemma:
    /// `ahrain` gives `ahraimeni`, not the `ahrainni` the bare base would.
    #[test]
    fn the_nominative_possessives_ride_the_oblique_stem() {
        let w = words("ahrain");
        assert!(w.contains(&"ahraimeni".to_string()), "ahraimeni missing");
        assert!(
            !w.contains(&"ahrainni".to_string()),
            "bare-base possessive emitted"
        );
    }

    /// The caller-chosen bases are what classes 39/40 need: both nominatives
    /// on the full oblique stem give `aakkostukseni` twice over.
    #[test]
    fn the_nominative_bases_are_caller_chosen() {
        let w: Vec<String> = inflect_graded_nom_base(
            "aakkostus",
            true,
            "aakkostukse",
            "aakkostukse",
            "aakkostukse",
        )
        .into_iter()
        .map(|f| f.word)
        .collect();
        assert!(w.contains(&"aakkostuksen".to_string())); // gen sg
        assert!(w.contains(&"aakkostuksia".to_string())); // part pl
    }
}
