// SPDX-License-Identifier: MIT OR Apache-2.0
//! Shared helpers for the torille command-line tools.
//!
//! The tools are small test drivers over the `torille` library: they load the
//! two Kotus data files and expose the inflector, the reverse index and the
//! suggestion path to a command line, so a morphology change can be poked
//! without writing a test for it.

use torille::kotus::{Gradation, Inflection, Lemma as LemmaRe};

pub mod der1;
pub mod front_coded;
pub mod gzip;
pub mod stem_key;

use torille::spec::Catalogues;
use torille::tags::PartOfSpeech;

/// Re-exported from `torille::expand` so the bin and integration tests can
/// keep using `torille_tools::find_bases*` as the entry point.
pub use torille::expand::{BasesReport, find_any_base_with, find_bases_with};

/// The paradigm catalogue, embedded in the library.
pub fn catalogue() -> Catalogues {
    Catalogues::embedded().clone()
}

/// The sanalista, embedded in the library and precomputed at build time.
///
/// This is the whole noun lexicon, so a tool needs no `data/` directory next to
/// it. Prefer [`lookup`] over materialising rows: the lexicon is a hash table,
/// so finding a word costs a probe rather than a walk over 104 743 rows.
pub fn sanalista() -> &'static torille::Sanalista {
    torille::kotus::sanalista()
}

/// Every row for `word`, or an empty iterator when it is not in the lexicon.
///
/// The one lookup every tool wants, and the reason the lexicon is a hash table.
pub fn lookup(word: &str) -> impl Iterator<Item = torille::EmbeddedLemma> {
    torille::kotus::sanalista().by_word(word)
}

/// Convenience wrapper around [`find_bases_with`] that loads the embedded
/// sanalista and catalogue for the caller and caches the parsed lexicon.
///
/// The sanalista's 104 743 rows are parsed once into a `Box::leak`-ed
/// `&'static [Lemma<'static>]` and reused on every call -- the same lifetime
/// trick the Python binding uses (see the brief). One parse per process, then
/// `O(1)` lookups.
pub fn find_bases(word: &str) -> BasesReport<'static> {
    use std::sync::OnceLock;
    static LEXICON: OnceLock<Vec<LemmaRe<'static>>> = OnceLock::new();
    let lexicon = LEXICON.get_or_init(|| sanalista().to_lemmas());
    find_bases_with(lexicon, &catalogue(), word)
}

/// Inflect `word` exactly as the binding's `Engine.inflect` does.
///
/// The generator binaries need the same decision the Python binding makes:
/// union every sanalista row's classes, take the noun reading when the word
/// can be a noun, and offer possessive suffixes only then.
pub fn inflect_lemma(
    word: &str,
    catalogue: &Catalogues,
    index: &torille::expand::LexiconIndex<'_>,
) -> Vec<torille::tags::Form> {
    let inflections = resolve_inflections(word);
    let rows: Vec<_> = lookup(word).collect();
    let noun_capable = rows
        .iter()
        .any(|row| PartOfSpeech::is_substantival(row.part_of_speech()));
    let pos = if noun_capable {
        PartOfSpeech::Noun
    } else {
        rows.first()
            .map(|row| PartOfSpeech::inflecting_reading(row.part_of_speech()))
            .unwrap_or(PartOfSpeech::Unknown)
    };
    torille::expand::inflect_all(word, &inflections, catalogue, Some(index), pos, noun_capable)
}

/// Short-circuit version of [`find_bases`] for the inflect check.
///
/// The inflect check answers "is there *any* base whose paradigm
/// emits this word?" -- a yes/no question the wasm's typing path
/// fires for every word. The full [`find_bases`] walk produces
/// 3097 neighbours and 165 generated forms for a single Finnish
/// verb; the inflect check doesn't need any of that. The
/// suggestion pool does, but the suggestion pool is a separate
/// code path (the popup, not the first pass).
///
/// [`find_any_base_with`] short-circuits on the first matching
/// base. The wasm's `check_fast` and `check_normal`'s gate 4 use
/// this; the suggestion ranker uses [`find_bases_with`].
///
/// Returns the first matching lemma's word, or `None`.
pub fn find_any_base(word: &str) -> Option<String> {
    use std::sync::OnceLock;
    static LEXICON: OnceLock<Vec<LemmaRe<'static>>> = OnceLock::new();
    let lexicon = LEXICON.get_or_init(|| sanalista().to_lemmas());
    find_any_base_with(lexicon, &catalogue(), word)
        .first()
        .map(|l| l.word.to_string())
}

