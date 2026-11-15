//! M0 fixed-weight candidate reranker.
//!
//! This is the runtime half of the Phase 3 experiments. The `neural-m0`
//! feature embeds the tags-only model; `neural-m0-stack` embeds the stack
//! variant, which adds deterministic reading-order, morphology and
//! neighbour-reading features on top of the same tags. Neither model depends
//! on per-reading lemma provenance, and both are off by default.

use std::sync::OnceLock;

use crate::disambiguation::{Disambiguation, ReadingDecision};
use crate::morphology::{MorphologyAnalysis, WordInfo};
use crate::tags::{Case, Clitic, Possessive, Tags};
use crate::tokenise::Token;

const MAGIC: &[u8; 4] = b"TDM0";
const MODEL_VERSION: u32 = 1;
const HASH_FNV: u8 = 1;

#[cfg(all(feature = "neural-m0", feature = "neural-m0-stack"))]
compile_error!("enable either neural-m0 or neural-m0-stack, not both");

#[cfg(feature = "neural-m0-stack")]
const FEATURE_SCHEMA_VERSION: u32 = 2;
#[cfg(all(feature = "neural-m0", not(feature = "neural-m0-stack")))]
const FEATURE_SCHEMA_VERSION: u32 = 1;

#[cfg(feature = "neural-m0-stack")]
const MODEL_BYTES: &[u8] = include_bytes!("../../models/m0-stack-v1.bin");
#[cfg(all(feature = "neural-m0", not(feature = "neural-m0-stack")))]
const MODEL_BYTES: &[u8] = include_bytes!("../../models/m0-tags-v1.bin");

struct Model {
    dim: usize,
    hidden: usize,
    w1: Vec<f32>,
    b1: Vec<f32>,
    w2: Vec<f32>,
    b2: f32,
}

/// The first reading of a neighbouring word, as deterministic context.
#[derive(Default)]
struct Neighbor {
    part_of_speech: Option<&'static str>,
    case: Option<&'static str>,
    number: Option<&'static str>,
}

/// Deterministic sentence context for one token.
struct Context {
    prev: Neighbor,
    next: Neighbor,
}

impl Context {
    fn new(index: usize, morphology: &MorphologyAnalysis) -> Self {
        Self {
            prev: neighbor(index.checked_sub(1).and_then(|at| morphology.word(at))),
            next: neighbor(morphology.word(index + 1)),
        }
    }
}

fn neighbor(info: Option<&WordInfo>) -> Neighbor {
    let Some(tags) = info.and_then(|info| info.readings().first()) else {
        return Neighbor::default();
    };
    Neighbor {
        part_of_speech: tags.part_of_speech.map(|value| value.name()),
        case: tags.case.map(|value| value.name()),
        number: tags.number.map(|value| value.name()),
    }
}

fn model() -> Option<&'static Model> {
    static MODEL: OnceLock<Option<Model>> = OnceLock::new();
    MODEL.get_or_init(|| parse_model(MODEL_BYTES)).as_ref()
}

fn parse_model(bytes: &[u8]) -> Option<Model> {
    if bytes.len() < 16 || &bytes[..4] != MAGIC {
        return None;
    }
    let version = bytes[4];
    let hash = bytes[5];
    let reserved = u16::from_le_bytes([bytes[6], bytes[7]]);
    if version != 1 || hash != HASH_FNV || reserved != 0 {
        return None;
    }
    let dim = read_u32(bytes, 8)? as usize;
    let hidden = read_u32(bytes, 12)? as usize;
    let expected = 16 + dim * hidden * 4 + hidden * 4 + hidden * 4 + 4;
    if bytes.len() != expected {
        return None;
    }
    let mut offset = 16;
    let w1 = read_f32_vec(bytes, &mut offset, dim * hidden)?;
    let b1 = read_f32_vec(bytes, &mut offset, hidden)?;
    let w2 = read_f32_vec(bytes, &mut offset, hidden)?;
    let b2 = read_f32_vec(bytes, &mut offset, 1)?[0];
    Some(Model {
        dim,
        hidden,
        w1,
        b1,
        w2,
        b2,
    })
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    bytes
        .get(offset..offset + 4)?
        .try_into()
        .ok()
        .map(u32::from_le_bytes)
}

