//! Sentence roles from morphology, on demand.
//!
//! This is the second, advisory layer of the sentence plan: it resolves each
//! word through the existing morphology engine and records the grammatical
//! reading that the role resolver will use. It is deliberately **not** called
//! from the per-keystroke checker; the hot path only needs
//! [`crate::sentence::analyze`].
//!
//! The role rules are conservative. A word is a [`SyntacticRole::Predicate`]
//! only when a finite verb reading exists; subjects, objects, complements, and
//! modifiers need a morphological cue as well as word order, and a nominal
//! that is also a verb form (such as `kirjaa`) keeps
//! [`RoleConfidence::Weak`]. Positions without a cue stay
//! [`SyntacticRole::Unknown`] rather than being guessed.
//!
//! Clause segmentation covers finite predicates, subordinators and relative
//! pronouns, a comma before a finite verb, `ja joi`-style coordination, and
//! infinitival clauses. Coordinated subjects and objects are marked, a
//! coordinated or non-finite clause shares the preceding subject, and a
//! passive participle after `olla` is a complement rather than a subject.

use std::collections::HashMap;
use std::ops::Range;

use crate::expand::find_bases_with_form_filter;
use crate::kotus::lemmas;
use crate::sentence::{self, SentenceSpan};
use crate::spec::Catalogues;
use crate::tags::{Case, Mood, PartOfSpeech, Polarity, Tags, add_reading};
use crate::tokenise::Token;

/// What a token is doing in its sentence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyntacticRole {
    Predicate,
    Subject,
    Object,
    Adverbial,
    /// Predicative or object complement (`Hän tuli iloiseksi`).
    Complement,
    /// Attribute, genitive modifier, title, or apposition.
    Modifier,
    Unknown,
}

impl SyntacticRole {
    /// A stable name for diagnostics and future UI code.
    pub fn name(self) -> &'static str {
        match self {
            SyntacticRole::Predicate => "predicate",
            SyntacticRole::Subject => "subject",
            SyntacticRole::Object => "object",
            SyntacticRole::Adverbial => "adverbial",
            SyntacticRole::Complement => "complement",
            SyntacticRole::Modifier => "modifier",
            SyntacticRole::Unknown => "unknown",
        }
    }
}

/// How much the role resolver trusts a role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RoleConfidence {
    /// Morphology and valency agree.
    Certain,
    /// Morphology fits and word order supports it.
    Likely,
    /// Positional fallback, or an ambiguous word.
    Weak,
}

/// One clause candidate.
///
/// Phase 5 creates one clause per sentence. Later phases split a sentence
/// into coordinated and subordinate clauses and fill the role slots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clause {
    /// Byte span in the source text.
    pub start: usize,
    /// Exclusive byte end.
    pub end: usize,
    /// Token indices. The range includes only the clause's words, not the
    /// separators around them.
    pub tokens: Range<usize>,
    /// First finite predicate token, when one could be found.
    pub predicate: Option<usize>,
    /// Filled by a later phase.
    pub subject: Option<usize>,
    /// Filled by a later phase.
    pub object: Option<usize>,
    /// True when the clause contains a negative auxiliary (`en`, `et`, ...).
    pub negative: bool,
    /// True for an existential construction (`pöydällä on kirja`).
    pub existential: bool,
}

/// What one token is doing, plus the morphology behind the choice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenSyntax {
    pub role: SyntacticRole,
    pub confidence: RoleConfidence,
    /// Index into [`SyntaxAnalysis::clauses`], or 0 when the text has no
    /// words.
    pub clause: usize,
    /// Governor token index: the predicate for a subject/object/adverbial,
    /// or the head for a modifier. `None` when no governor is known.
    pub head: Option<usize>,
    /// The case that licensed the role, where one did.
    pub case: Option<Case>,
    /// The chosen lemma reading. Alternatives remain in `readings`.
    pub lemma: Option<String>,
    /// The morphological readings found for the surface word.
    pub readings: Vec<Tags>,
}

impl Default for TokenSyntax {
    fn default() -> Self {
        Self {
            role: SyntacticRole::Unknown,
            confidence: RoleConfidence::Weak,
            clause: 0,
            head: None,
            case: None,
            lemma: None,
            readings: Vec::new(),
        }
    }
}

/// The full on-demand analysis of a text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxAnalysis {
    /// Sentence spans, in source order.
    pub sentences: Vec<SentenceSpan>,
    /// One clause per sentence in phase 5.
    pub clauses: Vec<Clause>,
    /// One entry per input token, separators included.
    pub tokens: Vec<TokenSyntax>,
}

/// Analyze a token stream with the embedded lexicon.
///
/// This walks the same morphology engine the checker uses, so callers see the
/// engine's reading of every word. It is intentionally separate from
/// [`crate::sentence::analyze`] and must not run in the keystroke path.
pub fn analyze(tokens: &[Token]) -> SyntaxAnalysis {
    analyze_with(tokens, lemmas(), Catalogues::embedded())
}

