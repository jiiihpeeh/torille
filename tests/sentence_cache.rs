//! The per-unit sentence cache must agree with the whole-document pass.
//!
//! `tests/fixtures/perf/line-wrap.txt` carries the three contexts that used to
//! diverge: a clause wrapped across a newline, a `--kun` continuation, and a
//! `kuule,` + `miten` continuation. The page-2000 test is the corpus gate.

use torille::morphology::MorphologyCache;
use torille::syntax_check::{self, SentenceCache, SyntaxDiagnostic};
use torille::tokenise::tokenise;

fn cache_check(text: &str) -> Vec<SyntaxDiagnostic> {
    let tokens = tokenise(text);
    let mut morphology_cache = MorphologyCache::new();
    let morphology = torille::morphology::analyze_morphology(&tokens, &mut morphology_cache);
    let mut cache = SentenceCache::new();
    cache.check(text, &tokens, &morphology, &mut morphology_cache)
}

fn whole_check(text: &str) -> Vec<SyntaxDiagnostic> {
    let tokens = tokenise(text);
    let analysis = torille::syntax::analyze(&tokens);
    syntax_check::check_analysis(&tokens, &analysis)
}

#[test]
fn the_cache_matches_the_whole_pass_on_wrapped_lines() {
    let text = include_str!("fixtures/perf/line-wrap.txt");
    assert_eq!(cache_check(text), whole_check(text));
}

#[test]
fn a_wrapped_clause_is_quiet_in_both_paths() {
    let text = include_str!("fixtures/perf/line-wrap.txt");
    for path in [cache_check(text), whole_check(text)] {
        let divergent: Vec<&str> = path
            .iter()
            .filter(|diagnostic| {
                matches!(
                    diagnostic.rule_id().as_str(),
                    "existential_subject_case" | "comma_missing" | "question_mark_missing"
                )
            })
            .map(|diagnostic| diagnostic.rule_id().as_str())
            .collect();
        assert!(divergent.is_empty(), "{divergent:?}: {path:#?}");
    }
}

/// The bare line fragments are exactly what the cache used to analyse, and
/// each still reproduces the old finding. This pins the mechanism the unit
/// grouping removes.
#[test]
fn the_bare_line_fragments_still_show_the_old_findings() {
    for (text, rule) in [
        ("Siellä oli laulujen\n", "existential_subject_case"),
        (
            "kun ajattelin niitä arvoja, mitkä niillä on maailmalle\n",
            "comma_missing",
        ),
        (
            "miten kauheasti puut heiluvat ja miten kovasti kaikki lehdet värisevät.\n",
            "question_mark_missing",
        ),
    ] {
        let diagnostics = syntax_check::check(text);
        assert!(
            diagnostics
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.rule_id().as_str() == rule),
            "{text:?} should still reproduce {rule}: {diagnostics:#?}"
        );
    }
}

#[test]
fn the_cache_matches_the_whole_pass_on_page_2000() {
    let text = include_str!("fixtures/perf/page-2000.txt");
    assert_eq!(cache_check(text), whole_check(text));
}
