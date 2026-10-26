//! Python / Rust parity for the Model-A inference path.
//!
//! The model is trained in Python (`tools/train/`) and inferred in Rust
//! (`src/rerank/mlp.rs`). This test loads the committed fixture — a
//! seeded Model A at production dimensions plus every intermediate the
//! Python side computed — and asserts the two agree on:
//!
//! * the character encoding, boundary framing and n-gram hashes,
//! * the edit-feature vector and the OSA operation counts behind it,
//! * the full feature matrix `x`,
//! * the raw logit,
//! * and, most importantly, the **ranking**: the argmax index and the
//!   complete candidate order.
//!
//! Logits are compared within a tolerance: both sides do float32
//! arithmetic but accumulate in a different order (numpy tiles the dot
//! product, Rust walks it linearly), so they agree to ~1e-6 rather than
//! bit-for-bit. Rankings are compared **exactly** — a ranking is the
//! product, and a tie broken the wrong way silently flips a candidate.
//!
//! Regenerate the fixtures with:
//!
//! ```text
//! python3 tools/train/parity_test.py --output tests/fixtures/rerank_parity.json
//! ```
//!
//! That writes both the JSON and the `.trm` weights next to it, so the
//! two can never describe different models.

use serde_json::Value;

use torille::rerank::mlp::{
    Candidate, ModelA, PAD_IDX, Scratch, bigram_features, boundary_tokens, edit_features,
    encode_chars, levenshtein_osa, trigram_features,
};

const TRM: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/rerank_parity.trm"
);
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/rerank_parity.json"
);

/// Tolerance for a float32 quantity computed in a different summation
/// order by numpy and by Rust.
const TOL: f32 = 1e-4;

fn fixture() -> (ModelA, Value) {
    let model = ModelA::load(TRM).unwrap_or_else(|e| panic!("load {TRM}: {e}"));
    let raw = std::fs::read_to_string(FIXTURE).unwrap_or_else(|e| panic!("read {FIXTURE}: {e}"));
    let value: Value = serde_json::from_str(&raw).expect("fixture is valid JSON");
    assert_eq!(
        value["schema_version"].as_u64(),
        Some(1),
        "fixture schema_version changed; regenerate both files together"
    );
    (model, value)
}

fn close(got: f32, want: f32) -> bool {
    (got - want).abs() <= TOL + TOL * want.abs()
}

fn as_f32(v: &Value) -> f32 {
    v.as_f64().expect("expected a number") as f32
}

/// Read a JSON array of single-character strings as `Vec<char>`.
fn chars_of(v: &Value) -> Vec<char> {
    v.as_array()
        .expect("expected an array of characters")
        .iter()
        .map(|s| {
            let s = s.as_str().expect("expected a string");
            let mut it = s.chars();
            let c = it.next().unwrap_or_else(|| panic!("empty string in {v}"));
            assert!(it.next().is_none(), "expected one character, got {s:?}");
            c
        })
        .collect()
}

fn num(v: &Value) -> usize {
    v.as_u64().expect("expected an integer") as usize
}

/// The order Python's `sorted(range(n), key=lambda i: (-logit, i))`
/// produces: highest logit first, ties broken toward the lowest index.
///
/// Rust's `ModelA::argmax` makes the same tie-break promise, and this
/// is where it gets checked.
fn python_order(logits: &[f32]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..logits.len()).collect();
    order.sort_by(|&a, &b| {
        let ord = logits[b]
            .partial_cmp(&logits[a])
            .expect("logits must not be NaN");
        ord.then(a.cmp(&b))
    });
    order
}

// ---------------------------------------------------------------------------
//   The model file and the architecture block.
// ---------------------------------------------------------------------------

