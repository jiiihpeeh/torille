//! The compile-time subset of the form set, embedded as a front-coded
//! [`StaticSet`].
//!
//! Build-time codegen: `build.rs` reads `data/all_forms.csv` (a
//! dump from `gen_all_forms` walking every lemma's
//! full paradigm) and emits a sorted, front-coded set blob containing
//! the most commonly-typed surface forms -- every bare lemma
//! (nominative singular, no possessive) plus every genitive
//! singular (no possessive), plus the citation of a plural-only lemma.
//! That's ~87 k unique forms.
//!
//! The same set as a `phf::Set` literal was ~10 MB of source and
//! duplicated every key as a `&'static str`; the blob front-codes the
//! sorted keys to ~1.5 MB and the compiler only sees `include_bytes!`.
//! The wasm's `find_misspells` binary-searches it; anything missing
//! falls through to the slow check (`find_bases_with`).
//!
//! We don't have corpus frequency data, so this is a "common
//! spelling" proxy, not a true top-N. The subset chosen (bare
//! lemmas + genitive singular) covers what a typical Finnish
//! typing path produces most often -- lemmas because the user
//! types them whole, genitive singular because it's the most
//! common inflected form for possessives and compounds.

use crate::expand::static_table::StaticSet;

/// The embedded form subset. Built by `build.rs` from
/// `data/all_forms.csv`.
///
/// The blob is checked in at `maps/all_forms.bin.gz` so a
/// fresh `cargo build` just `include_bytes!`s it; `build.rs`
/// overwrites it on CSV change. A probe is a binary search over the
/// front-coded keys (no phf table, no per-key literal).
pub static FORM_SET: StaticSet = StaticSet::new(include_bytes!(concat!(
        env!("OUT_DIR"),
        "/all_forms.bin",
    )));

/// True if `word` is in the embedded form subset.
pub fn is_known_inflected_form(word: &str) -> bool {
    FORM_SET.contains(word)
}

/// Always true: the form set is compile-time, so it's ready
/// immediately at process load. Kept as a function for API
/// compatibility with the previous `OnceLock`-based version --
/// callers that gated on warm-up can keep their guard.
pub fn form_set_ready() -> bool {
    true
}

/// The form set itself, returned for callers that want to iterate
/// or batch-probe.
pub fn form_set() -> &'static StaticSet {
    &FORM_SET
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_set_is_ready_immediately() {
        assert!(form_set_ready());
    }

    #[test]
    fn known_lemma_and_genitive_forms_probe_correctly() {
        // The subset covers lemmas (nominative singular) and
        // genitive singular. Both should land in the embedded
        // set.
        for word in [
            "vesi",   // lemma
            "kissa",  // lemma
            "talo",   // lemma
            "veden",  // gen sg
            "kissan", // gen sg
            "talon",  // gen sg
        ] {
            assert!(
                is_known_inflected_form(word),
                "{word}: form subset should contain this"
            );
        }
    }

    #[test]
    fn out_of_subset_forms_miss() {
        // Partitive, illative, inessive -- NOT in the subset.
        // The slow check handles these; the compile-time set
        // is the fast first-pass for lemmas + gen sg only.
        for word in ["vettä", "kissaa", "taloa", "vedessä"] {
            assert!(
                !is_known_inflected_form(word),
                "{word}: form subset should NOT contain this \
                 (only lemmas + gen sg are in the compile-time set)"
            );
        }
    }

    #[test]
    fn unknown_words_dont_probe() {
        for word in ["xyzzy", "qwerty", "asdfg", "zzz"] {
            assert!(
                !is_known_inflected_form(word),
                "{word}: form subset should NOT contain this"
            );
        }
    }
}
