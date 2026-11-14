//! Compact `surface -> WordInfo` codec for the parallel morphology pool.
//!
//! A worker resolves a group of surfaces and cannot hand an `Arc<WordInfo>`
//! across the wasm boundary. This module serialises the rows into a small
//! versioned byte string and rebuilds them in the owning state. It is a pure
//! memoisation transport: the decoded `WordInfo` is field-for-field the one
//! the worker resolved, so a seeded cache produces the same readings and
//! diagnostics as the single-thread path.
//!
//! The format is little-endian and deliberately simple:
//!
//! ```text
//! "TMOR" u8 version u32 row_count
//! row: surface flags lemma? verb_lemma? nominal_lemma? lemmas readings
//!      case number pos clitic possessive derivation depth
//! ```
//!
//! Every enum is a stable index into its `ALL` array; an `Option` uses `0`
//! for `None` and `index + 1` for `Some`.

use std::sync::Arc;

use crate::derivation::RULES;
use crate::tags::{
    Case, Clitic, Mood, Number, PartOfSpeech, Person, Polarity, Possessive, Tags, Tense, Voice,
};

use super::{Depth, WordInfo};

const MAGIC: &[u8; 4] = b"TMOR";
const VERSION: u8 = 1;

/// Encode the rows a worker resolved.
pub fn encode_rows(rows: &[(String, Arc<WordInfo>)]) -> Vec<u8> {
    let mut writer = Writer::new();
    writer.bytes.extend_from_slice(MAGIC);
    writer.u8(VERSION);
    writer.u32(rows.len() as u32);
    for (surface, info) in rows {
        writer
            .string(surface)
            .expect("a morphology surface is shorter than 64 KiB");
        writer.u8(u8::from(info.exact));
        writer
            .optional_string(info.lemma.as_deref())
            .expect("a lemma is shorter than 64 KiB");
        writer
            .optional_string(info.verb_lemma.as_deref())
            .expect("a lemma is shorter than 64 KiB");
        writer
            .optional_string(info.nominal_lemma.as_deref())
            .expect("a lemma is shorter than 64 KiB");
        writer
            .u16(info.lemmas.len())
            .expect("a lemma list is shorter than 64 KiB");
        for lemma in &info.lemmas {
            writer
                .string(lemma)
                .expect("a lemma is shorter than 64 KiB");
        }
        writer
            .u16(info.readings.len())
            .expect("a reading list is shorter than 64 KiB");
        for tags in &info.readings {
            encode_tags(&mut writer, tags);
        }
        writer.u8(optional_index(info.case, &Case::ALL));
        writer.u8(optional_index(info.number, &Number::ALL));
        writer.u8(optional_index(info.part_of_speech, &PartOfSpeech::ALL));
        writer.u8(optional_index(info.clitic, &Clitic::ALL));
        writer.u8(optional_index(info.possessive, &Possessive::ALL));
        match &info.derivation {
            None => writer.u8(0),
            Some(reverse) => {
                writer.u8(1);
                writer
                    .string(reverse.rule)
                    .expect("a derivation rule id is short");
                writer
                    .string(&reverse.source)
                    .expect("a derivation source is short");
            }
        }
        writer.u8(index(info.depth, &Depth::ALL));
    }
    writer.bytes
}

/// Decode rows written by [`encode_rows`].
pub fn decode_rows(bytes: &[u8]) -> Result<Vec<(String, Arc<WordInfo>)>, String> {
    let mut reader = Reader::new(bytes);
    if reader.take(4)? != MAGIC {
        return Err("morphology payload: bad magic".to_string());
    }
    if reader.u8()? != VERSION {
        return Err("morphology payload: unsupported version".to_string());
    }
    let count = reader.u32()?;
    let mut rows = Vec::with_capacity(count);
    for _ in 0..count {
        let surface = reader.string()?.to_string();
        let exact = reader.u8()? != 0;
        let lemma = reader.optional_string()?.map(str::to_string);
        let verb_lemma = reader.optional_string()?.map(str::to_string);
        let nominal_lemma = reader.optional_string()?.map(str::to_string);
        let lemma_count = reader.u16()?;
        let mut lemmas = Vec::with_capacity(lemma_count);
        for _ in 0..lemma_count {
            lemmas.push(reader.string()?.to_string());
        }
        let reading_count = reader.u16()?;
        let mut readings = Vec::with_capacity(reading_count);
        for _ in 0..reading_count {
            readings.push(decode_tags(&mut reader)?);
        }
        let case = optional_value(reader.u8()?, &Case::ALL)?;
        let number = optional_value(reader.u8()?, &Number::ALL)?;
        let part_of_speech = optional_value(reader.u8()?, &PartOfSpeech::ALL)?;
        let clitic = optional_value(reader.u8()?, &Clitic::ALL)?;
        let possessive = optional_value(reader.u8()?, &Possessive::ALL)?;
        let derivation = if reader.u8()? == 0 {
            None
        } else {
            let rule = reader.string()?;
            let source = reader.string()?.to_string();
            RULES
                .iter()
                .find(|candidate| candidate.id == rule)
                .map(|candidate| crate::derivation::Reverse {
                    rule: candidate.id,
                    source,
                })
        };
        let depth = value(reader.u8()?, &Depth::ALL)?;
        rows.push((
            surface.clone(),
            Arc::new(WordInfo {
                key: surface,
                lemma,
                verb_lemma,
                nominal_lemma,
                exact,
                lemmas,
                readings,
                case,
                number,
                part_of_speech,
                clitic,
                possessive,
                derivation,
                depth,
            }),
        ));
    }
    if reader.at != bytes.len() {
        return Err("morphology payload: trailing bytes".to_string());
    }
    Ok(rows)
}

