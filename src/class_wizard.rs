//! Find a word's Kotus inflection class from a few example forms.
//!
//! This is the inverse of the inflection engine. For each candidate class it
//! asks [`crate::expand::inflect_in`] what that class would generate for the
//! word and keeps the classes that reproduce the forms the user supplied. It
//! never appends an ending itself: the engine is the only authority, so a class
//! engine that gets a form wrong is a wizard failure rather than a second
//! implementation to keep in step.
//!
//! The data is text. [`SHEET`] is the committed class sheet, [`parse_answers`]
//! reads a wizard session and [`parse_user_words`] reads a saved list; each is
//! parsed once into a hash map and queried by key. The sheet is generated from
//! the catalogue, not maintained by hand, and [`check_sheet`] is the gate that
//! keeps it from drifting.
//!
//! Nominal classes use the eight catalogue slots; verb classes use a small
//! finite-form grid (`pres_1sg`, `past_3sg`, ...), so each part of speech gets
//! its own question pool and sheet rows.

use std::collections::{HashMap, HashSet};
use std::fmt::{self, Write as _};
use std::sync::OnceLock;

use crate::expand::{LexiconIndex, inflect_in};
use crate::grammar::is_verb_class;
use crate::kotus::{self, ClassId, Gradation, Inflection};
use crate::spec::Catalogues;
use crate::tags::{
    Case, Degree, Mood, Number, PartOfSpeech, Person, Polarity, Possessive, Tags, Tense, Voice,
};

/// The committed class sheet, embedded so the CLI and the wasm read the same
/// bytes and no runtime file is needed.
pub const SHEET: &str = include_str!("../data/class_examples.tsv");

/// The eight catalogue slots a nominal class always has, in slot order.
///
/// Slot position is the reading (`src/expand/slot.rs`): the singular
/// nominative, genitive, partitive and illative, then the same four plural.
pub const CORE_KEYS: [&str; 8] = [
    "nom_sg", "gen_sg", "par_sg", "ill_sg", "nom_pl", "gen_pl", "par_pl", "ill_pl",
];

/// The cells the wizard asks about, in the order it prefers when two cells
/// separate the candidates equally well.
///
/// The nominative is the word the user already typed, so it is not a question.
/// The accusative is left out because it is the nominative or the genitive
/// depending on the sentence, which no single answer can settle.
pub const QUESTION_POOL: [&str; 7] = [
    "gen_sg", "par_sg", "ill_sg", "nom_pl", "gen_pl", "par_pl", "ill_pl",
];

/// The questions for a plural-only word.
///
/// The nominative plural is the word the user typed, so it is not a question;
/// the singular cells cannot be answered by a word that has no singular, and
/// asking them only produces three `unknown` answers before the first useful
/// one.
pub const PLURAL_QUESTION_POOL: [&str; 3] = ["gen_pl", "par_pl", "ill_pl"];

/// The questions for a singular-only word. The nominative singular is the
/// word the user typed.
pub const SINGULAR_QUESTION_POOL: [&str; 3] = ["gen_sg", "par_sg", "ill_sg"];

/// The six verb cells in the sheet, and the five the wizard asks about.
///
/// The nominative is the lemma itself. `pres_1pl` is a sheet-only extra: the
/// five questions already separate the verb classes, and five is the budget.
pub const VERB_KEYS: [&str; 6] = [
    "pres_1sg", "pres_3sg", "past_3sg", "cond_3sg", "imp_2sg", "pres_1pl",
];

pub const VERB_QUESTION_POOL: [&str; 5] =
    ["pres_1sg", "pres_3sg", "past_3sg", "cond_3sg", "imp_2sg"];

/// The class the explicit "does not inflect" switch returns.
///
/// Kotus uses `99` for a word that is uninflected or only partly inflected;
/// the wizard uses it for the user's deliberate choice, and only the lemma
/// itself is accepted for such a saved word.
pub const INDECLINABLE_CLASS: ClassId = 99;

/// `class -> key -> value`, the shape every text file in this module parses
/// into. The sheet is the only producer today.
pub type Sheet = HashMap<String, HashMap<String, String>>;

/// `cell -> form`, one wizard session.
pub type Answers = HashMap<String, String>;

/// One saved word.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserWord {
    /// The word as the user typed it, capitalization included.
    pub word: String,
    /// The canonical class expression, e.g. `5*A` or `9, 10`.
    pub class: String,
    /// The reading the engine inflects with. A proper noun is a noun here.
    pub pos: PartOfSpeech,
    /// The label the user chose (`substantiivi`, `erisnimi`, `numeraali`, ...).
    pub pos_name: String,
    /// Which number the entry licenses. `None` means both; `Some` is the
    /// explicit singular-only/plural-only restriction the user chose.
    pub number: Option<Number>,
}

/// `lowercased word -> entry`, the saved list.
pub type UserWords = HashMap<String, UserWord>;

/// A line number and a reason, for a text file the user can fix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub line: usize,
    pub message: String,
}

impl ParseError {
    fn new(line: usize, message: impl Into<String>) -> Self {
        ParseError {
            line,
            message: message.into(),
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ParseError {}

/// Why the wizard cannot answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WizardError {
    EmptyWord,
    UnsupportedPartOfSpeech(PartOfSpeech),
}

impl fmt::Display for WizardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WizardError::EmptyWord => write!(f, "the word is empty"),
            WizardError::UnsupportedPartOfSpeech(pos) => {
                write!(f, "{} is not a supported part of speech", pos.name())
            }
        }
    }
}

impl std::error::Error for WizardError {}

/// A nominal grammatical cell: the dimensions a user's answer can name.
///
/// A dimension that is `None` is not part of the question and is left open, so
/// `gen_sg` accepts a form that also carries a possessive. A dimension that is
/// `Some` must be claimed by the form: a form whose reading does not say which
/// case it is does not answer the question. That is deliberately stricter than
/// [`Tags::matches`], which treats an absent dimension as a wildcard.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Cell {
    pub case: Option<Case>,
    pub number: Option<Number>,
    pub possessive: Option<Possessive>,
    pub person: Option<Person>,
    pub tense: Option<Tense>,
    pub mood: Option<Mood>,
    pub voice: Option<Voice>,
    pub degree: Option<Degree>,
    /// Verb cells require an affirmative form; a nominal cell leaves it open.
    pub polarity: Option<Polarity>,
}

impl Cell {
    /// Parses a cell key such as `gen_sg` or `pres_1sg`.
    pub fn parse(key: &str) -> Option<Cell> {
        if let Some(cell) = verb_cell(key) {
            return Some(cell);
        }
        let (name, number) = key.rsplit_once('_')?;
        let number = match number {
            "sg" => Number::Singular,
            "pl" => Number::Plural,
            _ => return None,
        };
        Some(Cell {
            case: Some(nominal_case(name)?),
            number: Some(number),
            ..Cell::default()
        })
    }

    /// The canonical key, when the cell is one the text format can name.
    pub fn key(self) -> Option<String> {
        if self.case.is_none() {
            return verb_key(self);
        }
        let name = nominal_case_name(self.case?)?;
        let number = match self.number? {
            Number::Singular => "sg",
            Number::Plural => "pl",
            Number::Collective => return None,
        };
        if self.possessive.is_some()
            || self.person.is_some()
            || self.tense.is_some()
            || self.mood.is_some()
            || self.voice.is_some()
            || self.degree.is_some()
            || self.polarity.is_some()
        {
            return None;
        }
        Some(format!("{name}_{number}"))
    }

    /// Whether `tags` claims this cell.
    ///
    /// A wanted dimension must be present and equal. A dimension the cell does
    /// not name is a wildcard. The asymmetry is the point: `Tags::matches`
    /// accepts a missing dimension on either side, which would let a case-less
    /// possessive form answer every case question.
    pub fn claims(self, tags: &Tags) -> bool {
        fn same<T: PartialEq>(want: Option<T>, have: Option<T>) -> bool {
            want.is_none() || have == want
        }
        same(self.case, tags.case)
            && same(self.number, tags.number)
            && same(self.possessive, tags.possessive)
            && same(self.person, tags.person)
            && same(self.tense, tags.tense)
            && same(self.mood, tags.mood)
            && same(self.voice, tags.voice)
            && same(self.degree, tags.degree)
            && self
                .polarity
                .is_none_or(|polarity| tags.polarity == polarity)
    }

    /// The cell as tags, for callers that have a `Tags`-shaped API.
    pub fn tags(self) -> Tags {
        Tags {
            case: self.case,
            number: self.number,
            possessive: self.possessive,
            person: self.person,
            tense: self.tense,
            mood: self.mood,
            voice: self.voice,
            degree: self.degree,
            polarity: self.polarity.unwrap_or(Polarity::Affirmative),
            ..Tags::UNKNOWN
        }
    }
}

