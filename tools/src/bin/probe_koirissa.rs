// SPDX-License-Identifier: MIT OR Apache-2.0
//! `probe_koirissa` — print the engine's candidate list for ambiguous
//! inputs like `koirissa` (genitive plural of "koira" vs. inessive
//! plural of "kori").
//!
//! The probe exists to check whether the engine surfaces *both*
//! interpretations before any later ranking layer runs: if it does not
//! surface `korissa` for `koirissa`, nothing downstream can recover it.
//!
//! Run: cargo run --release -p torille-tools --bin probe_koirissa

use torille::suggest::suggest_detailed;

fn main() {
    for word in &["koirissa", "korissa", "koiria", "koira"] {
        println!("=== {word} ===");
        let sugs = suggest_detailed(word, 30, 4);
        for s in sugs.iter().take(20) {
            let src = match &s.source {
                torille::suggest::SuggestionSource::DirectLemma => "direct_lemma",
                torille::suggest::SuggestionSource::NeighbourLemma => "neighbour_lemma",
                torille::suggest::SuggestionSource::GeneratedForm { .. } => "generated_form",
            };
            println!(
                "  ed={} src={:<14} lemma={:?} word={:?}",
                s.edit_distance, src, s.lemma, s.word,
            );
        }
    }
}
