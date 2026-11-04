//! Class 99 (the aika type) inflection, written by hand as a logical engine.
//!
//! Class 99 is the sanalista's marker for indeclinable words (aika, koko,
//! kyse, mi, kesken, ...): they carry no paradigm of their own and are offered
//! exactly as the lemma. Where such a word also inflects as a member of a real
//! class (aika is also class 10), those forms come from the other class through
//! the multi-class union, and this engine supplies the bare nominative.
//!
//! The rest are foreign/`-i` members that inflect on class 5/6's shared
//! [`build_i_loans`](super::utility::build_i_loans) grid, merged with the bare
//! nominative through a first-wins dedup.

#![allow(dead_code)]

use crate::tags::{Case, Confidence, Form, Number, PartOfSpeech};

use super::utility::{Builder, Decl, IPlural, build_i_loans, cell, front, nominal_tags};

pub const CLASS: &str = "99";

/// Whether `lemma` is a class-99 word with a real paradigm rather than the
/// shared i-loan guesses.
///
/// These are the words [`inflect`] delegates to a real class (`seesam` -> 5,
/// `tenä` and `ainut` -> 10/43) or spells out. They are the only class-99
/// words whose forms belong in the reverse index; indexing the guesses would
/// add thousands of forms the reference confirms for none.
pub fn has_real_paradigm(lemma: &str) -> bool {
    matches!(lemma, "seesam" | "tenä" | "ainut" | "viisi" | "pommi")
}

/// Class-99 words that do not inflect at all: indeclinable adverbs, particles
/// and interjections (oikein, pahoin, toisin, takaisin, usein, mi, ...) whose
/// whole paradigm, per the reference lexicon, is the bare lemma. They are
/// offered exactly as the lemma; running the i-loan machinery over them would
/// fabricate forms no oracle confirms. A `matches!` on string literals
/// compiles to a decision tree, faster than a linear scan or binary search
/// over a table.
fn no_inflect(lemma: &str) -> bool {
    matches!(
        lemma,
        "aamuisin"
            | "absurdisti"
            | "aiemmin"
            | "aikaisemmin"
            | "aikaisin"
            | "aimo"
            | "alias"
            | "alkuisin"
            | "arkisin"
            | "ehommin"
            | "eioo"
            | "ennen"
            | "eri"
            | "eukkonen"
            | "harvoin"
            | "hm"
            | "huuti"
            | "hyvin"
            | "hyvänen"
            | "höykänen"
            | "ilmoinen"
            | "iltaisin"
            | "in"
            | "inkognito"
            | "jonne"
            | "kahdeksisen"
            | "kahdeksisenkymmentä"
            | "kaks"
            | "kaltoin"
            | "kauemmin"
            | "kauimmin"
            | "keskiviikkoisin"
            | "kesäisin"
            | "keväisin"
            | "kohta"
            | "kolmisen"
            | "kolmisenkymmentä"
            | "kop"
            | "kotoisin"
            | "kovin"
            | "kukkokiekuu"
            | "kuutisen"
            | "kuutisenkymmentä"
            | "kymmenisen"
            | "kymmenisentuhatta"
            | "laisin"
            | "lauantaisin"
            | "liioin"
            | "likemmin"
            | "lähemmin"
            | "lähtöisin"
            | "läpikotaisin"
            | "ma"
            | "maanantaisin"
            | "maate"
            | "melkein"
            | "mi"
            | "mikäkin"
            | "moikka"
            | "monta"
            | "moro"
            | "muorinen"
            | "myöhemmin"
            | "nelisen"
            | "nelisenkymmentä"
            | "nenäkkäin"
            | "nykyisin"
            | "oikeammin"
            | "oikein"
            | "ookoo"
            | "pahemmin"
            | "pahoin"
            | "paremmin"
            | "parhaimmin"
            | "parisen"
            | "parisenkymmentä"
            | "parisensataa"
            | "perjantaisin"
            | "pikku"
            | "pitkin"
            | "plääh"
            | "puolisen"
            | "puolisenkymmentä"
            | "puolisensataa"
            | "puolisentoista"
            | "puolisentoistasataa"
            | "pyhäisin"
            | "roti"
            | "sa"
            | "sees"
            | "seitsemisen"
            | "sekaisin"
            | "so"
            | "sukuisin"
            | "sunnuntaisin"
            | "surku"
            | "syksyisin"
            | "syntyisin"
            | "takaisin"
            | "talvisin"
            | "tarkemmin"
            | "tarkoin"
            | "tiistaisin"
            | "toisin"
            | "tokko"
            | "torstaisin"
            | "tuhantisen"
            | "täysin"
            | "usein"
            | "varhemmin" | "varuilla" | "varuillaan"
            | "velinen"
            | "venät"
            | "viime"
            | "viimeisin"
            | "viitisen"
            | "viitisenkymmentä"
            | "viitisentoista"
            | "väärin"
            | "yhdeksisen"
            | "yhdeksisenkymmentä"
            | "yhtä"
            | "yks"
            | "ylösalaisin"
            | "öisin"
    )
}

