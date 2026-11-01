//! Class 38 (the nainen type) inflection, written by hand.
//!
//! Class 38 is the `-nen` family (aakkonen, nainen, aamiainen): the trailing
//! `-nen` reads `s` (`aakkonen -> aakkos`), whose `e`-stem carries the singular
//! obliques (`aakkosen`, `aakkosessa`) and whose bare consonant stem the
//! partitive and plural (`aakkosta`, `aakkosia`). It reuses the class-32
//! consonant-word engine with the computed stems.
#![allow(dead_code)]

use crate::tags::{Form, Number};

use super::class_32::inflect_with_stems_harmony;
use super::harmony::class_38_harmony;

pub const CLASS: &str = "38";

/// The `-nen`/`-set` -> `s` consonant stem: aakkonen -> aakkos, aakkoset -> aakkos.
fn istem(lemma: &str) -> String {
    if lemma.ends_with("nen") || lemma.ends_with("set") {
        format!("{}s", &lemma[..lemma.len() - 3])
    } else {
        lemma.to_string()
    }
}

/// The `-inen` singular of a plural-only `-iset` lemma.
///
/// `läksiäiset` is the nominative plural of `läksiäinen`, and the sanalista
/// lists the plural as the lemma; Wiktionary's `{{fi-subs-nainen|...|yks=-}}`
/// marks the singular as omitted. The singular cells the base would build are
/// therefore not forms of the word.
fn plural_only_inen(lemma: &str) -> Option<String> {
    let stem = lemma.strip_suffix("set")?;
    if stem.ends_with(['i', 'ä', 'ö']) {
        Some(format!("{stem}nen"))
    } else {
        None
    }
}

/// Inflect a class-38 noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    // A plural-only `-iset` lemma declines from its `-inen` singular and keeps
    // only the plural cells; the nominative plural the base builds is the lemma
    // itself (`läksiäiset`).
    if let Some(base) = plural_only_inen(lemma) {
        let mut forms = inflect(&base, possessives);
        forms.retain(|f| f.readings.iter().any(|r| r.number == Some(Number::Plural)));
        return forms;
    }
    let is = istem(lemma);
    let estem = format!("{is}e");
    // The `-nen` family is spelled with nothing but neutral vowels, so ordinary
    // harmony reads the word's *earlier* syllables and misses the foreign
    // element in a loanword: `biogeeninen` is front all the way through except
    // for the `o` of `geo`, and that `o` is what the unadjusted scan finds.
    // `class_38_harmony` is where the measured exceptions live.
    //
    // These are adjectives and nouns, so the class itself cannot say which
    // column a given lemma takes: that comes from the caller's part of speech.
    // Passing it through rather than deciding here is what lets an adjective
    // (`punainen`) drop its possessive column while a substantival one keeps it.
    inflect_with_stems_harmony(lemma, possessives, &is, &estem, class_38_harmony(lemma))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }
    #[test]
    fn nen() {
        let w = words("aakkonen");
        assert!(w.contains(&"aakkosen".to_string()));
        assert!(w.contains(&"aakkosta".to_string()));
        assert!(w.contains(&"aakkosena".to_string()));
        assert!(w.contains(&"aakkosia".to_string()));
        assert!(w.contains(&"aakkosien".to_string()));
    }

    /// A plural-only `-iset` lemma declines from its `-inen` singular and
    /// invents no singular cells.
    ///
    /// `läksiäiset` and `avajaiset` are the nominative plural of `läksiäinen`
    /// and `avajainen`, and Wiktionary marks them `{{monikollinen}}` with
    /// `yks=-`. The engine used to build `läksiäisen` and `läksiäistä`, which
    /// live Voikko rejects.
    #[test]
    fn iset_is_plural_only() {
        for (lemma, wants, bad) in [
            (
                "läksiäiset",
                &["läksiäiset", "läksiäisten", "läksiäisiä", "läksiäisissä"][..],
                &["läksiäisen", "läksiäistä", "läksiäisessä"][..],
            ),
            (
                "avajaiset",
                &["avajaiset", "avajaisten", "avajaisia"][..],
                &["avajaisen", "avajaista"][..],
            ),
        ] {
            let w = words(lemma);
            for want in wants {
                assert!(w.contains(&want.to_string()), "{lemma} missing {want}");
            }
            for wrong in bad {
                assert!(!w.contains(&wrong.to_string()), "{lemma} invented {wrong}");
            }
        }
    }
}
