//! Initial manual ambiguity fixture format and validator.
//!
//! The fixture is deliberately candidate-signature based, not reading-index
//! based. A reading index is an implementation detail of `WordInfo`; a manual
//! label should survive candidate regeneration. The harness resolves a
//! signature to an index at run time and records whether it is unavailable.

use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};
use torille::tags::{Case, Clitic, Mood, Number, PartOfSpeech, Person, Polarity, Possessive, Tags, Tense, Voice};

use super::mapping::UD_MAPPING_VERSION;

pub const AMBIGUITY_FORMAT_VERSION: u32 = 1;

/// A manual label for one candidate signature.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
pub struct CandidateSignature {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lemma: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub person: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tense: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mood: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polarity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub possessive: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clitic: Option<String>,
}

impl CandidateSignature {
    /// Whether `tags` matches every field the signature names. Unknown field
    /// names are a fixture validation error, not a silent wildcard.
    pub fn matches(&self, tags: &Tags) -> bool {
        if let Some(pos) = &self.pos
            && parse_pos(pos) != tags.part_of_speech
        {
            return false;
        }
        if let Some(case) = &self.case
            && parse_case(case) != tags.case
        {
            return false;
        }
        if let Some(number) = &self.number
            && parse_number(number) != tags.number
        {
            return false;
        }
        if let Some(person) = &self.person
            && parse_person(person) != tags.person
        {
            return false;
        }
        if let Some(tense) = &self.tense
            && parse_tense(tense) != tags.tense
        {
            return false;
        }
        if let Some(mood) = &self.mood
            && parse_mood(mood) != tags.mood
        {
            return false;
        }
        if let Some(voice) = &self.voice
            && parse_voice(voice) != tags.voice
        {
            return false;
        }
        if let Some(polarity) = &self.polarity
            && parse_polarity(polarity) != Some(tags.polarity)
        {
            return false;
        }
        if let Some(possessive) = &self.possessive
            && parse_possessive(possessive) != tags.possessive
        {
            return false;
        }
        if let Some(clitic) = &self.clitic
            && parse_clitic(clitic) != tags.clitic
        {
            return false;
        }
        true
    }

    /// Validate every named enum value. Returns an error naming the field.
    pub fn validate(&self) -> Result<(), String> {
        if let Some(value) = &self.pos {
            expect(parse_pos(value).is_some(), "pos", value)?;
        }
        if let Some(value) = &self.case {
            expect(parse_case(value).is_some(), "case", value)?;
        }
        if let Some(value) = &self.number {
            expect(parse_number(value).is_some(), "number", value)?;
        }
        if let Some(value) = &self.person {
            expect(parse_person(value).is_some(), "person", value)?;
        }
        if let Some(value) = &self.tense {
            expect(parse_tense(value).is_some(), "tense", value)?;
        }
        if let Some(value) = &self.mood {
            expect(parse_mood(value).is_some(), "mood", value)?;
        }
        if let Some(value) = &self.voice {
            expect(parse_voice(value).is_some(), "voice", value)?;
        }
        if let Some(value) = &self.polarity {
            expect(parse_polarity(value).is_some(), "polarity", value)?;
        }
        if let Some(value) = &self.possessive {
            expect(parse_possessive(value).is_some(), "possessive", value)?;
        }
        if let Some(value) = &self.clitic {
            expect(parse_clitic(value).is_some(), "clitic", value)?;
        }
        Ok(())
    }
}

fn expect(ok: bool, field: &str, value: &str) -> Result<(), String> {
    if ok {
        Ok(())
    } else {
        Err(format!("unknown {field} value {value:?}"))
    }
}