/// Parses a Kotus class expression such as `73`, `7*D` or `9*B*A*A` into an
/// [`Inflection`]. Returns `None` if it is not one.
pub fn parse_inflection(s: &str) -> Option<Inflection> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    // The class is the leading digits; the gradation letters follow any `*`.
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    let class: u16 = digits.parse().ok()?;
    let mut gradation = None;
    for c in s[digits.len()..].chars() {
        if c.is_ascii_uppercase() {
            gradation = Some(Gradation::from_letter(c)?);
        }
    }
    Some(Inflection { class, gradation })
}

/// The classes a lemma should be inflected in, given every row the sanalista
/// has for it.
///
/// This is the counterpart of the Python binding's `plan`, and it exists
/// because two callers resolving the same sanalista row differently is how the
/// `pieni` and `ori` disagreements happened in the first place. The binding
/// inflects the *union* of the rows' classes; taking the first row's first
/// class instead silently drops a whole paradigm, which for `ori` (`48,5`) is
/// 178 forms.
///
/// Three reductions, each for a stated reason:
///
/// - A class listed both bare and graded (`hepo` is `1*E` and `1`) keeps only
///   the graded one. Both confirm the same paradigm, so the bare entry
///   contributes ungradated forms no holder of the class confirms.
/// - A word listed as `99` (the indeclinable marker) beside a real class keeps
///   the real class. `99`'s own i-loan paradigm would be fabricated forms.
/// - A compound marker (50, 51) replaces the head class beside it. `nuoripari`
///   is `5, 51` and the 51 reading is the compound; running 5 over the whole
///   word gives `nuoripareiden`.
///
/// Distinct classes otherwise all run: `vihanta` is `10*J` *and* `9*J` and
/// really is two declensions. Order is the sanalista's, because it decides the
/// order forms are emitted in.
pub fn resolve_inflections(lemma: &str) -> Vec<Inflection> {
    // Collected inside the closure chain: `EmbeddedLemma::inflections` borrows the
    // row, so the flat_map cannot return through it.
    let mut all: Vec<Inflection> = Vec::new();
    for row in lookup(lemma) {
        all.extend(row.inflections());
    }
    if all.is_empty() {
        return all;
    }
    let graded: Vec<u16> = all
        .iter()
        .filter(|i| i.gradation.is_some())
        .map(|i| i.class)
        .collect();
    all.retain(|i| i.gradation.is_some() || !graded.contains(&i.class));
    if all.iter().any(|i| i.class != 99) {
        all.retain(|i| i.class != 99);
    }
    let markers: Vec<Inflection> = all.iter().copied().filter(is_marker).collect();
    if !markers.is_empty() {
        // A compound's class list names its head's class beside the marker
        // (`nuoripari` is `5, 51`), so the head class is dropped. Both markers
        // are kept when the row names both: `isoviha` and `isopurje` are
        // `51, 50`, and the 50 reading is the one that keeps the initial part
        // fixed (`isovihalta`, `isovihalle`), which the user-facing paradigm
        // needs beside the class-51 reading (`isonvihalta`).
        all = markers;
    }
    all
}

/// Whether an inflection is one of the compound type markers.
fn is_marker(inf: &Inflection) -> bool {
    inf.class == 50 || inf.class == 51
}

/// Parses a part-of-speech name into a [`PartOfSpeech`], accepting the Finnish
/// name the sanalista uses, the English name, or nothing (defaulting to the
/// word's own reading).
pub fn parse_pos(s: &str) -> Option<PartOfSpeech> {
    let s = s.trim().to_lowercase();
    if s.is_empty() {
        return None;
    }
    PartOfSpeech::parse_all(&s).into_iter().next()
}

/// Formats a form as `word [confidence]`, with the reading on the side when it
/// is not unknown.
pub fn format_form(f: &torille::tags::Form) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    write!(out, "{:<28}", f.word).unwrap();
    let conf = match f.confidence {
        torille::tags::Confidence::Plain => "plain",
        torille::tags::Confidence::Repaired => "repaired",
        torille::tags::Confidence::Guessed => "guessed",
    };
    write!(out, "{:<9}", conf).unwrap();
    let tags = f.union();
    let mut parts: Vec<&str> = Vec::new();
    if let Some(pos) = tags.part_of_speech {
        parts.push(pos.name());
    }
    if let Some(case) = tags.case {
        parts.push(case.name());
    }
    if let Some(n) = tags.number {
        parts.push(n.name());
    }
    if let Some(t) = tags.tense {
        parts.push(t.name());
    }
    if let Some(p) = tags.person {
        parts.push(p.name());
    }
    if let Some(m) = tags.mood {
        parts.push(m.name());
    }
    if let Some(v) = tags.voice {
        parts.push(v.name());
    }
    if let Some(p) = tags.possessive {
        parts.push(possessive_name(p));
    }
    if let Some(c) = tags.clitic {
        parts.push(clitic_name(c));
    }
    if tags.polarity == torille::tags::Polarity::Negative {
        parts.push("negative");
    }
    write!(out, "{}", parts.join(" ")).unwrap();
    out
}

