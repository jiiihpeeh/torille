//! M0 feature extraction for the Phase 3 experiments.
//!
//! This emits one JSONL row per ambiguous UD token whose gold reading is a
//! real torille candidate. It does not train or run a model. The Python
//! trainer uses the rows for the morphology-only, local-context and
//! sentence-context MLP comparison.

use serde::Serialize;
use torille::disambiguation::{CandidateView, LemmaProvenance};
use torille::morphology::{Depth, MorphologyCache, analyze_morphology};
use torille::syntax::analyze_syntax;
use torille::tags::{Case, Clitic, Mood, Number, Person, PartOfSpeech, Possessive, Tense, Voice};

use super::coverage::{all_candidate_views, matching_views, synthetic_tokens};
use super::mapping::{MappingStatus, map_token};
use super::reader::UdSentence;

/// One token example: all candidates plus the intended candidate index.
#[derive(Debug, Clone, Serialize)]
pub struct M0Example {
    pub sentence_id: String,
    pub token_index: usize,
    pub surface: String,
    pub lower: String,
    pub position: usize,
    pub sentence_length: usize,
    pub prev: Option<String>,
    pub next: Option<String>,
    /// The first reading of the previous/next word, when it has one. These are
    /// deterministic context features, not model output.
    pub prev_pos: Option<String>,
    pub prev_case: Option<String>,
    pub prev_number: Option<String>,
    pub next_pos: Option<String>,
    pub next_case: Option<String>,
    pub next_number: Option<String>,
    pub depth: String,
    pub candidate_count: usize,
    pub label: usize,
    pub candidates: Vec<M0Candidate>,
    pub sentence_tokens: Vec<String>,
}

/// One candidate's model features.
#[derive(Debug, Clone, Serialize)]
pub struct M0Candidate {
    pub index: usize,
    pub pos: Option<String>,
    pub case: Option<String>,
    pub number: Option<String>,
    pub person: Option<String>,
    pub tense: Option<String>,
    pub mood: Option<String>,
    pub voice: Option<String>,
    pub polarity: String,
    pub possessive: Option<String>,
    pub clitic: Option<String>,
    /// Lemmas that emit this exact reading; may contain more than one.
    pub lemmas: Vec<String>,
    pub provenance_known: bool,
}

/// Extract M0 examples from UD sentences.
pub fn examples(sentences: &[UdSentence]) -> Vec<M0Example> {
    let cat = torille::spec::Catalogues::embedded();
    let index = torille::expand::LexiconIndex::new(torille::kotus::lemmas());
    let mut out = Vec::new();

    for sentence in sentences {
        let (tokens, aligned) = synthetic_tokens(sentence);
        if tokens.is_empty() {
            continue;
        }
        let sentence_tokens: Vec<String> = tokens.iter().map(|token| token.text.to_lowercase()).collect();
        let mut cache = MorphologyCache::new();
        let morphology = analyze_morphology(&tokens, &mut cache);
        let syntax = analyze_syntax(&tokens, &morphology);
        for (ud_index, token_index) in aligned {
            let Some(token_index) = token_index else {
                continue;
            };
            let ud_token = &sentence.tokens[ud_index];
            let gold = map_token(ud_token);
            if !matches!(
                gold.status,
                MappingStatus::Exact | MappingStatus::Partial
            ) {
                continue;
            }
            let syntax_token = &syntax.tokens[token_index];
            if syntax_token.readings.len() < 2 {
                continue;
            }
            let views = all_candidate_views(syntax_token, token_index, cat, &index);
            let (matches, provenance_unavailable) =
                matching_views(syntax_token, token_index, &gold, cat, &index);
            if provenance_unavailable || matches.len() != 1 {
                continue;
            }
            let Some(gold_lemma) = gold.lemma.as_deref() else {
                continue;
            };
            let candidate = &matches[0];
            let label_ok = match &candidate.lemma_provenance {
                LemmaProvenance::Known(lemmas) => lemmas.contains(&gold_lemma),
                LemmaProvenance::Unavailable { .. } => false,
            };
            if !label_ok {
                continue;
            }
            let label = candidate.reading_index;
            let position = token_index;
            let prev = position.checked_sub(1).map(|at| tokens[at].text.to_lowercase());
            let next = tokens.get(position + 1).map(|token| token.text.to_lowercase());
            let prev_features = neighbor_features(
                position.checked_sub(1).and_then(|at| morphology.word(at)),
            );
            let next_features =
                neighbor_features(morphology.word(position + 1));
            let depth = morphology
                .word(token_index)
                .map_or(Depth::Unknown, |info| info.depth())
                .label()
                .to_string();
            out.push(M0Example {
                sentence_id: sentence.id.clone(),
                token_index: ud_token.id,
                surface: tokens[token_index].text.clone(),
                lower: tokens[token_index].text.to_lowercase(),
                position,
                sentence_length: tokens.len(),
                prev,
                next,
                prev_pos: prev_features.0,
                prev_case: prev_features.1,
                prev_number: prev_features.2,
                next_pos: next_features.0,
                next_case: next_features.1,
                next_number: next_features.2,
                depth,
                candidate_count: syntax_token.readings.len(),
                label,
                candidates: views
                    .into_iter()
                    .map(|view| candidate_from_view(view))
                    .collect(),
                sentence_tokens: sentence_tokens.clone(),
            });
        }
    }
    out
}