#[test]
fn weights_and_architecture_match_the_fixture() {
    let (m, f) = fixture();
    let a = &f["architecture"];
    let dim = |key: &str| -> usize {
        a[key]
            .as_u64()
            .unwrap_or_else(|| panic!("architecture.{key}")) as usize
    };
    assert_eq!(m.embed_dim, dim("embed_dim"));
    assert_eq!(m.hidden_dim, dim("hidden_dim"));
    assert_eq!(m.bigram_dim, dim("bigram_dim"));
    assert_eq!(m.trigram_dim, dim("trigram_dim"));
    assert_eq!(m.edit_dim, dim("edit_dim"));
    assert_eq!(m.n_in(), dim("n_in"));
    assert_eq!(m.param_count(), dim("param_count"));

    // The derived input width must be self-consistent with the stored
    // one, or every row in `score_batch` is sliced at the wrong offset.
    assert_eq!(
        m.n_in(),
        2 * (m.embed_dim + m.bigram_dim + m.trigram_dim) + m.edit_dim
    );

    // Production Model A: no morphology, no lemma, no frequency, no
    // edit-op counts. If this trips, the fixture is no longer Model A.
    assert_eq!(m.trigram_dim, 0, "production Model A has no trigrams");
    assert!(
        m.edit_dim < 10,
        "production Model A has no edit-op features (edit_dim={})",
        m.edit_dim
    );
}

// ---------------------------------------------------------------------------
//   Feature extraction.
// ---------------------------------------------------------------------------

#[test]
fn character_encoding_and_ngram_framing_match_python() {
    let (m, f) = fixture();
    for case in f["cases"].as_array().expect("cases") {
        for key in ["query_features", "candidate_features"] {
            let dump = &case[key];
            let word = dump["word"].as_str().expect("word");
            let ids = encode_chars(word);

            let want_ids = dump["char_ids"].as_array().expect("char_ids");
            assert_eq!(ids.len(), want_ids.len(), "char_ids length for {word:?}");
            for i in 0..ids.len() {
                assert_eq!(
                    ids[i],
                    num(&want_ids[i]) as u16,
                    "char_ids[{i}] for {word:?}"
                );
            }

            let want_mask = dump["char_valid_mask"].as_array().expect("valid mask");
            for i in 0..ids.len() {
                assert_eq!(
                    (ids[i] != PAD_IDX) as u64,
                    want_mask[i].as_u64().expect("mask bit"),
                    "char_valid_mask[{i}] for {word:?}"
                );
            }

            let want_tokens = chars_of(&dump["boundary_tokens"]);
            assert_eq!(
                boundary_tokens(word),
                want_tokens,
                "boundary tokens for {word:?}"
            );

            let want_bg = dump["bigram_indices"].as_array().expect("bigram_indices");
            let framed = boundary_tokens(word);
            assert_eq!(
                want_bg.len(),
                framed.len().saturating_sub(1),
                "bigram count for {word:?}"
            );
            for i in 0..want_bg.len() {
                let h = (framed[i] as u32 * 257 + framed[i + 1] as u32) as usize % m.bigram_dim;
                assert_eq!(num(&want_bg[i][0]), i, "bigram position {i} for {word:?}");
                assert_eq!(num(&want_bg[i][1]), h, "bigram hash {i} for {word:?}");
            }
        }
    }
}

/// The n-gram hashes themselves, at several widths.
///
/// Production Model A has `trigram_dim == 0`, so the model fixture on
/// its own never touches the trigram path — these vectors do. They are
/// integer counts, so the comparison is exact.
#[test]
fn ngram_hashes_match_python_exactly() {
    let (_m, f) = fixture();
    let entries = f["ngram_parity"].as_array().expect("ngram_parity");
    assert!(
        !entries.is_empty(),
        "fixture should cover the n-gram hashes"
    );
    for e in entries {
        let word = e["word"].as_str().expect("word");
        let dim = e["dim"].as_u64().expect("dim") as usize;
        assert_eq!(boundary_tokens(word), chars_of(&e["boundary_tokens"]));

        let want_bg = e["bigram"].as_array().expect("bigram vector");
        let got_bg = bigram_features(word, dim);
        assert_eq!(
            got_bg.len(),
            want_bg.len(),
            "bigram width for {word:?} @ {dim}"
        );
        for i in 0..want_bg.len() {
            assert_eq!(
                got_bg[i],
                as_f32(&want_bg[i]),
                "bigram[{i}] for {word:?} at dim {dim}"
            );
        }

        let want_tr = e["trigram"].as_array().expect("trigram vector");
        let got_tr = trigram_features(word, dim);
        assert_eq!(
            got_tr.len(),
            want_tr.len(),
            "trigram width for {word:?} @ {dim}"
        );
        for i in 0..want_tr.len() {
            assert_eq!(
                got_tr[i],
                as_f32(&want_tr[i]),
                "trigram[{i}] for {word:?} at dim {dim}"
            );
        }
    }
}