fn read_f32_vec(bytes: &[u8], offset: &mut usize, count: usize) -> Option<Vec<f32>> {
    let end = offset.checked_add(count.checked_mul(4)?)?;
    let slice = bytes.get(*offset..end)?;
    *offset = end;
    let mut values = Vec::with_capacity(count);
    for chunk in slice.chunks(4) {
        values.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    Some(values)
}

/// Score every reading and return a sidecar decision table.
pub(crate) fn disambiguate(tokens: &[Token], morphology: &MorphologyAnalysis) -> Disambiguation {
    let Some(model) = model() else {
        return Disambiguation::disabled();
    };
    let mut decisions = vec![None; tokens.len()];
    for (index, token) in tokens.iter().enumerate() {
        if !token.is_word() {
            continue;
        }
        let Some(info) = morphology.word(index) else {
            continue;
        };
        if info.readings().len() < 2 {
            continue;
        }
        let context = Context::new(index, morphology);
        let scores: Vec<f32> = info
            .readings()
            .iter()
            .enumerate()
            .map(|(reading_index, tags)| {
                model.score(token, reading_index, info, tags, &context)
            })
            .collect();
        let probabilities = softmax(&scores);
        let preferred = argmax(&scores);
        let margin = margin(&scores);
        let entropy = entropy(&probabilities);
        decisions[index] = Some(ReadingDecision {
            preferred,
            scores,
            margin,
            entropy,
            abstain: false,
        });
    }
    Disambiguation {
        decisions,
        model_version: MODEL_VERSION,
        feature_schema_version: FEATURE_SCHEMA_VERSION,
        enabled: true,
    }
}

impl Model {
    fn score(
        &self,
        token: &Token,
        index: usize,
        info: &WordInfo,
        tags: &Tags,
        context: &Context,
    ) -> f32 {
        let keys = feature_keys(token, index, info, tags, context);
        let mut input = vec![0.0f32; self.dim];
        for key in keys {
            input[(fnv1a(&key) % self.dim as u32) as usize] = 1.0;
        }
        let mut hidden = vec![0.0f32; self.hidden];
        for (unit, value) in hidden.iter_mut().enumerate() {
            let mut sum = self.b1[unit];
            for (feature_index, feature) in input.iter().enumerate() {
                sum += feature * self.w1[feature_index * self.hidden + unit];
            }
            *value = sum.max(0.0);
        }
        hidden
            .iter()
            .zip(&self.w2)
            .map(|(value, weight)| value * weight)
            .sum::<f32>()
            + self.b2
    }
}

fn feature_keys(
    token: &Token,
    index: usize,
    info: &WordInfo,
    tags: &Tags,
    context: &Context,
) -> Vec<String> {
    let mut keys = vec![
        format!("tok:surface={}", token.text),
        format!("tok:lower={}", token.text.to_lowercase()),
        format!("tok:depth={}", info.depth().label()),
        format!("tok:candidates={}", info.readings().len()),
    ];
    if let Some(value) = tags.part_of_speech {
        keys.push(format!("cand:pos={}", value.name()));
    }
    if let Some(value) = tags.case {
        keys.push(format!("cand:case={}", case_name(value)));
    }
    if let Some(value) = tags.number {
        keys.push(format!("cand:number={}", value.name()));
    }
    if let Some(value) = tags.person {
        keys.push(format!("cand:person={}", value.name()));
    }
    if let Some(value) = tags.tense {
        keys.push(format!("cand:tense={}", value.name()));
    }
    if let Some(value) = tags.mood {
        keys.push(format!("cand:mood={}", value.name()));
    }
    if let Some(value) = tags.voice {
        keys.push(format!("cand:voice={}", value.name()));
    }
    keys.push(format!("cand:polarity={}", tags.polarity.name()));
    if let Some(value) = tags.possessive {
        keys.push(format!("cand:possessive={}", possessive_name(value)));
    }
    if let Some(value) = tags.clitic {
        keys.push(format!("cand:clitic={}", clitic_name(value)));
    }
    #[cfg(feature = "neural-m0-stack")]
    append_stack_keys(&mut keys, index, info, tags, context);
    keys
}

/// Deterministic features for the stack model.
///
/// These mirror `tools/neural/train_m0.py --variants stack`: reading order,
/// candidate count, morphology depth and the first reading of the previous and
/// next words. They are cheap and do not need per-reading lemma provenance.
#[cfg(feature = "neural-m0-stack")]
fn append_stack_keys(
    keys: &mut Vec<String>,
    index: usize,
    info: &WordInfo,
    tags: &Tags,
    context: &Context,
) {
    keys.push(format!("det:index={index}"));
    keys.push(format!(
        "det:first={}",
        if index == 0 { 1 } else { 0 }
    ));
    keys.push(format!("det:candidate_count={}", info.readings().len()));
    keys.push(format!("det:depth={}", info.depth().label()));
    for (side, neighbor) in [("prev", &context.prev), ("next", &context.next)] {
        let fields = [
            (
                "pos",
                tags.part_of_speech.map(|value| value.name()),
                neighbor.part_of_speech,
            ),
            ("case", tags.case.map(|value| value.name()), neighbor.case),
            (
                "number",
                tags.number.map(|value| value.name()),
                neighbor.number,
            ),
        ];
        for (field, candidate, neighbor) in fields {
            keys.push(format!(
                "det:{side}_{field}={}",
                neighbor.unwrap_or("None")
            ));
            if let (Some(candidate), Some(neighbor)) = (candidate, neighbor) {
                keys.push(format!(
                    "det:agree_{side}_{field}={}",
                    if candidate == neighbor { "true" } else { "false" }
                ));
            }
        }
    }
}

fn case_name(case: Case) -> &'static str {
    case.name()
}