/// The name of a clitic, for the tools' output.
fn clitic_name(c: torille::tags::Clitic) -> &'static str {
    use torille::tags::Clitic;
    match c {
        Clitic::Ko => "clitic ko",
        Clitic::Kin => "clitic kin",
        Clitic::Kaan => "clitic kaan",
        Clitic::Han => "clitic han",
        Clitic::Pa => "clitic pa",
    }
}

/// The name of a possessive, for the tools' output.
fn possessive_name(p: torille::tags::Possessive) -> &'static str {
    use torille::tags::Possessive;
    match p {
        Possessive::FirstSingular => "poss 1sg",
        Possessive::SecondSingular => "poss 2sg",
        Possessive::Third => "poss 3",
        Possessive::FirstPlural => "poss 1pl",
        Possessive::SecondPlural => "poss 2pl",
    }
}

/// Decodes a single gradation letter (or its absence) the way the sanalista
/// spells it -- `A` for `A`, `B` for `B`, `C` for `C`, `D` for `D` -- so the
/// class string reads back exactly as it does in `data/kotus-tyypit.txt`.
pub fn gradation_letter(g: Option<Gradation>) -> String {
    match g {
        Some(t) => format!("*{}", t.0 as char),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classes(lemma: &str) -> Vec<u16> {
        resolve_inflections(lemma)
            .into_iter()
            .map(|i| i.class)
            .collect()
    }

    /// Every class a lemma is listed under is a paradigm to run.
    ///
    /// `ori` is `48,5` and the two grids are disjoint: the `48` grid reads
    /// `oreen, oreella, oreeseen` and the `5` grid `oreiden, oreilla, oreihin`.
    /// Taking the first -- which is what this did -- threw away 178 forms and
    /// was the last disagreement between the binary and the binding.
    #[test]
    fn a_multi_class_lemma_keeps_every_class() {
        assert_eq!(classes("ori"), vec![48, 5]);
        // A class listed bare and graded is one paradigm, not two.
        assert_eq!(classes("hepo"), vec![1]);
        // The indeclinable marker beside a real class loses.
        assert_eq!(classes("aika"), vec![9]);
        // A compound marker replaces the head class beside it.
        assert_eq!(classes("nuoripari"), vec![51]);
        assert_eq!(classes("ahkeraliisa"), vec![50]);
        // Both compound readings are kept when the row names both. `isoviha` is
        // `51, 50`: 51 inflects the initial part (`isonvihan`), 50 keeps it
        // fixed (`isovihan`, `isovihalta`, `isovihalle`).
        assert_eq!(classes("isoviha"), vec![51, 50]);
        assert_eq!(classes("isopurje"), vec![51, 50]);
        // And the ordinary case is untouched.
        assert_eq!(classes("talo"), vec![1]);
    }

    /// The reductions above are the whole of what this function does, so a
    /// lemma that should keep everything has to exist.
    ///
    /// Without this the test above passes vacuously if the function ever starts
    /// returning a single class for everything -- which is the defect it
    /// originally had.
    #[test]
    fn the_reductions_do_not_swallow_ordinary_multi_class_words() {
        let kept = resolve_inflections("vihanta");
        assert_eq!(
            kept.len(),
            2,
            "vihanta is 10*J and 9*J, two real declensions: {kept:?}"
        );
        assert!(kept.iter().any(|i| i.class == 9 && i.gradation.is_some()));
        assert!(kept.iter().any(|i| i.class == 10 && i.gradation.is_some()));
        assert!(!resolve_inflections("teatteri").is_empty());
        // An unknown word has nothing to resolve, rather than a wrong default.
        assert!(resolve_inflections("zzzznotaword").is_empty());
    }

    /// Smoke test on the re-exported find_bases. The four-phase contract is
    /// pinned by tests in `src/expand/find_bases.rs`; this one just makes
    /// sure the wrapper still talks to the engine after the move.
    #[test]
    fn find_bases_still_works_through_the_tools_wrapper() {
        let r = find_bases("arvailen");
        assert!(
            r.bases.iter().any(|l| l.word == "arvailla"),
            "arvailen -> arvailla: {r:?}"
        );
    }
}