/// One manually adjudicated ambiguity case.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct AmbiguityCase {
    pub id: String,
    pub sentence: String,
    /// Zero-based token index in the tokenised sentence.
    pub token_index: usize,
    pub surface: String,
    /// The intended reading, expressed as a candidate signature.
    pub preferred: CandidateSignature,
    /// Other acceptable readings. Empty means only `preferred` is accepted.
    #[serde(default)]
    pub acceptable: Vec<CandidateSignature>,
    /// Whether the checker may abstain on this case.
    #[serde(default)]
    pub abstain_ok: bool,
    #[serde(default)]
    pub note: Option<String>,
}

/// The fixture file.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct AmbiguityFixture {
    pub format_version: u32,
    pub mapping_version: String,
    #[serde(default)]
    pub cases: Vec<AmbiguityCase>,
}

#[derive(Debug)]
pub enum AmbiguityError {
    Io { path: String, source: io::Error },
    Json { path: String, source: serde_json::Error },
    Invalid { id: String, message: String },
}

impl fmt::Display for AmbiguityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AmbiguityError::Io { path, source } => write!(f, "{path}: {source}"),
            AmbiguityError::Json { path, source } => write!(f, "{path}: {source}"),
            AmbiguityError::Invalid { id, message } => write!(f, "{id}: {message}"),
        }
    }
}

impl std::error::Error for AmbiguityError {}

/// Read and validate a manual ambiguity fixture.
pub fn read_file(path: impl AsRef<Path>) -> Result<AmbiguityFixture, AmbiguityError> {
    let path = path.as_ref();
    let text = fs::read_to_string(path).map_err(|source| AmbiguityError::Io {
        path: path.display().to_string(),
        source,
    })?;
    let fixture: AmbiguityFixture =
        serde_json::from_str(&text).map_err(|source| AmbiguityError::Json {
            path: path.display().to_string(),
            source,
        })?;
    validate(&fixture)?;
    Ok(fixture)
}

/// Validate a fixture already parsed from JSON.
pub fn validate(fixture: &AmbiguityFixture) -> Result<(), AmbiguityError> {
    if fixture.format_version != AMBIGUITY_FORMAT_VERSION {
        return Err(AmbiguityError::Invalid {
            id: "<fixture>".to_string(),
            message: format!(
                "format_version is {}, expected {}",
                fixture.format_version, AMBIGUITY_FORMAT_VERSION
            ),
        });
    }
    if fixture.mapping_version != UD_MAPPING_VERSION {
        return Err(AmbiguityError::Invalid {
            id: "<fixture>".to_string(),
            message: format!(
                "mapping_version is {:?}, expected {:?}",
                fixture.mapping_version, UD_MAPPING_VERSION
            ),
        });
    }
    if fixture.cases.is_empty() {
        return Err(AmbiguityError::Invalid {
            id: "<fixture>".to_string(),
            message: "cases is empty".to_string(),
        });
    }
    let mut seen = std::collections::BTreeSet::new();
    for case in &fixture.cases {
        if !seen.insert(case.id.as_str()) {
            return Err(AmbiguityError::Invalid {
                id: case.id.clone(),
                message: "duplicate id".to_string(),
            });
        }
        if case.surface.trim().is_empty() {
            return Err(AmbiguityError::Invalid {
                id: case.id.clone(),
                message: "surface is empty".to_string(),
            });
        }
        case.preferred
            .validate()
            .map_err(|message| AmbiguityError::Invalid {
                id: case.id.clone(),
                message,
            })?;
        for acceptable in &case.acceptable {
            acceptable
                .validate()
                .map_err(|message| AmbiguityError::Invalid {
                    id: case.id.clone(),
                    message,
                })?;
        }
        if !case.acceptable.is_empty()
            && !case
                .acceptable
                .iter()
                .any(|candidate| candidate == &case.preferred)
        {
            return Err(AmbiguityError::Invalid {
                id: case.id.clone(),
                message: "preferred is not in acceptable".to_string(),
            });
        }
    }
    Ok(())
}

fn parse_pos(value: &str) -> Option<PartOfSpeech> {
    PartOfSpeech::ALL
        .into_iter()
        .find(|pos| pos.name() == value || pos.finnish() == value)
}