#[test]
fn edit_features_and_osa_counts_match_python() {
    let (m, f) = fixture();
    for case in f["cases"].as_array().expect("cases") {
        let q = case["query"].as_str().expect("query");
        let c = case["candidate"].as_str().expect("candidate");
        let ed = case["engine_edit_distance"].as_u64().expect("ed") as i32;

        let want = case["edit_features"].as_array().expect("edit_features");
        let got = edit_features(q, c, ed, m.edit_dim);
        assert_eq!(got.len(), want.len(), "edit dim for ({q:?}, {c:?})");
        for i in 0..want.len() {
            assert!(
                close(got[i], as_f32(&want[i])),
                "edit_features[{i}] for ({q:?}, {c:?}): got {}, want {}",
                got[i],
                as_f32(&want[i])
            );
        }

        // The OSA counts that feed the op-count and position blocks.
        let qv: Vec<char> = q.chars().collect();
        let cv: Vec<char> = c.chars().collect();
        let osa = levenshtein_osa(&qv, &cv);
        let w = &case["python_osa"];
        let expect = |key: &str| -> usize { w[key].as_u64().expect(key) as usize };
        assert_eq!(
            osa.distance,
            expect("distance"),
            "OSA distance for ({q:?}, {c:?})"
        );
        assert_eq!(
            osa.substitutions,
            expect("substitutions"),
            "substitutions for ({q:?}, {c:?})"
        );
        assert_eq!(
            osa.insertions,
            expect("insertions"),
            "insertions for ({q:?}, {c:?})"
        );
        assert_eq!(
            osa.deletions,
            expect("deletions"),
            "deletions for ({q:?}, {c:?})"
        );
        assert_eq!(
            osa.transpositions,
            expect("transpositions"),
            "transpositions for ({q:?}, {c:?})"
        );
        assert_eq!(
            osa.edit_start,
            expect("edit_start"),
            "edit_start for ({q:?}, {c:?})"
        );
        assert_eq!(
            osa.edit_end,
            expect("edit_end"),
            "edit_end for ({q:?}, {c:?})"
        );
    }
}

// ---------------------------------------------------------------------------
//   The forward pass.
// ---------------------------------------------------------------------------

#[test]
fn feature_matrix_matches_python() {
    let (m, f) = fixture();
    for case in f["cases"].as_array().expect("cases") {
        let q = case["query"].as_str().expect("query");
        let c = case["candidate"].as_str().expect("candidate");
        let ed = case["engine_edit_distance"].as_u64().expect("ed") as i32;

        let query = m.query_features(q);
        let cand = Candidate::new(c, ed);
        let got = m.feature_vector(&query, &cand);
        let want = case["x_vector"].as_array().expect("x_vector");

        assert_eq!(got.len(), want.len(), "x width for ({q:?}, {c:?})");
        let mut worst = (0usize, 0.0f32);
        for i in 0..want.len() {
            let w = as_f32(&want[i]);
            if !close(got[i], w) {
                let err = (got[i] - w).abs();
                if err > worst.1 {
                    worst = (i, err);
                }
            }
        }
        assert_eq!(
            worst,
            (0, 0.0),
            "x[{}] for ({q:?}, {c:?}): got {}, want {}",
            worst.0,
            got[worst.0],
            as_f32(&want[worst.0])
        );
    }
}

#[test]
fn logits_match_python() {
    let (m, f) = fixture();
    for case in f["cases"].as_array().expect("cases") {
        let q = case["query"].as_str().expect("query");
        let c = case["candidate"].as_str().expect("candidate");
        let ed = case["engine_edit_distance"].as_u64().expect("ed") as i32;

        let query = m.query_features(q);
        let mut scratch = Scratch::default();
        let logits = m.score_batch(&query, &[Candidate::new(c, ed)], &mut scratch);
        assert_eq!(logits.len(), 1);
        let want = as_f32(&case["logit"]);
        assert!(
            close(logits[0], want),
            "logit for ({q:?}, {c:?}): got {}, want {}",
            logits[0],
            want
        );
    }
}

// ---------------------------------------------------------------------------
//   Ranking -- the actual product.
// ---------------------------------------------------------------------------