fn verb_cell(key: &str) -> Option<Cell> {
    let (mood, rest) = key.split_once('_')?;
    let (person, number) = match rest {
        "1sg" => (Person::First, Number::Singular),
        "2sg" => (Person::Second, Number::Singular),
        "3sg" => (Person::Third, Number::Singular),
        "1pl" => (Person::First, Number::Plural),
        "2pl" => (Person::Second, Number::Plural),
        "3pl" => (Person::Third, Number::Plural),
        _ => return None,
    };
    let (mood, tense) = match mood {
        "pres" => (Mood::Indicative, Some(Tense::Present)),
        "past" => (Mood::Indicative, Some(Tense::Past)),
        "cond" => (Mood::Conditional, Some(Tense::Present)),
        "imp" => (Mood::Imperative, Some(Tense::Present)),
        _ => return None,
    };
    Some(Cell {
        number: Some(number),
        person: Some(person),
        tense,
        mood: Some(mood),
        voice: Some(Voice::Active),
        polarity: Some(Polarity::Affirmative),
        ..Cell::default()
    })
}

fn verb_key(cell: Cell) -> Option<String> {
    let mood = match cell.mood? {
        Mood::Indicative => match cell.tense? {
            Tense::Present => "pres",
            Tense::Past => "past",
            _ => return None,
        },
        Mood::Conditional => "cond",
        Mood::Imperative => "imp",
        _ => return None,
    };
    let person = match cell.person? {
        Person::First => "1",
        Person::Second => "2",
        Person::Third => "3",
    };
    let number = match cell.number? {
        Number::Singular => "sg",
        Number::Plural => "pl",
        Number::Collective => return None,
    };
    if cell.case.is_some()
        || cell.possessive.is_some()
        || cell.voice != Some(Voice::Active)
        || cell.polarity != Some(Polarity::Affirmative)
        || cell.degree.is_some()
    {
        return None;
    }
    Some(format!("{mood}_{person}{number}"))
}

fn nominal_case(name: &str) -> Option<Case> {
    Some(match name {
        "nom" => Case::Nominative,
        "gen" => Case::Genitive,
        "par" => Case::Partitive,
        "ill" => Case::Illative,
        "ine" => Case::Inessive,
        "ela" => Case::Elative,
        "ade" => Case::Adessive,
        "abl" => Case::Ablative,
        "all" => Case::Allative,
        "ess" => Case::Essive,
        "tra" => Case::Translative,
        "abe" => Case::Abessive,
        "com" => Case::Comitative,
        "pro" => Case::Prolative,
        "ins" => Case::Instructive,
        _ => return None,
    })
}

fn nominal_case_name(case: Case) -> Option<&'static str> {
    Some(match case {
        Case::Nominative => "nom",
        Case::Genitive => "gen",
        Case::Partitive => "par",
        Case::Illative => "ill",
        Case::Inessive => "ine",
        Case::Elative => "ela",
        Case::Adessive => "ade",
        Case::Ablative => "abl",
        Case::Allative => "all",
        Case::Essive => "ess",
        Case::Translative => "tra",
        Case::Abessive => "abe",
        Case::Comitative => "com",
        Case::Prolative => "pro",
        Case::Instructive => "ins",
        Case::Accusative => return None,
    })
}

/// Reads a part of speech from the names the wizard accepts: the sanalista's
/// Finnish names, the English ones, and `nominal` for the class sheet.
pub fn pos_from_name(name: &str) -> Option<PartOfSpeech> {
    let name = name.trim().to_lowercase();
    if name.is_empty() {
        return None;
    }
    let parsed = PartOfSpeech::parse(&name);
    if !matches!(parsed, PartOfSpeech::Other | PartOfSpeech::Unknown) {
        return Some(parsed);
    }
    Some(match name.as_str() {
        "noun" | "nominal" => PartOfSpeech::Noun,
        "adjective" | "adj" => PartOfSpeech::Adjective,
        "pronoun" => PartOfSpeech::Pronoun,
        "numeral" => PartOfSpeech::Numeral,
        "ordinal" => PartOfSpeech::Ordinal,
        "verb" => PartOfSpeech::Verb,
        "adverb" => PartOfSpeech::Adverb,
        // A proper noun is a noun for inflection; the label is kept in the
        // saved word so the list can say what the user meant.
        "erisnimi" | "proper noun" | "proper_noun" | "propernoun" => PartOfSpeech::Noun,
        _ => return None,
    })
}

/// A class expression and how well it explains the answers so far.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub class: Inflection,
    /// Answers the class reproduces in the asked cell.
    pub hits: usize,
    /// Asked cells where the class emits a form and it is the wrong one.
    pub misses: usize,
    /// Asked cells where the class emits nothing at all.
    pub unknown: usize,
    /// The sanalista records this exact expression for the word.
    pub recorded: bool,
    /// How often lemmas ending in the same three characters carry this exact
    /// expression. A tie-break that looks at the word's tail rather than the
    /// whole-lexicon frequency.
    pub tail_prior: usize,
    /// How many lemmas in the lexicon carry this base class; a tie-break only.
    pub prior: usize,
}

/// What happened to one answer, for the report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnswerStatus {
    Hit,
    Miss,
    Unknown,
    Skipped,
}

/// One answer and its verdict under the best class.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnswerOutcome {
    pub key: String,
    pub form: String,
    pub status: AnswerStatus,
}

/// The wizard's answer for one session.
#[derive(Clone, Debug)]
pub struct Report {
    pub lemma: String,
    pub pos: PartOfSpeech,
    /// The classes the sanalista records for this word, if it knows it.
    pub recorded: Vec<Inflection>,
    /// Every candidate class, best first.
    pub ranked: Vec<Candidate>,
    /// Each answer's verdict under `ranked[0]`.
    pub outcomes: Vec<AnswerOutcome>,
    /// The next question, or `None` when the answer is settled.
    pub question: Option<Cell>,
    /// More than one top class survives and no question separates them.
    pub tie: bool,
}

impl Report {
    /// The next question's key, which is what the text format and the UI use.
    pub fn question_key(&self) -> Option<String> {
        self.question.and_then(Cell::key)
    }
}

/// `class -> key -> value` from a class sheet.
pub fn parse_sheet(text: &str) -> Result<Sheet, ParseError> {
    check_version(text, "torille-class-examples", 1)?;
    let mut sheet = Sheet::new();
    for (line, fields) in data_rows(text, 3)? {
        let (class, key, value) = (fields[0], fields[1], fields[2]);
        if class.is_empty() || key.is_empty() {
            return Err(ParseError::new(line, "empty class or key"));
        }
        sheet
            .entry(class.to_string())
            .or_default()
            .insert(key.to_string(), value.to_string());
    }
    if sheet.is_empty() {
        return Err(ParseError::new(1, "the sheet has no class sections"));
    }
    Ok(sheet)
}

/// `cell -> form` from a session. An empty value means the question was
/// skipped; it is kept so the same question is not asked again.
pub fn parse_answers(text: &str) -> Result<Answers, ParseError> {
    check_version(text, "torille-wizard-answers", 1)?;
    let mut answers = Answers::new();
    for (line, fields) in data_rows(text, 2)? {
        let key = fields[0];
        if key.is_empty() {
            return Err(ParseError::new(line, "empty cell key"));
        }
        if Cell::parse(key).is_none() {
            return Err(ParseError::new(line, format!("unknown cell key {key:?}")));
        }
        answers.insert(key.to_string(), fields[1].to_string());
    }
    Ok(answers)
}

/// `word -> entry` from a saved list. The class expression is canonicalised,
/// so `5*A` stays `5*A` and `(5)` becomes `5`.
///
/// Version 1 has three columns; version 2 adds a `key=value;...` flag column.
/// Version 1 is still accepted, so an exported list keeps working.
pub fn parse_user_words(text: &str) -> Result<UserWords, ParseError> {
    let version = version_of(text, "torille-user-words", 2)?;
    let columns = if version >= 2 { 4 } else { 3 };
    let mut words = UserWords::new();
    for (line, fields) in data_rows(text, columns)? {
        let (word, class, pos) = (fields[0], fields[1], fields[2]);
        if word.is_empty() || class.is_empty() || pos.is_empty() {
            return Err(ParseError::new(line, "empty word, class or part of speech"));
        }
        let number = if columns == 4 {
            parse_user_word_flags(fields[3], line)?
        } else {
            None
        };
        let inflections = kotus::parse_inflections(class);
        if inflections.is_empty() {
            return Err(ParseError::new(
                line,
                format!("{class:?} is not a Kotus class expression"),
            ));
        }
        let pos_name = pos.trim().to_lowercase();
        let Some(pos) = pos_from_name(&pos_name) else {
            return Err(ParseError::new(
                line,
                format!("{pos_name:?} is not a part of speech"),
            ));
        };
        let canonical = inflections
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        words.insert(
            word.to_lowercase(),
            UserWord {
                word: word.to_string(),
                class: canonical,
                pos,
                pos_name,
                number,
            },
        );
    }
    Ok(words)
}

