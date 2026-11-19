//! Clause features for construction-aware word-order reasoning.
//!
//! Word order cannot be judged from positions alone: `Kirjan luin eilen` is
//! object fronting, `Pöydällä on kirja` is an existential, and `Tuleeko hän?`
//! is a question whose clitic marks the focus. The specification is
//! [`docs/word_order.md`](../../../docs/word_order.md); this module computes the
//! features it names so a rule never has to re-derive them from raw positions.
//!
//! Everything here is a **surface classification with a positive cue**. A
//! feature the analysis cannot defend keeps its neutral value rather than
//! guessing:
//!
//! - `focus_type` is `Contrastive` only for an emphasised all-caps word the
//!   lexicon knows; narrow/broad focus needs prosody and is not modelled.
//! - `register` is `Colloquial` only for an explicit colloquial form;
//!   there is no formal-register model.
//! - `question_type` is `Echo` only when a question's trailing question word
//!   was segmented as a one-word clause (`Hän asuu missä?`).
//!
//! Object case comes from morphology, never from the position after the verb
//! (§2 of the specification).

use crate::tags::{Case, Clitic, Mood};
use crate::tokenise::Token;

use super::predicates::is_finite_predicate;
use super::{Clause, SyntacticRole, TokenSyntax};
use crate::syntax::is_subordinator;

/// The clause's sentence type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClauseType {
    Declarative,
    Interrogative,
    Imperative,
    /// A relative or conjunction-introduced clause.
    Subordinate,
}

impl ClauseType {
    pub fn name(self) -> &'static str {
        match self {
            ClauseType::Declarative => "declarative",
            ClauseType::Interrogative => "interrogative",
            ClauseType::Imperative => "imperative",
            ClauseType::Subordinate => "subordinate",
        }
    }
}

/// The syntactic construction the clause realises.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Construction {
    /// An ordinary transitive or intransitive clause.
    Ordinary,
    /// `Pöydällä on kirja`, `Talossa asuu ihmisiä`.
    Existential,
    /// `Täällä puhutaan suomea`, `Asia on jo käsitelty`.
    Passive,
    /// `Talo on kaunis`, `Hän tuli iloiseksi`.
    Copular,
    /// `Minulla on kirja` — an adessive possessor.
    Possessive,
    /// `Minun täytyy mennä`, `Hänen pitää mennä` — a genitive subject with a
    /// necessity predicate.
    Necessive,
}

impl Construction {
    pub fn name(self) -> &'static str {
        match self {
            Construction::Ordinary => "ordinary",
            Construction::Existential => "existential",
            Construction::Passive => "passive",
            Construction::Copular => "copular",
            Construction::Possessive => "possessive",
            Construction::Necessive => "necessive",
        }
    }
}

/// How the clause subject is realised.
///
/// This is a construction/function feature, not a new [`crate::syntax::SyntacticRole`].
/// The role stays `Subject`; the subject's shape and the agreement behaviour
/// are read here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SubjectKind {
    /// A normal nominative subject.
    Nominative,
    /// A partitive subject, typically existential or quantity.
    Partitive,
    /// The genitive subject of a necessity construction.
    GenitiveNecessive,
    /// The postverbal subject of an existential.
    Existential,
    /// No overt subject.
    Omitted,
    /// A subject whose case is not one of the modelled shapes.
    Other,
}

impl SubjectKind {
    pub fn name(self) -> &'static str {
        match self {
            SubjectKind::Nominative => "nominative",
            SubjectKind::Partitive => "partitive",
            SubjectKind::GenitiveNecessive => "genitive_necessive",
            SubjectKind::Existential => "existential",
            SubjectKind::Omitted => "omitted",
            SubjectKind::Other => "other",
        }
    }
}

/// What kind of question the clause is, when it is a question.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuestionType {
    /// `Missä hän asuu?`
    Wh,
    /// `Tuleeko hän?`, `Helsingissäkö hän asuu?`
    YesNo,
    /// `Tuletko vai jäätkö?`
    Alternative,
    /// `Hän asuu missä?` — a marked question with a postverbal question word.
    Echo,
}