/// The same analysis against a caller-supplied lexicon and catalogue.
///
/// Kept private for now: the crate's embedded data is the only production
/// source, and a public override would freeze an API before a caller needs it.
fn analyze_with(
    tokens: &[Token],
    lexicon: &[crate::kotus::Lemma<'_>],
    cat: &Catalogues,
) -> SyntaxAnalysis {
    let contexts = sentence::analyze(tokens);
    let sentences = sentence::spans(tokens);

    let mut out = vec![TokenSyntax::default(); tokens.len()];
    let mut nominal_lemmas: Vec<Option<String>> = vec![None; tokens.len()];
    let mut cache: HashMap<String, WordAnalysis> = HashMap::new();

    // Morphology first: every later rule reads the same resolved readings.
    for (index, token) in tokens.iter().enumerate() {
        if !token.is_word() {
            continue;
        }
        let lower = token.text.to_lowercase();
        let analysis = cache
            .entry(lower)
            .or_insert_with(|| resolve_word(lexicon, cat, &token.text.to_lowercase()));
        let mut readings = analysis.readings.clone();
        let mut lemma = analysis.lemma.clone();
        nominal_lemmas[index] = analysis.nominal_lemma.clone();
        if readings.is_empty()
            && token.is_capitalised()
            && let Some(name) = crate::expand::proper_names::lemma(&token.text)
        {
            // The name table knows the lemma but not the case. Only the
            // citation spelling is safely nominative; an inflected name
            // (`Helsingissä`, `Liisalle`) gets a case-less reading rather
            // than a guessed nominative, so it cannot become a subject or
            // object by accident. See PLAN_SYNTAX.md S2 for the case column
            // that will replace this.
            add_reading(&mut readings, proper_name_reading(&token.text, name));
            lemma = Some(name.to_string());
            nominal_lemmas[index] = Some(name.to_string());
        }
        let (role, confidence, case) = classify(&readings);
        if role == SyntacticRole::Predicate {
            if let Some(verb_lemma) = &analysis.verb_lemma {
                lemma = Some(verb_lemma.clone());
            }
        } else if let Some(nominal_lemma) = &analysis.nominal_lemma {
            lemma = Some(nominal_lemma.clone());
        }
        out[index] = TokenSyntax {
            role,
            confidence,
            clause: 0,
            head: None,
            case,
            lemma,
            readings,
        };
    }

    let mut sentence_words: Vec<Vec<usize>> = vec![Vec::new(); sentences.len()];
    for (index, token) in tokens.iter().enumerate() {
        if token.is_word()
            && let Some(words) = sentence_words.get_mut(contexts[index].sentence_index)
        {
            words.push(index);
        }
    }

    let mut clauses: Vec<Clause> = Vec::new();
    let mut clause_sentences: Vec<usize> = Vec::new();
    for (sentence_index, words) in sentence_words.iter().enumerate() {
        if words.is_empty() {
            continue;
        }
        for segment in segment_clause_words(words, tokens, &out) {
            let Some(&first) = segment.first() else {
                continue;
            };
            let last = *segment.last().unwrap_or(&first);
            let clause_index = clauses.len();
            for &word in &segment {
                out[word].clause = clause_index;
            }
            clauses.push(Clause {
                start: tokens[first].start,
                end: tokens[last].end,
                tokens: first..last + 1,
                predicate: None,
                subject: None,
                object: None,
                negative: false,
                existential: false,
            });
            clause_sentences.push(sentence_index);
        }
    }

    // Separators follow the clause they trail, matching `SentenceSpan`.
    let mut current_clause = 0usize;
    for (index, token) in tokens.iter().enumerate() {
        if token.is_word() {
            current_clause = out[index].clause;
        } else {
            out[index].clause = current_clause;
        }
    }

    for clause in &mut clauses {
        resolve_clause(clause, tokens, &mut out, &nominal_lemmas);
    }
    share_clause_roles(&mut clauses, &clause_sentences, tokens, &out);

    SyntaxAnalysis {
        sentences,
        clauses,
        tokens: out,
    }
}

/// The morphology of one surface word, reduced to what a role rule needs.
#[derive(Debug, Clone, Default)]
struct WordAnalysis {
    lemma: Option<String>,
    /// The lemma of a finite-verb reading, when one exists. `tuli` is both a
    /// noun and a past-tense verb; the context decides which, so both are
    /// kept until a role is known.
    verb_lemma: Option<String>,
    /// The lemma of a nominal reading, for the same reason.
    nominal_lemma: Option<String>,
    /// True when the ordinary search found this exact surface, not just a
    /// clitic-stripped host. A clitic word (`onko`, `talohan`) is only
    /// re-analysed through its host when this is false.
    exact: bool,
    readings: Vec<Tags>,
}

/// Resolve a lowercase word, peeling its clitic layers when the host is the
/// real word.
///
/// The base search recognises a clitic-bearing word through its host, but it
/// keeps no form for the surface (`onko` has no generated `onko`), so the
/// ordinary resolution falls back to a bare lemma reading with no mood or
/// case. `onko` then cannot be the predicate of a question. Resolving the
/// host and carrying its readings over fixes that. The clitic itself is kept
/// on the reading; it is discourse information, not a role cue.
fn resolve_word(
    lexicon: &[crate::kotus::Lemma<'_>],
    cat: &Catalogues,
    lower: &str,
) -> WordAnalysis {
    let ordinary = resolve_word_plain(lexicon, cat, lower);
    if ordinary.exact {
        return ordinary;
    }
    let (host, clitics) = crate::expand::clitic::strip_all_clitics(lower);
    if clitics.is_empty() || host == lower {
        return ordinary;
    }
    let mut resolved = resolve_word_plain(lexicon, cat, &host);
    if resolved.readings.is_empty() {
        return ordinary;
    }
    // `strip_all_clitics` returns the stack from the outside in. One slot is
    // all `Tags` has, so record the outermost particle.
    for reading in &mut resolved.readings {
        reading.clitic = clitics.first().copied();
    }
    resolved.exact = true;
    resolved
}

/// The ordinary, no-clitic resolution.
fn resolve_word_plain(
    lexicon: &[crate::kotus::Lemma<'_>],
    cat: &Catalogues,
    lower: &str,
) -> WordAnalysis {
    let report = find_bases_with_form_filter(lexicon, cat, lower, |form| form == lower);
    let mut out = WordAnalysis {
        exact: !report.direct.is_empty() || !report.forms.is_empty(),
        ..WordAnalysis::default()
    };

    if let Some(base) = report.bases.first() {
        out.lemma = Some(base.word.to_string());
    } else if let Some(direct) = report.direct.first() {
        out.lemma = Some(direct.word().to_string());
    }

    for source in &report.forms {
        if source.form.word != lower {
            continue;
        }
        if out.lemma.is_none() {
            out.lemma = Some(source.lemma.to_string());
        }
        let pos = lemma_pos(source.lemma);
        for reading in &source.form.readings {
            add_reading(&mut out.readings, reading_with_pos(*reading, pos));
        }
        let form_has_finite = source
            .form
            .readings
            .iter()
            .any(|reading| is_finite_predicate(&reading_with_pos(*reading, pos)));
        if form_has_finite {
            out.verb_lemma = Some(source.lemma.to_string());
        }
        let form_has_nominal = source
            .form
            .readings
            .iter()
            .any(|reading| is_nominal_pos(reading_with_pos(*reading, pos).part_of_speech));
        if form_has_nominal && is_nominal_pos(pos) {
            out.nominal_lemma = Some(source.lemma.to_string());
        }
    }

    // The prefix walks miss some stem-changing paradigm members (`söi` is
    // `syödä` across a two-character stem change). The cheap stem-map gate
    // can name the base even when those walks do not, so use it as a
    // fallback and re-inflect the base for the readings.
    if out.readings.is_empty()
        && let Some(base) = cheap_base(lower)
    {
        out.lemma = Some(base.clone());
        add_readings_from_base(&base, lower, cat, &mut out.readings);
        if is_nominal_pos(lemma_pos(&base)) {
            out.nominal_lemma = Some(base);
        }
    }

    // A direct lemma can hide a homonymous verb: `tuli` is the noun and the
    // past tense of `tulla`, and the lemma probe stops before the verb stem
    // map is asked. Recover the verb reading when the ordinary walk did not
    // find one; the words themselves are context-disambiguated later.
    if !out.readings.iter().any(is_verb_reading)
        && let Some(base) = crate::expand::check_normal::find_base_via_verb_stem_map(lower, false)
    {
        add_readings_from_base(&base, lower, cat, &mut out.readings);
        if out.readings.iter().any(is_finite_predicate) {
            out.verb_lemma = Some(base);
        }
    }

    // A direct lemma or a base with no retained form still has one reading:
    // the sanalista's part of speech and, for a nominal, the citation case.
    if out.readings.is_empty() {
        for direct in &report.direct {
            add_reading(&mut out.readings, direct_reading(direct));
            if is_nominal_pos(
                PartOfSpeech::parse_all(direct.part_of_speech())
                    .first()
                    .copied(),
            ) {
                out.nominal_lemma = Some(direct.word().to_string());
            }
        }
        if out.readings.is_empty() {
            for base in &report.bases {
                if let Some(row) = crate::kotus::sanalista().by_word(base.word).next() {
                    add_reading(&mut out.readings, direct_reading(&row));
                    if is_nominal_pos(
                        PartOfSpeech::parse_all(row.part_of_speech())
                            .first()
                            .copied(),
                    ) {
                        out.nominal_lemma = Some(base.word.to_string());
                    }
                }
            }
        }
    }

    // The negation auxiliary is a closed table, not a general verb
    // paradigm. Add the auxiliary reading even when a homonymous noun
    // reading exists (`ei` is also a noun), and leave the noun reading alone;
    // the clause flag keys off the auxiliary, not on every reading.
    if let Some(base) = crate::expand::negation::negation_base(lower) {
        if !out
            .readings
            .iter()
            .any(|reading| reading.part_of_speech == Some(PartOfSpeech::NegativeVerb))
        {
            add_reading(
                &mut out.readings,
                Tags {
                    part_of_speech: Some(PartOfSpeech::NegativeVerb),
                    polarity: Polarity::Negative,
                    ..Tags::UNKNOWN
                },
            );
        } else {
            for reading in &mut out.readings {
                if reading.part_of_speech == Some(PartOfSpeech::NegativeVerb) {
                    reading.polarity = Polarity::Negative;
                }
            }
        }
        if out.lemma.is_none() {
            out.lemma = Some(base.to_string());
        }
    }

    out
}

/// Name a base through the cheap per-word pipeline, without the prefix walks.
///
/// `check_normal_fast` runs the lemma, form, clitic, and stem-map gates. It is
/// exactly the set that catches a stem change the prefix neighbourhood misses,
/// and it is much cheaper than the full reverse index.
fn cheap_base(lower: &str) -> Option<String> {
    use crate::expand::check_normal::{NormalizedOutcome, check_normal_fast};

    match check_normal_fast(lower, false) {
        NormalizedOutcome::Lemma => Some(lower.to_string()),
        NormalizedOutcome::Inflected { base, .. } => Some(base),
        NormalizedOutcome::Unknown => None,
    }
}

/// Inflect `base` and add the readings for `lower`.
fn add_readings_from_base(base: &str, lower: &str, cat: &Catalogues, readings: &mut Vec<Tags>) {
    let Some(row) = crate::kotus::sanalista().by_word(base).next() else {
        return;
    };
    let pos = PartOfSpeech::parse_all(row.part_of_speech())
        .first()
        .copied()
        .unwrap_or(PartOfSpeech::Unknown);
    let inflections: Vec<_> = row.inflections().collect();
    for form in crate::expand::inflect_all(base, &inflections, cat, None, pos, true) {
        if form.word != lower {
            continue;
        }
        for reading in &form.readings {
            add_reading(readings, reading_with_pos(*reading, Some(pos)));
        }
    }
    if readings.is_empty() {
        add_reading(readings, direct_reading(&row));
    }
}

/// Replace a generated reading's part of speech with the source lemma's.
///
/// The base search inflects every lemma with `PartOfSpeech::Noun` (the
/// suggestion pool only needs surfaces), so an adverb or pronoun form arrives
/// tagged as a noun. The lemma row is the authority here: `eilen` must stay
/// an adverb and `sinä` a pronoun, or the role rules mistake them for
/// nominals. A form may be a reading of several lemmas, and each
/// `SourcedForm` names its own lemma, so this stays correct for homonyms
/// (`tuli` the noun vs. `tulla` the verb).
fn reading_with_pos(mut tags: Tags, pos: Option<PartOfSpeech>) -> Tags {
    if let Some(pos) = pos {
        tags.part_of_speech = Some(pos);
    }
    tags
}

/// The sanalista's first part of speech for a lemma.
fn lemma_pos(lemma: &str) -> Option<PartOfSpeech> {
    crate::kotus::sanalista()
        .by_word(lemma)
        .next()
        .and_then(|row| {
            PartOfSpeech::parse_all(row.part_of_speech())
                .first()
                .copied()
        })
}

/// A reading for a lemma that no generated form reached.
fn direct_reading(row: &crate::kotus::EmbeddedLemma) -> Tags {
    let pos = PartOfSpeech::parse_all(row.part_of_speech())
        .first()
        .copied();
    let mut tags = match pos {
        Some(
            PartOfSpeech::Noun
            | PartOfSpeech::Adjective
            | PartOfSpeech::Pronoun
            | PartOfSpeech::Numeral
            | PartOfSpeech::Ordinal,
        ) => Tags::nominal(Case::Nominative),
        _ => Tags::UNKNOWN,
    };
    tags.part_of_speech = pos;
    tags
}

/// Assign the roles phase 5 can defend from morphology alone.
fn classify(readings: &[Tags]) -> (SyntacticRole, RoleConfidence, Option<Case>) {
    if readings.is_empty() {
        return (SyntacticRole::Unknown, RoleConfidence::Weak, None);
    }

    // An infinitive reading wins over a spurious finite reading: `syödä`
    // is the infinitive of `syödä`, not a passive indicative. Grammar
    // context, not morphology alone, decides a genuinely ambiguous form;
    // phase 5 stays conservative.
    if readings
        .iter()
        .any(|reading| reading.mood == Some(Mood::Infinitive))
    {
        return (SyntacticRole::Unknown, RoleConfidence::Weak, None);
    }

    let finite = readings
        .iter()
        .filter(|reading| is_finite_predicate(reading))
        .count();
    if finite > 0 {
        let confidence = if finite == readings.len() {
            RoleConfidence::Likely
        } else {
            RoleConfidence::Weak
        };
        return (SyntacticRole::Predicate, confidence, None);
    }

    let adverbs = readings
        .iter()
        .filter(|reading| matches!(reading.part_of_speech, Some(PartOfSpeech::Adverb)))
        .count();
    if adverbs > 0 {
        let confidence = if adverbs == readings.len() {
            RoleConfidence::Likely
        } else {
            RoleConfidence::Weak
        };
        return (SyntacticRole::Adverbial, confidence, None);
    }

    let adverbial_cases: Vec<Case> = readings
        .iter()
        .filter_map(|reading| reading.case)
        .filter(|case| is_adverbial_case(*case))
        .collect();
    if let Some(case) = adverbial_cases.first().copied() {
        let confidence = if adverbial_cases.len() == readings.len() {
            RoleConfidence::Likely
        } else {
            RoleConfidence::Weak
        };
        return (SyntacticRole::Adverbial, confidence, Some(case));
    }

    (SyntacticRole::Unknown, RoleConfidence::Weak, None)
}

/// A reading that can be the finite predicate of a clause.
fn is_finite_predicate(reading: &Tags) -> bool {
    match reading.part_of_speech {
        Some(PartOfSpeech::NegativeVerb) => true,
        Some(PartOfSpeech::Verb) => matches!(
            reading.mood,
            Some(Mood::Indicative | Mood::Conditional | Mood::Imperative | Mood::Potential)
        ),
        _ => false,
    }
}

/// Cases that license an adverbial reading without clause context.
///
/// Essive and translative are deliberately absent: they are also the cases of
/// a predicate complement, and phase 6 needs the predicate's valency to tell
/// `Hän tuli iloiseksi` from an ordinary adverbial.
fn is_adverbial_case(case: Case) -> bool {
    matches!(
        case,
        Case::Inessive
            | Case::Elative
            | Case::Illative
            | Case::Adessive
            | Case::Ablative
            | Case::Allative
            | Case::Abessive
            | Case::Comitative
            | Case::Prolative
            | Case::Instructive
    )
}

/// A reading for a recognized proper name.
///
/// The citation spelling is nominative. Any other form is a case the name
/// table does not carry, so it stays case-less: role selection then leaves it
/// `Unknown` instead of inventing a nominative.
fn proper_name_reading(surface: &str, lemma: &str) -> Tags {
    if surface.eq_ignore_ascii_case(lemma) {
        Tags::nominal(Case::Nominative).of_pos(PartOfSpeech::Noun)
    } else {
        Tags::UNKNOWN.of_pos(PartOfSpeech::Noun)
    }
}

/// Whether any reading of a token is a verb.
fn is_verb_reading(reading: &Tags) -> bool {
    matches!(
        reading.part_of_speech,
        Some(PartOfSpeech::Verb | PartOfSpeech::NegativeVerb)
    )
}

/// Whether a part of speech is nominal.
fn is_nominal_pos(pos: Option<PartOfSpeech>) -> bool {
    matches!(
        pos,
        Some(
            PartOfSpeech::Noun
                | PartOfSpeech::Adjective
                | PartOfSpeech::Pronoun
                | PartOfSpeech::Numeral
                | PartOfSpeech::Ordinal
        )
    )
}

/// Whether any reading of a token is nominal.
fn is_nominal(syntax: &TokenSyntax) -> bool {
    syntax
        .readings
        .iter()
        .any(|reading| is_nominal_pos(reading.part_of_speech))
}

/// A nominal role is only `Likely` when the word is not also a verb.
fn nominal_confidence(syntax: &TokenSyntax) -> RoleConfidence {
    let has_verb = syntax.readings.iter().any(is_verb_reading);
    let has_nominal = is_nominal(syntax);
    if has_verb && has_nominal {
        RoleConfidence::Weak
    } else {
        RoleConfidence::Likely
    }
}

/// Whether a token has a reading in one of `cases`.
fn has_case(syntax: &TokenSyntax, cases: &[Case]) -> bool {
    syntax
        .readings
        .iter()
        .filter_map(|reading| reading.case)
        .any(|case| cases.contains(&case))
}

/// The first case from `cases` that the token actually has.
fn best_case(syntax: &TokenSyntax, cases: &[Case]) -> Option<Case> {
    cases.iter().copied().find(|case| {
        syntax
            .readings
            .iter()
            .any(|reading| reading.case == Some(*case))
    })
}

/// A finite predicate candidate, excluding a bare negative auxiliary when a
/// main verb is also present.
fn is_predicate_candidate(index: usize, out: &[TokenSyntax]) -> bool {
    out[index].role == SyntacticRole::Predicate
        && !out[index]
            .readings
            .iter()
            .all(|reading| reading.part_of_speech == Some(PartOfSpeech::NegativeVerb))
}

/// An infinitive reading, which can head an infinitival clause.
fn is_infinitive(index: usize, out: &[TokenSyntax]) -> bool {
    out[index]
        .readings
        .iter()
        .any(|reading| reading.mood == Some(Mood::Infinitive))
}

/// Split one sentence's words into clause-sized runs.
///
/// This is deliberately conservative. A coordinating conjunction starts a
/// clause only when the next word is a finite verb (`ja joi`), so `Sami ja
/// Liisa söivät` stays one clause. Subordinators and relative pronouns always
/// start a clause. A comma before a finite verb also starts one, which keeps
/// `Kun hän tuli, söin` from merging into a single predicate.
fn segment_clause_words(words: &[usize], tokens: &[Token], out: &[TokenSyntax]) -> Vec<Vec<usize>> {
    let mut starts = vec![0usize];
    for position in 1..words.len() {
        let lower = tokens[words[position]].text.to_lowercase();
        if is_subordinator(&lower) || is_relative(&lower) {
            starts.push(position);
            continue;
        }
        if is_coordinating(&lower) {
            if position + 1 < words.len() && is_predicate_candidate(words[position + 1], out) {
                starts.push(position);
            }
            continue;
        }
        if is_infinitive(words[position], out)
            && words[..position]
                .iter()
                .any(|&index| is_predicate_candidate(index, out))
        {
            starts.push(position);
            continue;
        }
        if is_predicate_candidate(words[position], out)
            && has_separator_between(tokens, words[position - 1], words[position], ',')
            && words[..position]
                .iter()
                .any(|&index| is_predicate_candidate(index, out))
        {
            starts.push(position);
        }
    }
    starts.sort_unstable();
    starts.dedup();

    let mut segments = Vec::new();
    for (index, &start) in starts.iter().enumerate() {
        let end = starts.get(index + 1).copied().unwrap_or(words.len());
        if start < end {
            segments.push(words[start..end].to_vec());
        }
    }
    segments
}

fn is_coordinating(word: &str) -> bool {
    matches!(
        word,
        "ja" | "mutta" | "tai" | "eli" | "sekä" | "vai" | "vaan"
    )
}

fn is_subordinator(word: &str) -> bool {
    matches!(
        word,
        "että" | "kun" | "jos" | "vaikka" | "koska" | "jotta" | "mikäli" | "ellei"
    )
}

fn is_relative(word: &str) -> bool {
    matches!(word, "joka" | "mikä" | "ken")
}

/// A function word that must not be mistaken for a subject or object merely
/// because the lexicon also gives it a noun reading (`ja`, `kun`).
fn is_clause_marker(index: usize, tokens: &[Token]) -> bool {
    let lower = tokens[index].text.to_lowercase();
    is_coordinating(&lower) || is_subordinator(&lower) || is_relative(&lower)
}

fn has_separator_between(tokens: &[Token], left: usize, right: usize, needle: char) -> bool {
    tokens
        .iter()
        .skip(left + 1)
        .take(right.saturating_sub(left + 1))
        .filter(|token| !token.is_word())
        .any(|token| token.text.contains(needle))
}

/// The first finite predicate candidate in a clause, preferring an
/// unambiguous one over an ambiguous noun/verb word.
fn choose_predicate(words: &[usize], out: &[TokenSyntax]) -> Option<usize> {
    words
        .iter()
        .copied()
        .filter(|&index| is_predicate_candidate(index, out))
        .find(|&index| out[index].confidence != RoleConfidence::Weak)
        .or_else(|| {
            words
                .iter()
                .copied()
                .find(|&index| is_predicate_candidate(index, out))
        })
        .or_else(|| {
            words
                .iter()
                .copied()
                .find(|&index| is_infinitive(index, out))
        })
}

/// Resolve the core roles in one clause.
fn resolve_clause(
    clause: &mut Clause,
    tokens: &[Token],
    out: &mut [TokenSyntax],
    nominal_lemmas: &[Option<String>],
) {
    let words: Vec<usize> = (clause.tokens.start..clause.tokens.end)
        .filter(|&index| tokens[index].is_word())
        .collect();
    let Some(predicate) = choose_predicate(&words, out) else {
        return;
    };
    clause.predicate = Some(predicate);
    if out[predicate].role != SyntacticRole::Predicate {
        out[predicate].role = SyntacticRole::Predicate;
        out[predicate].confidence = RoleConfidence::Likely;
    }
    // One clause has one predicate. A second finite reading that also has a
    // nominal reading (`kirjaa` is the imperative of `kirjata` and the
    // partitive of `kirja`) is re-read as the nominal it can be, so the
    // object/subject pass can use it. Auxiliaries are not demoted: the
    // negation auxiliary is phase 3's `Auxiliary`, and `olla`/participles are
    // phase 3's compound predicate.
    for &index in &words {
        if index == predicate || out[index].role != SyntacticRole::Predicate {
            continue;
        }
        let auxiliary = out[index]
            .readings
            .iter()
            .any(|reading| reading.part_of_speech == Some(PartOfSpeech::NegativeVerb));
        let nominal = out[index]
            .readings
            .iter()
            .any(|reading| is_nominal_pos(reading.part_of_speech));
        if !auxiliary && nominal {
            out[index].role = SyntacticRole::Unknown;
            out[index].confidence = RoleConfidence::Weak;
        }
    }
    let predicate_position = words
        .iter()
        .position(|&index| index == predicate)
        .unwrap_or(0);
    let before = &words[..predicate_position];
    let after = &words[predicate_position + 1..];

    let passive = out[predicate]
        .readings
        .iter()
        .any(|reading| reading.voice == Some(crate::tags::Voice::Passive));
    // The negative auxiliary is the only thing that makes a clause negative.
    // A connegative main verb (`syö` in `en syö`) also carries a negative
    // polarity reading, but it does not negate an affirmative imperative
    // (`Syö puuro!`) or an infinitive (`Hänen pitää mennä`).
    let negative = words.iter().any(|&index| {
        out[index]
            .readings
            .iter()
            .any(|reading| reading.part_of_speech == Some(PartOfSpeech::NegativeVerb))
    });
    clause.negative = negative;

    let lemma = out[predicate].lemma.clone().unwrap_or_default();
    let is_olla = lemma == "olla";
    let is_change = matches!(lemma.as_str(), "tulla" | "tulee");
    let locative_before = before.iter().any(|&index| {
        out[index].role == SyntacticRole::Adverbial && out[index].case.is_some_and(is_local_case)
    });
    let nominative_after = after.iter().any(|&index| {
        is_nominal(&out[index]) && has_case(&out[index], &[Case::Nominative, Case::Partitive])
    });
    clause.existential = is_olla && locative_before && nominative_after;

    let genitive_before = before.iter().rev().copied().find(|&index| {
        out[index].role == SyntacticRole::Unknown
            && is_nominal(&out[index])
            && has_case(&out[index], &[Case::Genitive])
            && !is_clause_marker(index, tokens)
    });
    let necessity = is_olla && genitive_before.is_some() && necessity_complement(after, out);
    let passive_construction = is_olla && !necessity && passive_participle_after(after, out);

    let mut subject = None;
    if !passive && !passive_construction {
        if clause.existential {
            subject = after.iter().copied().find(|&index| {
                out[index].role == SyntacticRole::Unknown
                    && is_nominal(&out[index])
                    && has_case(&out[index], &[Case::Nominative, Case::Partitive])
                    && !is_clause_marker(index, tokens)
            });
        } else if necessity {
            subject = genitive_before;
        }
        if subject.is_none() {
            // A true name (`Sami`) wins over a following capitalized common
            // word (`Ruotsalainen`) so a multiword name stays one subject.
            // The lemma's capitalization is the marker: a common word that
            // happens to be in the name table (`Vanha`) has a lowercase
            // lemma, and an inflected name has no case at all (S2).
            subject = before.iter().copied().find(|&index| {
                out[index].role == SyntacticRole::Unknown
                    && out[index]
                        .lemma
                        .as_deref()
                        .and_then(|lemma| lemma.chars().next())
                        .is_some_and(|first| first.is_uppercase())
                    && is_nominal(&out[index])
                    && has_case(&out[index], &[Case::Nominative, Case::Partitive])
                    && !is_clause_marker(index, tokens)
            });
        }
        if subject.is_none() {
            subject = before.iter().rev().copied().find(|&index| {
                out[index].role == SyntacticRole::Unknown
                    && is_nominal(&out[index])
                    && has_case(&out[index], &[Case::Nominative])
                    && !is_clause_marker(index, tokens)
            });
        }
    }
    if let Some(index) = subject {
        out[index].role = SyntacticRole::Subject;
        out[index].confidence = nominal_confidence(&out[index]);
        if let Some(noun) = &nominal_lemmas[index] {
            out[index].lemma = Some(noun.clone());
        }
        out[index].case = best_case(
            &out[index],
            &[Case::Nominative, Case::Partitive, Case::Genitive],
        );
        out[index].head = Some(predicate);
        clause.subject = Some(index);
        mark_coordinated_subjects(before, index, predicate, tokens, out);
    }

    let object_cases: &[Case] = if negative {
        &[Case::Partitive]
    } else {
        &[
            Case::Partitive,
            Case::Genitive,
            Case::Accusative,
            Case::Nominative,
        ]
    };
    let object = if passive || passive_construction {
        before.iter().chain(after).copied().find(|&index| {
            out[index].role == SyntacticRole::Unknown
                && is_nominal(&out[index])
                && has_case(&out[index], object_cases)
                && !is_clause_marker(index, tokens)
        })
    } else if !is_olla && !is_change {
        after.iter().copied().find(|&index| {
            Some(index) != subject
                && out[index].role == SyntacticRole::Unknown
                && is_nominal(&out[index])
                && has_case(&out[index], object_cases)
                && !is_clause_marker(index, tokens)
        })
    } else {
        None
    };
    if let Some(index) = object {
        out[index].role = SyntacticRole::Object;
        out[index].confidence = nominal_confidence(&out[index]);
        if let Some(noun) = &nominal_lemmas[index] {
            out[index].lemma = Some(noun.clone());
        }
        out[index].case = best_case(&out[index], object_cases);
        out[index].head = Some(predicate);
        clause.object = Some(index);
        mark_coordinated_objects(after, index, predicate, object_cases, tokens, out);
    }

    if is_olla || is_change {
        let complement_cases: &[Case] = if is_change {
            &[Case::Essive, Case::Translative]
        } else {
            &[
                Case::Nominative,
                Case::Partitive,
                Case::Essive,
                Case::Translative,
            ]
        };
        let complement = after.iter().copied().find(|&index| {
            Some(index) != subject
                && out[index].role == SyntacticRole::Unknown
                && is_nominal(&out[index])
                && has_case(&out[index], complement_cases)
                && !is_clause_marker(index, tokens)
        });
        // A participle after `olla` is the complement too: `Minun on mentävä`,
        // `Talo on rakennettu`. It has a verb reading and a case, not a noun
        // reading, so the nominal search above cannot see it.
        let complement = complement.or_else(|| {
            after.iter().copied().find(|&index| {
                Some(index) != subject
                    && out[index].readings.iter().any(|reading| {
                        reading.part_of_speech == Some(PartOfSpeech::Verb)
                            && reading.case.is_some()
                            && reading.mood != Some(Mood::Infinitive)
                    })
            })
        });
        if let Some(index) = complement {
            out[index].role = SyntacticRole::Complement;
            out[index].confidence = nominal_confidence(&out[index]);
            if let Some(noun) = &nominal_lemmas[index] {
                out[index].lemma = Some(noun.clone());
            }
            out[index].case = best_case(&out[index], complement_cases);
            out[index].head = Some(predicate);
        }
    }

    mark_modifiers(&words, predicate, subject, tokens, out);

    for &index in &words {
        if out[index].role == SyntacticRole::Adverbial && out[index].head.is_none() {
            out[index].head = Some(predicate);
        }
    }
}

/// The `Minun on mentävä` construction: a genitive subject plus a participle.
fn necessity_complement(after: &[usize], out: &[TokenSyntax]) -> bool {
    after.iter().any(|&index| {
        out[index]
            .readings
            .iter()
            .any(|reading| reading.part_of_speech == Some(PartOfSpeech::Verb))
    })
}

/// A passive participle after `olla`: `Talo on rakennettu`.
fn passive_participle_after(after: &[usize], out: &[TokenSyntax]) -> bool {
    after.iter().any(|&index| {
        out[index].readings.iter().any(|reading| {
            reading.part_of_speech == Some(PartOfSpeech::Verb)
                && reading.voice == Some(crate::tags::Voice::Passive)
                && reading.case.is_some()
                && reading.mood != Some(Mood::Infinitive)
        })
    })
}

/// Attributes, genitive modifiers, titles, and appositions.
fn mark_modifiers(
    words: &[usize],
    predicate: usize,
    subject: Option<usize>,
    tokens: &[Token],
    out: &mut [TokenSyntax],
) {
    for &index in words {
        if out[index].role != SyntacticRole::Unknown
            || !is_nominal(&out[index])
            || is_clause_marker(index, tokens)
        {
            continue;
        }

        // A capitalized word between the subject and the predicate is the rest
        // of a multiword name (`Sami Ruotsalainen`), not an object.
        if let Some(subject) = subject
            && index > subject
            && index < predicate
            && tokens[index].is_capitalised()
        {
            out[index].role = SyntacticRole::Modifier;
            out[index].confidence = RoleConfidence::Weak;
            out[index].head = Some(subject);
            continue;
        }

        let next = words
            .iter()
            .copied()
            .find(|&other| other > index && other != predicate);
        if let Some(next) = next.filter(|&other| other != predicate) {
            let genitive = has_case(&out[index], &[Case::Genitive]);
            let next_is_nominal = is_nominal(&out[next]);
            if genitive && next_is_nominal {
                out[index].role = SyntacticRole::Modifier;
                out[index].confidence = RoleConfidence::Likely;
                out[index].case = Some(Case::Genitive);
                out[index].head = Some(next);
                continue;
            }
            if Some(next) == subject && index < predicate {
                out[index].role = SyntacticRole::Modifier;
                out[index].confidence = RoleConfidence::Weak;
                out[index].head = Some(next);
            }
        }
    }
}

/// Mark `maidon` in `Sami söi puuron ja maidon` as a coordinated object.
fn mark_coordinated_objects(
    after: &[usize],
    object: usize,
    predicate: usize,
    object_cases: &[Case],
    tokens: &[Token],
    out: &mut [TokenSyntax],
) {
    for &index in after {
        if index == object
            || out[index].role != SyntacticRole::Unknown
            || !is_nominal(&out[index])
            || !has_case(&out[index], object_cases)
            || is_clause_marker(index, tokens)
        {
            continue;
        }
        let (left, right) = if index < object {
            (index, object)
        } else {
            (object, index)
        };
        if has_coordinator_between(tokens, left, right) {
            out[index].role = SyntacticRole::Object;
            out[index].confidence = nominal_confidence(&out[index]);
            out[index].case = best_case(&out[index], object_cases);
            out[index].head = Some(predicate);
        }
    }
}

/// Mark `Liisa` in `Sami ja Liisa söivät` as a coordinated subject.
fn mark_coordinated_subjects(
    before: &[usize],
    subject: usize,
    predicate: usize,
    tokens: &[Token],
    out: &mut [TokenSyntax],
) {
    for &index in before {
        if index == subject
            || out[index].role == SyntacticRole::Subject
            || !is_nominal(&out[index])
            || !has_case(&out[index], &[Case::Nominative])
            || is_clause_marker(index, tokens)
        {
            continue;
        }
        let (left, right) = if index < subject {
            (index, subject)
        } else {
            (subject, index)
        };
        if has_coordinator_between(tokens, left, right) {
            out[index].role = SyntacticRole::Subject;
            out[index].confidence = nominal_confidence(&out[index]);
            out[index].case = Some(Case::Nominative);
            out[index].head = Some(predicate);
        }
    }
}

/// Is there a coordinating conjunction between two token indices?
fn has_coordinator_between(tokens: &[Token], left: usize, right: usize) -> bool {
    tokens
        .iter()
        .skip(left + 1)
        .take(right.saturating_sub(left + 1))
        .filter(|token| token.is_word())
        .any(|token| is_coordinating(&token.text.to_lowercase()))
}

/// Share the subject of a coordinated or non-finite clause with the one before.
fn share_clause_roles(
    clauses: &mut [Clause],
    clause_sentences: &[usize],
    tokens: &[Token],
    out: &[TokenSyntax],
) {
    for index in 1..clauses.len() {
        if clause_sentences[index] != clause_sentences[index - 1] {
            continue;
        }
        if clauses[index].subject.is_some() {
            continue;
        }
        let starts_with_coordinator = tokens
            .get(clauses[index].tokens.start)
            .is_some_and(|token| is_coordinating(&token.text.to_lowercase()));
        let predicate_is_non_finite = clauses[index].predicate.is_some_and(|predicate| {
            out[predicate]
                .readings
                .iter()
                .any(|reading| matches!(reading.mood, Some(Mood::Infinitive | Mood::Instructive)))
        });
        if starts_with_coordinator || predicate_is_non_finite {
            clauses[index].subject = clauses[index - 1].subject;
        }
    }
}

/// Local cases that can license an adverbial.
fn is_local_case(case: Case) -> bool {
    matches!(
        case,
        Case::Inessive
            | Case::Elative
            | Case::Illative
            | Case::Adessive
            | Case::Ablative
            | Case::Allative
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenise::tokenise;

    fn analyzed(text: &str) -> (Vec<Token>, SyntaxAnalysis) {
        let tokens = tokenise(text);
        let analysis = analyze(&tokens);
        (tokens, analysis)
    }

    fn word_index(tokens: &[Token], word: &str) -> usize {
        tokens
            .iter()
            .position(|token| token.text == word)
            .unwrap_or_else(|| panic!("{word} not in {tokens:?}"))
    }

    #[test]
    fn a_subject_and_object_get_their_cases() {
        let (tokens, analysis) = analyzed("Sami söi aamupuuronsa.");
        let sami = word_index(&tokens, "Sami");
        let soi = word_index(&tokens, "söi");
        let object = word_index(&tokens, "aamupuuronsa");
        assert_eq!(analysis.tokens[sami].role, SyntacticRole::Subject);
        assert_eq!(analysis.tokens[sami].lemma.as_deref(), Some("Sami"));
        assert_eq!(analysis.tokens[soi].role, SyntacticRole::Predicate);
        assert_eq!(analysis.tokens[object].role, SyntacticRole::Object);
        assert_eq!(
            analysis.tokens[object].head,
            Some(soi),
            "the object is governed by the predicate"
        );
        assert_eq!(
            analysis.tokens[object].case,
            Some(Case::Nominative),
            "aamupuuronsa is read as the possessive object form"
        );
    }

    #[test]
    fn a_negative_clause_takes_a_partitive_object() {
        let (tokens, analysis) = analyzed("En syö puuroa.");
        let puuroa = word_index(&tokens, "puuroa");
        assert!(analysis.clauses[0].negative);
        assert_eq!(analysis.tokens[puuroa].role, SyntacticRole::Object);
        assert_eq!(analysis.tokens[puuroa].case, Some(Case::Partitive));
    }

    #[test]
    fn partitive_and_total_objects_differ() {
        let (partitive_tokens, partitive) = analyzed("Luen kirjaa.");
        let (total_tokens, total) = analyzed("Luen kirjan.");
        let partitive_object = word_index(&partitive_tokens, "kirjaa");
        let total_object = word_index(&total_tokens, "kirjan");
        assert_eq!(
            partitive.tokens[partitive_object].role,
            SyntacticRole::Object
        );
        assert_eq!(
            partitive.tokens[partitive_object].case,
            Some(Case::Partitive)
        );
        assert_eq!(
            partitive.tokens[partitive_object].lemma.as_deref(),
            Some("kirja")
        );
        assert_eq!(
            partitive.tokens[partitive_object].confidence,
            RoleConfidence::Weak,
            "kirjaa is also a verb form, so the nominal role is not certain"
        );
        assert_eq!(total.tokens[total_object].role, SyntacticRole::Object);
        assert_eq!(total.tokens[total_object].case, Some(Case::Genitive));
    }

    #[test]
    fn an_existential_has_a_post_predicate_subject() {
        let (tokens, analysis) = analyzed("Pöydällä on kirja.");
        let poydalla = word_index(&tokens, "Pöydällä");
        let kirja = word_index(&tokens, "kirja");
        assert!(analysis.clauses[0].existential);
        assert_eq!(analysis.tokens[poydalla].role, SyntacticRole::Adverbial);
        assert_eq!(analysis.tokens[kirja].role, SyntacticRole::Subject);
    }

    #[test]
    fn a_necessity_subject_is_genitive() {
        let (tokens, analysis) = analyzed("Minun on mentävä.");
        let minun = word_index(&tokens, "Minun");
        assert_eq!(analysis.tokens[minun].role, SyntacticRole::Subject);
        assert_eq!(analysis.tokens[minun].case, Some(Case::Genitive));
    }

    #[test]
    fn coordinated_clauses_share_the_subject() {
        let (tokens, analysis) = analyzed("Sami söi puuron ja joi maidon.");
        assert_eq!(analysis.clauses.len(), 2);
        let sami = word_index(&tokens, "Sami");
        let puuron = word_index(&tokens, "puuron");
        let maidon = word_index(&tokens, "maidon");
        assert_eq!(analysis.clauses[0].subject, Some(sami));
        assert_eq!(analysis.clauses[1].subject, Some(sami));
        assert_eq!(analysis.tokens[puuron].role, SyntacticRole::Object);
        assert_eq!(analysis.tokens[maidon].role, SyntacticRole::Object);
    }

    #[test]
    fn a_translative_after_a_change_verb_is_a_complement() {
        let (tokens, analysis) = analyzed("Hän tuli iloiseksi.");
        let tuli = word_index(&tokens, "tuli");
        let iloiseksi = word_index(&tokens, "iloiseksi");
        assert_eq!(analysis.tokens[tuli].role, SyntacticRole::Predicate);
        assert_eq!(analysis.tokens[tuli].lemma.as_deref(), Some("tulla"));
        assert_eq!(analysis.tokens[iloiseksi].role, SyntacticRole::Complement);
        assert_eq!(analysis.tokens[iloiseksi].case, Some(Case::Translative));
    }

    #[test]
    fn a_copula_complement_is_not_an_object() {
        let (tokens, analysis) = analyzed("Talo on kaunis.");
        let talo = word_index(&tokens, "Talo");
        let kaunis = word_index(&tokens, "kaunis");
        assert_eq!(analysis.tokens[talo].role, SyntacticRole::Subject);
        assert_eq!(analysis.tokens[kaunis].role, SyntacticRole::Complement);
    }

    #[test]
    fn noun_phrase_coordination_does_not_split_the_clause() {
        let (_, analysis) = analyzed("Sami ja Liisa söivät.");
        assert_eq!(analysis.clauses.len(), 1);
    }

    #[test]
    fn a_comma_before_a_finite_verb_starts_a_clause() {
        let (_, analysis) = analyzed("Kun hän tuli, söin.");
        assert_eq!(analysis.clauses.len(), 2);
    }

    #[test]
    fn coordinated_subjects_are_both_subjects() {
        let (tokens, analysis) = analyzed("Sami ja Liisa söivät.");
        let sami = word_index(&tokens, "Sami");
        let liisa = word_index(&tokens, "Liisa");
        assert_eq!(analysis.tokens[sami].role, SyntacticRole::Subject);
        assert_eq!(analysis.tokens[liisa].role, SyntacticRole::Subject);
    }

    #[test]
    fn coordinated_objects_are_both_objects() {
        let (tokens, analysis) = analyzed("Sami söi puuron ja maidon.");
        let puuron = word_index(&tokens, "puuron");
        let maidon = word_index(&tokens, "maidon");
        assert_eq!(analysis.tokens[puuron].role, SyntacticRole::Object);
        assert_eq!(analysis.tokens[maidon].role, SyntacticRole::Object);
        assert_eq!(analysis.tokens[maidon].head, analysis.clauses[0].predicate);
    }

    #[test]
    fn an_infinitive_clause_takes_an_object_and_shares_the_subject() {
        let (tokens, analysis) = analyzed("Sami haluaa syödä puuron.");
        assert_eq!(analysis.clauses.len(), 2);
        let sami = word_index(&tokens, "Sami");
        let syoda = word_index(&tokens, "syödä");
        let puuron = word_index(&tokens, "puuron");
        assert_eq!(analysis.tokens[syoda].role, SyntacticRole::Predicate);
        assert_eq!(analysis.tokens[puuron].role, SyntacticRole::Object);
        assert_eq!(analysis.clauses[1].subject, Some(sami));
    }

    #[test]
    fn a_participle_after_olla_is_a_complement() {
        let (tokens, analysis) = analyzed("Minun on mentävä.");
        let mentava = word_index(&tokens, "mentävä");
        assert_eq!(analysis.tokens[mentava].role, SyntacticRole::Complement);
    }

    #[test]
    fn a_perfect_passive_has_no_subject_either() {
        let (tokens, analysis) = analyzed("Talo on rakennettu.");
        let talo = word_index(&tokens, "Talo");
        let rakennettu = word_index(&tokens, "rakennettu");
        assert_eq!(analysis.clauses[0].subject, None);
        assert_eq!(analysis.tokens[talo].role, SyntacticRole::Object);
        assert_eq!(analysis.tokens[rakennettu].role, SyntacticRole::Complement);
    }

    #[test]
    fn a_passive_has_no_subject_but_keeps_the_logical_object() {
        let (tokens, analysis) = analyzed("Talo rakennettiin.");
        let talo = word_index(&tokens, "Talo");
        assert_eq!(analysis.clauses[0].subject, None);
        assert_eq!(analysis.tokens[talo].role, SyntacticRole::Object);
    }

    /// The name table knows the forms but not their cases. Only the citation
    /// spelling is safely nominative; `Helsingissä` must not become a
    /// nominative subject just because the form is capitalized.
    #[test]
    fn an_inflected_name_is_not_guessed_nominative() {
        let (tokens, analysis) = analyzed("Helsingissä asuva mies on opettaja.");
        let helsinki = word_index(&tokens, "Helsingissä");
        let mies = word_index(&tokens, "mies");
        assert_eq!(analysis.tokens[helsinki].case, None);
        assert_eq!(analysis.tokens[helsinki].role, SyntacticRole::Unknown);
        assert_eq!(analysis.tokens[mies].role, SyntacticRole::Subject);
    }

    /// The `-ko` clitic must not cost the auxiliary its mood. A question is
    /// not a negative clause just because a connegative reading exists.
    #[test]
    fn a_clitic_question_is_not_a_negative_clause() {
        let (tokens, analysis) = analyzed("Onko sinulla kirjaa?");
        let onko = word_index(&tokens, "Onko");
        assert_eq!(analysis.tokens[onko].role, SyntacticRole::Predicate);
        assert_eq!(analysis.tokens[onko].lemma.as_deref(), Some("olla"));
        assert!(!analysis.clauses[0].negative);
    }

    #[test]
    fn a_multiword_name_is_one_subject() {
        let (tokens, analysis) = analyzed("Sami Ruotsalainen söi puuron.");
        let sami = word_index(&tokens, "Sami");
        let ruotsalainen = word_index(&tokens, "Ruotsalainen");
        let puuron = word_index(&tokens, "puuron");
        assert_eq!(analysis.tokens[sami].role, SyntacticRole::Subject);
        assert_ne!(analysis.tokens[ruotsalainen].role, SyntacticRole::Object);
        assert_eq!(analysis.tokens[puuron].role, SyntacticRole::Object);
    }

    #[test]
    fn one_clause_per_sentence() {
        let (_, analysis) = analyzed("Talo on kaunis. Kissa istuu.");
        assert_eq!(analysis.clauses.len(), 2);
        assert_eq!(analysis.sentences.len(), 2);
        assert!(analysis.clauses[0].predicate.is_some());
        assert!(analysis.clauses[1].predicate.is_some());
    }

    #[test]
    fn a_finite_verb_is_a_predicate() {
        let (tokens, analysis) = analyzed("Sami söi aamupuuronsa.");
        let soi = word_index(&tokens, "söi");
        assert_eq!(analysis.tokens[soi].role, SyntacticRole::Predicate);
        assert_eq!(analysis.tokens[soi].confidence, RoleConfidence::Likely);
        assert_eq!(analysis.tokens[soi].lemma.as_deref(), Some("syödä"));
        assert!(
            analysis.tokens[soi]
                .readings
                .iter()
                .any(|reading| reading.part_of_speech == Some(PartOfSpeech::Verb))
        );
    }

    #[test]
    fn a_local_case_is_an_adverbial() {
        let (tokens, analysis) = analyzed("Pöydällä on kirja.");
        let poydalla = word_index(&tokens, "Pöydällä");
        assert_eq!(analysis.tokens[poydalla].role, SyntacticRole::Adverbial);
        assert_eq!(analysis.tokens[poydalla].case, Some(Case::Adessive));
        let on = word_index(&tokens, "on");
        assert_eq!(analysis.tokens[on].role, SyntacticRole::Predicate);
        assert_eq!(analysis.tokens[poydalla].head, Some(on));
    }

    #[test]
    fn an_infinitive_heads_its_own_clause() {
        let (tokens, analysis) = analyzed("Haluan syödä.");
        let haluan = word_index(&tokens, "Haluan");
        let syoda = word_index(&tokens, "syödä");
        assert_eq!(analysis.clauses.len(), 2);
        assert_eq!(analysis.tokens[haluan].role, SyntacticRole::Predicate);
        assert_eq!(analysis.tokens[syoda].role, SyntacticRole::Predicate);
    }

    #[test]
    fn negation_is_visible_on_the_clause() {
        let (_, analysis) = analyzed("En syö puuroa.");
        assert!(analysis.clauses[0].negative);
    }

    #[test]
    fn an_unknown_word_stays_unknown() {
        let (tokens, analysis) = analyzed("Zorblax istuu.");
        let unknown = word_index(&tokens, "Zorblax");
        assert_eq!(analysis.tokens[unknown].role, SyntacticRole::Unknown);
        assert_eq!(analysis.tokens[unknown].confidence, RoleConfidence::Weak);
        assert!(analysis.tokens[unknown].readings.is_empty());
    }

    #[test]
    fn token_output_is_aligned_with_the_input() {
        let (tokens, analysis) = analyzed("Talo on kaunis. Kissa istuu.");
        assert_eq!(analysis.tokens.len(), tokens.len());
        for (token, syntax) in tokens.iter().zip(&analysis.tokens) {
            if token.is_word() {
                assert!(syntax.clause < analysis.clauses.len());
            } else {
                assert_eq!(syntax.role, SyntacticRole::Unknown);
            }
        }
    }
}