/// Reads the version 2 flag column.
///
/// The column is `key=value` pairs separated by `;`, with `-` meaning no
/// flags. Unknown keys and values are errors: a typo in a restriction must not
/// silently become "accept everything".
fn parse_user_word_flags(flags: &str, line: usize) -> Result<Option<Number>, ParseError> {
    let flags = flags.trim();
    if flags.is_empty() || flags == "-" {
        return Ok(None);
    }
    let mut number = None;
    for flag in flags.split(';') {
        let flag = flag.trim();
        if flag.is_empty() {
            continue;
        }
        let Some((key, value)) = flag.split_once('=') else {
            return Err(ParseError::new(
                line,
                format!("{flag:?} is not a key=value flag"),
            ));
        };
        match key.trim() {
            "number" => {
                let value = value.trim();
                number = Some(match value {
                    "sg" => Number::Singular,
                    "pl" => Number::Plural,
                    other => {
                        return Err(ParseError::new(
                            line,
                            format!("{other:?} is not number=sg or number=pl"),
                        ));
                    }
                });
            }
            other => {
                return Err(ParseError::new(line, format!("unknown flag {other:?}")));
            }
        }
    }
    Ok(number)
}

fn number_flag(number: Option<Number>) -> &'static str {
    match number {
        Some(Number::Singular) => "number=sg",
        Some(Number::Plural) => "number=pl",
        _ => "-",
    }
}

/// Serialises a saved-word map back to the text format.
///
/// The output is sorted by the lower-cased word, so an export diffs cleanly
/// and a repeated add/remove round-trips to the same bytes.
pub fn format_user_words(words: &UserWords) -> String {
    let mut entries: Vec<&UserWord> = words.values().collect();
    entries.sort_by(|a, b| {
        a.word
            .to_lowercase()
            .cmp(&b.word.to_lowercase())
            .then(a.word.cmp(&b.word))
    });
    let mut out = String::from("# torille-user-words v2\n");
    for entry in entries {
        writeln!(
            out,
            "{}\t{}\t{}\t{}",
            entry.word,
            entry.class,
            entry.pos_name,
            number_flag(entry.number)
        )
        .unwrap();
    }
    out
}

/// Adds or replaces one word and returns the new text.
///
/// `text` may be empty; a non-empty blob must parse, so a corrupt store is
/// reported instead of being silently overwritten.
pub fn add_user_word(text: &str, word: &str, class: &str, pos: &str) -> Result<String, ParseError> {
    add_user_word_with(text, word, class, pos, None)
}

/// [`add_user_word`] with an explicit number restriction.
pub fn add_user_word_with(
    text: &str,
    word: &str,
    class: &str,
    pos: &str,
    number: Option<Number>,
) -> Result<String, ParseError> {
    let mut words = if text.trim().is_empty() {
        UserWords::new()
    } else {
        parse_user_words(text)?
    };
    let line = format!(
        "# torille-user-words v2\n{word}\t{class}\t{pos}\t{}\n",
        number_flag(number)
    );
    for (key, value) in parse_user_words(&line)? {
        check_citation_number(&value)?;
        words.insert(key, value);
    }
    Ok(format_user_words(&words))
}

/// A number restriction must match the word the entry is cited by.
///
/// `kiikarit` with `number=pl` is a plural-only entry; `kiikari` with
/// `number=pl` would accept the singular citation and the plural cells, which
/// is neither reading. The UI says the same thing before calling this, and the
/// check keeps a hand-edited or imported list from breaking the invariant.
fn check_citation_number(entry: &UserWord) -> Result<(), ParseError> {
    let Some(want) = entry.number else {
        return Ok(());
    };
    let inflections = kotus::parse_inflections(&entry.class);
    if inflections
        .iter()
        .any(|inflection| inflection.class == INDECLINABLE_CLASS)
    {
        return Ok(());
    }
    let cat = Catalogues::embedded();
    let index = LexiconIndex::new(kotus::lemmas());
    let lemma = entry.word.trim().to_lowercase();
    for inflection in inflections {
        for form in inflect_in(&lemma, inflection, cat, Some(&index), entry.pos, false) {
            if form.word.eq_ignore_ascii_case(&entry.word) && form.union().number == Some(want) {
                return Ok(());
            }
        }
    }
    let (flag, label) = match want {
        Number::Plural => ("number=pl", "plural"),
        Number::Singular => ("number=sg", "singular"),
        Number::Collective => ("number=collective", "collective"),
    };
    Err(ParseError::new(
        2,
        format!("{flag} needs a {label} citation form; type the nominative {label}"),
    ))
}

/// Removes one word, case-insensitively, and returns the new text.
pub fn remove_user_word(text: &str, word: &str) -> Result<String, ParseError> {
    let mut words = if text.trim().is_empty() {
        UserWords::new()
    } else {
        parse_user_words(text)?
    };
    words.remove(&word.trim().to_lowercase());
    Ok(format_user_words(&words))
}

/// Every lower-cased surface one saved entry licenses.
///
/// The entry's citation form is always accepted; the generated forms are
/// filtered by the entry's number when it has one. Only a form that carries
/// the *other* number is dropped: a numberless reading stays, because the
/// restriction is about number and the form does not assert one. A plural-only
/// entry therefore keeps `kiikariihin` and drops `kiikaria`, and a
/// singular-only entry keeps `maidolla` and drops `maidoilla`. Class 99 stays
/// lemma-only: its loan machinery must not invent forms.
pub fn user_word_forms(
    entry: &UserWord,
    cat: &Catalogues,
    index: &LexiconIndex<'_>,
) -> Vec<String> {
    use std::collections::BTreeSet;

    let lemma = entry.word.trim().to_lowercase();
    let mut forms: BTreeSet<String> = BTreeSet::new();
    forms.insert(lemma.clone());
    for inflection in kotus::parse_inflections(&entry.class) {
        if inflection.class == INDECLINABLE_CLASS {
            continue;
        }
        for form in inflect_in(&lemma, inflection, cat, Some(index), entry.pos, false) {
            if entry.number.is_some_and(|want| {
                form.union()
                    .number
                    .is_some_and(|have| have != want)
            }) {
                continue;
            }
            forms.insert(form.word.to_lowercase());
        }
    }
    forms.into_iter().collect()
}

fn data_rows(text: &str, columns: usize) -> Result<Vec<(usize, Vec<&str>)>, ParseError> {
    let mut rows = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        let line = line.trim_start_matches('\u{feff}');
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').map(str::trim).collect();
        if fields.len() != columns {
            return Err(ParseError::new(
                index + 1,
                format!(
                    "expected {columns} tab-separated fields, found {}",
                    fields.len()
                ),
            ));
        }
        if fields
            .iter()
            .any(|field| field.chars().any(|c| c.is_control()))
        {
            return Err(ParseError::new(index + 1, "control character in a field"));
        }
        rows.push((index + 1, fields));
    }
    Ok(rows)
}

fn check_version(text: &str, tag: &str, supported: u32) -> Result<(), ParseError> {
    version_of(text, tag, supported).map(|_| ())
}

/// The declared format version, defaulting to 1 when the header is absent.
///
/// A version newer than `supported` is refused rather than misread, exactly as
/// [`check_version`] did.
fn version_of(text: &str, tag: &str, supported: u32) -> Result<u32, ParseError> {
    let mut version = 1;
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim_start_matches('\u{feff}').trim();
        if line.is_empty() {
            continue;
        }
        if !line.starts_with('#') {
            return Ok(version);
        }
        let body = line.trim_start_matches('#').trim();
        if let Some(rest) = body.strip_prefix(tag)
            && let Some(rest) = rest.trim().strip_prefix('v')
        {
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            if let Ok(parsed) = digits.parse::<u32>() {
                if parsed > supported {
                    return Err(ParseError::new(
                        index + 1,
                        format!("{tag} version {parsed} is newer than supported v{supported}"),
                    ));
                }
                version = parsed;
            }
        }
        return Ok(version);
    }
    Ok(version)
}

