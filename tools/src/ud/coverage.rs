//! Candidate-coverage harness for UD morphology.
//!
//! Phase 1 only: read UD, map the gold reading, build the torille candidate
//! set, and separate **mapping status** (exact/partial/ambiguous/unavailable)
//! from **candidate outcome** (exact/partial/ambiguous/missing). No model, no
//! scoring, no production path.

use std::collections::BTreeMap;

use torille::expand::{LexiconIndex, form_readings};
use torille::kotus::{lemmas, sanalista};
use torille::morphology::{Depth, MorphologyCache, analyze_morphology};
use torille::syntax::{TokenSyntax, analyze_syntax};
use torille::tags::{Confidence, Tags};
use torille::tokenise::{Token, TokenKind};

use super::mapping::{GoldReading, MappingStatus, UD_MAPPING_VERSION, map_token};
use super::reader::UdSentence;

/// Outcome of matching an available gold reading against the torille
/// candidates. Mapping-ambiguous and mapping-unavailable tokens have no
/// candidate outcome at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CandidateOutcome {
    Exact,
    Partial,
    Ambiguous,
    Missing,
}

impl CandidateOutcome {
    pub fn name(self) -> &'static str {
        match self {
            CandidateOutcome::Exact => "exact",
            CandidateOutcome::Partial => "partial",
            CandidateOutcome::Ambiguous => "ambiguous",
            CandidateOutcome::Missing => "missing",
        }
    }
}

/// Counters for one report group.
///
/// Mapping counts cover every word token; candidate counts cover only tokens
/// whose gold mapping was exact or partial. This keeps an unavailable gold
/// reading out of the model-error denominator.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counts {
    pub tokens: usize,
    pub mapping_exact: usize,
    pub mapping_partial: usize,
    pub mapping_ambiguous: usize,
    pub mapping_unavailable: usize,
    pub candidate_exact: usize,
    pub candidate_partial: usize,
    pub candidate_ambiguous: usize,
    pub candidate_missing: usize,
    /// Tokens where at least one matching reading's lemma could not be
    /// attributed.
    pub provenance_unavailable: usize,
}

impl Counts {
    pub fn record(
        &mut self,
        status: MappingStatus,
        outcome: Option<CandidateOutcome>,
        provenance_unavailable: bool,
    ) {
        self.tokens += 1;
        match status {
            MappingStatus::Exact => self.mapping_exact += 1,
            MappingStatus::Partial => self.mapping_partial += 1,
            MappingStatus::Ambiguous => self.mapping_ambiguous += 1,
            MappingStatus::Unavailable => self.mapping_unavailable += 1,
        }
        match outcome {
            Some(CandidateOutcome::Exact) => self.candidate_exact += 1,
            Some(CandidateOutcome::Partial) => self.candidate_partial += 1,
            Some(CandidateOutcome::Ambiguous) => self.candidate_ambiguous += 1,
            Some(CandidateOutcome::Missing) => self.candidate_missing += 1,
            None => {}
        }
        if provenance_unavailable {
            self.provenance_unavailable += 1;
        }
    }

    /// Candidate exact + partial over exact + partial + missing. Tokens whose
    /// gold mapping is ambiguous or unavailable are excluded.
    pub fn coverage(&self) -> Option<f64> {
        let denominator =
            self.candidate_exact + self.candidate_partial + self.candidate_missing;
        (denominator > 0).then(|| {
            (self.candidate_exact + self.candidate_partial) as f64 / denominator as f64
        })
    }
}

/// Provenance of a candidate reading's lemma.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LemmaProvenance<'a> {
    /// One or more lemmas emit this exact reading. Never assume the length is
    /// one; the model must not pick a unique lemma without checking.
    Known(Vec<&'a str>),
    Unavailable { reason: &'static str },
}

/// A value that the current `WordInfo` either provides or cannot provide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provenance<T> {
    Known(T),
    Unavailable { reason: &'static str },
}

/// The adapter between `WordInfo.readings` and the model's candidate features.
#[derive(Debug, Clone)]
pub struct CandidateView<'a> {
    pub token_index: usize,
    pub reading_index: usize,
    pub tags: Tags,
    pub lemma_provenance: LemmaProvenance<'a>,
    pub source_confidence: Provenance<Confidence>,
}

