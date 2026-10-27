//! `torille` as a Python extension, for the Omorfi + Voikko harness.
//!
//! The harness used to shell out to the `inflect` binary once per word. That
//! costs a process spawn and a fresh parse of the paradigm catalogue and the
//! sanalista for *every* lemma, which is nearly all of the work: inflecting one
//! noun takes tens of microseconds once the data is loaded, against 230ms to
//! load it again. An `Engine` here holds the parsed data once and hands it to
//! every call, and `inflect` runs outside the GIL so a thread pool gets real
//! parallelism instead of a queue in front of one core.
//!
//! Usage:
//!     from torille import Engine
//!     engine = Engine()
//!     for form in engine.inflect("kala"):
//!         form.word, form.confidence, form.tags
//!     for point in engine.hyphenate("tietokoneohjelmointi"):
//!         point.byte_offset, point.char_offset, point.source
//!
//! `tags` is the same list of words the `inflect` binary prints after the
//! confidence, so anything written against that output keeps working.

// PyO3's `#[pymethods]` expands the return expression through a `PyResult`
// conversion that clippy sees as a no-op on the already-converted type.
#![allow(clippy::useless_conversion)]

use std::sync::OnceLock;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyList;

use torille::hyphenation::{
    BreakSource, CompoundAnalyzer, Exceptions, HyphenationOptions, LexicalCompoundAnalyzer,
    NoCompoundAnalyzer, VowelBreaks, positions_with,
};
use torille::kotus::{Inflection, Lemma};
use torille::spec::Catalogues;
use torille::tags::{Confidence, PartOfSpeech};

/// The parsed Kotus data, shared by every `Engine` in the process.
///
/// Both the catalogue and the sanalista are read-only once parsed and nothing
/// mutates them during inflection, so one copy serves all engines. The lexicon
/// is a few hundred thousand lemmas, but it is embedded in the binary and
/// precomputed at build time, so this is cheap: no data file to find and
/// nothing to parse.
struct Data {
    catalogue: Catalogues,
    /// The lexicon, leaked on purpose so that [`Self::index`] can borrow it.
    ///
    /// The leak costs nothing this did not already pay: the `Data` lives in a
    /// `OnceLock` for the life of the process, so owning a `Vec` here held the
    /// same 104 743 rows for the same length of time. What owning would *not*
    /// give is a `'static` name, and without one the index below cannot borrow
    /// the rows it points at -- a struct cannot hold a reference into its own
    /// field. Leaking moves the owner out of the struct and the borrow becomes
    /// ordinary.
    lexicon: &'static [Lemma<'static>],
    /// The lexicon grouped for lookup, built once and shared by every `Engine`.
    ///
    /// Compound splitting asks "which lemmas could this word be made of", which
    /// used to be a scan of all 104 743 rows per compound. See `lexicon_index`.
    index: torille::expand::LexiconIndex<'static>,
}

/// The lexicon, materialised once and kept for the life of the process.
fn lexicon() -> &'static [Lemma<'static>] {
    static LEXICON: OnceLock<&'static [Lemma<'static>]> = OnceLock::new();
    LEXICON.get_or_init(|| Box::leak(torille::kotus::sanalista().to_lemmas().into_boxed_slice()))
}

impl Data {
    fn load() -> &'static Data {
        static DATA: OnceLock<Data> = OnceLock::new();
        DATA.get_or_init(|| {
            let lexicon = lexicon();
            Data {
                catalogue: Catalogues::embedded().clone(),
                lexicon,
                index: torille::expand::LexiconIndex::new(lexicon),
            }
        })
    }
}

/// One generated form, as the harness sees it.
///
/// The reading is exposed as named fields rather than as the list of words the
/// `inflect` binary prints, because that list is whitespace to be split and
/// position to be counted: `poss 1pl` and `third person` are each two tokens in
/// the printed output, so a caller parsing it has to know the layout to survive
/// a reordering. Named fields have neither problem, and the printed form stays
/// available as `tags` for anyone who wants to show the reading.
#[pyclass(frozen, get_all)]
struct PyForm {
    /// The word as it would be written.
    word: String,
    /// `plain`, `repaired` or `guessed`.
    confidence: &'static str,
    /// Whether the confidence is `plain`, for the common test.
    plain: bool,
    /// The agreed reading of the form, one field per grammatical dimension.
    /// `None` where the reading says nothing, which is not the same as a field
    /// the reading has but is unnamed: `taloin` has no number at all.
    pos: Option<&'static str>,
    case: Option<&'static str>,
    number: Option<&'static str>,
    tense: Option<&'static str>,
    person: Option<&'static str>,
    mood: Option<&'static str>,
    voice: Option<&'static str>,
    possessive: Option<&'static str>,
    clitic: Option<&'static str>,
    /// `negative` for the `en puhu` construction, `None` for affirmative.
    negative: bool,
    /// The reading as the `inflect` binary prints it, for display.
    tags: Vec<String>,
}