/// Inflects every class model and returns the text of a class sheet.
///
/// The sheet is the engine's own rendering of the catalogue's model word for
/// each nominal and verb class, plus the base-class lemma count the browser
/// uses to order the exemplars. It is generated, committed and checked by
/// [`check_sheet`]; do not hand-edit it. The catalogue's correctness is
/// `tests/catalogue.rs`'s job; this sheet is what the wizard will actually
/// generate, so the UI and the inference cannot disagree.
pub fn render_sheet(cat: &Catalogues) -> String {
    let counts = &observed().base_counts;
    let index = LexiconIndex::new(kotus::lemmas());
    let mut out = String::new();
    out.push_str("# torille-class-examples v1\n");
    out.push_str("# class\tkey\tvalue\n");
    for (class, paradigm) in cat.iter() {
        if *class == 50 || *class == 51 {
            continue;
        }
        let model = paradigm.model();
        if model.is_empty() {
            continue;
        }
        write_sheet_section(
            &mut out,
            *class,
            model,
            is_verb_class(*class),
            counts.get(class).copied().unwrap_or(0),
            cat,
            &index,
        );
    }
    // Classes 76-78 are lexicon-only verb classes; they have no catalogue
    // model, so the representative is a short common lemma.
    for (class, model) in [(76u16, "tietää"), (77, "vipajaa"), (78, "raksaa")] {
        write_sheet_section(
            &mut out,
            class,
            model,
            true,
            counts.get(&class).copied().unwrap_or(0),
            cat,
            &index,
        );
    }
    out
}

fn write_sheet_section(
    out: &mut String,
    class: ClassId,
    model: &str,
    verb: bool,
    lemmas: usize,
    cat: &Catalogues,
    index: &LexiconIndex<'_>,
) {
    let (pos_name, pos) = if verb {
        ("verb", PartOfSpeech::Verb)
    } else {
        ("nominal", PartOfSpeech::Noun)
    };
    writeln!(out, "{class}\tmodel\t{model}").unwrap();
    writeln!(out, "{class}\tpos\t{pos_name}").unwrap();
    writeln!(out, "{class}\tlemmas\t{lemmas}").unwrap();
    // The catalogue model's own recorded gradation matters for a verb such as
    // `tietää` (76*F), whose finite forms weaken `t -> d`.
    let inflection = kotus::sanalista()
        .by_word(model)
        .flat_map(|row| row.inflections().collect::<Vec<_>>())
        .find(|inflection| inflection.class == class)
        .unwrap_or(Inflection {
            class,
            gradation: None,
        });
    let forms = inflect_in(model, inflection, cat, Some(index), pos, false);
    let keys: &[&str] = if verb { &VERB_KEYS } else { &CORE_KEYS };
    for key in keys {
        let Some(cell) = Cell::parse(key) else {
            continue;
        };
        let form = forms
            .iter()
            .filter(|form| form.readings.iter().any(|tags| cell.claims(tags)))
            .min_by_key(|form| (!form.plain(), form.word.as_str()));
        if let Some(form) = form {
            writeln!(out, "{class}\t{key}\t{}", form.word).unwrap();
        }
    }
}

/// A row of the sheet that the engine does not reproduce.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SheetMismatch {
    pub class: String,
    pub key: String,
    pub expected: String,
    pub actual: String,
}

/// Checks the committed sheet against a fresh rendering of the engine.
///
/// This is the drift gate: a class engine change that moves a model form, or a
/// lexicon change that moves a class count, turns this red until the sheet is
/// regenerated. It compares the parsed rows, not the byte order, so a
/// reordered file passes as long as every row says the same thing.
pub fn check_sheet(cat: &Catalogues) -> Result<Vec<SheetMismatch>, ParseError> {
    check_sheet_text(SHEET, cat)
}

/// [`check_sheet`] over arbitrary text, for tests and `--check-sheet`.
pub fn check_sheet_text(text: &str, cat: &Catalogues) -> Result<Vec<SheetMismatch>, ParseError> {
    let actual = parse_sheet(text)?;
    let expected = parse_sheet(&render_sheet(cat))?;
    let mut mismatches = Vec::new();
    for (class, section) in &expected {
        let Some(found) = actual.get(class) else {
            mismatches.push(SheetMismatch {
                class: class.clone(),
                key: "<section>".to_string(),
                expected: "present".to_string(),
                actual: "missing".to_string(),
            });
            continue;
        };
        for (key, value) in section {
            if key == "lemmas" {
                // The count only orders the exemplar list. A sanalista addition
                // must not force a sheet regeneration; the forms are the gate.
                continue;
            }
            match found.get(key) {
                None => mismatches.push(SheetMismatch {
                    class: class.clone(),
                    key: key.clone(),
                    expected: value.clone(),
                    actual: "<missing>".to_string(),
                }),
                Some(actual) if actual != value => mismatches.push(SheetMismatch {
                    class: class.clone(),
                    key: key.clone(),
                    expected: value.clone(),
                    actual: actual.clone(),
                }),
                Some(_) => {}
            }
        }
        for key in found.keys() {
            if key == "lemmas" {
                continue;
            }
            if !section.contains_key(key) {
                mismatches.push(SheetMismatch {
                    class: class.clone(),
                    key: key.clone(),
                    expected: "<unexpected>".to_string(),
                    actual: found[key].clone(),
                });
            }
        }
    }
    for (class, section) in &actual {
        if !expected.contains_key(class) {
            mismatches.push(SheetMismatch {
                class: class.clone(),
                key: "<section>".to_string(),
                expected: "absent".to_string(),
                actual: format!("{} rows", section.len()),
            });
        }
    }
    mismatches.sort_by(|a, b| {
        class_number(&a.class)
            .cmp(&class_number(&b.class))
            .then(a.class.cmp(&b.class))
            .then(a.key.cmp(&b.key))
    });
    Ok(mismatches)
}

fn class_number(class: &str) -> u16 {
    class
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .unwrap_or(u16::MAX)
}

/// Infers the class from `answers`, using the sanalista's own class as a
/// tie-break when the word is known.
pub fn infer(
    lemma: &str,
    pos: PartOfSpeech,
    answers: &Answers,
    cat: &Catalogues,
) -> Result<Report, WizardError> {
    infer_with(lemma, pos, answers, cat, true)
}

/// The explicit "does not inflect" result.
///
/// Returns a one-candidate report with class [`INDECLINABLE_CLASS`] and no
/// question. The lemma may still be in the sanalista; the report lists its
/// recorded classes for display, but the user's choice is the candidate.
pub fn indeclinable(lemma: &str, pos: PartOfSpeech) -> Result<Report, WizardError> {
    let surface = lemma.trim();
    if surface.is_empty() {
        return Err(WizardError::EmptyWord);
    }
    if !is_supported(pos) {
        return Err(WizardError::UnsupportedPartOfSpeech(pos));
    }
    let lower = surface.to_lowercase();
    let mut recorded: Vec<Inflection> = Vec::new();
    for row in kotus::sanalista().by_word(&lower) {
        recorded.extend(row.inflections());
    }
    recorded.sort_by_key(|inflection| (inflection.class, inflection.gradation));
    recorded.dedup();
    Ok(Report {
        lemma: surface.to_string(),
        pos,
        recorded,
        ranked: vec![Candidate {
            class: Inflection {
                class: INDECLINABLE_CLASS,
                gradation: None,
            },
            hits: 0,
            misses: 0,
            unknown: 0,
            recorded: false,
            tail_prior: 0,
            prior: 0,
        }],
        outcomes: Vec::new(),
        question: None,
        tie: false,
    })
}

/// [`infer`] with the recorded-class tie-break optionally disabled.
///
/// The evaluation samples words that are *in* the sanalista, so ranking the
/// recorded expression first would make the measurement circular. It passes
/// `false`; the UI passes `true` and still sees the recorded list in
/// [`Report::recorded`] for display.
pub fn infer_with(
    lemma: &str,
    pos: PartOfSpeech,
    answers: &Answers,
    cat: &Catalogues,
    use_recorded: bool,
) -> Result<Report, WizardError> {
    infer_with_number(lemma, pos, answers, cat, use_recorded, None)
}