/// A single UD coverage report.
#[derive(Debug, Clone, Default)]
pub struct CoverageReport {
    pub mapping_version: String,
    pub sentences: usize,
    pub word_tokens: usize,
    pub skipped_tokens: usize,
    pub multiword_tokens: usize,
    pub empty_nodes: usize,
    pub totals: Counts,
    pub by_pos: BTreeMap<String, Counts>,
    pub by_case: BTreeMap<String, Counts>,
    pub by_depth: BTreeMap<String, Counts>,
}

/// Run the coverage harness over a parsed UD corpus.
pub fn coverage(sentences: &[UdSentence]) -> CoverageReport {
    let cat = torille::spec::Catalogues::embedded();
    let index = LexiconIndex::new(lemmas());
    let mut report = CoverageReport {
        mapping_version: UD_MAPPING_VERSION.to_string(),
        sentences: sentences.len(),
        ..CoverageReport::default()
    };

    for sentence in sentences {
        let (tokens, aligned) = synthetic_tokens(sentence);
        report.skipped_tokens += sentence
            .tokens
            .iter()
            .filter(|token| !token.is_word_like())
            .count();
        report.multiword_tokens += sentence.multiword_tokens;
        report.empty_nodes += sentence.empty_nodes;

        let mut cache = MorphologyCache::new();
        let morphology = analyze_morphology(&tokens, &mut cache);
        let syntax = analyze_syntax(&tokens, &morphology);
        for (ud_index, token_index) in aligned {
            let Some(token_index) = token_index else {
                continue;
            };
            let ud_token = &sentence.tokens[ud_index];
            let gold = map_token(ud_token);
            let depth = morphology
                .word(token_index)
                .map_or(Depth::Unknown, |info| info.depth());
            let (outcome, provenance_unavailable) = match gold.status {
                MappingStatus::Unavailable | MappingStatus::Ambiguous => (None, false),
                MappingStatus::Exact | MappingStatus::Partial => {
                    let (outcome, provenance_unavailable) = classify(
                        &syntax.tokens[token_index],
                        token_index,
                        &gold,
                        cat,
                        &index,
                    );
                    (Some(outcome), provenance_unavailable)
                }
            };
            report
                .totals
                .record(gold.status, outcome, provenance_unavailable);

            let pos = ud_token.upos.clone();
            let case = ud_token
                .feature("Case")
                .unwrap_or("none")
                .to_string();
            let depth = depth.label().to_string();
            report
                .by_pos
                .entry(pos)
                .or_default()
                .record(gold.status, outcome, provenance_unavailable);
            report
                .by_case
                .entry(case)
                .or_default()
                .record(gold.status, outcome, provenance_unavailable);
            report
                .by_depth
                .entry(depth)
                .or_default()
                .record(gold.status, outcome, provenance_unavailable);
            report.word_tokens += 1;
        }
    }
    report
}

fn classify(
    syntax: &TokenSyntax,
    token_index: usize,
    gold: &GoldReading,
    cat: &torille::spec::Catalogues,
    index: &LexiconIndex<'_>,
) -> (CandidateOutcome, bool) {
    let mut matches = Vec::new();
    let mut provenance_unavailable = false;
    for reading_index in 0..syntax.readings.len() {
        let tags = syntax.readings[reading_index];
        if !gold.tags_match(&tags) {
            continue;
        }
        let provenance = lemma_provenance(syntax, &tags, cat, index);
        if matches!(provenance, LemmaProvenance::Unavailable { .. }) {
            provenance_unavailable = true;
        }
        matches.push(CandidateView {
            token_index,
            reading_index,
            tags,
            lemma_provenance: provenance,
            source_confidence: Provenance::Unavailable {
                reason: "WordInfo does not retain Form::confidence per reading",
            },
        });
        if matches.len() > 1 {
            break;
        }
    }

    let outcome = match matches.len() {
        0 => CandidateOutcome::Missing,
        n if n > 1 => CandidateOutcome::Ambiguous,
        _ => {
            let candidate = &matches[0];
            let candidate_provenance_unavailable =
                matches!(candidate.lemma_provenance, LemmaProvenance::Unavailable { .. });
            provenance_unavailable |= candidate_provenance_unavailable;
            let lemma_ok = gold.lemma.as_deref().is_some_and(|gold_lemma| {
                match &candidate.lemma_provenance {
                    LemmaProvenance::Known(lemmas) => lemmas.contains(&gold_lemma),
                    LemmaProvenance::Unavailable { .. } => false,
                }
            });
            if gold.status == MappingStatus::Exact
                && !candidate_provenance_unavailable
                && lemma_ok
            {
                CandidateOutcome::Exact
            } else if gold.lemma.is_some() && !candidate_provenance_unavailable && !lemma_ok {
                // A candidate with the same tags but a different lemma is not
                // the gold candidate. Do not count it as coverage.
                CandidateOutcome::Missing
            } else {
                CandidateOutcome::Partial
            }
        }
    };
    (outcome, provenance_unavailable)
}