/// One permitted Finnish line break, as the harness sees it.
#[pyclass(frozen, get_all)]
struct PyHyphenationPoint {
    /// Byte offset of the start of the next fragment, in the original
    /// word. Always a UTF-8 character boundary.
    byte_offset: usize,
    /// The same position as a character offset.
    char_offset: usize,
    /// Whether the morphology layer recognised a compound boundary.
    compound: bool,
    /// `pattern`, `compound`, `exception`, or `soft-hyphen`.
    source: &'static str,
    /// Lower is preferred; this is a ranking hint, not a confidence.
    penalty: u16,
}

fn confidence_name(c: Confidence) -> &'static str {
    match c {
        Confidence::Plain => "plain",
        Confidence::Repaired => "repaired",
        Confidence::Guessed => "guessed",
    }
}

fn possessive_name(p: torille::tags::Possessive) -> &'static str {
    use torille::tags::Possessive;
    match p {
        Possessive::FirstSingular => "1sg",
        Possessive::SecondSingular => "2sg",
        Possessive::Third => "3",
        Possessive::FirstPlural => "1pl",
        Possessive::SecondPlural => "2pl",
    }
}

fn clitic_name(c: torille::tags::Clitic) -> &'static str {
    use torille::tags::Clitic;
    match c {
        Clitic::Ko => "ko",
        Clitic::Kin => "kin",
        Clitic::Kaan => "kaan",
        Clitic::Han => "han",
        Clitic::Pa => "pa",
    }
}

/// The reading as the words the `inflect` binary prints, in its order.
///
/// Kept in step with `torille_tools::format_form` so the printed form cannot
/// drift from the library; this is for display, and nothing should parse it.
fn tag_words(t: &torille::tags::Tags) -> Vec<String> {
    let mut parts: Vec<String> = Vec::new();
    if let Some(pos) = t.part_of_speech {
        parts.push(pos.name().to_string());
    }
    if let Some(case) = t.case {
        parts.push(case.name().to_string());
    }
    if let Some(n) = t.number {
        parts.push(n.name().to_string());
    }
    if let Some(x) = t.tense {
        parts.push(x.name().to_string());
    }
    if let Some(p) = t.person {
        parts.push(p.name().to_string());
    }
    if let Some(m) = t.mood {
        parts.push(m.name().to_string());
    }
    if let Some(v) = t.voice {
        parts.push(v.name().to_string());
    }
    if let Some(p) = t.possessive {
        parts.push(format!("poss {}", possessive_name(p)));
    }
    if let Some(c) = t.clitic {
        parts.push(format!("clitic {}", clitic_name(c)));
    }
    if t.polarity == torille::tags::Polarity::Negative {
        parts.push("negative".to_string());
    }
    parts
}

/// Parses a Kotus class expression such as `73`, `7*D` or `9*B*A*A`.
fn parse_inflection(s: &str) -> Option<Inflection> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let digits: String = s.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return None;
    }
    let class: u16 = digits.parse().ok()?;
    let mut gradation = None;
    for c in s[digits.len()..].chars() {
        if c.is_ascii_uppercase() {
            gradation = Some(torille::kotus::Gradation::from_letter(c)?);
        }
    }
    Some(Inflection { class, gradation })
}

/// Keep only the gradated member of a class/gradation pair.
///
/// The sanalista lists a graded subtype and its plain base as separate homonym
/// rows (`hepo` as `1*E` and `1`); both confirm the same gradated paradigm, so
/// unioning them would also offer the plain base's ungradated forms. When the
/// same base `class` appears both with a gradation and without, the graded one
/// is the whole declension and the plain entry is dropped. A genuinely
/// multi-class word (palvi in 5 and 7) keeps both classes, because those base
/// classes differ.
fn dedup_graded(inflections: &mut Vec<Inflection>) {
    // Which base classes have a graded member already present.
    let graded: Vec<u16> = inflections
        .iter()
        .filter(|i| i.gradation.is_some())
        .map(|i| i.class)
        .collect();
    inflections.retain(|i| i.gradation.is_some() || !graded.contains(&i.class));
}

/// Drop the indeclinable marker (class 99) when the word has a real class.
///
/// The sanalista flags some words as `99` (the indeclinable marker) beside a
/// real class (`kuti` in 5 and 99, `aika` in 10 and 99). The `99` entry is
/// meant to supply only the bare nominative -- the real class already does --
/// so unioning class 99's own i-loan paradigm in would fabricate forms no
/// holder of the real class confirms. When any non-99 class was kept, the
/// marker is dropped.
fn drop_marker(inflections: &mut Vec<Inflection>) {
    if inflections.iter().any(|i| i.class != 99) {
        inflections.retain(|i| i.class != 99);
    }
}

fn parse_pos(s: &str) -> Option<PartOfSpeech> {
    let s = s.trim().to_lowercase();
    if s.is_empty() {
        return None;
    }
    PartOfSpeech::parse_all(&s).into_iter().next()
}

