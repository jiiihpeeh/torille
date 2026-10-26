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
        IPlural::All,
    ));
    let mut seen = std::collections::HashSet::new();
    out.retain(|f| seen.insert(f.word.clone()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indeclinable() {
        let w = inflect("kyse", true)
            .into_iter()
            .map(|f| f.word)
            .collect::<Vec<_>>();
        assert!(w.contains(&"kyse".to_string()));
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