fn parse_case(value: &str) -> Option<Case> {
    Case::ALL.into_iter().find(|case| case.name() == value)
}

fn parse_number(value: &str) -> Option<Number> {
    Number::ALL.into_iter().find(|number| number.name() == value)
}

fn parse_person(value: &str) -> Option<Person> {
    Person::ALL.into_iter().find(|person| person.name() == value)
}

fn parse_tense(value: &str) -> Option<Tense> {
    Tense::ALL.into_iter().find(|tense| tense.name() == value)
}

fn parse_mood(value: &str) -> Option<Mood> {
    Mood::ALL.into_iter().find(|mood| mood.name() == value)
}

fn parse_voice(value: &str) -> Option<Voice> {
    Voice::ALL.into_iter().find(|voice| voice.name() == value)
}

fn parse_polarity(value: &str) -> Option<Polarity> {
    Polarity::ALL
        .into_iter()
        .find(|polarity| polarity.name() == value)
}

fn parse_possessive(value: &str) -> Option<Possessive> {
    Possessive::ALL.into_iter().find(|possessive| match value {
        "first_singular" => *possessive == Possessive::FirstSingular,
        "second_singular" => *possessive == Possessive::SecondSingular,
        "third" => *possessive == Possessive::Third,
        "first_plural" => *possessive == Possessive::FirstPlural,
        "second_plural" => *possessive == Possessive::SecondPlural,
        _ => false,
    })
}

fn parse_clitic(value: &str) -> Option<Clitic> {
    Clitic::ALL.into_iter().find(|clitic| match value {
        "ko" => *clitic == Clitic::Ko,
        "kin" => *clitic == Clitic::Kin,
        "kaan" => *clitic == Clitic::Kaan,
        "han" => *clitic == Clitic::Han,
        "pa" => *clitic == Clitic::Pa,
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "format_version": 1,
        "mapping_version": "fi-ud-tags-1",
        "cases": [
            {
                "id": "tuli_noun",
                "sentence": "Tuli sammui.",
                "token_index": 0,
                "surface": "Tuli",
                "preferred": {"lemma": "tuli", "pos": "noun", "case": "nominative", "number": "singular"},
                "acceptable": [
                    {"lemma": "tuli", "pos": "noun", "case": "nominative", "number": "singular"}
                ],
                "abstain_ok": false,
                "note": "fire"
            }
        ]
    }"#;

    #[test]
    fn a_minimal_fixture_validates() {
        let fixture: AmbiguityFixture = serde_json::from_str(SAMPLE).unwrap();
        validate(&fixture).unwrap();
        assert_eq!(fixture.cases.len(), 1);
        assert!(fixture.cases[0].preferred.matches(&Tags {
            part_of_speech: Some(PartOfSpeech::Noun),
            case: Some(Case::Nominative),
            number: Some(Number::Singular),
            ..Tags::UNKNOWN
        }));
    }

    #[test]
    fn a_wrong_mapping_version_is_rejected() {
        let mut fixture: AmbiguityFixture = serde_json::from_str(SAMPLE).unwrap();
        fixture.mapping_version = "old".to_string();
        assert!(validate(&fixture).is_err());
    }

    #[test]
    fn an_unknown_signature_value_is_rejected() {
        let mut fixture: AmbiguityFixture = serde_json::from_str(SAMPLE).unwrap();
        fixture.cases[0].preferred.case = Some("not-a-case".to_string());
        assert!(validate(&fixture).is_err());
    }

    #[test]
    fn the_seed_fixture_validates() {
        let fixture: AmbiguityFixture = serde_json::from_str(include_str!(
            "../../../tests/fixtures/neural/manual_ambiguity.json"
        ))
        .unwrap();
        validate(&fixture).unwrap();
        assert_eq!(fixture.cases.len(), 6);
    }
}
