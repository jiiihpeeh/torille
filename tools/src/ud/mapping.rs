//! Versioned UD → torille `Tags` mapping for the Phase 1 coverage harness.
//!
//! The mapping is intentionally partial and explicit. A UD reading can be
//! exact, partial (a feature is approximated), ambiguous (the feature has more
//! than one torille value), or unavailable (torille has no representation for
//! it). Only exact/partial readings enter the model-error denominator; an
//! unavailable gold reading is never counted as a model prediction error.
//!
//! Bump [`UD_MAPPING_VERSION`] whenever a mapping decision changes. The
//! candidate-coverage report and the manual ambiguity fixture both carry this
//! version.

use torille::tags::{Case, Clitic, Mood, Number, PartOfSpeech, Person, Polarity, Possessive, Tense, Tags, Voice};

use super::reader::UdToken;

/// Version of the UD-to-`Tags` mapping in this module.
pub const UD_MAPPING_VERSION: &str = "fi-ud-tags-1";

/// A bit for each `Tags` dimension the gold mapping actually claims.
pub mod mask {
    pub const POS: u16 = 1 << 0;
    pub const CASE: u16 = 1 << 1;
    pub const NUMBER: u16 = 1 << 2;
    pub const PERSON: u16 = 1 << 3;
    pub const TENSE: u16 = 1 << 4;
    pub const MOOD: u16 = 1 << 5;
    pub const VOICE: u16 = 1 << 6;
    pub const POSSESSIVE: u16 = 1 << 7;
    pub const CLITIC: u16 = 1 << 8;
    pub const POLARITY: u16 = 1 << 9;
}

/// How faithfully a UD reading can be represented.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MappingStatus {
    /// Every claimed feature has an exact torille equivalent.
    Exact,
    /// The reading is usable, but at least one feature is approximated or
    /// missing from `Tags`.
    Partial,
    /// A UD feature has more than one torille interpretation, so there is no
    /// single gold `Tags`.
    Ambiguous,
    /// The reading needs a feature torille cannot represent. It is excluded
    /// from model-error accounting.
    Unavailable,
}

/// A gold UD reading mapped onto the `Tags` dimensions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoldReading {
    pub tags: Tags,
    /// Which `Tags` dimensions are claimed by the UD annotation.
    pub mask: u16,
    pub status: MappingStatus,
    /// Features or values that were not representable.
    pub unsupported: Vec<String>,
    /// One-line explanation of an approximation or ambiguity.
    pub note: Option<String>,
    /// The UD lemma, unless the corpus gives `_`.
    pub lemma: Option<String>,
}

impl GoldReading {
    /// Whether the candidate tags agree on every dimension the gold reading
    /// actually claims. A missing candidate dimension is not a wildcard here.
    pub fn tags_match(&self, candidate: &Tags) -> bool {
        if self.mask & mask::POS != 0 && candidate.part_of_speech != self.tags.part_of_speech {
            return false;
        }
        if self.mask & mask::CASE != 0 && candidate.case != self.tags.case {
            return false;
        }
        if self.mask & mask::NUMBER != 0 && candidate.number != self.tags.number {
            return false;
        }
        if self.mask & mask::PERSON != 0 && candidate.person != self.tags.person {
            return false;
        }
        if self.mask & mask::TENSE != 0 && candidate.tense != self.tags.tense {
            return false;
        }
        if self.mask & mask::MOOD != 0 && candidate.mood != self.tags.mood {
            return false;
        }
        if self.mask & mask::VOICE != 0 && candidate.voice != self.tags.voice {
            return false;
        }
        if self.mask & mask::POSSESSIVE != 0 && candidate.possessive != self.tags.possessive {
            return false;
        }
        if self.mask & mask::CLITIC != 0 && candidate.clitic != self.tags.clitic {
            return false;
        }
        if self.mask & mask::POLARITY != 0 && candidate.polarity != self.tags.polarity {
            return false;
        }
        true
    }

    /// The UD lemma is available for matching.
    pub fn has_lemma(&self) -> bool {
        self.lemma.is_some()
    }
}