fn parse_vowel_breaks(s: &str) -> Result<VowelBreaks, String> {
    match s.trim().to_lowercase().as_str() {
        "compound-only" | "compound" => Ok(VowelBreaks::CompoundOnly),
        "never" => Ok(VowelBreaks::Never),
        "always" => Ok(VowelBreaks::Always),
        other => Err(format!(
            "{other:?}: expected `compound-only`, `never`, or `always`"
        )),
    }
}

fn source_name(source: BreakSource) -> &'static str {
    source.as_str()
}

/// What [`Engine::inflect`] decided to do, worked out before any work happens.
///
/// Separate from the method that acts on it so the decision can be tested
/// without a Python interpreter: it is a plain value, and it is where both
/// defects that shaped it lived -- the inflection list could name the same
/// paradigm twice, and the part of speech could be read off the first row only.
#[derive(Debug)]
struct Plan {
    /// The paradigms to run, each at most once and in the order first claimed.
    inflections: Vec<Inflection>,
    part_of_speech: PartOfSpeech,
    /// Whether the word's class takes possessive suffixes.
    possessives: bool,
}

/// Why a lemma cannot be inflected.
///
/// A plain value rather than a [`PyErr`] so that [`Engine::plan`] needs no
/// interpreter and can be tested on its own; [`PlanError::into_pyerr`] is the
/// only place Python is involved.
#[derive(Debug, PartialEq, Eq)]
enum PlanError {
    /// Not in the sanalista, and no class given to inflect it with anyway.
    UnknownLemma(String),
    /// A class was given that is not a class expression.
    BadClass(String),
}

impl PlanError {
    fn message(&self) -> String {
        match self {
            PlanError::UnknownLemma(w) => {
                format!("{w}: not in the sanalista; give a class to inflect it")
            }
            PlanError::BadClass(c) => {
                format!("{c}: not a class expression (expected e.g. 73, 7*D)")
            }
        }
    }

    fn into_pyerr(self) -> PyErr {
        pyo3::exceptions::PyValueError::new_err(self.message())
    }
}

/// The inflector, holding the Kotus data.
#[pyclass]
struct Engine {
    data: &'static Data,
    /// `word -> indices into the lexicon`, so a lookup is a hash lookup rather
    /// than a scan of a few hundred thousand lemmas per word.
    index: std::collections::HashMap<String, Vec<usize>>,
}

#[pymethods]
impl Engine {
    #[new]
    fn new() -> Self {
        let data = Data::load();
        let mut index: std::collections::HashMap<String, Vec<usize>> =
            std::collections::HashMap::new();
        for (i, lemma) in data.lexicon.iter().enumerate() {
            index.entry(lemma.word.to_string()).or_default().push(i);
        }
        Engine { data, index }
    }

    /// Inflects `lemma` and returns every generated form with its confidence and
    /// reading.
    ///
    /// `klass` is a Kotus class expression and `pos` a part of speech; when
    /// either is omitted the lemma's own from the sanalista is used. Raises
    /// `ValueError` if the lemma is unknown and no `klass` is given, which is
    /// the one case with no sensible default.
    ///
    /// The class argument is called `klass`, not `class`, because Python will not
    /// accept a reserved word as a keyword argument: `inflect(w, class="9")` is
    /// a `SyntaxError` at parse time, before this function is ever reached. The
    /// name that reads best in Rust is a name no caller could have used.
    #[pyo3(signature = (lemma, klass=None, pos=None))]
    fn inflect(
        &self,
        py: Python<'_>,
        lemma: &str,
        klass: Option<&str>,
        pos: Option<&str>,
    ) -> PyResult<Py<PyList>> {
        let plan = self
            .plan(lemma, klass, pos)
            .map_err(PlanError::into_pyerr)?;
        let forms = py.allow_threads(|| self.generate(lemma, &plan));

        let mut out: Vec<PyForm> = Vec::with_capacity(forms.len());
        for f in &forms {
            // The union, not the first reading: the binary prints the agreed
            // reading, and where a form has two readings the first one is not
            // necessarily what the two of them agree on.
            let t = f.union();
            out.push(PyForm {
                word: f.word.clone(),
                confidence: confidence_name(f.confidence),
                plain: f.plain(),
                pos: t.part_of_speech.map(|p| p.name()),
                case: t.case.map(|c| c.name()),
                number: t.number.map(|n| n.name()),
                tense: t.tense.map(|x| x.name()),
                person: t.person.map(|p| p.name()),
                mood: t.mood.map(|m| m.name()),
                voice: t.voice.map(|v| v.name()),
                possessive: t.possessive.map(possessive_name),
                clitic: t.clitic.map(clitic_name),
                negative: t.polarity == torille::tags::Polarity::Negative,
                tags: tag_words(&t),
            });
        }
        // `*_bound` rather than the GIL-Ref constructors: the prelude's `PyList`
        // shadows the `&PyList` ones. A `#[pyclass]` has no blanket
        // `ToPyObject` in pyo3 0.22, so each form is wrapped as it is appended.
        let list = PyList::empty_bound(py);
        for item in out {
            list.append(Py::new(py, item)?)?;
        }
        Ok(list.unbind())
    }

