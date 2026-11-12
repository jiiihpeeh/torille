//! The negation auxiliary and the words it fuses with in writing.
//!
//! Finnish negation is analytic: the auxiliary `en`, `et`, `ei`, `emme`,
//! `ette`, `eivät` precedes a connegative main verb (`en tule`). In
//! writing it fuses with two things:
//!
//!   * the negative clitic `-kä`, the negated reading of `-kin` --
//!     `enkä` = `en` + `-kä`, `eivätkä` = `eivät` + `-kä`; and
//!   * the conjunctions `että` and `ellei` -- `ettet` = `että` + `et`,
//!     `ellette` = `ellei` + `ette`.
//!
//! Neither is a form of a verb paradigm the engine can generate, and both
//! sets are closed. They are recorded in the verb database (the
//! `connectives` table, `tools/oracle/build_verb_lexicon.py`) and embedded
//! by `build.rs` from `data/verb_connectives.csv` as a form -> lemma map,
//! which is what this module reads. The `-kä` glue itself stays a rule:
//! a clitic-stacked form the table does not list (`enkäkö`) still peels
//! back to a finite auxiliary the table does.

use crate::expand::clitic::strip_all_clitics;
use crate::tags::{Number, Person};

mod table {
    use crate::expand::static_table::StemMap;

    /// Negation fusions and finite auxiliaries: form -> lemma, recorded by
    /// `tools/oracle/build_verb_lexicon.py`.
    pub static CONNECTIVES: StemMap = StemMap::new(include_bytes!(concat!(
        env!("OUT_DIR"),
        "/connectives.bin",
    )));
}

/// The base lemma of a negation word, or `None` when `word` is not one.
///
/// Covers the six finite auxiliaries, the twelve fused conjunctions, and
/// either of those with one further clitic (`etteikö`, `enkään`). The
/// negative clitic `-kä` is peeled from an auxiliary only -- `-kä` is not
/// a general clitic, so `kukakä` and the like are not accepted by
/// accident.
///
/// `word` is expected lowercase, as everywhere in the walks. The check is
/// cheap: the auxiliaries start with `e` and the `jollei` fusions with `j`,
/// so a common word is rejected on its first character before any clitic
/// scan.
pub fn negation_base(word: &str) -> Option<&'static str> {
    // The imperative auxiliaries are the same closed construction in a
    // different mood: `älä syö`, `älkää syökö`. They are not forms of `ei`,
    // and `älä` has no verb paradigm of its own in the sanalista, so the
    // table must name the auxiliary itself.
    if matches!(word, "älä" | "älkää" | "älköön" | "älkööt" | "älkäämme") {
        return Some("älä");
    }
    // `vaikka` fused with a finite negative auxiliary is a closed family:
    // `vaikken`, `vaikket`, `vaikkei`, `vaikkemme`, `vaikkette`,
    // `vaikkeivät`. Like `miksei`, it is a constructional word rather than
    // a verb paradigm, and it does not start with `e`/`j`.
    if let Some(base) = vaikka_negation_base(word) {
        return Some(base);
    }
    // `miksei` is an adverb fused with the negation auxiliary. Its six
    // finite forms (`miksen`, `mikset`, ...) are not a verb paradigm the
    // engine can generate, and they do not start with `e`/`j`, so handle
    // the closed family before the cheap prefix gate. A clitic can sit on
    // top (`miksetkö`), so check the stripped host too.
    if let Some(base) = miksei_base(word) {
        return Some(base);
    }
    let (stripped, _) = strip_all_clitics(word);
    if let Some(base) = vaikka_negation_base(stripped.as_str()) {
        return Some(base);
    }
    if let Some(base) = miksei_base(stripped.as_str()) {
        return Some(base);
    }
    if !word.starts_with('e') && !word.starts_with('j') {
        return None;
    }
    if let Some(base) = table::CONNECTIVES.bases(word).next() {
        return Some(base);
    }
    let (host, _stack) = strip_all_clitics(word);
    let host = host.as_str();
    if let Some(base) = table::CONNECTIVES.bases(host).next() {
        return Some(base);
    }
    if let Some(base) = miksei_base(host) {
        return Some(base);
    }
    // The negative clitic `-kä` on an auxiliary the table lists bare.
    if let Some(stem) = host.strip_suffix('ä')
        && let Some(stem) = stem.strip_suffix('k')
        && let Some(base) = table::CONNECTIVES.bases(stem).next()
    {
        return Some(base);
    }
    None
}

/// The finite negative auxiliary for a person and number.
///
/// This is the declarative paradigm (`en`, `et`, `ei`, `emme`, `ette`,
/// `eivät`). The imperative auxiliaries (`älä`, `älkää`, ...) are handled by
/// [`person_number`] for analysis; a correction in a declarative clause uses
/// this table.
pub fn auxiliary(person: Person, number: Number) -> Option<&'static str> {
    Some(match (person, number) {
        (Person::First, Number::Singular) => "en",
        (Person::Second, Number::Singular) => "et",
        (Person::Third, Number::Singular) => "ei",
        (Person::First, Number::Plural) => "emme",
        (Person::Second, Number::Plural) => "ette",
        (Person::Third, Number::Plural) => "eivät",
        _ => return None,
    })
}

