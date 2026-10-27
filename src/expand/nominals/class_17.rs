//! Class 17 (the ehtoo/harmaa type) inflection, written by hand.
//!
//! Class 17 is the long-vowel `-oo`/`-uu`/`-aa` family (ehtoo, epuu, harmaa).
//! The singular is the lemma as its stem, with the long partitive `-ta`
//! (`ehtoota`) and the `-seen` illative (`ehtooseen`), as a long final vowel
//! takes. The plural stem drops one of the final vowels (`ehtoo -> ehto`,
//! `harmaa -> harma`) and carries the `i`-plural: `ehtoiden`/`ehtoitten`,
//! `ehtoita`, `ehtoissa`.
//!
//! Two things here are not pairs of spellings, which is why `Ending` is more
//! than a `[back, front]` array. The singular illative picks its ending from the
//! stem's *last letter* rather than from the word's harmony (`ehtoo` takes
//! `-seen`, the `-i` words that reach this class from 18 take `-ihin`), so it is
//! an `Ending::Given` chosen at run time. And several cells here are alternants
//! the reference wraps in parentheses, which is what `as_rare` says.

#![allow(dead_code)]

use crate::tags::{Case, Form, Number, PartOfSpeech};

use super::utility::{Builder, Decl, cell, nominal_tags, plain};

pub const CLASS: &str = "17";

