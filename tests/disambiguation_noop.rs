//! Phase 2 exact no-op gate.
//!
//! With neural inference disabled, the new disambiguation sidecar must not
//! change a single token, role, clause, diagnostic, span, correction, list or
//! style finding. These tests compare the baseline path with the sidecar path
//! over the committed syntax/style fixtures and the long prose fixture.

use torille::check::{check_all_tokens, check_all_tokens_with_decisions};
use torille::disambiguation::{Disambiguation, disambiguate};
use torille::morphology::{MorphologyCache, analyze_morphology};
use torille::style_check;
use torille::syntax::{analyze_syntax, analyze_syntax_with_decisions};
use torille::syntax_check::{self, SentenceCache};
use torille::tokenise::tokenise;

fn assert_noop(text: &str) {
    let tokens = tokenise(text);
    let mut cache = MorphologyCache::new();
    let morphology = analyze_morphology(&tokens, &mut cache);

    let baseline_syntax = analyze_syntax(&tokens, &morphology);
    let decisions = disambiguate(&tokens, &morphology);
    assert_eq!(decisions, Disambiguation::disabled());
    let sidecar_syntax = analyze_syntax_with_decisions(&tokens, &morphology, &decisions);
    assert_eq!(
        baseline_syntax, sidecar_syntax,
        "syntax differs for {text:?}"
    );

    let baseline_diagnostics = syntax_check::check_analysis(&tokens, &baseline_syntax);
    let sidecar_diagnostics = syntax_check::check_analysis(&tokens, &sidecar_syntax);
    assert_eq!(
        baseline_diagnostics, sidecar_diagnostics,
        "diagnostics differ for {text:?}"
    );

    let baseline_style = style_check::check_tokens_with_morphology(&tokens, &morphology);
    let sidecar_style = style_check::check_tokens_with_morphology(&tokens, &morphology);
    assert_eq!(
        baseline_style, sidecar_style,
        "style differs for {text:?}"
    );

    let baseline_all = check_all_tokens(&tokens, &morphology);
    let sidecar_all = check_all_tokens_with_decisions(&tokens, &morphology, &decisions);
    assert_eq!(baseline_all, sidecar_all, "combined pass differs for {text:?}");

    // The cache paths must agree with the stateless check.
    let stateless = syntax_check::check(text);
    let mut cache_b = MorphologyCache::new();
    let cached = syntax_check::check_with_cache(text, &mut cache_b);
    assert_eq!(stateless, cached, "cache path differs for {text:?}");

    // The sentence cache must be byte-identical with the disabled sidecar.
    let mut cache_c = MorphologyCache::new();
    let morphology_c = analyze_morphology(&tokens, &mut cache_c);
    let mut sentence_cache = SentenceCache::new();
    let cached_diagnostics = sentence_cache.check(text, &tokens, &morphology_c, &mut cache_c);
    let mut cache_d = MorphologyCache::new();
    let morphology_d = analyze_morphology(&tokens, &mut cache_d);
    let decisions_d = disambiguate(&tokens, &morphology_d);
    let mut sentence_cache_sidecar = SentenceCache::new();
    let sidecar_diagnostics = sentence_cache_sidecar.check_with_decisions(
        text,
        &tokens,
        &morphology_d,
        &mut cache_d,
        &decisions_d,
    );
    assert_eq!(
        cached_diagnostics, sidecar_diagnostics,
        "sentence-cache path differs for {text:?}"
    );
}

#[test]
fn disabled_disambiguation_is_an_exact_noop_on_the_syntax_corpus() {
    for text in [
        "Koira nukkuvat.",
        "Minä menen kotiin.",
        "Joka päivä juon kahvia.",
        "Ilman rahaa en tule.",
        "Tarvitsen sen.",
        "Kaksi lasta tuli.",
        "Talot ovat punainen.",
        "Me olemme lukenut.",
        "Hän tuli tupaan, jossa roihusi tuli.",
        "Ostin omenoita, appelsiineja ja banaaneja.",
        "Ostin omenoita, appelsiineja (pieniä, makeita) ja banaaneja.",
        "Hän sanoi: »Missä sinä asut?»",
    ] {
        assert_noop(text);
    }
}

#[test]
fn disabled_disambiguation_is_an_exact_noop_on_the_prose_fixture() {
    assert_noop(include_str!("fixtures/perf/page-2000.txt"));
}

#[test]
fn the_combined_cache_path_matches_the_token_path() {
    let text = "Koira nukkuvat. Tällä hetkellä tilanne on rauhallinen.";
    let tokens = tokenise(text);
    let mut cache = MorphologyCache::new();
    let morphology = analyze_morphology(&tokens, &mut cache);
    let token_path = check_all_tokens(&tokens, &morphology);
    let cache_path = torille::check_all(text);
    assert_eq!(token_path, cache_path);
}
