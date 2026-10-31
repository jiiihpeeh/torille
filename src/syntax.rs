//! Sentence roles from morphology, on demand.
//!
//! This is the second, advisory layer of the sentence plan: it resolves each
//! word through the existing morphology engine and records the grammatical
//! reading that the role resolver will use. It is deliberately **not** called
//! from the per-keystroke checker; the hot path only needs
//! [`crate::sentence::analyze`].
//!
//! Phase 5 keeps the role rules conservative. A word is a [`SyntacticRole::Predicate`]
//! only when a finite verb reading exists, and an [`SyntacticRole::Adverbial`]
//! only when a local or manner case licenses that reading. Subjects, objects,
//! complements, and modifiers stay [`SyntacticRole::Unknown`] until clause
//! segmentation and valency rules can place them. In particular, no role is
//! guessed from word order alone: positions without a morphological cue are
//! `Unknown` with [`RoleConfidence::Weak`].

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

    let mut clauses: Vec<Clause> = sentences
        .iter()
        .map(|span| Clause {
            start: span.start,
            end: span.end,
            tokens: 0..0,
            predicate: None,
            subject: None,
            object: None,
            negative: false,
            existential: false,
        })
        .collect();

    let mut out = vec![TokenSyntax::default(); tokens.len()];
    let mut words_by_clause: Vec<Vec<usize>> = vec![Vec::new(); clauses.len()];
    // Leading separators belong to the first sentence; later separators
    // belong to the sentence they follow, matching `SentenceSpan`'s
    // partition rule.
    let mut current_clause = 0usize;
    let mut cache: HashMap<String, WordAnalysis> = HashMap::new();

    for (index, token) in tokens.iter().enumerate() {
        if !token.is_word() {
            out[index].clause = current_clause;
            continue;
        }
        let clause = contexts[index].sentence_index;
        if clause >= clauses.len() {
            // `sentence::analyze` and `sentence::spans` are built from the
            // same walk, so this cannot happen; keep the token aligned
            // rather than panicking if the invariant ever changes.
            continue;
        }
        current_clause = clause;
        words_by_clause[clause].push(index);

        let lower = token.text.to_lowercase();
        let analysis = cache
            .entry(lower.clone())
            .or_insert_with(|| resolve_word(lexicon, cat, &lower));
        let (role, confidence, case) = classify(&analysis.readings);
        out[index] = TokenSyntax {
            role,
            confidence,
            clause,
            head: None,
            case,
            lemma: analysis.lemma.clone(),
            readings: analysis.readings.clone(),
        };
    }

    for (clause_index, words) in words_by_clause.iter().enumerate() {
        let Some(&first) = words.first() else {
            continue;
        };
        let last = *words.last().unwrap_or(&first);
        let clause = &mut clauses[clause_index];
        clause.tokens = first..last + 1;
        clause.predicate = words
            .iter()
            .copied()
            .find(|&index| out[index].role == SyntacticRole::Predicate);
        clause.negative = words.iter().any(|&index| {
            out[index]
                .readings
                .iter()
                .any(|reading| reading.polarity == Polarity::Negative)
        });
    }

    // An adverbial's governor is the clause predicate once one is known.
    for token in &mut out {
        if token.role == SyntacticRole::Adverbial {
            token.head = clauses
                .get(token.clause)
                .and_then(|clause| clause.predicate);
        }
    }

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
    readings: Vec<Tags>,
}

/// Resolve a lowercase word through the engine's base search.
fn resolve_word(
    lexicon: &[crate::kotus::Lemma<'_>],
    cat: &Catalogues,
    lower: &str,
) -> WordAnalysis {
    let report = find_bases_with_form_filter(lexicon, cat, lower, |form| form == lower);
    let mut out = WordAnalysis::default();

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
            let mut tags = *reading;
            if tags.part_of_speech.is_none() {
                tags.part_of_speech = pos;
            }
            add_reading(&mut out.readings, tags);
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
    }

    // A direct lemma or a base with no retained form still has one reading:
    // the sanalista's part of speech and, for a nominal, the citation case.
    if out.readings.is_empty() {
        for direct in &report.direct {
            add_reading(&mut out.readings, direct_reading(direct));
        }
        if out.readings.is_empty() {
            for base in &report.bases {
                if let Some(row) = crate::kotus::sanalista().by_word(base.word).next() {
                    add_reading(&mut out.readings, direct_reading(&row));
                }
            }
        }
    }

    // The negation auxiliary is a closed table, not a general verb
    // paradigm. Mark it negative so the clause-level flag is reliable.
    if crate::expand::negation::negation_base(lower).is_some() {
        if out.readings.is_empty() {
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
                reading.polarity = Polarity::Negative;
                if reading.part_of_speech.is_none() {
                    reading.part_of_speech = Some(PartOfSpeech::NegativeVerb);
                }
            }
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
            let mut tags = *reading;
            if tags.part_of_speech.is_none() {
                tags.part_of_speech = Some(pos);
            }
            add_reading(readings, tags);
        }
    }
    if readings.is_empty() {
        add_reading(readings, direct_reading(&row));
    }
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
    fn an_infinitive_is_not_a_finite_predicate() {
        let (tokens, analysis) = analyzed("Haluan syödä.");
        let haluan = word_index(&tokens, "Haluan");
        let syoda = word_index(&tokens, "syödä");
        assert_eq!(analysis.tokens[haluan].role, SyntacticRole::Predicate);
        assert_ne!(analysis.tokens[syoda].role, SyntacticRole::Predicate);
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