impl QuestionType {
    pub fn name(self) -> &'static str {
        match self {
            QuestionType::Wh => "wh",
            QuestionType::YesNo => "yes_no",
            QuestionType::Alternative => "alternative",
            QuestionType::Echo => "echo",
        }
    }
}

/// Where the clause subject stands relative to the predicate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SubjectPosition {
    Preverbal,
    Postverbal,
    /// No overt subject: pro-drop, an imperative, a passive or an existential.
    Omitted,
}

impl SubjectPosition {
    pub fn name(self) -> &'static str {
        match self {
            SubjectPosition::Preverbal => "preverbal",
            SubjectPosition::Postverbal => "postverbal",
            SubjectPosition::Omitted => "omitted",
        }
    }
}

/// The case of the clause object, as morphology resolved it.
///
/// Never inferred from position: `Kirjan luin` is a genitive object that
/// happens to stand first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectCase {
    Partitive,
    Accusative,
    Genitive,
    Nominative,
}

impl ObjectCase {
    pub fn name(self) -> &'static str {
        match self {
            ObjectCase::Partitive => "partitive",
            ObjectCase::Accusative => "accusative",
            ObjectCase::Genitive => "genitive",
            ObjectCase::Nominative => "nominative",
        }
    }

    fn from_case(case: Case) -> Option<Self> {
        Some(match case {
            Case::Partitive => ObjectCase::Partitive,
            Case::Accusative => ObjectCase::Accusative,
            Case::Genitive => ObjectCase::Genitive,
            Case::Nominative => ObjectCase::Nominative,
            _ => return None,
        })
    }
}

/// How much of the clause carries emphasis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FocusType {
    Neutral,
    /// An emphasised all-caps constituent (`JUURI HÄN teki sen`).
    Contrastive,
}

impl FocusType {
    pub fn name(self) -> &'static str {
        match self {
            FocusType::Neutral => "neutral",
            FocusType::Contrastive => "contrastive",
        }
    }
}

/// Whether a constituent occupies the clause-initial slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TopicPosition {
    /// A constituent precedes the finite verb — the default in Finnish.
    Initial,
    /// The clause is verb-initial, so the topic follows the verb.
    NonInitial,
}

impl TopicPosition {
    pub fn name(self) -> &'static str {
        match self {
            TopicPosition::Initial => "initial",
            TopicPosition::NonInitial => "non_initial",
        }
    }
}

/// The register the clause is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Register {
    Neutral,
    /// A colloquial form the engine recognises (`mä`, `sun`, `tää`, ...).
    Colloquial,
}

impl Register {
    pub fn name(self) -> &'static str {
        match self {
            Register::Neutral => "neutral",
            Register::Colloquial => "colloquial",
        }
    }
}

/// The word-order-relevant features of one clause.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClauseFeatures {
    pub clause_type: ClauseType,
    pub construction: Construction,
    pub subject_kind: SubjectKind,
    pub question_type: Option<QuestionType>,
    pub subject_position: SubjectPosition,
    pub object_case: Option<ObjectCase>,
    pub focus_type: FocusType,
    pub topic_position: TopicPosition,
    pub register: Register,
}

impl Default for ClauseFeatures {
    fn default() -> Self {
        Self {
            clause_type: ClauseType::Declarative,
            construction: Construction::Ordinary,
            subject_kind: SubjectKind::Omitted,
            question_type: None,
            subject_position: SubjectPosition::Omitted,
            object_case: None,
            focus_type: FocusType::Neutral,
            topic_position: TopicPosition::Initial,
            register: Register::Neutral,
        }
    }
}