/// Map one UD token to a gold `Tags` reading.
pub fn map_token(token: &UdToken) -> GoldReading {
    let mut gold = GoldReading {
        tags: Tags::UNKNOWN,
        mask: 0,
        status: MappingStatus::Exact,
        unsupported: Vec::new(),
        note: None,
        lemma: (token.lemma != "_").then(|| token.lemma.clone()),
    };
    if gold.lemma.is_none() {
        gold.partial("missing UD lemma");
    }

    map_upos(token, &mut gold);
    if gold.status == MappingStatus::Unavailable {
        return gold;
    }

    if let Some(value) = token.feature("Case") {
        match map_case(value) {
            Some(case) => {
                gold.tags.case = Some(case);
                gold.mask |= mask::CASE;
                if value == "Acc" {
                    gold.ambiguous(
                        "UD Case=Acc maps to Accusative/Genitive/Nominative in torille",
                    );
                }
            }
            None => {
                gold.unavailable(format!("Case={value}"));
                return gold;
            }
        }
    }
    if let Some(value) = token.feature("Number") {
        match map_number(value) {
            Some(number) => {
                gold.tags.number = Some(number);
                gold.mask |= mask::NUMBER;
            }
            None => gold.partial(format!("Number={value}")),
        }
    }
    if let Some(value) = token.feature("Person")
        && value != "0"
    {
        if let Some(person) = map_person(value) {
            gold.tags.person = Some(person);
            gold.mask |= mask::PERSON;
        } else {
            gold.partial(format!("Person={value}"));
        }
    }
    if let Some(value) = token.feature("Tense") {
        match value {
            "Pres" => {
                gold.tags.tense = Some(Tense::Present);
                gold.mask |= mask::TENSE;
            }
            "Past" => {
                gold.tags.tense = Some(Tense::Past);
                gold.mask |= mask::TENSE;
            }
            other => gold.partial(format!("Tense={other}")),
        }
    }
    if let Some(value) = token.feature("Mood") {
        match value {
            "Ind" => {
                gold.tags.mood = Some(Mood::Indicative);
                gold.mask |= mask::MOOD;
            }
            "Cnd" => {
                gold.tags.mood = Some(Mood::Conditional);
                gold.mask |= mask::MOOD;
            }
            "Imp" => {
                gold.tags.mood = Some(Mood::Imperative);
                gold.mask |= mask::MOOD;
            }
            "Pot" => {
                gold.tags.mood = Some(Mood::Potential);
                gold.mask |= mask::MOOD;
            }
            other => gold.partial(format!("Mood={other}")),
        }
    }
    if let Some(value) = token.feature("Voice") {
        match value {
            "Act" => {
                gold.tags.voice = Some(Voice::Active);
                gold.mask |= mask::VOICE;
            }
            "Pass" => {
                gold.tags.voice = Some(Voice::Passive);
                gold.mask |= mask::VOICE;
            }
            other => gold.partial(format!("Voice={other}")),
        }
    }
    if let Some(value) = token.feature("VerbForm") {
        match value {
            "Fin" => {}
            "Inf" => {
                gold.tags.mood = Some(Mood::Infinitive);
                gold.mask |= mask::MOOD;
                gold.partial("VerbForm=Inf (infinitive form not fully represented)");
            }
            "Part" => {
                gold.partial("VerbForm=Part (participle form not fully represented)");
            }
            other => gold.partial(format!("VerbForm={other}")),
        }
    }
    if let Some(value) = token.feature("Connegative")
        && value == "Yes"
    {
        gold.tags.polarity = Polarity::Negative;
        gold.mask |= mask::POLARITY;
    }
    if let Some(value) = token.feature("Polarity") {
        match value {
            "Neg" => {
                gold.tags.polarity = Polarity::Negative;
                gold.mask |= mask::POLARITY;
            }
            "Pos" => {
                gold.tags.polarity = Polarity::Affirmative;
                gold.mask |= mask::POLARITY;
            }
            other => gold.partial(format!("Polarity={other}")),
        }
    }
    if let Some(value) = token.feature("Poss")
        && value == "Yes"
    {
        match possessive_from_feats(token) {
            Some(possessive) => {
                gold.tags.possessive = Some(possessive);
                gold.mask |= mask::POSSESSIVE;
            }
            None => gold.ambiguous("Poss=Yes without Person[psor]/Number[psor]"),
        }
    }
    if let Some(value) = token.feature("Clitic") {
        match map_clitic(value) {
            Some(clitic) => {
                gold.tags.clitic = Some(clitic);
                gold.mask |= mask::CLITIC;
            }
            None => gold.partial(format!("Clitic={value}")),
        }
    }

    for key in [
        "Degree",
        "NumType",
        "PronType",
        "Derivation",
        "Reflex",
        "Foreign",
        "Typo",
        "Style",
        "Abbr",
        "PartForm",
        "InfForm",
    ] {
        if let Some(value) = token.feature(key) {
            gold.unsupported.push(format!("{key}={value}"));
        }
    }
    if !gold.unsupported.is_empty() && gold.status == MappingStatus::Exact {
        gold.status = MappingStatus::Partial;
        gold.note = Some(format!(
            "unsupported feature(s): {}",
            gold.unsupported.join(", ")
        ));
    }
    gold
}