fn encode_tags(writer: &mut Writer, tags: &Tags) {
    writer.u8(optional_index(tags.part_of_speech, &PartOfSpeech::ALL));
    writer.u8(optional_index(tags.case, &Case::ALL));
    writer.u8(optional_index(tags.number, &Number::ALL));
    writer.u8(optional_index(tags.person, &Person::ALL));
    writer.u8(optional_index(tags.tense, &Tense::ALL));
    writer.u8(optional_index(tags.mood, &Mood::ALL));
    writer.u8(optional_index(tags.voice, &Voice::ALL));
    writer.u8(optional_index(tags.possessive, &Possessive::ALL));
    writer.u8(optional_index(tags.clitic, &Clitic::ALL));
    writer.u8(index(tags.polarity, &Polarity::ALL));
}

fn decode_tags(reader: &mut Reader<'_>) -> Result<Tags, String> {
    Ok(Tags {
        part_of_speech: optional_value(reader.u8()?, &PartOfSpeech::ALL)?,
        case: optional_value(reader.u8()?, &Case::ALL)?,
        number: optional_value(reader.u8()?, &Number::ALL)?,
        person: optional_value(reader.u8()?, &Person::ALL)?,
        tense: optional_value(reader.u8()?, &Tense::ALL)?,
        mood: optional_value(reader.u8()?, &Mood::ALL)?,
        voice: optional_value(reader.u8()?, &Voice::ALL)?,
        possessive: optional_value(reader.u8()?, &Possessive::ALL)?,
        clitic: optional_value(reader.u8()?, &Clitic::ALL)?,
        polarity: value(reader.u8()?, &Polarity::ALL)?,
    })
}

fn index<T: Copy + PartialEq>(value: T, all: &[T]) -> u8 {
    all.iter()
        .position(|candidate| *candidate == value)
        .unwrap_or(0) as u8
}

fn optional_index<T: Copy + PartialEq>(value: Option<T>, all: &[T]) -> u8 {
    value.map_or(0, |value| index(value, all) + 1)
}

fn value<T: Copy>(index: u8, all: &[T]) -> Result<T, String> {
    all.get(index as usize)
        .copied()
        .ok_or_else(|| format!("morphology payload: bad enum index {index}"))
}

fn optional_value<T: Copy>(index: u8, all: &[T]) -> Result<Option<T>, String> {
    if index == 0 {
        Ok(None)
    } else {
        value(index - 1, all).map(Some)
    }
}

struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn new() -> Self {
        Self {
            bytes: Vec::with_capacity(4096),
        }
    }

    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn u16(&mut self, value: usize) -> Result<(), String> {
        let value = u16::try_from(value).map_err(|_| "value does not fit in u16")?;
        self.bytes.extend_from_slice(&value.to_le_bytes());
        Ok(())
    }

    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn string(&mut self, value: &str) -> Result<(), String> {
        self.u16(value.len())?;
        self.bytes.extend_from_slice(value.as_bytes());
        Ok(())
    }

    fn optional_string(&mut self, value: Option<&str>) -> Result<(), String> {
        match value {
            Some(value) => {
                self.u8(1);
                self.string(value)
            }
            None => {
                self.u8(0);
                Ok(())
            }
        }
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], String> {
        let slice = self
            .bytes
            .get(self.at..self.at + count)
            .ok_or_else(|| "morphology payload: truncated".to_string())?;
        self.at += count;
        Ok(slice)
    }

    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<usize, String> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes(bytes.try_into().expect("two bytes")) as usize)
    }

    fn u32(&mut self) -> Result<usize, String> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes(bytes.try_into().expect("four bytes")) as usize)
    }

    fn string(&mut self) -> Result<&'a str, String> {
        let length = self.u16()?;
        let bytes = self.take(length)?;
        std::str::from_utf8(bytes).map_err(|error| format!("morphology payload: {error}"))
    }

    fn optional_string(&mut self) -> Result<Option<&'a str>, String> {
        if self.u8()? == 0 {
            Ok(None)
        } else {
            self.string().map(Some)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::morphology::{MorphologyCache, analyze_morphology};
    use crate::tokenise::tokenise;

    fn tags() -> Tags {
        Tags {
            part_of_speech: Some(PartOfSpeech::Noun),
            case: Some(Case::Inessive),
            number: Some(Number::Plural),
            person: None,
            tense: None,
            mood: None,
            voice: None,
            possessive: Some(Possessive::FirstSingular),
            clitic: Some(Clitic::Kin),
            polarity: Polarity::Affirmative,
        }
    }

    fn row() -> (String, Arc<WordInfo>) {
        let tags = tags();
        (
            "taloissa".to_string(),
            Arc::new(WordInfo {
                key: "taloissa".to_string(),
                lemma: Some("talo".to_string()),
                verb_lemma: None,
                nominal_lemma: Some("talo".to_string()),
                exact: true,
                lemmas: vec!["talo".to_string(), "talota".to_string()],
                readings: vec![tags],
                case: Some(Case::Inessive),
                number: Some(Number::Plural),
                part_of_speech: Some(PartOfSpeech::Noun),
                clitic: Some(Clitic::Kin),
                possessive: Some(Possessive::FirstSingular),
                derivation: Some(crate::derivation::Reverse {
                    rule: "AGENT_JA",
                    source: "talo".to_string(),
                }),
                depth: Depth::StemMap,
            }),
        )
    }

    #[test]
    fn a_row_round_trips() {
        let rows = vec![row()];
        let decoded = decode_rows(&encode_rows(&rows)).expect("valid payload");
        assert_eq!(decoded.len(), 1);
        let (surface, info) = &decoded[0];
        assert_eq!(surface, "taloissa");
        assert_eq!(info.key, "taloissa");
        assert_eq!(info.lemma.as_deref(), Some("talo"));
        assert_eq!(info.lemmas, vec!["talo", "talota"]);
        assert_eq!(info.readings, vec![tags()]);
        assert_eq!(info.case, Some(Case::Inessive));
        assert_eq!(info.number, Some(Number::Plural));
        assert_eq!(info.part_of_speech, Some(PartOfSpeech::Noun));
        assert_eq!(info.clitic, Some(Clitic::Kin));
        assert_eq!(info.possessive, Some(Possessive::FirstSingular));
        assert_eq!(info.derivation.as_ref().map(|r| r.rule), Some("AGENT_JA"));
        assert_eq!(info.depth, Depth::StemMap);
    }

    #[test]
    fn a_real_analysis_round_trips() {
        let text = "Koira nukkuu talossa. Kaksi lasta leikkii.";
        let tokens = tokenise(text);
        let mut cache = MorphologyCache::new();
        let analysis = analyze_morphology(&tokens, &mut cache);
        let mut rows: Vec<(String, Arc<WordInfo>)> = Vec::new();
        for (token, info) in tokens.iter().zip(&analysis.tokens) {
            if let (true, Some(info)) = (token.is_word(), info) {
                let surface = token.text.to_lowercase();
                if !rows.iter().any(|(seen, _)| *seen == surface) {
                    rows.push((surface, Arc::clone(info)));
                }
            }
        }
        let decoded = decode_rows(&encode_rows(&rows)).expect("valid payload");
        assert_eq!(decoded.len(), rows.len());
        for ((surface, original), (decoded_surface, decoded_info)) in rows.iter().zip(&decoded) {
            assert_eq!(surface, decoded_surface);
            assert_eq!(original.key, decoded_info.key);
            assert_eq!(original.lemma, decoded_info.lemma);
            assert_eq!(original.readings, decoded_info.readings);
            assert_eq!(original.case, decoded_info.case);
            assert_eq!(original.depth, decoded_info.depth);
        }
    }

    #[test]
    fn a_seeded_cache_reproduces_the_single_thread_analysis() {
        let text = "Kaksi lasta leikkii. Koira nukkuu talossa.";
        let tokens = tokenise(text);
        let mut cold = MorphologyCache::new();
        let expected = analyze_morphology(&tokens, &mut cold);

        let mut worker = MorphologyCache::new();
        let rows = crate::morphology::resolve_surfaces(&tokens, &mut worker);
        let decoded =
            decode_rows(&encode_rows(&rows)).expect("the worker payload is valid");
        let mut seeded = MorphologyCache::new();
        for (surface, info) in decoded {
            seeded.seed(surface, info);
        }
        let actual = analyze_morphology(&tokens, &mut seeded);

        assert_eq!(expected.tokens.len(), actual.tokens.len());
        for (expected_token, actual_token) in expected.tokens.iter().zip(&actual.tokens) {
            match (expected_token, actual_token) {
                (Some(expected), Some(actual)) => {
                    assert_eq!(
                        format!("{expected:?}"),
                        format!("{actual:?}"),
                        "a seeded row changed the analysis"
                    );
                }
                (None, None) => {}
                _ => panic!("a seeded row changed the token shape"),
            }
        }
    }

    #[test]
    fn a_truncated_payload_is_rejected() {
        let bytes = encode_rows(&[row()]);
        assert!(decode_rows(&bytes[..bytes.len() - 1]).is_err());
        assert!(decode_rows(b"nope").is_err());
    }
}