impl ClauseFeatures {
    /// The stable name of one feature, for fixtures and diagnostics.
    ///
    /// `question_type` and `object_case` are `None` when the clause has no
    /// question or no object; the caller decides whether that is a failure.
    pub fn value(&self, feature: &str) -> Option<&'static str> {
        Some(match feature {
            "clause_type" => self.clause_type.name(),
            "construction" => self.construction.name(),
            "subject_kind" => self.subject_kind.name(),
            "question_type" => self.question_type?.name(),
            "subject_position" => self.subject_position.name(),
            "object_case" => self.object_case?.name(),
            "focus_type" => self.focus_type.name(),
            "topic_position" => self.topic_position.name(),
            "register" => self.register.name(),
            _ => return None,
        })
    }

    /// Every feature name, in specification order.
    pub const NAMES: [&'static str; 9] = [
        "clause_type",
        "construction",
        "subject_kind",
        "question_type",
        "subject_position",
        "object_case",
        "focus_type",
        "topic_position",
        "register",
    ];
}

/// Interrogative surfaces that open a direct wh-question.
///
/// The inflected forms of `kuka`/`ken` and `mikä` are included because the
/// opener is often `Kenen`, `Missä` or `Mitä`, not the citation form. `koska`
/// is absent because it is also the subordinator “because”.
///
/// This is the single list for the syntax layer and the punctuation rule; the
/// question-mark rule reads it through [`super::INTERROGATIVE_SURFACES`].
pub(crate) const INTERROGATIVE_SURFACES: &[&str] = &[
    "kuka", "ken", "ketä", "kenen", "kelle", "keneltä", "kenelle", "kenessä", "kenestä",
    "kenellä", "ketkä", "keiden", "keitä", "keissä", "keistä", "keihin", "keillä", "keiltä",
    "keille", "mikä", "mitä", "minkä", "missä", "mistä", "mihin", "millä", "miltä", "mille",
    "miksi", "mitkä", "millainen", "millaisia", "millaiset", "kumpi", "kumman", "kumpaa",
    "kummat", "kumpia", "milloin", "miten", "kuinka", "montako",
];

/// Colloquial forms that are not standard written Finnish.
const COLLOQUIAL_SURFACES: &[&str] = &[
    "mä", "mun", "mua", "mut", "sä", "sun", "sua", "sut", "tää", "tän", "tätä", "toi", "nää",
    "noi", "meijän", "teijän", "tommonen", "semmonen", "tällänen", "ku",
];

/// Assign the features to every clause.
///
/// Runs after role resolution, so the subject/object/case fields are final.
/// Features are computed read-only first and written back at the end, because
/// `question_type` needs to look at a clause's children.
pub(super) fn assign(clauses: &mut [Clause], tokens: &[Token], out: &[TokenSyntax]) {
    let mut features: Vec<ClauseFeatures> = clauses
        .iter()
        .map(|clause| features_of(clause, tokens, out))
        .collect();

    for (index, clause) in clauses.iter().enumerate() {
        if is_echo_question(clause, clauses, tokens, out, &features) {
            features[index].clause_type = ClauseType::Interrogative;
            features[index].question_type = Some(QuestionType::Echo);
        }
    }

    for (clause, features) in clauses.iter_mut().zip(features) {
        clause.features = features;
    }
}

fn features_of(clause: &Clause, tokens: &[Token], out: &[TokenSyntax]) -> ClauseFeatures {
    let words = clause_words(clause, tokens);
    let terminator = terminator(tokens, &words);
    let predicate = clause.predicate;

    let clause_type = clause_type_of(clause, &words, terminator, predicate, out);
    let question_type = if clause_type == ClauseType::Interrogative {
        Some(question_type_of(&words, terminator, out))
    } else {
        None
    };
    let construction = construction_of(clause, &words, predicate, out);

    ClauseFeatures {
        clause_type,
        construction,
        subject_kind: subject_kind_of(clause, construction, out),
        question_type,
        subject_position: subject_position_of(clause, predicate),
        object_case: clause
            .object
            .and_then(|index| out[index].case)
            .and_then(ObjectCase::from_case),
        focus_type: focus_type_of(&words, tokens, out),
        topic_position: topic_position_of(&words, predicate, out),
        register: register_of(&words, out),
    }
}

