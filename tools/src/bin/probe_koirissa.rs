// SPDX-License-Identifier: MIT OR Apache-2.0
//! `probe_koirissa` — print the engine's candidate list for ambiguous
//! inputs like `koirissa` (genitive plural of "koira" vs. inessive
//! plural of "kori").
//!
//! This is a one-off probe, used during the Slice-3 evaluation to
//! confirm the deterministic engine surfaces *both* interpretations
//! before the neural reranker adds anything. The user's review
//! specifically called out `koirissa → korissa` as the kind of case
//! where the reranker should help; if the engine doesn't surface
//! `korissa` for `koirissa`, no reranker can fix it.
//!
//! Run: cargo run --release -p torille-tools --bin probe_koirissa

use torille::suggest::suggest_detailed;

fn main() {
    for word in &["koirissa", "korissa", "koiria", "koira"] {
        println!("=== {word} ===");
        let sugs = suggest_detailed(word, 30, 4);
        for s in sugs.iter().take(20) {
            let src = match &s.source {
                torille::rerank::SuggestionSource::DirectLemma => "direct_lemma",
                torille::rerank::SuggestionSource::NeighbourLemma => "neighbour_lemma",
                torille::rerank::SuggestionSource::GeneratedForm { .. } => "generated_form",
                torille::rerank::SuggestionSource::MorphologyHypothesis { .. } => "morph_hyp",
            };
            println!(
                "  ed={} src={:<14} lemma={:?} word={:?}",
                s.edit_distance, src, s.lemma, s.word,
            );
        }
    }
}