/// Attribute a candidate reading to the token's candidate lemmas.
///
/// `TokenSyntax.lemmas` gives every base that produced the surface. With one
/// base the attribution is direct. With several bases the harness re-inflects
/// each base for the surface and keeps only the bases that emit this exact
/// reading; this is a Phase 1 adapter, not a production API.
fn lemma_provenance<'a>(
    syntax: &'a TokenSyntax,
    tags: &Tags,
    cat: &torille::spec::Catalogues,
    index: &LexiconIndex<'_>,
) -> LemmaProvenance<'a> {
    if syntax.lemmas.is_empty() {
        return LemmaProvenance::Unavailable {
            reason: "no base lemma on TokenSyntax",
        };
    }
    if syntax.lemmas.len() == 1 {
        return LemmaProvenance::Known(vec![syntax.lemmas[0].as_str()]);
    }
    let surface = syntax.key.as_str();
    let mut known = Vec::new();
    for lemma in &syntax.lemmas {
        let Some(row) = sanalista().by_word(lemma).next() else {
            continue;
        };
        let inflections: Vec<_> = row.inflections().collect();
        let pos = torille::tags::PartOfSpeech::parse_all(row.part_of_speech())
            .first()
            .copied()
            .unwrap_or(torille::tags::PartOfSpeech::Unknown);
        let Some(readings) = form_readings(
            lemma,
            &inflections,
            cat,
            Some(index),
            pos,
            true,
            surface,
        ) else {
            continue;
        };
        if readings.iter().any(|reading| reading == tags) {
            known.push(lemma.as_str());
        }
    }
    if known.is_empty() {
        LemmaProvenance::Unavailable {
            reason: "no candidate base emits this exact reading",
        }
    } else {
        LemmaProvenance::Known(known)
    }
}

/// Build a synthetic token stream for one UD sentence.
///
/// UD forms are already tokenised syntactic words. We deliberately do not
/// re-tokenise `# text`: multiword rows and punctuation would make the
/// alignment ambiguous. Separators are omitted; sentence analysis only needs
/// the word tokens, and each UD sentence is processed independently.
fn synthetic_tokens(sentence: &UdSentence) -> (Vec<Token>, Vec<(usize, Option<usize>)>) {
    let mut tokens = Vec::new();
    let mut aligned = Vec::with_capacity(sentence.tokens.len());
    let mut offset = 0usize;
    for (ud_index, ud_token) in sentence.tokens.iter().enumerate() {
        if !ud_token.is_word_like() {
            aligned.push((ud_index, None));
            continue;
        }
        let text = ud_token.form.clone();
        let end = offset + text.len();
        tokens.push(Token {
            text,
            start: offset,
            end,
            kind: TokenKind::Word,
        });
        aligned.push((ud_index, Some(tokens.len() - 1)));
        offset = end + 1;
    }
    (tokens, aligned)
}

/// Render a stable text report for the CLI.
pub fn format_report(report: &CoverageReport) -> String {
    let mut out = String::new();
    out.push_str("UD candidate coverage\n");
    out.push_str(&format!("mapping version: {}\n", report.mapping_version));
    out.push_str(&format!("sentences: {}\n", report.sentences));
    out.push_str(&format!("word tokens: {}\n", report.word_tokens));
    out.push_str(&format!("skipped non-word: {}\n", report.skipped_tokens));
    out.push_str(&format!("multiword rows: {}\n", report.multiword_tokens));
    out.push_str(&format!("empty nodes: {}\n", report.empty_nodes));
    out.push_str(&format!(
        "gold mapping: exact={} partial={} ambiguous={} unavailable={}\n",
        report.totals.mapping_exact,
        report.totals.mapping_partial,
        report.totals.mapping_ambiguous,
        report.totals.mapping_unavailable
    ));
    out.push_str(&format!(
        "candidate outcome: exact={} partial={} ambiguous={} missing={}\n",
        report.totals.candidate_exact,
        report.totals.candidate_partial,
        report.totals.candidate_ambiguous,
        report.totals.candidate_missing
    ));
    let coverage = report
        .totals
        .coverage()
        .map_or_else(|| "n/a".to_string(), |value| format!("{:.1}%", value * 100.0));
    out.push_str(&format!("candidate coverage (exact+partial): {coverage}\n"));
    out.push_str(&format!(
        "provenance unavailable: {}\n",
        report.totals.provenance_unavailable
    ));
    push_group(&mut out, "by POS", &report.by_pos);
    push_group(&mut out, "by case", &report.by_case);
    push_group(&mut out, "by depth", &report.by_depth);
    out
}