fn map_upos(token: &UdToken, gold: &mut GoldReading) {
    match token.upos.as_str() {
        "NOUN" => {
            gold.tags.part_of_speech = Some(PartOfSpeech::Noun);
            gold.mask |= mask::POS;
        }
        "PROPN" => {
            gold.tags.part_of_speech = Some(PartOfSpeech::Noun);
            gold.mask |= mask::POS;
            gold.partial("UPOS=PROPN collapsed to Noun");
        }
        "ADJ" => {
            gold.tags.part_of_speech = Some(PartOfSpeech::Adjective);
            gold.mask |= mask::POS;
        }
        "PRON" => {
            gold.tags.part_of_speech = Some(PartOfSpeech::Pronoun);
            gold.mask |= mask::POS;
        }
        "NUM" => {
            gold.tags.part_of_speech = Some(PartOfSpeech::Numeral);
            gold.mask |= mask::POS;
        }
        "VERB" => {
            gold.tags.part_of_speech = Some(PartOfSpeech::Verb);
            gold.mask |= mask::POS;
        }
        "AUX" => {
            if matches!(token.lemma.as_str(), "ei" | "älä" | "älkää") {
                gold.tags.part_of_speech = Some(PartOfSpeech::NegativeVerb);
            } else {
                gold.tags.part_of_speech = Some(PartOfSpeech::Verb);
            }
            gold.mask |= mask::POS;
            gold.partial("UPOS=AUX collapsed to Verb/NegativeVerb");
        }
        "ADV" => {
            gold.tags.part_of_speech = Some(PartOfSpeech::Adverb);
            gold.mask |= mask::POS;
        }
        "ADP" => match token.feature("AdpType") {
            Some("Prep") => {
                gold.tags.part_of_speech = Some(PartOfSpeech::Preposition);
                gold.mask |= mask::POS;
            }
            Some("Post") => {
                gold.tags.part_of_speech = Some(PartOfSpeech::Postposition);
                gold.mask |= mask::POS;
            }
            Some(other) => {
                gold.tags.part_of_speech = Some(PartOfSpeech::Other);
                gold.mask |= mask::POS;
                gold.partial(format!("AdpType={other}"));
            }
            None => gold.ambiguous("UPOS=ADP without AdpType"),
        },
        "CCONJ" | "SCONJ" => {
            gold.tags.part_of_speech = Some(PartOfSpeech::Conjunction);
            gold.mask |= mask::POS;
        }
        "INTJ" => {
            gold.tags.part_of_speech = Some(PartOfSpeech::Interjection);
            gold.mask |= mask::POS;
        }
        "PUNCT" | "SYM" | "X" | "DET" | "PART" => {
            gold.unavailable(format!("UPOS={}", token.upos));
        }
        other => gold.unavailable(format!("UPOS={other}")),
    }
}

fn map_case(value: &str) -> Option<Case> {
    Some(match value {
        "Nom" => Case::Nominative,
        "Acc" => Case::Accusative,
        "Gen" => Case::Genitive,
        "Par" => Case::Partitive,
        "Ine" => Case::Inessive,
        "Ela" => Case::Elative,
        "Ill" => Case::Illative,
        "Ade" => Case::Adessive,
        "Abl" => Case::Ablative,
        "All" => Case::Allative,
        "Ess" => Case::Essive,
        "Tra" => Case::Translative,
        "Abe" => Case::Abessive,
        "Com" => Case::Comitative,
        "Pro" => Case::Prolative,
        "Ins" => Case::Instructive,
        _ => return None,
    })
}

fn map_number(value: &str) -> Option<Number> {
    Some(match value {
        "Sing" => Number::Singular,
        "Plur" => Number::Plural,
        _ => return None,
    })
}

fn map_person(value: &str) -> Option<Person> {
    Some(match value {
        "1" => Person::First,
        "2" => Person::Second,
        "3" => Person::Third,
        _ => return None,
    })
}

fn map_clitic(value: &str) -> Option<Clitic> {
    Some(match value {
        "Ko" => Clitic::Ko,
        "Kin" => Clitic::Kin,
        "Kaan" => Clitic::Kaan,
        "Han" => Clitic::Han,
        "Pa" => Clitic::Pa,
        _ => return None,
    })
}

fn possessive_from_feats(token: &UdToken) -> Option<Possessive> {
    let person = token.feature("Person[psor]")?;
    let number = token.feature("Number[psor]").unwrap_or("Sing");
    Some(match (person, number) {
        ("1", "Sing") => Possessive::FirstSingular,
        ("2", "Sing") => Possessive::SecondSingular,
        ("3", _) => Possessive::Third,
        ("1", "Plur") => Possessive::FirstPlural,
        ("2", "Plur") => Possessive::SecondPlural,
        _ => return None,
    })
}