fn possessive_name(value: Possessive) -> &'static str {
    match value {
        Possessive::FirstSingular => "first_singular",
        Possessive::SecondSingular => "second_singular",
        Possessive::Third => "third",
        Possessive::FirstPlural => "first_plural",
        Possessive::SecondPlural => "second_plural",
    }
}

fn clitic_name(value: Clitic) -> &'static str {
    match value {
        Clitic::Ko => "ko",
        Clitic::Kin => "kin",
        Clitic::Kaan => "kaan",
        Clitic::Han => "han",
        Clitic::Pa => "pa",
    }
}

fn fnv1a(value: &str) -> u32 {
    let mut hash = 2_166_136_261u32;
    for byte in value.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(16_777_619);
    }
    hash
}

fn softmax(scores: &[f32]) -> Vec<f32> {
    if scores.is_empty() {
        return Vec::new();
    }
    let max = scores.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let exp: Vec<f32> = scores.iter().map(|score| (score - max).exp()).collect();
    let sum: f32 = exp.iter().sum();
    exp.into_iter().map(|value| value / sum).collect()
}

fn argmax(scores: &[f32]) -> usize {
    scores
        .iter()
        .enumerate()
        .max_by(|(left_index, left), (right_index, right)| {
            left.total_cmp(right)
                .then_with(|| right_index.cmp(left_index))
        })
        .map_or(0, |(index, _)| index)
}

fn margin(scores: &[f32]) -> f32 {
    let mut sorted = scores.to_vec();
    sorted.sort_by(|a, b| b.total_cmp(a));
    match sorted.as_slice() {
        [best, second, ..] => best - second,
        [only] => *only,
        [] => 0.0,
    }
}

fn entropy(probabilities: &[f32]) -> f32 {
    probabilities
        .iter()
        .filter(|value| **value > 0.0)
        .map(|value| -value * value.ln())
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenise::tokenise;

    #[test]
    fn the_embedded_model_parses() {
        let model = model().expect("models/m0-tags-v1.bin should parse");
        assert_eq!(model.dim, 512);
        assert_eq!(model.hidden, 32);
        assert_eq!(model.w1.len(), 512 * 32);
    }

    #[cfg(feature = "neural-m0-stack")]
    #[test]
    fn stack_features_use_the_candidate_index_and_neighbour_context() {
        let tokens = tokenise("Tuli sammui.");
        let mut cache = crate::morphology::MorphologyCache::new();
        let morphology = crate::morphology::analyze_morphology(&tokens, &mut cache);
        let info = morphology.word(0).expect("Tuli should resolve");
        assert!(info.readings().len() >= 2);
        let context = Context::new(0, &morphology);
        let first = feature_keys(&tokens[0], 0, info, &info.readings()[0], &context);
        let second = feature_keys(&tokens[0], 1, info, &info.readings()[1], &context);
        assert!(first.contains(&"det:index=0".to_string()));
        assert!(second.contains(&"det:index=1".to_string()));
        assert!(second.iter().any(|key| key.starts_with("det:next_pos=")));
    }

    #[test]
    fn a_word_with_two_readings_gets_a_decision() {
        let tokens = tokenise("Tuli sammui.");
        let mut cache = crate::morphology::MorphologyCache::new();
        let morphology = crate::morphology::analyze_morphology(&tokens, &mut cache);
        let decisions = disambiguate(&tokens, &morphology);
        assert!(decisions.enabled);
        assert!(decisions.decisions.iter().any(Option::is_some));
    }
}