/// The person and number carried by a negative auxiliary surface.
///
/// The morphological tags for the bare negative auxiliary do not reliably
/// carry person/number (`en` is a single-token form of `ei` with no finite
/// person), so the closed table supplies them. Clitics and the negative
/// `-kä` are peeled first: `enkä`, `eikö`, `etten`.
pub fn person_number(word: &str) -> Option<(Person, Number)> {
    let lower = word.to_lowercase();
    if let Some(found) = bare_person_number(&lower) {
        return Some(found);
    }
    let (host, _) = strip_all_clitics(&lower);
    if let Some(found) = bare_person_number(&host) {
        return Some(found);
    }
    for candidate in [lower.as_str(), host.as_str()] {
        for suffix in ["kä", "ka"] {
            if let Some(stem) = candidate.strip_suffix(suffix)
                && let Some(found) = bare_person_number(stem)
            {
                return Some(found);
            }
        }
    }
    None
}

fn bare_person_number(word: &str) -> Option<(Person, Number)> {
    Some(match word {
        "en" => (Person::First, Number::Singular),
        "et" | "älä" => (Person::Second, Number::Singular),
        "ei" | "älköön" => (Person::Third, Number::Singular),
        "emme" | "älkäämme" => (Person::First, Number::Plural),
        "ette" | "älkää" => (Person::Second, Number::Plural),
        "eivät" | "älkööt" => (Person::Third, Number::Plural),
        _ => return None,
    })
}

/// The six finite forms of the fused conjunction + negation `vaikka`.
/// `vaikka` drops its final `a` before the auxiliary: `vaikk` + `en` =
/// `vaikken`, `vaikk` + `eivät` = `vaikkeivät`.
fn vaikka_negation_base(word: &str) -> Option<&'static str> {
    let rest = word.strip_prefix("vaikk")?;
    matches!(rest, "en" | "et" | "ei" | "emme" | "ette" | "eivät").then_some("vaikka")
}

/// The six finite forms of the fused adverb + negation `miksei`.
fn miksei_base(word: &str) -> Option<&'static str> {
    let rest = word.strip_prefix("miks")?;
    matches!(rest, "en" | "et" | "ei" | "emme" | "ette" | "eivät").then_some("miksei")
}

#[cfg(test)]
mod tests {
    use super::*;

    const FINITE: [&str; 6] = ["en", "et", "ei", "emme", "ette", "eivät"];

    #[test]
    fn the_auxiliaries_name_ei() {
        for word in FINITE {
            assert_eq!(negation_base(word), Some("ei"), "{word}");
        }
    }

    #[test]
    fn the_auxiliary_paradigm_carries_person_and_number() {
        use crate::tags::{Number, Person};
        for (word, person, number) in [
            ("en", Person::First, Number::Singular),
            ("et", Person::Second, Number::Singular),
            ("ei", Person::Third, Number::Singular),
            ("emme", Person::First, Number::Plural),
            ("ette", Person::Second, Number::Plural),
            ("eivät", Person::Third, Number::Plural),
            ("enkä", Person::First, Number::Singular),
            ("eikö", Person::Third, Number::Singular),
            ("älä", Person::Second, Number::Singular),
            ("älkää", Person::Second, Number::Plural),
        ] {
            assert_eq!(person_number(word), Some((person, number)), "{word}");
            assert!(
                auxiliary(person, number).is_some(),
                "{word} has a declarative correction"
            );
        }
        assert_eq!(auxiliary(Person::Third, Number::Singular), Some("ei"));
    }

    #[test]
    fn the_imperative_auxiliaries_name_ala() {
        for word in ["älä", "älkää", "älköön", "älkööt", "älkäämme"] {
            assert_eq!(negation_base(word), Some("älä"), "{word}");
        }
    }

    #[test]
    fn the_negative_clitic_is_peeled_from_an_auxiliary() {
        for (word, base) in [
            ("enkä", "ei"),
            ("etkä", "ei"),
            ("eikä", "ei"),
            ("emmekä", "ei"),
            ("ettekä", "ei"),
            ("eivätkä", "ei"),
            // a further clitic on top of the -kä
            ("enkäkö", "ei"),
            ("enkään", "ei"),
        ] {
            assert_eq!(negation_base(word), Some(base), "{word}");
        }
        // -kä is not a general clitic: a noun ending in it stays unknown.
        for word in ["kukakä", "kissakä", "matkakä"] {
            assert_eq!(negation_base(word), None, "{word}");
        }
    }

    #[test]
    fn the_fused_conjunctions_name_their_lemma() {
        for (word, base) in [
            ("etten", "että"),
            ("ettet", "että"),
            ("ettei", "että"),
            ("ettemme", "että"),
            ("ettette", "että"),
            ("etteivät", "että"),
            ("ellen", "ellei"),
            ("ellet", "ellei"),
            ("ellei", "ellei"),
            ("ellemme", "ellei"),
            ("ellette", "ellei"),
            ("elleivät", "ellei"),
            // and with a clitic
            ("etteikö", "että"),
            ("ettenkin", "että"),
        ] {
            assert_eq!(negation_base(word), Some(base), "{word}");
        }
    }

    #[test]
    fn the_fused_vaikka_family_names_the_conjunction() {
        for word in [
            "vaikken",
            "vaikket",
            "vaikkei",
            "vaikkemme",
            "vaikkette",
            "vaikkeivät",
            "vaikkeivätkö",
        ] {
            assert_eq!(negation_base(word), Some("vaikka"), "{word}");
        }
    }

    #[test]
    fn the_fused_miksei_family_names_miksei() {
        for word in [
            "miksen",
            "mikset",
            "miksei",
            "miksemme",
            "miksette",
            "mikseivät",
        ] {
            assert_eq!(negation_base(word), Some("miksei"), "{word}");
        }
        assert_eq!(negation_base("miksetkö"), Some("miksei"));
    }

    #[test]
    fn ordinary_e_words_are_left_alone() {
        for word in ["elämä", "eteinen", "esine", "enemmän", "enää"] {
            assert_eq!(negation_base(word), None, "{word}");
        }
    }
}