/// Inflect a class-17 noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    let mut b = Builder::new(lemma, possessives);
    let stem = lemma;
    let (sg, pl) = (Number::Singular, Number::Plural);
    let mut pstem = stem.to_string();
    pstem.pop(); // drop one final vowel: ehtoo -> ehto
    let plural_only = stem.ends_with('t'); // häät-type plural-only nouns

    // The illative chooses by the stem's final vowel: a long-vowel word takes
    // `-seen` (`ehtooseen`, `harmaaseen`), an `-i` word takes `-ihin`
    // (`aamunkoi` -> `aamunkoihin`, which the class-18 words reach through
    // here). The other spellings are real for other classes but the reference
    // confirms one per word here, so they are offered repaired, never plain.
    let last_v = stem.chars().last().unwrap();
    // The class-18 short words that reach this engine (`kyy`, `pelakuu`) take
    // the `-hVn` illative (`kyyhyn`, `pelakuuhun`), not the long-vowel
    // `-seen` that `ehtoo`/`harmaa` want. The `-i` words (`pii`, `torstai`)
    // take `-hin` as before.
    const SHORT_H: &[&str] = &["kyy", "pelakuu"];
    let (ill_v, ill_base): (String, String) = if last_v == 'i' {
        ("hin".to_string(), "hi".to_string())
    } else if SHORT_H.contains(&lemma) {
        (format!("h{last_v}n"), format!("h{last_v}"))
    } else {
        ("seen".to_string(), "see".to_string())
    };

    // A plural-only word (häät) is already plural; the nominative is the lemma
    // itself and its possessives sit on the singular stem.
    let plural_nom = if plural_only {
        stem.to_string()
    } else {
        format!("{stem}t")
    };

    // The `-isiin` illative is confirmed alongside `-ihin` for the long-vowel
    // words (`ehtoisiin`) but not for the `-i` words that reach here from class
    // 18 (`aamunkoi` has only `aamunkoihin`), so it is repaired there.
    let isiin = cell(Case::Illative, pl, &pstem, "isiin", false).on_ending("isii");
    let isiin = if stem.ends_with('i') || lemma == "pelakuu" {
        isiin.as_rare()
    } else {
        isiin
    };

    b.decline(
        &Decl::new(vec![
            // ---- the singular (on the stem) ----
            cell(Case::Nominative, sg, stem, ["", ""], false).on_stem(),
            cell(Case::Genitive, sg, stem, ["n", "n"], false).on_stem(),
            cell(Case::Partitive, sg, stem, ["ta", "tä"], true),
            cell(Case::Essive, sg, stem, ["na", "nä"], true),
            cell(Case::Translative, sg, stem, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            cell(Case::Illative, sg, stem, ill_v, false).on_ending(ill_base),
            cell(Case::Illative, sg, stem, "hen", false)
                .on_ending("he")
                .as_rare(),
            cell(Case::Illative, sg, stem, "han", false)
                .on_ending("ha")
                .as_rare(),
            // The `-hVn` illative mirrors the stem's final vowel: ehtoo ->
            // ehtoohon, pelakuu -> pelakuuhun, sää -> säähän, revyy -> revyyhyn.
            cell(Case::Illative, sg, stem, format!("h{last_v}n"), false)
                .on_ending(format!("h{last_v}"))
                .as_rare(),
            cell(Case::Inessive, sg, stem, ["ssa", "ssä"], true),
            cell(Case::Elative, sg, stem, ["sta", "stä"], true),
            cell(Case::Adessive, sg, stem, ["lla", "llä"], true),
            cell(Case::Ablative, sg, stem, ["lta", "ltä"], true),
            cell(Case::Allative, sg, stem, ["lle", "lle"], true),
            cell(Case::Abessive, sg, stem, ["tta", "ttä"], true),
            // ---- the plural, on the dropped stem with the i-plural ----
            // The nominative's bare is the plural word (ehtoot, or the lemma
            // itself for a plural-only word like häät) but its possessives sit
            // on the *singular* stem: ehtooni, not ehtootni.
            cell(Case::Nominative, pl, &plural_nom, ["", ""], false)
                .base_on(stem.to_string(), ["", ""]),
            cell(Case::Genitive, pl, &pstem, ["iden", "iden"], false).on_ending(["ide", "ide"]),
            cell(Case::Genitive, pl, &pstem, ["itten", "itten"], false).on_ending(["itte", "itte"]),
            cell(Case::Partitive, pl, &pstem, ["ita", "itä"], true),
            cell(Case::Essive, pl, &pstem, ["ina", "inä"], true),
            cell(Case::Translative, pl, &pstem, ["iksi", "iksi"], true).on_ending(["ikse", "ikse"]),
            cell(Case::Illative, pl, &pstem, ["ihin", "ihin"], false).on_ending(["ihi", "ihi"]),
            isiin,
            cell(Case::Inessive, pl, &pstem, ["issa", "issä"], true),
            cell(Case::Elative, pl, &pstem, ["ista", "istä"], true),
            cell(Case::Adessive, pl, &pstem, ["illa", "illä"], true),
            cell(Case::Ablative, pl, &pstem, ["ilta", "iltä"], true),
            cell(Case::Allative, pl, &pstem, ["ille", "ille"], true),
            cell(Case::Abessive, pl, &pstem, ["itta", "ittä"], true),
            plain(Case::Instructive, pl, &pstem, ["in", "in"]),
        ]),
        |case, number, poss| nominal_tags(PartOfSpeech::Noun, case, number, poss),
    );
    b.comitative(&pstem);

    b.into_forms()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }

    #[test]
    fn long_vowel() {
        let w = words("ehtoo");
        assert!(w.contains(&"ehtoon".to_string()));
        assert!(w.contains(&"ehtoota".to_string()));
        assert!(w.contains(&"ehtooseen".to_string()));
        assert!(w.contains(&"ehtoiden".to_string()));
        assert!(w.contains(&"ehtoita".to_string()));
    }

    /// `kyy` and `pelakuu` are class-18 short words delegated here; their
    /// illative is `-hVn`, not the long-vowel `-seen`.
    /// The short `-yy`/`-uu` words keep the ordinary plural: `kyyt`, `kyitä`,
    /// `kyihin`, `kyiden`, `kyissä`; only the singular illative is special.
    #[test]
    fn kyy_keeps_the_ordinary_plural() {
        let w = words("kyy");
        for good in ["kyyt", "kyitä", "kyihin", "kyiden", "kyissä"] {
            assert!(w.contains(&good.to_string()), "kyy is missing {good}");
        }
    }

    #[test]
    fn short_words_take_the_hvn_illative() {
        for (lemma, good, bad) in [
            ("kyy", "kyyhyn", "kyyseen"),
            ("pelakuu", "pelakuuhun", "pelakuuseen"),
        ] {
            let w = words(lemma);
            assert!(w.contains(&good.to_string()), "{lemma} is missing {good}");
            assert!(!w.contains(&bad.to_string()), "{lemma} must not offer {bad}");
        }
    }
}