/// The word token indices inside a clause, in source order.
fn clause_words(clause: &Clause, tokens: &[Token]) -> Vec<usize> {
    clause
        .tokens
        .clone()
        .filter(|&index| tokens[index].is_word())
        .collect()
}

/// The first terminal mark after the clause's last word.
///
/// A line break is not a terminator: scanning stops at the next word, so only
/// `?`/`!` in the separators immediately after the clause are reported.
fn terminator(tokens: &[Token], words: &[usize]) -> Option<char> {
    let last = *words.last()?;
    for token in &tokens[last + 1..] {
        if token.is_word() {
            break;
        }
        if token.text.contains('?') {
            return Some('?');
        }
        if token.text.contains('!') {
            return Some('!');
        }
    }
    None
}

fn clause_type_of(
    clause: &Clause,
    words: &[usize],
    terminator: Option<char>,
    predicate: Option<usize>,
    out: &[TokenSyntax],
) -> ClauseType {
    let Some(&first) = words.first() else {
        return ClauseType::Declarative;
    };

    // An imperative needs the mood cue and one of the two structural signals:
    // no overt subject (`Avaa ovi!`) or an exclamation mark. `Hän avaa oven`
    // is a declarative whose verb also has an imperative homonym.
    let imperative = predicate.is_some_and(|predicate| {
        out[predicate]
            .readings
            .iter()
            .any(|reading| reading.mood == Some(Mood::Imperative) && is_finite_predicate(reading))
    }) && (clause.subject.is_none() || terminator == Some('!'));
    if imperative {
        return ClauseType::Imperative;
    }

    if terminator == Some('?') || ko_clitic(words, out) {
        return ClauseType::Interrogative;
    }

    if clause.parent.is_some() || is_subordinator(out[first].key.as_str()) {
        return ClauseType::Subordinate;
    }
    ClauseType::Declarative
}

fn question_type_of(
    words: &[usize],
    terminator: Option<char>,
    out: &[TokenSyntax],
) -> QuestionType {
    let first = words.first().copied().unwrap_or(0);
    let wh_initial = INTERROGATIVE_SURFACES.contains(&out[first].key.as_str());
    // The `-ko`/`-kö` clitic is read from morphology. A proper name whose
    // paradigm the name table owns (`Helsingissäkö`) carries no clitic
    // reading, so a question mark plus a surface ending is the fallback.
    let clitic =
        ko_clitic(words, out) || (terminator == Some('?') && has_ko_surface(&out[first].key));
    let alternative = words.iter().any(|&index| out[index].key == "vai");

    if alternative && (clitic || terminator == Some('?')) {
        return QuestionType::Alternative;
    }
    if clitic {
        return QuestionType::YesNo;
    }
    if wh_initial {
        return QuestionType::Wh;
    }
    // A bare question mark on a declarative is a yes/no question with
    // question intonation (`Hän tuli?`).
    QuestionType::YesNo
}

fn ko_clitic(words: &[usize], out: &[TokenSyntax]) -> bool {
    words.iter().any(|&index| {
        out[index]
            .readings
            .iter()
            .any(|reading| reading.clitic == Some(Clitic::Ko))
    })
}

fn has_ko_surface(key: &str) -> bool {
    key.len() > 2 && (key.ends_with("ko") || key.ends_with("kö"))
}

/// `Hän asuu missä?` — a question whose trailing question word was segmented
/// as a one-word clause of its own.
///
/// The host becomes `Echo`; the trailing word keeps its own clause, because
/// the linear clause array stays the source of truth. A trailing question word
/// is the surface cue a checker has for a marked, echo-like question, and it
/// is one of the exceptions `FI_QUESTION_001` names.
fn is_echo_question(
    clause: &Clause,
    clauses: &[Clause],
    tokens: &[Token],
    out: &[TokenSyntax],
    features: &[ClauseFeatures],
) -> bool {
    let Some(&child) = clause.children.last() else {
        return false;
    };
    let child_clause = &clauses[child];
    if child_clause.predicate.is_some() || !child_clause.children.is_empty() {
        return false;
    }
    let words = clause_words(child_clause, tokens);
    let [word] = words.as_slice() else {
        return false;
    };
    INTERROGATIVE_SURFACES.contains(&out[*word].key.as_str())
        && features[child].question_type == Some(QuestionType::Wh)
}