#[test]
fn rankings_match_python_argmax_and_order() {
    let (m, f) = fixture();
    let rankings = f["rankings"].as_array().expect("rankings");
    assert!(!rankings.is_empty());
    for r in rankings {
        let q = r["query"].as_str().expect("query");
        let words: Vec<&str> = r["candidates"]
            .as_array()
            .expect("candidates")
            .iter()
            .map(|v| v.as_str().expect("candidate"))
            .collect();
        let eds: Vec<i32> = r["edit_distances"]
            .as_array()
            .expect("edit_distances")
            .iter()
            .map(|v| v.as_i64().expect("ed") as i32)
            .collect();
        let want_logits = r["logits"].as_array().expect("logits");
        let want_order: Vec<usize> = r["order"]
            .as_array()
            .expect("order")
            .iter()
            .map(|v| v.as_u64().expect("index") as usize)
            .collect();
        let want_argmax = r["argmax"].as_u64().expect("argmax") as usize;

        let query = m.query_features(q);
        let cands: Vec<Candidate> = words
            .iter()
            .zip(eds.iter())
            .map(|(&w, &e)| Candidate::new(w, e))
            .collect();

        let mut scratch = Scratch::default();
        let logits = m.score_batch(&query, &cands, &mut scratch).to_vec();

        assert_eq!(logits.len(), want_logits.len(), "logit count for {q:?}");
        for i in 0..logits.len() {
            assert!(
                close(logits[i], as_f32(&want_logits[i])),
                "logits[{i}] ({q:?} -> {:?}): got {}, want {}",
                words[i],
                logits[i],
                as_f32(&want_logits[i])
            );
        }

        let got_order = python_order(&logits);
        assert_eq!(
            got_order, want_order,
            "candidate order for {q:?}: logits {:?}",
            logits
        );
        assert_eq!(
            ModelA::argmax(&logits),
            Some(want_argmax),
            "argmax for {q:?}"
        );
        assert_eq!(
            got_order[0], want_argmax,
            "argmax must be the top of the order"
        );
        assert_eq!(m.top1(&query, &cands, &mut scratch), Some(want_argmax));
    }
}

/// An empty candidate list must score to nothing at all — not one
/// phantom candidate, not a panic. The fixture's own API permits it, so
/// the production path has to handle it exactly.
#[test]
fn empty_candidate_list_is_empty_end_to_end() {
    let (m, f) = fixture();
    let q = f["rankings"][0]["query"].as_str().expect("query");
    let query = m.query_features(q);
    let mut scratch = Scratch::default();
    let logits = m.score_batch(&query, &[], &mut scratch);
    assert!(logits.is_empty());
    assert_eq!(m.top1(&query, &[], &mut scratch), None);
}

/// Scoring a query twice from the same `QueryFeatures` must give the
/// same answer, and scoring it through a *reused* `Scratch` must too —
/// that is what makes the "query features computed once" property safe
/// across a batch of queries in a long-running process.
#[test]
fn reused_scratch_does_not_leak_between_queries() {
    let (m, f) = fixture();
    let mut scratch = Scratch::default();
    let mut results: Vec<Vec<f32>> = Vec::new();
    for r in f["rankings"].as_array().expect("rankings") {
        let query = m.query_features(r["query"].as_str().expect("query"));
        let cands: Vec<Candidate> = r["candidates"]
            .as_array()
            .expect("candidates")
            .iter()
            .zip(r["edit_distances"].as_array().expect("eds"))
            .map(|(w, e)| Candidate::new(w.as_str().expect("word"), e.as_i64().expect("ed") as i32))
            .collect();
        results.push(m.score_batch(&query, &cands, &mut scratch).to_vec());
    }
    // Now replay the first query into the used scratch; it must match
    // the fresh-scratch result.
    let r0 = &f["rankings"][0];
    let query = m.query_features(r0["query"].as_str().expect("query"));
    let cands: Vec<Candidate> = r0["candidates"]
        .as_array()
        .expect("candidates")
        .iter()
        .zip(r0["edit_distances"].as_array().expect("eds"))
        .map(|(w, e)| Candidate::new(w.as_str().expect("word"), e.as_i64().expect("ed") as i32))
        .collect();
    let replay = m.score_batch(&query, &cands, &mut scratch).to_vec();
    assert_eq!(replay, results[0], "reused scratch changed the answer");
}