    /// The lemma's class expressions and part of speech, as the sanalista has
    /// them. Empty when the lemma is not in the sanalista.
    #[pyo3(signature = (lemma))]
    fn lookup(&self, lemma: &str) -> Vec<(String, String)> {
        self.index
            .get(lemma)
            .map(|entries| {
                entries
                    .iter()
                    .map(|i| {
                        let l = &self.data.lexicon[*i];
                        let classes: Vec<String> =
                            l.inflections.iter().map(ToString::to_string).collect();
                        (classes.join(","), l.part_of_speech.to_string())
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Every lemma of the given part of speech that has a class, with its first
    /// class. This is the wordlist the harness samples from, read through the
    /// sanalista parser rather than by splitting the file again in Python.
    ///
    /// Lemmas with no class are left out: a compound or an indeclinable has no
    /// paradigm of its own and cannot be inflected, so offering one here would
    /// only fill the sampler's `--limit` with words that get skipped.
    #[pyo3(signature = (part_of_speech))]
    fn words(&self, part_of_speech: &str) -> Vec<(String, String)> {
        let wanted = part_of_speech.trim();
        self.data
            .lexicon
            .iter()
            .filter(|l| l.part_of_speech.split(", ").any(|p| p.trim() == wanted))
            .filter_map(|l| {
                l.inflections
                    .first()
                    .map(|i| (l.word.to_string(), i.to_string()))
            })
            .collect()
    }

    /// Permitted Finnish line breaks in `word`.
    ///
    /// Returns the same points the CLI's `--json` mode and the wasm
    /// `hyphenate` export use. `vowel_breaks` is `compound-only`
    /// (default), `never`, or `always`. `compound_breaks=False` gives
    /// the pattern-only fallback, useful for diagnostics.
    ///
    /// Existing soft hyphens (`U+00AD`) are kept in the source and
    /// returned as `soft-hyphen` points; the point sits after the
    /// invisible character, so it belongs to the preceding fragment.
    #[pyo3(signature = (word, left_min=2, right_min=2, compound_breaks=true, vowel_breaks="compound-only"))]
    fn hyphenate(
        &self,
        py: Python<'_>,
        word: &str,
        left_min: usize,
        right_min: usize,
        compound_breaks: bool,
        vowel_breaks: &str,
    ) -> PyResult<Py<PyList>> {
        let vowel_breaks = parse_vowel_breaks(vowel_breaks).map_err(PyValueError::new_err)?;
        let options = HyphenationOptions {
            left_min,
            right_min,
            compound_breaks,
            vowel_breaks,
        };
        let points = py.allow_threads(|| {
            let compound = LexicalCompoundAnalyzer;
            let no_compound = NoCompoundAnalyzer;
            let analyzer: &dyn CompoundAnalyzer = if compound_breaks {
                &compound
            } else {
                &no_compound
            };
            positions_with(word, &options, analyzer, Exceptions::finnish())
        });

        let list = PyList::empty_bound(py);
        for point in points {
            list.append(Py::new(
                py,
                PyHyphenationPoint {
                    byte_offset: point.byte_offset,
                    char_offset: point.char_offset,
                    compound: point.compound_boundary,
                    source: source_name(point.source),
                    penalty: point.penalty,
                },
            )?)?;
        }
        Ok(list.unbind())
    }
}
/// The decision half of [`Engine::inflect`], kept out of `#[pymethods]`
/// so it stays ordinary Rust that a test can call.
impl Engine {
    /// Work out which paradigms to run and how to read the word, before any
    /// inflection happens.
    ///
    /// Split out of [`Engine::inflect`] so it can be tested without a Python
    /// interpreter. That is not only for convenience: the duplicate-inflection
    /// defect this now guards lived here, and being able to assert on the plan
    /// is what turned "372 lemmas emit half their forms twice" from a
    /// measurement into a test.
    fn plan(&self, lemma: &str, class: Option<&str>, pos: Option<&str>) -> Result<Plan, PlanError> {
        // A class given outright is the escape hatch this error message has been
        // recommending all along: the sanalista is consulted for the part of
        // speech and the possessive capability, not for permission to inflect.
        // Before, the index lookup ran first and refused the word regardless of
        // the class, so `inflect(word, class="1")` raised the very error that
        // told the caller to do that. An unknown word simply contributes no
        // lexicon rows, and with no `pos` either, no possessive suffixes.
        let found: Vec<&Lemma> = match self.index.get(lemma) {
            Some(entries) => entries.iter().map(|i| &self.data.lexicon[*i]).collect(),
            None if class.is_some() => Vec::new(),
            None => return Err(PlanError::UnknownLemma(lemma.to_string())),
        };

        // A word takes possessive suffixes only when it can be a noun: `mieto`
        // (adjektiivi) does not, `marto` (adjektiivi, substantiivi) does. The
        // raw sanalista POS string carries the substiivi capability that the
        // parsed single PartOfSpeech loses.
        //
        // `any`, not `first`: a homonym can be a noun in one reading and an
        // adjective in another, and offering the column it is entitled to in
        // one reading beats withholding it because the first row was not it.
        // An explicit `pos` overrides the lexicon, since that is the caller
        // stating the reading directly.
        let noun_capable = match pos {
            Some(name) => parse_pos(name) == Some(PartOfSpeech::Noun),
            None => found
                .iter()
                .any(|l| PartOfSpeech::is_substantival(l.part_of_speech)),
        };

        // The reading, and it has to agree with `noun_capable`. These used to be
        // two independent decisions and they contradicted each other: the flag
        // asked whether the word could be a noun and said yes, while the reading
        // took the field's first entry and said adjective. `inflect_in` then
        // stamped `of_pos` over the finished grid, so `pieneemme` came back
        // carrying `-mme` and tagged `adjective` -- a possessive form asserting
        // it is not a noun, which is the same defect as `ehtooheni` being tagged
        // a bare singular illative. One rule now decides both, so they cannot.
        let part_of_speech = match pos {
            Some(name) => parse_pos(name).unwrap_or(PartOfSpeech::Unknown),
            None if noun_capable => PartOfSpeech::Noun,
            None => found
                .first()
                .map(|l| PartOfSpeech::inflecting_reading(l.part_of_speech))
                .unwrap_or(PartOfSpeech::Unknown),
        };

        let inflections: Vec<Inflection> = match class {
            Some(c) => vec![parse_inflection(c).ok_or_else(|| PlanError::BadClass(c.to_string()))?],
            // A lemma may be listed under more than one class (palvi in 5 and
            // 7); union all of its paradigms so every confirmed form is offered.
            // But a class and one of its own gradation subtypes are the *same*
            // declension: the sanalista lists `hepo` as both `1*E` and `1`, and
            // the two homonym rows confirm the same 153 gradated forms. The
            // graded subtype is the whole paradigm, not an extra one to union
            // in -- inflecting `hepo` also as plain `1` would offer the
            // ungradated `hepo*` forms beside the confirmed `hevo*`. Where the
            // same base class appears with and without a gradation, keep only
            // the graded entry.
            None => {
                let mut all: Vec<Inflection> = found
                    .iter()
                    .flat_map(|l| l.inflections.iter().copied())
                    .collect();
                dedup_graded(&mut all);
                // Two rows can claim the same class, and claiming a class twice
                // builds the identical grid twice: `aarnio` has two sanalista
                // rows both in class 32, and came back with all 165 of its
                // forms doubled -- 372 lemmas emit 266 151 forms of which
                // 136 111 are exact repeats. Distinct classes are kept, since
                // `vihanta` in `10*J` and `9*J` really is two different grids;
                // only exact repeats go, and first occurrence order is
                // preserved because it decides the order forms are emitted in.
                let mut claimed = std::collections::HashSet::new();
                all.retain(|i| claimed.insert(*i));
                drop_marker(&mut all);
                // A compound's class list names its head's class beside the
                // compound marker (`nuoripari` is `5, 51`) and can name a
                // second compound type. Drop the head class and keep both
                // markers when the row names both: `isoviha` and `isopurje` are
                // `51, 50`, and the 50 reading keeps the initial part fixed
                // (`isovihalta`, `isovihalle`) beside the class-51 reading
                // (`isonvihalta`).
                let markers: Vec<Inflection> = all
                    .iter()
                    .filter(|i| i.class == 50 || i.class == 51)
                    .copied()
                    .collect();
                if !markers.is_empty() {
                    all = markers;
                }
                all
            }
        };
        // A lemma with no class used to be an error, but the engine still
        // derives from such a word: `inflect_all` adds an adjective's degree
        // paradigms and a `-toista` numeral's, which need no class of their
        // own (`kuusitoista` is a classless numeral). Only a lemma that is
        // not in the lexicon at all, and given no class, is an error -- and
        // `UnknownLemma` above already caught that.

        Ok(Plan {
            inflections,
            part_of_speech,
            possessives: noun_capable,
        })
    }

    /// Run the paradigms [`Engine::plan`] chose.
    ///
    /// Takes no GIL token on purpose: the caller wraps it in
    /// `py.allow_threads`, so this reads shared read-only data and allocates
    /// only into its own `Vec`, and a thread pool scales across cores instead
    /// of queueing behind the interpreter lock.
    fn generate(&self, lemma: &str, plan: &Plan) -> Vec<torille::expand::Generated> {
        // `inflect_all`, not a loop over `inflect_in`: two of a lemma's classes
        // can spell a form identically, and the same word with the same reading
        // twice is one row of output the caller has to notice and throw away.
        torille::expand::inflect_all(
            lemma,
            &plan.inflections,
            &self.data.catalogue,
            Some(&self.data.index),
            plan.part_of_speech,
            plan.possessives,
        )
    }
}

/// The module itself.
///
/// Not named `torille`: a function by that name at the crate root would shadow
/// the `torille` crate in every path expression here, so `torille::kotus` would
/// stop resolving. The Python-side name is set explicitly instead.
#[pymodule(name = "torille")]
fn torille_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Engine>()?;
    m.add_class::<PyForm>()?;
    m.add_class::<PyHyphenationPoint>()?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// The same `Engine` `#[new]` builds, without a Python interpreter.
    fn engine() -> Engine {
        let data = Data::load();
        let mut index: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, lemma) in data.lexicon.iter().enumerate() {
            index.entry(lemma.word.to_string()).or_default().push(i);
        }
        Engine { data, index }
    }

    /// Every lemma the embedded lexicon lists more than once.
    fn homonyms(e: &Engine) -> Vec<&str> {
        let mut rows: HashMap<&str, usize> = HashMap::new();
        for lemma in e.data.lexicon.iter() {
            *rows.entry(lemma.word).or_default() += 1;
        }
        let mut out: Vec<&str> = rows
            .into_iter()
            .filter(|(_, n)| *n > 1)
            .map(|(w, _)| w)
            .collect();
        out.sort_unstable();
        out
    }

    /// A paradigm is inflected once, however many sanalista rows claim it.
    ///
    /// Claiming a class twice builds the identical grid twice. `aarnio` has
    /// two rows both in class 32 and came back with all 165 of its forms
    /// doubled; across the lexicon 372 lemmas were emitting 266 151 forms of
    /// which 136 111 were exact repeats -- more than half the output of every
    /// one of them. Nothing deduplicated the result, so the duplicates reached
    /// the caller: the Python list had two identical entries, and the
    /// fingerprint counted both.
    ///
    /// The distinction that keeps this from over-claiming is the one between a
    /// repeated claim and two different classes. `vihanta` is in `10*J` *and*
    /// `9*J`, which really are two declensions, and the two legitimately
    /// produce overlapping words. So the assertion is on the plan -- no
    /// inflection appears twice -- rather than on the forms, which overlap
    /// across distinct classes by design.
    #[test]
    fn a_paradigm_is_claimed_at_most_once() {
        let e = engine();
        let words = homonyms(&e);
        assert!(
            words.len() > 400,
            "only {} homonyms in the lexicon",
            words.len()
        );

        let mut repeated: Vec<String> = Vec::new();
        let mut two_class: Vec<&str> = Vec::new();
        let mut no_class: Vec<&str> = Vec::new();
        for word in &words {
            // A homonym can be in the lexicon with nothing usable to inflect
            // from -- `iso‑` is two rows, neither of which names a class. That
            // is `inflect`'s `NoClass` case, covered by the refusal test, and it
            // cannot be a repeat-claim because there is no claim to repeat.
            let Ok(plan) = e.plan(word, None, None) else {
                no_class.push(word);
                continue;
            };
            let mut seen = std::collections::HashSet::new();
            let mut dupes = 0;
            for inf in &plan.inflections {
                if !seen.insert(*inf) {
                    dupes += 1;
                }
            }
            if dupes > 0 {
                repeated.push(format!(
                    "{word}: {dupes} repeat(s) of {:?}",
                    plan.inflections
                ));
            }
            if plan.inflections.len() > 1 {
                two_class.push(word);
            }
        }

        assert!(
            repeated.is_empty(),
            "{} of {} homonyms claim a paradigm twice ({} have no class):\n  {}",
            repeated.len(),
            words.len(),
            no_class.len(),
            repeated[..repeated.len().min(10)].join("\n  ")
        );
        // The check above would also pass if `plan` simply dropped every
        // second inflection, so pin that the multi-class homonyms kept both of
        // theirs: `vihanta` is `10*J` and `9*J` and needs two grids. 26 of the
        // 450 homonyms keep two classes each, the rest are single-class or have
        // no class at all -- so the count alone proves nothing and the word does.
        let vihanta = e.plan("vihanta", None, None).unwrap();
        assert_eq!(
            vihanta.inflections.len(),
            2,
            "vihanta is 10*J and 9*J and needs both: {:?}",
            vihanta.inflections
        );
        assert!(
            two_class.len() >= 26,
            "only {} homonyms span two classes, so nothing proves they survive",
            two_class.len()
        );
    }

    /// Inflecting a homonym gives the same forms as inflecting it once.
    ///
    /// This is the effect the plan-level test above implies, asserted directly
    /// so a future change to `generate` cannot reintroduce the doubling without
    /// failing something that looks at output rather than at intent. `aarnio`
    /// is the witness: it claims class 32 on two rows and used to answer with
    /// every form twice.
    #[test]
    fn a_homonym_in_one_class_answers_with_each_form_once() {
        let e = engine();
        for word in ["aarnio", "ajokas", "ahtauttaa"] {
            let plan = e.plan(word, None, None).unwrap();
            assert_eq!(
                plan.inflections.len(),
                1,
                "{word} should claim one paradigm, claims {:?}",
                plan.inflections
            );
            let forms = e.generate(word, &plan);
            let mut counts: HashMap<(&str, torille::tags::Tags), usize> = HashMap::new();
            for f in &forms {
                *counts.entry((f.word.as_str(), f.union())).or_default() += 1;
            }
            let mut repeated: Vec<String> = counts
                .iter()
                .filter(|(_, n)| **n > 1)
                .map(|((w, r), n)| format!("{w} x{n} {r:?}"))
                .collect();
            repeated.sort();
            assert!(
                repeated.is_empty(),
                "{word}: {} of {} forms are exact repeats: {}",
                repeated.len(),
                forms.len(),
                repeated[..repeated.len().min(3)].join(", ")
            );
            assert!(forms.len() > 100, "{word}: only {} forms", forms.len());
        }
    }

    /// A word takes possessive suffixes only when the dictionary says it can be
    /// a noun.
    ///
    /// The decision used to be `!found.is_empty()`, which is constant `true`
    /// because an unknown lemma has already raised by the time it is reached.
    /// `PartOfSpeech::parse_all` collapses `adjektiivi, substantiivi` to the
    /// adjective, so the noun reading has to be looked for in the raw field.
    #[test]
    fn possessive_suffixes_follow_the_lexicon_not_the_first_row() {
        let e = engine();
        let owned = |word: &str| -> usize {
            e.generate(word, &e.plan(word, None, None).unwrap())
                .iter()
                .filter(|f| f.readings.iter().any(|r| r.possessive.is_some()))
                .count()
        };
        // `mieto` is `adjektiivi` alone, so it takes none.
        assert_eq!(owned("mieto"), 0, "mieto is an adjective only");
        // `marto` is `adjektiivi, substantiivi`, so it does.
        assert!(owned("marto") > 100, "marto can be a noun");
        // A homonym that is a noun in one reading keeps them on that strength,
        // though its first row is the adverb.
        assert!(owned("aika") > 100, "aika is a noun in class 9*D");
        // An explicit `pos` overrides the lexicon, in both directions.
        let mieto_as_noun = e.plan("mieto", None, Some("substantiivi")).unwrap();
        assert!(mieto_as_noun.possessives, "an explicit noun reading wins");
        let talo_as_adj = e.plan("talo", None, Some("adjektiivi")).unwrap();
        assert!(
            !talo_as_adj.possessives,
            "an explicit adjective reading wins"
        );
    }

    /// The reading a form is tagged with agrees with whether it has an owner.
    ///
    /// These used to be two decisions that contradicted each other. The
    /// possessive column was decided by looking for a noun token anywhere in the
    /// sanalista field, while the *reading* was the field's first entry, so for
    /// `pieni` (`adjektiivi, substantiivi`) one said "takes possessives" and the
    /// other said "adjective". `inflect_in` then applies the reading to the
    /// finished grid with `of_pos`, and `pieneemme` came back carrying the
    /// `-mme` suffix and asserting it was not a noun.
    ///
    /// Asserted on the output rather than on the plan, because the contradiction
    /// only ever became visible there: the plan's two fields were individually
    /// defensible and jointly wrong.
    #[test]
    fn a_possessive_form_is_never_tagged_as_something_that_is_not_a_noun() {
        let e = engine();
        // Every word in the embedded lexicon whose field admits a noun reading
        // and leads with another one. 956 such lemmas, of which 585 get a
        // possessive column to check -- the rest are classes with none.
        let mut checked = 0;
        for lemma in e.data.lexicon.iter() {
            let field = lemma.part_of_speech;
            if !PartOfSpeech::is_substantival(field)
                || PartOfSpeech::parse(field) == PartOfSpeech::Noun
            {
                continue;
            }
            let word = lemma.word;
            let Ok(plan) = e.plan(word, None, None) else {
                continue;
            };
            let owned: Vec<_> = e
                .generate(word, &plan)
                .into_iter()
                .filter(|f| f.readings.iter().any(|r| r.possessive.is_some()))
                .collect();
            if owned.is_empty() {
                continue;
            }
            checked += 1;
            let bad: Vec<&str> = owned
                .iter()
                .filter(|f| f.union().part_of_speech != Some(PartOfSpeech::Noun))
                .map(|f| f.word.as_str())
                .take(3)
                .collect();
            assert!(
                bad.is_empty(),
                "{word} is `{field}`: {} owned forms, but these are not tagged noun: {bad:?}",
                owned.len()
            );
        }
        assert!(
            checked >= 585,
            "only {checked} of the 585 words that need checking were checked; \
             the sweep has stopped covering the corpus"
        );
        // And the case the sweep cannot see, because it is not ambiguous: a plain
        // noun adjective is still tagged as whatever it is.
        let mieto = e.plan("mieto", None, None).unwrap();
        assert_eq!(mieto.part_of_speech, PartOfSpeech::Adjective);
        assert!(!mieto.possessives);
    }

    /// An unknown lemma, or one with no usable class, is an error rather than
    /// an empty answer.
    #[test]
    fn a_lemma_with_nothing_to_go_on_is_refused() {
        let e = engine();
        assert_eq!(
            e.plan("tämäei ole sanalistassa", None, None).unwrap_err(),
            PlanError::UnknownLemma("tämäei ole sanalistassa".to_string())
        );
        // A class that is not a class expression is a different mistake and
        // says so, rather than being reported as an unknown word.
        assert_eq!(
            e.plan("talo", Some("väärä"), None).unwrap_err(),
            PlanError::BadClass("väärä".to_string())
        );
        // A known lemma with an explicit class is fine even when the lexicon
        // has nothing for it, which is the documented escape hatch.
        assert!(e.plan("kokeilusanana", Some("1"), None).is_ok());
    }

    /// The escape hatch the error message recommends actually works.
    ///
    /// `inflect` has always refused a word that is not in the sanalista, and
    /// told the caller to give a class to inflect it with -- but the index
    /// lookup ran first and refused the word anyway, class or no class. So the
    /// advice was impossible to take, and a caller who only wanted the shape of
    /// `kokeilusanana` had no way to get it. The class is the whole input a
    /// caller needs, so it is enough.
    #[test]
    fn a_class_alone_is_enough_to_inflect_a_word_the_sanalista_has_never_seen() {
        let e = engine();
        let plan = e.plan("kokeilusanana", Some("1"), None).unwrap();
        assert_eq!(plan.inflections, vec![parse_inflection("1").unwrap()]);
        // No lexicon row means no evidence that the word is a noun, so no
        // possessive suffixes; `pos` is how the caller says otherwise, exactly
        // as for a word the sanalista does have.
        assert!(!plan.possessives);
        assert!(
            e.plan("kokeilusanana", Some("1"), Some("substantiivi"))
                .unwrap()
                .possessives
        );

        let forms = e.generate("kokeilusanana", &plan);
        assert!(
            !forms.is_empty(),
            "the class alone should still produce a paradigm"
        );
        assert!(
            forms.iter().all(|f| f.word.starts_with("kokeilusan")),
            "the class was applied to the word given"
        );

        // An unknown word in a class is not a degraded answer: it is the whole
        // paradigm, and nothing but the possessive suffixes ever comes from the
        // lexicon. Inflecting `talo` under the plan built for a word nobody has
        // ever heard of gives exactly what `talo`'s own class-1 plan gives.
        // Both are read as adjectives, since that is the reading under which
        // neither gets possessive suffixes.
        let words = |g: Vec<torille::expand::Generated>| -> Vec<String> {
            g.into_iter().map(|f| f.word).collect()
        };
        let as_adjective = e
            .plan("kokeilusanana", Some("1"), Some("adjektiivi"))
            .unwrap();
        let bare = words(e.generate("talo", &as_adjective));
        assert!(
            bare.len() > 20,
            "class 1 should be a full paradigm, got {} forms",
            bare.len()
        );
        assert_eq!(
            bare,
            words(
                e.generate(
                    "talo",
                    &e.plan("talo", Some("1"), None)
                        .map(|mut p| {
                            p.possessives = false;
                            p
                        })
                        .unwrap()
                )
            )
        );
    }

    #[test]
    fn vowel_break_names_are_the_cli_spellings() {
        assert_eq!(
            parse_vowel_breaks("compound-only"),
            Ok(VowelBreaks::CompoundOnly)
        );
        assert_eq!(
            parse_vowel_breaks("compound"),
            Ok(VowelBreaks::CompoundOnly)
        );
        assert_eq!(parse_vowel_breaks("never"), Ok(VowelBreaks::Never));
        assert_eq!(parse_vowel_breaks("always"), Ok(VowelBreaks::Always));
        assert!(parse_vowel_breaks("sometimes").is_err());
    }

    /// The Python surface exposes the same point names as the CLI and
    /// wasm surfaces, including the soft-hyphen source.
    #[test]
    fn hyphenation_points_have_one_name_for_each_source() {
        let word = "kissa\u{00AD}koira";
        let points = torille::hyphenation::positions(word);
        let soft = points
            .iter()
            .find(|point| point.source == BreakSource::SoftHyphen)
            .expect("the supplied soft hyphen");
        assert_eq!(soft.char_offset, 6);
        assert_eq!(source_name(soft.source), "soft-hyphen");
        assert_eq!(
            torille::hyphenation::split(word, &points).concat(),
            word,
            "the binding never modifies the source"
        );
    }
}