/// [`infer_with`] restricted to one number.
///
/// `number` changes the question pool, not the candidate scoring: a
/// plural-only word is asked `gen_pl`, `par_pl` and `ill_pl`, and its saved
/// entry filters the singular cells out. `None` asks the full nominal pool.
pub fn infer_with_number(
    lemma: &str,
    pos: PartOfSpeech,
    answers: &Answers,
    cat: &Catalogues,
    use_recorded: bool,
    number: Option<Number>,
) -> Result<Report, WizardError> {
    let surface = lemma.trim();
    if surface.is_empty() {
        return Err(WizardError::EmptyWord);
    }
    if !is_supported(pos) {
        return Err(WizardError::UnsupportedPartOfSpeech(pos));
    }
    let pool = question_pool(pos, number);
    // The engine's lexicon and class engines work on the lower-case headword.
    // The surface is kept for display; the UI restores capitalization when it
    // shows the forms.
    let lemma = surface.to_lowercase();

    let mut recorded: Vec<Inflection> = Vec::new();
    for row in kotus::sanalista().by_word(&lemma) {
        recorded.extend(row.inflections());
    }
    recorded.sort_by_key(|inflection| (inflection.class, inflection.gradation));
    recorded.dedup();

    let mut asked: Vec<(String, Cell)> = answers
        .keys()
        .filter_map(|key| Cell::parse(key).map(|cell| (key.clone(), cell)))
        .collect();
    asked.sort_by(|a, b| {
        question_rank(&a.0, pool)
            .cmp(&question_rank(&b.0, pool))
            .then(a.0.cmp(&b.0))
    });

    let mut cells: Vec<Cell> = pool.iter().filter_map(|key| Cell::parse(key)).collect();
    for (_, cell) in &asked {
        if !cells.contains(cell) {
            cells.push(*cell);
        }
    }

    // Normalise once; the answers are compared to engine output, which is
    // already lower case.
    let answered: Vec<(&str, Cell, String)> = asked
        .iter()
        .filter_map(|(key, cell)| {
            answers
                .get(key)
                .map(|form| (key.as_str(), *cell, form.trim().to_lowercase()))
        })
        .collect();

    let lexicon = kotus::lemmas();
    let index = LexiconIndex::new(lexicon);
    let mut scored: Vec<Scored> = Vec::new();
    for (inflection, prior, tail_prior) in candidates_for(&lemma, pos, cat) {
        let forms = inflect_in(&lemma, inflection, cat, Some(&index), pos, false);
        let mut predictions: HashMap<Cell, HashSet<String>> = HashMap::new();
        for form in &forms {
            let word = form.word.to_lowercase();
            for cell in &cells {
                if form.readings.iter().any(|tags| cell.claims(tags)) {
                    predictions.entry(*cell).or_default().insert(word.clone());
                }
            }
        }
        let mut hits = 0;
        let mut misses = 0;
        let mut unknown = 0;
        for (_, cell, answer) in &answered {
            if answer.is_empty() {
                continue;
            }
            match predictions.get(cell) {
                None => unknown += 1,
                Some(forms) if forms.contains(answer) => hits += 1,
                Some(_) => misses += 1,
            }
        }
        scored.push(Scored {
            candidate: Candidate {
                class: inflection,
                hits,
                misses,
                unknown,
                recorded: use_recorded && recorded.contains(&inflection),
                tail_prior,
                prior,
            },
            predictions,
        });
    }

    scored.sort_by(|a, b| {
        b.candidate
            .hits
            .cmp(&a.candidate.hits)
            .then(a.candidate.misses.cmp(&b.candidate.misses))
            .then(b.candidate.recorded.cmp(&a.candidate.recorded))
            .then(b.candidate.tail_prior.cmp(&a.candidate.tail_prior))
            .then(b.candidate.prior.cmp(&a.candidate.prior))
            .then(a.candidate.class.class.cmp(&b.candidate.class.class))
            .then(
                a.candidate
                    .class
                    .gradation
                    .cmp(&b.candidate.class.gradation),
            )
    });

    let outcomes = scored.first().map_or_else(Vec::new, |best| {
        answered
            .iter()
            .map(|(key, cell, answer)| {
                let status = if answer.is_empty() {
                    AnswerStatus::Skipped
                } else {
                    match best.predictions.get(cell) {
                        None => AnswerStatus::Unknown,
                        Some(forms) if forms.contains(answer) => AnswerStatus::Hit,
                        Some(_) => AnswerStatus::Miss,
                    }
                };
                AnswerOutcome {
                    key: (*key).to_string(),
                    form: answers.get(*key).cloned().unwrap_or_default(),
                    status,
                }
            })
            .collect()
    });

    let question = next_question(&scored, &asked, pool);
    let tie = question.is_none() && top_tied(&scored);
    Ok(Report {
        lemma: surface.to_string(),
        pos,
        recorded,
        ranked: scored.iter().map(|scored| scored.candidate).collect(),
        outcomes,
        question,
        tie,
    })
}

fn is_supported(pos: PartOfSpeech) -> bool {
    matches!(
        pos,
        PartOfSpeech::Noun
            | PartOfSpeech::Adjective
            | PartOfSpeech::Pronoun
            | PartOfSpeech::Numeral
            | PartOfSpeech::Ordinal
            | PartOfSpeech::Verb
            | PartOfSpeech::NegativeVerb
            | PartOfSpeech::Unknown
    )
}

fn wants_verb(pos: PartOfSpeech) -> bool {
    matches!(pos, PartOfSpeech::Verb | PartOfSpeech::NegativeVerb)
}

fn question_pool(pos: PartOfSpeech, number: Option<Number>) -> &'static [&'static str] {
    if wants_verb(pos) {
        return &VERB_QUESTION_POOL;
    }
    match number {
        Some(Number::Plural) => &PLURAL_QUESTION_POOL,
        Some(Number::Singular) => &SINGULAR_QUESTION_POOL,
        _ => &QUESTION_POOL,
    }
}

struct Scored {
    candidate: Candidate,
    predictions: HashMap<Cell, HashSet<String>>,
}

fn question_rank(key: &str, pool: &[&str]) -> usize {
    pool.iter()
        .position(|candidate| *candidate == key)
        .unwrap_or(usize::MAX)
}

/// The classes that are still in the running: the best hit count, and the
/// fewest misses among the classes with it.
fn top_hits(scored: &[Scored]) -> (usize, usize) {
    let hits = scored.first().map_or(0, |scored| scored.candidate.hits);
    let misses = scored
        .iter()
        .filter(|scored| scored.candidate.hits == hits)
        .map(|scored| scored.candidate.misses)
        .min()
        .unwrap_or(0);
    (hits, misses)
}

/// Whether the best evidence cannot separate the top class from a rival.
///
/// Hits and misses are the answers; recorded, the tail prior and the
/// whole-lexicon prior are the tie-breaks. The class number and gradation
/// order are deliberately not evidence: two expressions that agree on
/// everything else are a real tie, not a win for the smaller number.
fn top_tied(scored: &[Scored]) -> bool {
    let Some(first) = scored.first() else {
        return false;
    };
    scored.iter().skip(1).any(|other| {
        other.candidate.hits == first.candidate.hits
            && other.candidate.misses == first.candidate.misses
            && other.candidate.recorded == first.candidate.recorded
            && other.candidate.tail_prior == first.candidate.tail_prior
            && other.candidate.prior == first.candidate.prior
    })
}

fn next_question(scored: &[Scored], asked: &[(String, Cell)], pool: &[&str]) -> Option<Cell> {
    let (hits, misses) = top_hits(scored);
    let live: Vec<&Scored> = scored
        .iter()
        .filter(|scored| scored.candidate.hits == hits && scored.candidate.misses == misses)
        .collect();
    if live.len() <= 1 || asked.len() >= 5 {
        return None;
    }
    if asked.is_empty() {
        return Cell::parse(pool.first()?);
    }
    let mut best: Option<(usize, Cell)> = None;
    for key in pool {
        if asked.iter().any(|(asked_key, _)| asked_key == key) {
            continue;
        }
        let Some(cell) = Cell::parse(key) else {
            continue;
        };
        let mut groups: HashMap<Vec<&str>, usize> = HashMap::new();
        let mut empty = 0;
        for candidate in &live {
            let mut forms: Vec<&str> = candidate
                .predictions
                .get(&cell)
                .map(|forms| forms.iter().map(String::as_str).collect())
                .unwrap_or_default();
            if forms.is_empty() {
                empty += 1;
            }
            forms.sort_unstable();
            *groups.entry(forms).or_default() += 1;
        }
        let largest = groups.values().copied().max().unwrap_or(0);
        let gain = live.len().saturating_sub(largest);
        if gain == 0 || empty * 2 > live.len() {
            continue;
        }
        if best.is_none_or(|(best_gain, _)| gain > best_gain) {
            best = Some((gain, cell));
        }
    }
    best.map(|(_, cell)| cell)
}

/// How many trailing characters the tail prior keys on.
const TAIL_LEN: usize = 3;