fn push_group(out: &mut String, label: &str, groups: &BTreeMap<String, Counts>) {
    out.push_str(&format!("\n{label}:\n"));
    out.push_str(
        "key\ttokens\tmap_exact\tmap_partial\tmap_ambiguous\tmap_unavailable\tcand_exact\tcand_partial\tcand_ambiguous\tcand_missing\tprov_unavailable\n",
    );
    for (key, counts) in groups {
        out.push_str(&format!(
            "{key}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            counts.tokens,
            counts.mapping_exact,
            counts.mapping_partial,
            counts.mapping_ambiguous,
            counts.mapping_unavailable,
            counts.candidate_exact,
            counts.candidate_partial,
            counts.candidate_ambiguous,
            counts.candidate_missing,
            counts.provenance_unavailable
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ud::reader::read_str;

    const SAMPLE: &str = "\
# sent_id = s1
# text = Kissa istuu.
1\tKissa\tkissa\tNOUN\t_\tCase=Nom|Number=Sing\t2\tnsubj\t_\t_
2\tistuu\tistua\tVERB\t_\tMood=Ind|Person=3|Tense=Pres|VerbForm=Fin\t0\troot\t_\t_
3\t.\t.\tPUNCT\t_\t_\t2\tpunct\t_\t_

# sent_id = s2
# text = Kaksi lasta tuli.
1\tKaksi\tkaksi\tNUM\t_\tCase=Nom|Number=Sing\t3\tnsubj\t_\t_
2\tlasta\tlapsi\tNOUN\t_\tCase=Par|Number=Sing\t3\tnsubj\t_\t_
3\ttuli\ttulla\tVERB\t_\tMood=Ind|Person=3|Tense=Past|VerbForm=Fin\t0\troot\t_\t_
4\t.\t.\tPUNCT\t_\t_\t3\tpunct\t_\t_
";

    #[test]
    fn a_small_corpus_produces_counts() {
        let sentences = read_str(SAMPLE).unwrap();
        let report = coverage(&sentences);
        assert_eq!(report.sentences, 2);
        assert_eq!(report.word_tokens, 5);
        assert_eq!(report.totals.tokens, 5);
        assert!(report.totals.mapping_exact >= report.totals.candidate_exact);
        assert!(report.totals.candidate_exact + report.totals.candidate_partial > 0);
        assert!(report.by_pos.contains_key("NOUN"));
        assert!(report.by_case.contains_key("Nom"));
        assert!(report.by_depth.contains_key("direct"));
        let text = format_report(&report);
        assert!(text.contains("UD candidate coverage"));
        assert!(text.contains("gold mapping:"));
        assert!(text.contains("candidate outcome:"));
        assert!(text.contains("by POS"));
    }

    #[test]
    fn a_punctuation_token_is_skipped() {
        let sentences = read_str(SAMPLE).unwrap();
        let report = coverage(&sentences);
        assert_eq!(report.skipped_tokens, 2);
    }

    #[test]
    fn the_committed_sample_is_readable() {
        let sentences = read_str(include_str!(
            "../../../tests/fixtures/neural/ud_sample.conllu"
        ))
        .unwrap();
        let report = coverage(&sentences);
        assert_eq!(report.mapping_version, UD_MAPPING_VERSION);
        assert_eq!(report.sentences, 4);
        assert_eq!(report.word_tokens, 11);
        assert_eq!(report.totals.candidate_missing, 0);
    }

    #[test]
    fn candidate_coverage_excludes_unavailable_and_ambiguous() {
        let mut counts = Counts {
            candidate_exact: 1,
            candidate_partial: 1,
            candidate_missing: 2,
            candidate_ambiguous: 10,
            mapping_unavailable: 20,
            mapping_ambiguous: 5,
            ..Counts::default()
        };
        assert_eq!(counts.coverage(), Some(0.5));
        counts.candidate_missing = 0;
        assert_eq!(counts.coverage(), Some(1.0));
        assert_eq!(Counts::default().coverage(), None);
    }
}