fn construction_of(
    clause: &Clause,
    words: &[usize],
    predicate: Option<usize>,
    out: &[TokenSyntax],
) -> Construction {
    if clause.passive {
        return Construction::Passive;
    }
    if is_possessive(clause, words, predicate, out) {
        return Construction::Possessive;
    }
    if clause.existential {
        return Construction::Existential;
    }
    if is_necessive(clause, predicate, out) {
        return Construction::Necessive;
    }
    let copular = predicate.is_some_and(|predicate| {
        out[predicate].lemma.as_deref() == Some("olla") && clause.complement.is_some()
    });
    if copular {
        return Construction::Copular;
    }
    Construction::Ordinary
}

/// `Minun täytyy mennä`, `Hänen pitää mennä`, `Minun on mentävä`: a genitive
/// subject with a necessity predicate.
fn is_necessive(clause: &Clause, predicate: Option<usize>, out: &[TokenSyntax]) -> bool {
    let Some(predicate) = predicate else {
        return false;
    };
    let Some(subject) = clause.subject else {
        return false;
    };
    if out[subject].case != Some(Case::Genitive) {
        return false;
    }
    match out[predicate].lemma.as_deref() {
        Some("täytyä" | "pitää" | "tarvita") => true,
        Some("olla") => clause.complement.is_some_and(|complement| {
            out[complement].readings.iter().any(|reading| {
                reading.voice == Some(crate::tags::Voice::Passive)
                    && reading.mood != Some(Mood::Infinitive)
            })
        }),
        _ => false,
    }
}

fn subject_kind_of(
    clause: &Clause,
    construction: Construction,
    out: &[TokenSyntax],
) -> SubjectKind {
    if construction == Construction::Existential {
        return SubjectKind::Existential;
    }
    let Some(subject) = clause.subject else {
        return SubjectKind::Omitted;
    };
    match out[subject].case {
        Some(Case::Nominative) => SubjectKind::Nominative,
        Some(Case::Partitive) => SubjectKind::Partitive,
        Some(Case::Genitive) if construction == Construction::Necessive => {
            SubjectKind::GenitiveNecessive
        }
        _ => SubjectKind::Other,
    }
}

/// `Minulla on kirja`, `Hänellä on auto`: an adessive personal possessor.
///
/// The adessive *place* of `Pöydällä on kirja` is deliberately not possessive:
/// the two constructions share a surface pattern, and Finnish distinguishes
/// them by the possessor being a person. Only the personal pronouns are
/// modelled, so `Talolla on katto` stays existential.
fn is_possessive(
    _clause: &Clause,
    words: &[usize],
    predicate: Option<usize>,
    out: &[TokenSyntax],
) -> bool {
    let Some(predicate) = predicate else {
        return false;
    };
    if out[predicate].lemma.as_deref() != Some("olla") {
        return false;
    }
    words
        .iter()
        .take_while(|&&index| index != predicate)
        .any(|&index| {
            out[index].case == Some(Case::Adessive)
                && out[index].lemma.as_deref().is_some_and(is_personal_pronoun)
        })
}

fn is_personal_pronoun(lemma: &str) -> bool {
    matches!(lemma, "minä" | "sinä" | "hän" | "me" | "te" | "he")
}

fn subject_position_of(clause: &Clause, predicate: Option<usize>) -> SubjectPosition {
    let (Some(subject), Some(predicate)) = (clause.subject, predicate) else {
        return SubjectPosition::Omitted;
    };
    if subject < predicate {
        SubjectPosition::Preverbal
    } else {
        SubjectPosition::Postverbal
    }
}