/// Inflect a class-99 (mostly indeclinable, some foreign `-i`) word.
///
/// Most are offered as the bare lemma; the foreign/`-i` members (miss, seesam,
/// aprilli, kuti) inflect on the shared i-loan stem, and the genuinely
/// indeclinable ones only give the lemma.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    let sg = Number::Singular;
    let mut out = Vec::new();
    if no_inflect(lemma) {
        // A genuinely indeclinable word has no paradigm and no possessives:
        // it is offered exactly as the lemma.
        out.push(
            Form::new(
                lemma.to_string(),
                nominal_tags(PartOfSpeech::Noun, Case::Nominative, sg, None),
            )
            .with(Confidence::Plain),
        );
        return out;
    }
    // Two class-99 rows are not indeclinable and not i-loans either. `seesam`
    // is the consonant-final `seesami` (class 5's shape), and `tenä` is a
    // regular `-ä` singular noun (class 10's shape). The shared i-loan grid
    // invented `seesamnsa` and `tenäeiden`.
    if lemma == "seesam" {
        return super::class_5::inflect(lemma, possessives);
    }
    // `ainut` is marked 99 because only the nominative and partitive are
    // common, but it declines like class 43 (`ohut`): Wiktionary gives
    // `ainuen`, `ainutta`, `ainueen`, `ainuella`, ... and the reference
    // confirms the common cells. The i-loan grid invented `ainutia` instead.
    if lemma == "ainut" {
        return super::class_43::inflect(lemma, possessives);
    }
    // `viisi` and `pommi` have a real class row beside their class-99 marker
    // (27 numeral and 5 noun). The class-99 i-loan grid invented
    // `viiseittä`/`pommeja`; delegate to the real class instead.
    if lemma == "viisi" {
        return super::class_27::inflect(lemma, possessives);
    }
    if lemma == "pommi" {
        return super::class_5::inflect(lemma, possessives);
    }
    if lemma == "tenä" {
        // Wiktionary: "Sana taipuu vain harvoissa sijoissa" -- only the
        // nominative and a few oblique/plural forms. Live Voikko confirms
        // exactly these five and rejects the rest of the a-stem paradigm.
        return [
            (Case::Nominative, sg, "tenä"),
            (Case::Genitive, sg, "tenän"),
            (Case::Partitive, sg, "tenää"),
            (Case::Inessive, sg, "tenässä"),
            (Case::Nominative, Number::Plural, "tenät"),
        ]
        .into_iter()
        .map(|(case, number, word)| {
            Form::new(
                word.to_string(),
                nominal_tags(PartOfSpeech::Noun, case, number, None),
            )
            .with(Confidence::Plain)
        })
        .collect();
    }
    let mut b = Builder::new(lemma, possessives);
    b.decline(
        &Decl::new(vec![
            cell(Case::Nominative, sg, lemma, ["", ""], false).on_stem(),
        ]),
        |case, number, poss| nominal_tags(PartOfSpeech::Noun, case, number, poss),
    );
    out = b.into_forms();
    out.extend(build_i_loans(
        lemma,
        possessives,
        front(lemma),
        IPlural::ALL,
    ));
    let mut seen = std::collections::HashSet::new();
    out.retain(|f| seen.insert(f.word.clone()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `ainut` is class 99 in the sanalista, but it is the class-43 `ohut`
    /// shape: partitive `ainutta`, genitive `ainuen`, illative `ainueen`.
    /// The reference confirms nominative and partitive; the rest is the same
    /// paradigm Wiktionary gives.
    #[test]
    fn ainut_declines_as_class_43() {
        let forms = inflect("ainut", true);
        let words: Vec<&str> = forms.iter().map(|f| f.word.as_str()).collect();
        for want in ["ainut", "ainutta", "ainuen", "ainueen", "ainuella"] {
            assert!(words.contains(&want), "{want} missing");
        }
        assert!(
            !words.iter().any(|f| *f == "ainutia"),
            "the i-loan partitive must not be offered"
        );
    }

    /// `viisi` and `pommi` are class-99 marked but have a real class row.
    /// The i-loan grid must not invent `viiseittä` or `pommeja`.
    #[test]
    fn a_class_99_word_with_a_real_class_delegates() {
        let viisi = inflect("viisi", true);
        assert!(viisi.iter().any(|f| f.word == "viisittä" && f.plain()));
        assert!(!viisi.iter().any(|f| f.word == "viiseittä" && f.plain()));
        let pommi = inflect("pommi", true);
        assert!(pommi.iter().any(|f| f.word == "pommien" && f.plain()));
        assert!(!pommi.iter().any(|f| f.word == "pommeiden" && f.plain()));
    }

    #[test]
    fn indeclinable() {
        let w = inflect("kyse", true)
            .into_iter()
            .map(|f| f.word)
            .collect::<Vec<_>>();
        assert!(w.contains(&"kyse".to_string()));
    }

    /// `tenä` is defective: Wiktionary gives only a few forms, and live
    /// Voikko confirms exactly these five. The i-loan grid invented
    /// `tenäeiden`/`tenäin`.
    #[test]
    fn tena_is_defective() {
        let w: Vec<String> = inflect("tenä", true).into_iter().map(|f| f.word).collect();
        for good in ["tenä", "tenän", "tenää", "tenässä", "tenät"] {
            assert!(w.contains(&good.to_string()), "tenä is missing {good}");
        }
        assert_eq!(w.len(), 5, "tenä should have only its five confirmed forms");
    }

    /// `seesam` inflects as `seesami`, not with a bare-stem possessive
    /// (`*seesamnsa`).
    #[test]
    fn seesam_uses_the_i_stem() {
        let w: Vec<String> = inflect("seesam", true).into_iter().map(|f| f.word).collect();
        assert!(w.contains(&"seesamin".to_string()));
        assert!(!w.contains(&"seesamnsa".to_string()));
    }

    #[test]
    fn no_inflect_adverb_is_lemma_only() {
        // oikein is a genuinely indeclinable adverbial: no i-loan forms.
        let w = inflect("oikein", true)
            .into_iter()
            .map(|f| f.word)
            .collect::<Vec<_>>();
        assert!(w.contains(&"oikein".to_string()));
        assert!(!w.contains(&"oikeinin".to_string()));
        assert_eq!(w.len(), 1, "an indeclinable offers only the lemma");
    }

    #[test]
    fn function_words_do_not_get_i_loan_forms() {
        // `ennen` is an adverb/preposition and `in` is a colloquial adverb;
        // neither takes the i-loan plural. `ennenin` was showing up as a
        // suggestion for `ennenkuin`.
        for lemma in ["ennen", "in", "varuilla", "varuillaan"] {
            let w: Vec<String> = inflect(lemma, true)
                .into_iter()
                .map(|f| f.word)
                .collect();
            assert_eq!(w, vec![lemma.to_string()], "{lemma}");
        }
    }

    #[test]
    fn i_loan_still_inflects() {
        let w = inflect("miss", true)
            .into_iter()
            .map(|f| f.word)
            .collect::<Vec<_>>();
        assert!(w.contains(&"miss".to_string()));
        assert!(w.contains(&"misseihin".to_string()) || w.contains(&"missin".to_string()));
    }
}