/// The last `TAIL_LEN` characters, counted by `char` rather than bytes so a
/// multibyte letter is never split.
fn tail_of(word: &str) -> String {
    let count = word.chars().count();
    if count <= TAIL_LEN {
        return word.to_string();
    }
    word.chars().skip(count - TAIL_LEN).collect()
}

struct Observed {
    inflections: Vec<(Inflection, usize)>,
    base_counts: HashMap<ClassId, usize>,
    /// `(is_verb, last three chars) -> expression -> how many lemmas with that
    /// tail carry it`. The tie-break that prefers the declension the word's
    /// own ending usually takes. Kept per part-of-speech family: a verb ending
    /// says nothing about a noun's class.
    tail_counts: HashMap<(bool, String), HashMap<Inflection, usize>>,
}

fn observed() -> &'static Observed {
    static OBSERVED: OnceLock<Observed> = OnceLock::new();
    OBSERVED.get_or_init(|| {
        let mut counts: HashMap<Inflection, usize> = HashMap::new();
        let mut base_counts: HashMap<ClassId, usize> = HashMap::new();
        let mut tail_counts: HashMap<(bool, String), HashMap<Inflection, usize>> = HashMap::new();
        for lemma in kotus::lemmas() {
            let tail = tail_of(lemma.word);
            let mut classes: HashSet<ClassId> = HashSet::new();
            for inflection in &lemma.inflections {
                if inflection.class == 50
                    || inflection.class == 51
                    || !kotus::carries_a_model(inflection.class)
                {
                    continue;
                }
                *counts.entry(*inflection).or_default() += 1;
                *tail_counts
                    .entry((is_verb_class(inflection.class), tail.clone()))
                    .or_default()
                    .entry(*inflection)
                    .or_default() += 1;
                if classes.insert(inflection.class) {
                    *base_counts.entry(inflection.class).or_default() += 1;
                }
            }
        }
        let mut inflections: Vec<(Inflection, usize)> = counts.into_iter().collect();
        inflections.sort_by_key(|(inflection, _)| (inflection.class, inflection.gradation));
        Observed {
            inflections,
            base_counts,
            tail_counts,
        }
    })
}