/// The first reading of a neighbouring word as deterministic context.
fn neighbor_features(
    info: Option<&torille::morphology::WordInfo>,
) -> (Option<String>, Option<String>, Option<String>) {
    let Some(tags) = info.and_then(|info| info.readings().first()) else {
        return (None, None, None);
    };
    (
        tags.part_of_speech.map(|pos| pos.name().to_string()),
        tags.case.map(|case| case.name().to_string()),
        tags.number.map(|number| number.name().to_string()),
    )
}

fn candidate_from_view(view: CandidateView<'_>) -> M0Candidate {
    let lemmas = match &view.lemma_provenance {
        LemmaProvenance::Known(lemmas) => lemmas.iter().map(|lemma| (*lemma).to_string()).collect(),
        LemmaProvenance::Unavailable { .. } => Vec::new(),
    };
    M0Candidate {
        index: view.reading_index,
        pos: view.tags.part_of_speech.map(pos_name),
        case: view.tags.case.map(case_name),
        number: view.tags.number.map(number_name),
        person: view.tags.person.map(person_name),
        tense: view.tags.tense.map(tense_name),
        mood: view.tags.mood.map(mood_name),
        voice: view.tags.voice.map(voice_name),
        polarity: view.tags.polarity.name().to_string(),
        possessive: view.tags.possessive.map(possessive_name),
        clitic: view.tags.clitic.map(clitic_name),
        provenance_known: matches!(view.lemma_provenance, LemmaProvenance::Known(_)),
        lemmas,
    }
}

fn pos_name(pos: PartOfSpeech) -> String {
    pos.name().to_string()
}

fn case_name(case: Case) -> String {
    case.name().to_string()
}

fn number_name(number: Number) -> String {
    number.name().to_string()
}

fn person_name(person: Person) -> String {
    person.name().to_string()
}

fn tense_name(tense: Tense) -> String {
    tense.name().to_string()
}

fn mood_name(mood: Mood) -> String {
    mood.name().to_string()
}

fn voice_name(voice: Voice) -> String {
    voice.name().to_string()
}

fn possessive_name(possessive: Possessive) -> String {
    match possessive {
        Possessive::FirstSingular => "first_singular",
        Possessive::SecondSingular => "second_singular",
        Possessive::Third => "third",
        Possessive::FirstPlural => "first_plural",
        Possessive::SecondPlural => "second_plural",
    }
    .to_string()
}

fn clitic_name(clitic: Clitic) -> String {
    match clitic {
        Clitic::Ko => "ko",
        Clitic::Kin => "kin",
        Clitic::Kaan => "kaan",
        Clitic::Han => "han",
        Clitic::Pa => "pa",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ud::reader::read_str;

    const SAMPLE: &str = "\
# sent_id = s
# text = Tuli sammui.
1\tTuli\ttuli\tNOUN\t_\tCase=Nom|Number=Sing\t2\tnsubj\t_\t_
2\tsammui\tsammua\tVERB\t_\tMood=Ind|Person=3|Tense=Past|VerbForm=Fin\t0\troot\t_\t_
";

    #[test]
    fn an_ambiguous_noun_is_emitted() {
        let sentences = read_str(SAMPLE).unwrap();
        let rows = examples(&sentences);
        assert!(!rows.is_empty(), "tuli should have noun and verb readings");
        let row = &rows[0];
        assert_eq!(row.surface, "Tuli");
        assert_eq!(row.candidates[row.label].index, row.label);
        assert!(row.candidate_count >= 2);
    }
}