fn focus_type_of(words: &[usize], tokens: &[Token], out: &[TokenSyntax]) -> FocusType {
    let contrastive = words.iter().any(|&index| {
        // Emphasis is a property of the source spelling, not the analysis key:
        // `TokenSyntax::key` is lowercased.
        let text = tokens[index].text.as_str();
        text.chars().count() > 1
            && text
                .chars()
                .all(|character| !character.is_alphabetic() || character.is_uppercase())
            && text.chars().any(|character| character.is_alphabetic())
            // An acronym (`EU`, `DNA`) has no lexical reading; only an
            // emphasised ordinary word is contrastive focus.
            && !out[index].readings.is_empty()
    });
    if contrastive {
        FocusType::Contrastive
    } else {
        FocusType::Neutral
    }
}

fn topic_position_of(
    words: &[usize],
    predicate: Option<usize>,
    out: &[TokenSyntax],
) -> TopicPosition {
    let Some(&first) = words.first() else {
        return TopicPosition::Initial;
    };
    let verb_first = Some(first) == predicate || out[first].role == SyntacticRole::Auxiliary;
    if verb_first {
        TopicPosition::NonInitial
    } else {
        TopicPosition::Initial
    }
}

fn register_of(words: &[usize], out: &[TokenSyntax]) -> Register {
    let colloquial = words
        .iter()
        .any(|&index| COLLOQUIAL_SURFACES.contains(&out[index].key.as_str()));
    if colloquial {
        Register::Colloquial
    } else {
        Register::Neutral
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::analyze;
    use crate::tokenise::tokenise;

    fn features(text: &str) -> ClauseFeatures {
        let tokens = tokenise(text);
        let analysis = analyze(&tokens);
        analysis.clauses[0].features
    }

    fn question(text: &str) -> ClauseFeatures {
        let tokens = tokenise(text);
        let analysis = analyze(&tokens);
        analysis
            .clauses
            .iter()
            .map(|clause| clause.features)
            .find(|features| features.question_type.is_some())
            .unwrap_or_else(|| panic!("{text}: no question clause"))
    }

    #[test]
    fn a_wh_question_is_interrogative_and_wh() {
        for text in ["Missä hän asuu?", "Miksi tulit tänne?"] {
            let features = question(text);
            assert_eq!(features.clause_type, ClauseType::Interrogative, "{text}");
            assert_eq!(features.question_type, Some(QuestionType::Wh), "{text}");
        }
    }

    #[test]
    fn a_ko_question_is_yes_no() {
        let features = question("Tuleeko hän?");
        assert_eq!(features.clause_type, ClauseType::Interrogative);
        assert_eq!(features.question_type, Some(QuestionType::YesNo));
        assert_eq!(
            features.topic_position,
            TopicPosition::NonInitial,
            "the clitic constituent opens the clause"
        );
    }

    #[test]
    fn a_name_whose_paradigm_has_no_clitic_reading_still_reads_as_yes_no() {
        // `Helsingissäkö` is resolved through the name table, so the only cue
        // is the surface ending plus the question mark.
        let features = question("Helsingissäkö hän asuu?");
        assert_eq!(features.question_type, Some(QuestionType::YesNo));
    }

    #[test]
    fn an_alternative_question_is_its_own_type() {
        let tokens = tokenise("Tuletko vai jäätkö?");
        let analysis = analyze(&tokens);
        let alternatives: Vec<QuestionType> = analysis
            .clauses
            .iter()
            .filter_map(|clause| clause.features.question_type)
            .filter(|kind| *kind == QuestionType::Alternative)
            .collect();
        assert_eq!(alternatives, [QuestionType::Alternative]);
    }

    #[test]
    fn a_trailing_question_word_marks_an_echo_question() {
        let features = question("Hän asuu missä?");
        assert_eq!(features.clause_type, ClauseType::Interrogative);
        assert_eq!(features.question_type, Some(QuestionType::Echo));
    }

    #[test]
    fn an_imperative_needs_no_subject() {
        for text in ["Tule tänne!", "Avaa ovi!", "Lukekaa kirja!"] {
            let features = features(text);
            assert_eq!(features.clause_type, ClauseType::Imperative, "{text}");
            assert_eq!(
                features.subject_position,
                SubjectPosition::Omitted,
                "{text}"
            );
        }
    }

    #[test]
    fn an_overt_subject_keeps_a_verb_with_an_imperative_homonym_declarative() {
        let features = features("Hän avaa oven.");
        assert_eq!(features.clause_type, ClauseType::Declarative);
        assert_eq!(features.subject_position, SubjectPosition::Preverbal);
        assert_eq!(features.object_case, Some(ObjectCase::Genitive));
    }

    #[test]
    fn a_neutral_declarative_is_subject_verb_object() {
        let features = features("Minä ostin auton.");
        assert_eq!(features.clause_type, ClauseType::Declarative);
        assert_eq!(features.construction, Construction::Ordinary);
        assert_eq!(features.subject_position, SubjectPosition::Preverbal);
        assert_eq!(features.object_case, Some(ObjectCase::Genitive));
        assert_eq!(features.topic_position, TopicPosition::Initial);
        assert_eq!(features.question_type, None);
    }

    #[test]
    fn an_existential_has_a_postverbal_subject() {
        for text in [
            "Pöydällä on kirja.",
            "Talossa asuu ihmisiä.",
            "Puutarhassa kasvaa kukkia.",
        ] {
            let features = features(text);
            assert_eq!(features.construction, Construction::Existential, "{text}");
            assert_eq!(
                features.subject_position,
                SubjectPosition::Postverbal,
                "{text}"
            );
            assert_eq!(features.topic_position, TopicPosition::Initial, "{text}");
        }
    }

    #[test]
    fn a_transitive_partitive_object_does_not_become_an_existential() {
        // `ostaa` is not an existence verb: the locative opener and the
        // partitive must not turn the clause into an existential.
        let features = features("Kaupassa ostan ruokaa.");
        assert_eq!(features.construction, Construction::Ordinary);
        assert_eq!(features.object_case, Some(ObjectCase::Partitive));
    }

    #[test]
    fn a_nominative_subject_keeps_an_appearance_verb_out_of_the_existential() {
        // A nominative subject agrees with its verb; only the partitive
        // pattern is the existential one.
        let features = features("Naapurissa asuvat Liisa ja Pekka.");
        assert_eq!(features.construction, Construction::Ordinary);
    }

    #[test]
    fn a_personal_adorative_possessor_is_possessive() {
        let features = features("Minulla on kirja.");
        assert_eq!(features.construction, Construction::Possessive);
        assert_eq!(features.subject_position, SubjectPosition::Postverbal);
    }

    #[test]
    fn a_place_adorative_stays_existential() {
        let features = features("Pöydällä on kirja.");
        assert_eq!(features.construction, Construction::Existential);
    }

    #[test]
    fn a_passive_has_no_subject() {
        for text in [
            "Täällä puhutaan suomea.",
            "Asia on jo käsitelty.",
            "Asia käsiteltiin eilen.",
        ] {
            let features = features(text);
            assert_eq!(features.construction, Construction::Passive, "{text}");
            assert_eq!(
                features.subject_position,
                SubjectPosition::Omitted,
                "{text}"
            );
        }
    }

    #[test]
    fn a_copula_with_a_complement_is_copular() {
        for text in ["Talo on kaunis.", "Oletko valmis?"] {
            assert_eq!(features(text).construction, Construction::Copular, "{text}");
        }
    }

    #[test]
    fn a_subordinator_marks_a_subordinate_clause() {
        let tokens = tokenise("Tiedän, että hän tulee.");
        let analysis = analyze(&tokens);
        let subordinate = analysis
            .clauses
            .iter()
            .find(|clause| analysis.tokens[clause.tokens.start].key == "että")
            .expect("the että clause");
        assert_eq!(subordinate.features.clause_type, ClauseType::Subordinate);
    }

    #[test]
    fn a_relative_clause_is_subordinate() {
        let tokens = tokenise("Mies, joka tuli, söi.");
        let analysis = analyze(&tokens);
        let relative = analysis
            .clauses
            .iter()
            .find(|clause| clause.parent.is_some())
            .expect("a relative clause");
        assert_eq!(relative.features.clause_type, ClauseType::Subordinate);
    }

    #[test]
    fn object_fronting_keeps_its_morphological_case() {
        let features = features("Kirjan luin eilen.");
        assert_eq!(features.subject_position, SubjectPosition::Omitted);
        assert_eq!(features.object_case, Some(ObjectCase::Genitive));
        assert_eq!(features.topic_position, TopicPosition::Initial);
        assert_eq!(features.construction, Construction::Ordinary);
    }

    #[test]
    fn verb_initial_order_has_a_non_initial_topic() {
        for text in ["Söi Sami puuron.", "Tuleeko hän?"] {
            let features = features(text);
            assert_eq!(features.topic_position, TopicPosition::NonInitial, "{text}");
        }
    }

    #[test]
    fn a_fronted_adverbial_keeps_the_topic_initial() {
        let features = features("Eilen kävin kaupassa.");
        assert_eq!(features.topic_position, TopicPosition::Initial);
        assert_eq!(features.subject_position, SubjectPosition::Omitted);
    }

    #[test]
    fn an_emphasised_word_marks_contrastive_focus() {
        assert_eq!(
            features("JUURI HÄN teki sen.").focus_type,
            FocusType::Contrastive
        );
    }

    #[test]
    fn an_acronym_is_not_contrastive_focus() {
        assert_eq!(features("EU on tärkeä.").focus_type, FocusType::Neutral);
    }

    #[test]
    fn a_colloquial_form_marks_the_register() {
        assert_eq!(features("Mä tuun kotiin.").register, Register::Colloquial);
        assert_eq!(features("Minä tulen kotiin.").register, Register::Neutral);
    }

    #[test]
    fn a_necessity_clause_has_a_genitive_subject() {
        let features = features("Minun täytyy mennä.");
        assert_eq!(features.construction, Construction::Necessive);
        assert_eq!(features.subject_kind, SubjectKind::GenitiveNecessive);
    }

    #[test]
    fn a_necessity_olla_has_a_genitive_subject() {
        let features = features("Minun on mentävä.");
        assert_eq!(features.construction, Construction::Necessive);
        assert_eq!(features.subject_kind, SubjectKind::GenitiveNecessive);
    }

    #[test]
    fn a_normal_subject_is_nominative_and_an_existential_is_marked() {
        assert_eq!(
            features("Talo on kaunis.").subject_kind,
            SubjectKind::Nominative
        );
        assert_eq!(
            features("Pöydällä on kirja.").subject_kind,
            SubjectKind::Existential
        );
        assert_eq!(
            features("Täällä puhutaan suomea.").subject_kind,
            SubjectKind::Omitted
        );
    }

    #[test]
    fn every_feature_has_a_stable_name() {
        let existential = features("Talossa asuu ihmisiä.");
        for name in [
            "clause_type",
            "construction",
            "subject_kind",
            "subject_position",
            "focus_type",
            "topic_position",
            "register",
        ] {
            assert!(existential.value(name).is_some(), "{name}");
        }
        // A declarative existential has no question and no object.
        assert_eq!(existential.value("question_type"), None);
        assert_eq!(existential.value("object_case"), None);
        assert_eq!(existential.value("nonsense"), None);
        assert_eq!(
            existential.value("construction"),
            Some("existential"),
            "the fixture vocabulary is the stable wire format"
        );

        let question = features("Missä hän asuu?");
        assert_eq!(question.value("question_type"), Some("wh"));
        let object = features("Kirjan luin eilen.");
        assert_eq!(object.value("object_case"), Some("genitive"));
    }
}