impl GoldReading {
    fn partial(&mut self, note: impl Into<String>) {
        if self.status == MappingStatus::Exact {
            self.status = MappingStatus::Partial;
        }
        let note = note.into();
        match self.note.as_mut() {
            Some(existing) => {
                if !existing.contains(&note) {
                    existing.push_str("; ");
                    existing.push_str(&note);
                }
            }
            None => self.note = Some(note),
        }
    }

    fn ambiguous(&mut self, note: impl Into<String>) {
        if matches!(
            self.status,
            MappingStatus::Exact | MappingStatus::Partial
        ) {
            self.status = MappingStatus::Ambiguous;
        }
        self.note = Some(note.into());
    }

    fn unavailable(&mut self, note: impl Into<String>) {
        self.status = MappingStatus::Unavailable;
        self.note = Some(note.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ud::reader::UdToken;
    use std::collections::BTreeMap;

    fn token(upos: &str, lemma: &str, feats: &[(&str, &str)]) -> UdToken {
        UdToken {
            id: 1,
            form: lemma.to_string(),
            lemma: lemma.to_string(),
            upos: upos.to_string(),
            feats: feats
                .iter()
                .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
                .collect::<BTreeMap<_, _>>(),
            head: None,
            deprel: "_".to_string(),
        }
    }

    #[test]
    fn a_plain_noun_is_exact() {
        let gold = map_token(&token(
            "NOUN",
            "kissa",
            &[("Case", "Nom"), ("Number", "Sing")],
        ));
        assert_eq!(gold.status, MappingStatus::Exact);
        assert_eq!(gold.tags.part_of_speech, Some(PartOfSpeech::Noun));
        assert_eq!(gold.tags.case, Some(Case::Nominative));
        assert_eq!(gold.tags.number, Some(Number::Singular));
        assert_eq!(gold.mask & mask::CASE, mask::CASE);
    }

    #[test]
    fn a_degree_adjective_is_partial_not_exact() {
        let gold = map_token(&token(
            "ADJ",
            "punainen",
            &[("Case", "Nom"), ("Number", "Sing"), ("Degree", "Pos")],
        ));
        assert_eq!(gold.status, MappingStatus::Partial);
        assert!(gold.unsupported.iter().any(|item| item.starts_with("Degree=")));
    }

    #[test]
    fn a_connegative_verb_maps_polarity() {
        let gold = map_token(&token(
            "VERB",
            "mennä",
            &[
                ("VerbForm", "Fin"),
                ("Mood", "Ind"),
                ("Connegative", "Yes"),
            ],
        ));
        assert_eq!(gold.status, MappingStatus::Exact);
        assert_eq!(gold.tags.polarity, Polarity::Negative);
        assert_eq!(gold.mask & mask::POLARITY, mask::POLARITY);
    }

    #[test]
    fn a_proper_noun_is_partial() {
        let gold = map_token(&token(
            "PROPN",
            "Helsinki",
            &[("Case", "Nom"), ("Number", "Sing")],
        ));
        assert_eq!(gold.status, MappingStatus::Partial);
        assert_eq!(gold.tags.part_of_speech, Some(PartOfSpeech::Noun));
    }

    #[test]
    fn an_adp_without_adptype_is_ambiguous() {
        let gold = map_token(&token("ADP", "alla", &[("Case", "Gen")]));
        assert_eq!(gold.status, MappingStatus::Ambiguous);
    }

    #[test]
    fn an_accusative_object_case_is_ambiguous() {
        let gold = map_token(&token(
            "NOUN",
            "kirja",
            &[("Case", "Acc"), ("Number", "Sing")],
        ));
        assert_eq!(gold.status, MappingStatus::Ambiguous);
        assert_eq!(gold.tags.case, Some(Case::Accusative));
    }

    #[test]
    fn a_punctuation_token_is_unavailable() {
        let gold = map_token(&token("PUNCT", ".", &[]));
        assert_eq!(gold.status, MappingStatus::Unavailable);
    }

    #[test]
    fn masking_only_compares_claimed_dimensions() {
        let gold = map_token(&token(
            "NOUN",
            "kissa",
            &[("Case", "Nom"), ("Number", "Sing")],
        ));
        let candidate = Tags {
            case: Some(Case::Nominative),
            number: Some(Number::Singular),
            ..Tags::UNKNOWN
        };
        // POS is claimed by the UD mapping, so a candidate without it fails.
        assert!(!gold.tags_match(&candidate));
        let candidate = Tags {
            part_of_speech: Some(PartOfSpeech::Noun),
            ..candidate
        };
        assert!(gold.tags_match(&candidate));
    }
}
