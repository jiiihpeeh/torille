//! Benchmarks for the torille spell-checker hot path.
//!
//! Run with `cargo bench --bench torille`. Three groups cover the per-keystroke
//! path the Firefox extension actually exercises:
//!
//!   * `levenshtein` — the edit-distance kernel inside `suggest`. Bounded
//!     distance 2 is the value the daemon and the wasm worker both pass.
//!   * `suggest` — the full "rank candidates, return top-N" path against the
//!     embedded sanalista.
//!   * `form_of` — the new "ask for a specific shape" API: `form_of` over a
//!     lemma filtered to a single Tags.
//!
//! These exist so future optimisations have something to verify against and
//! so the effect of the inlining / `lto = "fat"` / `panic = "abort"` settings
//! is visible in numbers, not in guesswork.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use torille::expand::form_of;
use torille::suggest::suggest as suggest_fn;
use torille::tags::{Case, Number, PartOfSpeech, Tags};

fn catalogue() -> &'static torille::spec::Catalogues {
    // `Catalogues::embedded()` already caches the decoded catalogue in a
    // process-wide `OnceLock`.
    torille::spec::Catalogues::embedded()
}

fn bench_levenshtein(c: &mut Criterion) {
    let cases: &[(&str, &str, usize)] = &[
        ("kitten", "sitting", 3),
        ("taloo", "talo", 1),
        ("koirra", "koira", 2),
        ("xyzzy", "puhtaan", 2),
        ("puhtaan", "öihin", 2),
    ];
    let mut group = c.benchmark_group("levenshtein");
    for (a, b, max_d) in cases {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{a}->{b}/{max_d}")),
            max_d,
            |bencher, &max_d| bencher.iter(|| torille::suggest::levenshtein(a, b, max_d)),
        );
    }
    group.finish();
}

fn bench_suggest(c: &mut Criterion) {
    let words: &[&str] = &["taloo", "koirra", "puhtaan", "öihin", "xyzzy"];
    let mut group = c.benchmark_group("suggest");
    for word in words {
        group.bench_with_input(BenchmarkId::from_parameter(word), word, |bencher, &word| {
            bencher.iter(|| {
                let _ = suggest_fn(word, 5, 2);
            })
        });
    }
    // Single big run to amortise the once-only cold caches.
    group.bench_function("cold_then_hot", |b| {
        b.iter(|| {
            let _ = suggest_fn("taloo", 5, 2);
            let _ = suggest_fn("koirra", 5, 2);
            let _ = suggest_fn("puhtaan", 5, 2);
        })
    });
    group.finish();
}

fn bench_form_of(c: &mut Criterion) {
    let cat = catalogue();
    let cases: &[(&str, &str, u16, Tags)] = &[
        (
            "talo",
            "inessive+sg",
            1,
            Tags {
                case: Some(Case::Inessive),
                number: Some(Number::Singular),
                ..Tags::default()
            },
        ),
        (
            "talo",
            "nominative",
            1,
            Tags {
                case: Some(Case::Nominative),
                ..Tags::default()
            },
        ),
        (
            "lukea",
            "verb-wildcard",
            52,
            Tags {
                part_of_speech: Some(PartOfSpeech::Verb),
                ..Tags::default()
            },
        ),
    ];
    let mut group = c.benchmark_group("form_of");
    for (word, label, class, want) in cases {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{word}/{label}")),
            &(class, *want),
            |bencher, &(class, ref want)| {
                let inf = torille::kotus::Inflection {
                    class: *class,
                    gradation: None,
                };
                bencher.iter(|| form_of(word, inf, cat, want, PartOfSpeech::Noun, true))
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_levenshtein, bench_suggest, bench_form_of);
criterion_main!(benches);