/// The candidate readings for `lemma`: every nominal class in the catalogue,
/// plus the gradation letters whose strong pair actually occurs in the word.
/// The lexicon's own readings are added too, so a gradation the site check
/// cannot see is still tested. The third field is the tail prior: how many
/// lemmas ending in the same three characters carry this exact expression.
fn candidates_for(
    lemma: &str,
    pos: PartOfSpeech,
    cat: &Catalogues,
) -> Vec<(Inflection, usize, usize)> {
    let observed = observed();
    let tail = tail_of(lemma);
    let verb = wants_verb(pos);
    let mut candidates: HashSet<Inflection> = HashSet::new();
    for (class, _) in cat.iter() {
        if *class == 50 || *class == 51 || is_verb_class(*class) != verb {
            continue;
        }
        candidates.insert(Inflection {
            class: *class,
            gradation: None,
        });
        for letter in 'A'..='M' {
            if let Some(gradation) = Gradation::from_letter(letter)
                && gradation.site(lemma).is_some()
            {
                candidates.insert(Inflection {
                    class: *class,
                    gradation: Some(gradation),
                });
            }
        }
    }
    for (inflection, _) in &observed.inflections {
        if is_verb_class(inflection.class) == verb {
            candidates.insert(*inflection);
        }
    }
    let mut candidates: Vec<(Inflection, usize, usize)> = candidates
        .into_iter()
        .map(|inflection| {
            let prior = observed
                .base_counts
                .get(&inflection.class)
                .copied()
                .unwrap_or(0);
            let tail_prior = observed
                .tail_counts
                .get(&(verb, tail.clone()))
                .and_then(|counts| counts.get(&inflection))
                .copied()
                .unwrap_or(0);
            (inflection, prior, tail_prior)
        })
        .collect();
    candidates.sort_by_key(|(inflection, _, _)| (inflection.class, inflection.gradation));
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalogue() -> &'static Catalogues {
        Catalogues::embedded()
    }

    fn gradation(letter: char) -> Option<Gradation> {
        Gradation::from_letter(letter)
    }

    #[test]
    fn the_sheet_is_current() {
        let mismatches = check_sheet(catalogue()).expect("the committed sheet parses");
        assert!(
            mismatches.is_empty(),
            "the class sheet is stale: {mismatches:#?}"
        );
    }

    #[test]
    fn the_sheet_has_the_nominal_classes() {
        let sheet = parse_sheet(SHEET).unwrap();
        for class in ["1", "5", "9", "12", "49"] {
            let section = sheet
                .get(class)
                .unwrap_or_else(|| panic!("class {class} is missing"));
            assert_eq!(section.get("pos").map(String::as_str), Some("nominal"));
            assert!(section.contains_key("model"));
            for key in CORE_KEYS {
                assert!(section.contains_key(key), "class {class} is missing {key}");
            }
        }
        for class in ["52", "76"] {
            let section = sheet
                .get(class)
                .unwrap_or_else(|| panic!("verb class {class} is missing"));
            assert_eq!(section.get("pos").map(String::as_str), Some("verb"));
            for key in VERB_KEYS {
                assert!(section.contains_key(key), "class {class} is missing {key}");
            }
        }
        // Classes 77/78 are the defective sound-verb classes: only the
        // third-person present and conditional are in the paradigm.
        for class in ["77", "78"] {
            let section = sheet
                .get(class)
                .unwrap_or_else(|| panic!("verb class {class} is missing"));
            assert_eq!(section.get("pos").map(String::as_str), Some("verb"));
            for key in ["pres_3sg", "cond_3sg"] {
                assert!(section.contains_key(key), "class {class} is missing {key}");
            }
        }
        for class in ["50", "51"] {
            assert!(
                !sheet.contains_key(class),
                "class {class} should not be here"
            );
        }
    }

    #[test]
    fn the_sheet_names_every_model_and_form() {
        let sheet = parse_sheet(SHEET).unwrap();
        for (class, section) in &sheet {
            let model = section.get("model").unwrap();
            assert!(!model.is_empty(), "class {class} has an empty model");
            let keys: &[&str] = if section.get("pos").map(String::as_str) == Some("verb") {
                &VERB_KEYS
            } else {
                &CORE_KEYS
            };
            for key in keys {
                if let Some(value) = section.get(*key) {
                    assert!(!value.is_empty(), "class {class} has an empty {key}");
                }
            }
            assert!(
                keys.iter().any(|key| section.contains_key(*key)),
                "class {class} has no form rows"
            );
        }
    }

    #[test]
    fn an_answer_must_claim_the_case() {
        let cell = Cell::parse("gen_sg").unwrap();
        let case_less = Tags {
            case: None,
            number: Some(Number::Singular),
            ..Tags::UNKNOWN
        };
        assert!(!cell.claims(&case_less));
        let genitive = Tags {
            case: Some(Case::Genitive),
            number: Some(Number::Singular),
            ..Tags::UNKNOWN
        };
        assert!(cell.claims(&genitive));
    }

    #[test]
    fn a_cell_key_round_trips() {
        for key in CORE_KEYS {
            let cell = Cell::parse(key).unwrap();
            assert_eq!(cell.key().as_deref(), Some(key));
        }
        for key in QUESTION_POOL {
            let cell = Cell::parse(key).unwrap();
            assert_eq!(cell.key().as_deref(), Some(key));
        }
        for key in VERB_KEYS {
            let cell = Cell::parse(key).unwrap();
            assert_eq!(cell.key().as_deref(), Some(key));
        }
    }

    #[test]
    fn the_answers_keep_a_skip() {
        let answers =
            parse_answers("# torille-wizard-answers v1\ngen_sg\tmökin\npar_sg\t\nnom_pl\tmökit\n")
                .unwrap();
        assert_eq!(answers.get("gen_sg").map(String::as_str), Some("mökin"));
        assert_eq!(answers.get("par_sg").map(String::as_str), Some(""));
        assert_eq!(answers.get("nom_pl").map(String::as_str), Some("mökit"));
    }

    #[test]
    fn the_user_words_parse_and_canonicalise() {
        let words =
            parse_user_words("# torille-user-words v1\nmökki\t5*A\tsubstantiivi\nHelsinki\t5\t\n");
        assert!(words.is_err(), "an empty part of speech is not a word");

        let words = parse_user_words(
            "# torille-user-words v1\nmökki\t5*A\tsubstantiivi\nHelsinki\t5\terisnimi\n",
        )
        .unwrap();
        assert_eq!(words.len(), 2);
        let word = &words["mökki"];
        assert_eq!(word.word, "mökki");
        assert_eq!(word.class, "5*A");
        assert_eq!(word.pos, PartOfSpeech::Noun);
        assert_eq!(word.pos_name, "substantiivi");
        assert_eq!(words["helsinki"].word, "Helsinki");
        assert_eq!(words["helsinki"].pos, PartOfSpeech::Noun);
        assert_eq!(words["helsinki"].pos_name, "erisnimi");
        assert!(
            format_user_words(&words).contains("Helsinki\t5\terisnimi\t-"),
            "the proper-noun label survives the round trip"
        );
    }

    #[test]
    fn the_part_of_speech_names_include_the_special_nominals() {
        assert_eq!(pos_from_name("numeraali"), Some(PartOfSpeech::Numeral));
        assert_eq!(pos_from_name("järjestysluku"), Some(PartOfSpeech::Ordinal));
        assert_eq!(pos_from_name("pronomini"), Some(PartOfSpeech::Pronoun));
        assert_eq!(pos_from_name("erisnimi"), Some(PartOfSpeech::Noun));
        assert_eq!(pos_from_name("proper noun"), Some(PartOfSpeech::Noun));
    }

    #[test]
    fn the_user_words_add_remove_and_sort() {
        let text = "# torille-user-words v1\nmökki\t5*A\tsubstantiivi\n";
        let text = add_user_word(text, "Helsinki", "5", "substantiivi").unwrap();
        let words = parse_user_words(&text).unwrap();
        assert_eq!(words.len(), 2);
        assert_eq!(words["helsinki"].class, "5");
        // Sorted by the lower-cased headword: helsinki before mökki.
        let first = text
            .lines()
            .find(|line| !line.starts_with('#') && !line.is_empty())
            .unwrap();
        assert!(first.starts_with("Helsinki\t"));

        let text = remove_user_word(&text, "MÖKKI").unwrap();
        let words = parse_user_words(&text).unwrap();
        assert_eq!(words.len(), 1);
        assert!(!words.contains_key("mökki"));
    }

    #[test]
    fn the_user_words_edits_an_empty_store() {
        let text = add_user_word("", "mökki", "5*A", "substantiivi").unwrap();
        assert!(text.starts_with("# torille-user-words v2\n"));
        assert!(text.contains("mökki\t5*A\tsubstantiivi\t-"));
        let text = remove_user_word(&text, "mökki").unwrap();
        assert_eq!(text, "# torille-user-words v2\n");
    }

    #[test]
    fn a_version_two_list_reads_the_number_flag() {
        let words = parse_user_words(
            "# torille-user-words v2\n\
             kiikarit\t7\tsubstantiivi\tnumber=pl\n\
             maito\t1*F\tsubstantiivi\tnumber=sg\n\
             mökki\t5*A\tsubstantiivi\t-\n",
        )
        .unwrap();
        assert_eq!(words["kiikarit"].number, Some(Number::Plural));
        assert_eq!(words["maito"].number, Some(Number::Singular));
        assert_eq!(words["mökki"].number, None);
    }

    #[test]
    fn a_version_one_list_still_parses() {
        let words = parse_user_words("# torille-user-words v1\nmökki\t5*A\tsubstantiivi\n")
            .unwrap();
        assert_eq!(words["mökki"].number, None);
    }

    #[test]
    fn an_unknown_saved_flag_is_rejected() {
        let error =
            parse_user_words("# torille-user-words v2\nmökki\t5*A\tsubstantiivi\tbogus=1\n")
                .unwrap_err();
        assert!(error.message.contains("unknown flag"), "{error}");
    }

    #[test]
    fn a_saved_number_round_trips() {
        let text = add_user_word_with("", "kiikarit", "1", "substantiivi", Some(Number::Plural))
            .unwrap();
        assert!(text.contains("kiikarit\t1\tsubstantiivi\tnumber=pl"));
        let words = parse_user_words(&text).unwrap();
        assert_eq!(words["kiikarit"].number, Some(Number::Plural));
    }

    #[test]
    fn a_number_restriction_needs_a_matching_citation() {
        assert!(
            add_user_word_with("", "kiikarit", "1", "substantiivi", Some(Number::Plural)).is_ok()
        );
        let error =
            add_user_word_with("", "kiikari", "1", "substantiivi", Some(Number::Plural))
                .unwrap_err();
        assert!(error.message.contains("plural citation"), "{error}");
        let error =
            add_user_word_with("", "kiikarit", "1", "substantiivi", Some(Number::Singular))
                .unwrap_err();
        assert!(error.message.contains("singular citation"), "{error}");
    }

    #[test]
    fn a_plural_only_entry_asks_plural_questions() {
        let report = infer_with_number(
            "kiikarit",
            PartOfSpeech::Noun,
            &Answers::new(),
            catalogue(),
            false,
            Some(Number::Plural),
        )
        .unwrap();
        assert_eq!(report.question_key().as_deref(), Some("gen_pl"));

        let report = infer_with_number(
            "kiikarit",
            PartOfSpeech::Noun,
            &Answers::new(),
            catalogue(),
            false,
            Some(Number::Singular),
        )
        .unwrap();
        assert_eq!(report.question_key().as_deref(), Some("gen_sg"));
    }

    #[test]
    fn a_plural_only_entry_drops_the_singular_forms() {
        let entry = UserWord {
            word: "kiikarit".to_string(),
            class: "1".to_string(),
            pos: PartOfSpeech::Noun,
            pos_name: "substantiivi".to_string(),
            number: Some(Number::Plural),
        };
        let index = LexiconIndex::new(kotus::lemmas());
        let forms = user_word_forms(&entry, catalogue(), &index);
        assert!(forms.iter().any(|form| form == "kiikariihin"));
        assert!(!forms.iter().any(|form| form == "kiikaria"));
    }

    #[test]
    fn a_singular_only_entry_drops_the_plural_forms() {
        let entry = UserWord {
            word: "maito".to_string(),
            class: "1*F".to_string(),
            pos: PartOfSpeech::Noun,
            pos_name: "substantiivi".to_string(),
            number: Some(Number::Singular),
        };
        let index = LexiconIndex::new(kotus::lemmas());
        let forms = user_word_forms(&entry, catalogue(), &index);
        assert!(forms.iter().any(|form| form == "maidolla"));
        assert!(!forms.iter().any(|form| form == "maidoilla"));
    }

    #[test]
    fn a_parser_names_the_line() {
        let error = parse_answers("gen_sg\tmökin\nbogus_key\tx\n").unwrap_err();
        assert_eq!(error.line, 2);
        assert!(error.message.contains("bogus_key"));

        let error = parse_sheet("1\tmodel\n").unwrap_err();
        assert_eq!(error.line, 1);
        assert!(error.message.contains("3 tab-separated fields"));
    }

    #[test]
    fn a_newer_format_is_refused() {
        let error = parse_answers("# torille-wizard-answers v2\ngen_sg\tmökin\n").unwrap_err();
        assert!(error.message.contains("newer"));
    }

    #[test]
    fn crlf_and_comments_are_accepted() {
        let answers =
            parse_answers("# torille-wizard-answers v1\r\n\r\n# a comment\r\ngen_sg\tmökin\r\n")
                .unwrap();
        assert_eq!(answers.get("gen_sg").map(String::as_str), Some("mökin"));
    }

    #[test]
    fn the_first_question_is_the_genitive() {
        let report = infer("mökki", PartOfSpeech::Noun, &Answers::new(), catalogue()).unwrap();
        assert_eq!(report.question_key().as_deref(), Some("gen_sg"));
        assert!(report.ranked.len() > 1);
        assert!(report.outcomes.is_empty());
    }

    #[test]
    fn the_worked_example_finds_the_gradation() {
        let mut answers = Answers::new();
        answers.insert("gen_sg".to_string(), "mökin".to_string());
        answers.insert("par_sg".to_string(), "mökkiä".to_string());
        answers.insert("ill_sg".to_string(), "mökkiin".to_string());
        let report = infer("mökki", PartOfSpeech::Noun, &answers, catalogue()).unwrap();

        assert_eq!(
            report.ranked[0].class,
            Inflection {
                class: 5,
                gradation: gradation('A'),
            }
        );
        assert_eq!(report.ranked[0].hits, 3);
        assert_eq!(report.ranked[0].misses, 0);
        assert!(report.ranked[0].recorded);
        assert!(report.recorded.contains(&report.ranked[0].class));
        assert!(
            report
                .outcomes
                .iter()
                .all(|outcome| outcome.status == AnswerStatus::Hit)
        );
    }

    #[test]
    fn every_nominal_catalogue_class_is_a_candidate() {
        let classes: std::collections::HashSet<u16> =
            candidates_for("talo", PartOfSpeech::Noun, catalogue())
                .into_iter()
                .map(|(inflection, _, _)| inflection.class)
                .collect();
        for class in 1..=49 {
            assert!(classes.contains(&class), "class {class} is missing");
        }
        for class in [50, 51, 52, 76] {
            assert!(
                !classes.contains(&class),
                "class {class} should not be here"
            );
        }
    }

    #[test]
    fn every_nominal_class_inflects_an_odd_word_without_panicking() {
        // The wizard tries every class against every word, including shapes
        // the class was never meant for. A byte-sliced stem there is a panic
        // in the wasm, not a wrong form, so this is a hard robustness gate.
        let cat = catalogue();
        let lexicon = kotus::lemmas();
        let index = LexiconIndex::new(lexicon);
        for word in [
            "pöräkkö",
            "mökki",
            "ä",
            "ö",
            "talo",
            "xyzzy",
            "kissa",
            "aave",
            "kärpänen",
            "mies",
            "olut",
            "tuhannes",
            "auer",
            "vakaus",
        ] {
            for (class, _) in cat.iter() {
                if *class == 50 || *class == 51 || is_verb_class(*class) {
                    continue;
                }
                println!("{word} {class}");
                let _ = inflect_in(
                    word,
                    Inflection {
                        class: *class,
                        gradation: None,
                    },
                    cat,
                    Some(&index),
                    PartOfSpeech::Noun,
                    false,
                );
            }
        }
    }

    #[test]
    fn the_wizard_handles_a_lexicon_sample_without_panicking() {
        // Every class is tried against every word; the lexicon sample keeps
        // the byte-sliced stem assumptions from turning into a wasm panic.
        let cat = catalogue();
        let lemmas = kotus::lemmas();
        let step = (lemmas.len() / 200).max(1);
        for lemma in lemmas.iter().step_by(step) {
            if lemma.inflections.is_empty() {
                continue;
            }
            println!("infer {}", lemma.word);
            let _ = infer(lemma.word, PartOfSpeech::Noun, &Answers::new(), cat);
        }
    }

    #[test]
    fn the_recorded_prior_can_be_hidden_for_evaluation() {
        let report = infer_with(
            "mökki",
            PartOfSpeech::Noun,
            &Answers::new(),
            catalogue(),
            false,
        )
        .unwrap();
        assert!(report.ranked.iter().all(|candidate| !candidate.recorded));
        assert!(report.recorded.iter().any(|class| class.class == 5));
        let report = infer("mökki", PartOfSpeech::Noun, &Answers::new(), catalogue()).unwrap();
        assert!(report.ranked[0].recorded);
    }

    #[test]
    fn the_indeclinable_result_is_class_99() {
        let report = indeclinable("mökki", PartOfSpeech::Noun).unwrap();
        assert_eq!(report.ranked.len(), 1);
        assert_eq!(report.ranked[0].class.to_string(), "99");
        assert!(report.question.is_none());
        assert!(!report.tie);
        assert!(report.recorded.iter().any(|class| class.class == 5));
    }

    #[test]
    fn the_tail_of_a_word_is_the_last_three_chars() {
        assert_eq!(tail_of("mökki"), "kki");
        assert_eq!(tail_of("äly"), "äly");
        assert_eq!(tail_of("ä"), "ä");
    }

    #[test]
    fn the_tail_prior_settles_the_worked_example_tie() {
        let mut answers = Answers::new();
        answers.insert("gen_sg".to_string(), "mökin".to_string());
        answers.insert("par_sg".to_string(), "mökkiä".to_string());
        answers.insert("ill_sg".to_string(), "mökkiin".to_string());
        answers.insert("gen_pl".to_string(), "mökkien".to_string());
        answers.insert("nom_pl".to_string(), "mökit".to_string());
        let report = infer("mökki", PartOfSpeech::Noun, &answers, catalogue()).unwrap();
        assert_eq!(report.ranked[0].class.to_string(), "5*A");
        assert!(!report.tie, "the tail prior separates 5*A from the rivals");
    }

    #[test]
    fn the_tail_prior_prefers_the_usual_gradation() {
        // `pöräkki` is not in the dictionary; it declines like `mökki`.
        // 5*A and 5*D produce the same answers here, so the tail prior
        // (`-kki` words are mostly 5*A) is what separates them.
        let mut answers = Answers::new();
        answers.insert("gen_sg".to_string(), "pöräkin".to_string());
        answers.insert("par_sg".to_string(), "pöräkkiä".to_string());
        answers.insert("ill_sg".to_string(), "pöräkkiin".to_string());
        let report = infer("pöräkki", PartOfSpeech::Noun, &answers, catalogue()).unwrap();
        assert!(report.recorded.is_empty());
        assert_eq!(report.ranked[0].class.to_string(), "5*A");
        assert!(report.ranked[0].tail_prior > 0);
    }

    #[test]
    fn a_capitalised_word_uses_its_lowercase_headword() {
        let mut answers = Answers::new();
        answers.insert("gen_sg".to_string(), "mökin".to_string());
        let report = infer("Mökki", PartOfSpeech::Noun, &answers, catalogue()).unwrap();
        assert_eq!(report.lemma, "Mökki");
        assert_eq!(report.ranked[0].class.to_string(), "5*A");
        assert!(report.ranked[0].recorded);
    }

    #[test]
    fn a_wrong_answer_is_a_miss_and_does_not_stop_the_wizard() {
        let mut answers = Answers::new();
        answers.insert("gen_sg".to_string(), "mökkin".to_string());
        let report = infer("mökki", PartOfSpeech::Noun, &answers, catalogue()).unwrap();
        let best = &report.ranked[0];
        assert_eq!(best.hits, 1);
        assert_eq!(best.misses, 0);
        assert_eq!(
            report.outcomes[0].status,
            AnswerStatus::Hit,
            "the best class is the one that predicts the answer"
        );

        let mut answers = Answers::new();
        answers.insert("gen_sg".to_string(), "not-a-word".to_string());
        let report = infer("mökki", PartOfSpeech::Noun, &answers, catalogue()).unwrap();
        assert_eq!(report.ranked[0].misses, 1);
        assert_eq!(report.outcomes[0].status, AnswerStatus::Miss);
    }

    #[test]
    fn a_skipped_answer_is_not_a_miss() {
        let mut answers = Answers::new();
        answers.insert("gen_sg".to_string(), String::new());
        let report = infer("mökki", PartOfSpeech::Noun, &answers, catalogue()).unwrap();
        assert_eq!(report.outcomes[0].status, AnswerStatus::Skipped);
        assert_eq!(report.ranked[0].misses, 0);
        assert_ne!(report.question_key().as_deref(), Some("gen_sg"));
    }

    #[test]
    fn a_verb_finds_its_class() {
        let mut answers = Answers::new();
        answers.insert("pres_1sg".to_string(), "puhun".to_string());
        answers.insert("pres_3sg".to_string(), "puhuu".to_string());
        answers.insert("past_3sg".to_string(), "puhui".to_string());
        let report = infer("puhua", PartOfSpeech::Verb, &answers, catalogue()).unwrap();
        assert_eq!(report.ranked[0].class.to_string(), "52");
        assert_eq!(report.outcomes.len(), 3);
        assert!(
            report
                .outcomes
                .iter()
                .all(|outcome| outcome.status == AnswerStatus::Hit)
        );
        assert!(!report.recorded.is_empty());
    }

    #[test]
    fn an_ordinal_finds_its_class() {
        let mut answers = Answers::new();
        answers.insert("gen_sg".to_string(), "kahdeksannen".to_string());
        answers.insert("par_sg".to_string(), "kahdeksatta".to_string());
        answers.insert("ill_sg".to_string(), "kahdeksanteen".to_string());
        let report = infer("kahdeksas", PartOfSpeech::Ordinal, &answers, catalogue()).unwrap();
        assert_eq!(report.ranked[0].class.to_string(), "45");
        assert!(report.recorded.iter().any(|class| class.class == 45));
    }

    #[test]
    fn the_first_question_for_a_verb_is_pres_1sg() {
        let report = infer("puhua", PartOfSpeech::Verb, &Answers::new(), catalogue()).unwrap();
        assert_eq!(report.question_key().as_deref(), Some("pres_1sg"));
    }

    #[test]
    fn an_adverb_is_not_supported() {
        let error = infer("hyvin", PartOfSpeech::Adverb, &Answers::new(), catalogue()).unwrap_err();
        assert_eq!(
            error,
            WizardError::UnsupportedPartOfSpeech(PartOfSpeech::Adverb)
        );
    }
}
