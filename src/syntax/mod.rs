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

use std::cell::RefCell;
use std::ops::Range;

mod cache;
mod predicates;
mod roles;
mod segments;

use crate::kotus::lemmas;
use crate::sentence::{self, SentenceSpan};
use crate::spec::Catalogues;
use crate::tags::{Case, Tags, add_reading};
use crate::tokenise::Token;

pub use cache::AnalysisCache;
pub(crate) use predicates::{
    is_coordinating, is_relative_word, is_subordinator, predicate_is_passive,
};
use cache::proper_name_reading;
use roles::{classify, resolve_clause, share_clause_roles};
use segments::{clause_groups, segment_clause_words};

/// What a token is doing in its sentence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyntacticRole {
    Predicate,
    /// A negation or tense auxiliary governed by the clause predicate
    /// (`en` in `en syö`, `olen` in `olen lukenut`).
    Auxiliary,
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
            SyntacticRole::Auxiliary => "auxiliary",
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
    /// Morphology fits and word order supports it.
    Likely,
    /// Positional fallback, or an ambiguous word.
    Weak,
}

/// One clause candidate.
///
/// Phase 5 creates one clause per sentence. Later phases split a sentence
/// into coordinated and subordinate clauses and fill the role slots.
///
/// A clause may span more than one [`crate::sentence::SentenceSpan`] when the
/// sentence layer split at a parenthetical bracket: the syntax layer merges
/// bracket-only boundaries before segmenting, so `omena (pieni, makea) ja
/// banaani` is one clause even though the sentence layer sees three spans.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clause {
    /// Byte span in the source text.
    pub start: usize,
    /// Exclusive byte end.
    pub end: usize,
    /// Token indices. The range includes only the clause's words, not the
    /// separators around them.
    pub tokens: Range<usize>,
    /// The predicate token, when one could be found. In a compound tense this
    /// is the main verb; the auxiliary carries `head = predicate`.
    pub predicate: Option<usize>,
    /// The clause subject, when one could be found.
    pub subject: Option<usize>,
    /// The clause object, when one could be found.
    pub object: Option<usize>,
    /// The predicative or object complement, when one could be found
    /// (`Talo on kaunis`, `Hän näyttää väsyneeltä`).
    pub complement: Option<usize>,
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
    /// Nominal, clause and predicate lists found in the clauses.
    pub lists: Vec<crate::syntax_list::List>,
    /// One entry per input token, separators included.
    pub tokens: Vec<TokenSyntax>,
}

/// Analyze a token stream with the embedded lexicon.
///
/// This walks the same morphology engine the checker uses, so callers see the
/// engine's reading of every word. It is intentionally separate from
/// [`crate::sentence::analyze`] and must not run in the keystroke path.
///
/// Resolved words are kept in a capped thread-local cache, so re-analysing a
/// document does not repeat the morphology for the words it shares with the
/// previous analysis. Use [`analyze_with_cache`] when the caller owns a
/// document-scoped cache (the wasm textarea state does).
pub fn analyze(tokens: &[Token]) -> SyntaxAnalysis {
    thread_local! {
        static CACHE: RefCell<AnalysisCache> = RefCell::new(AnalysisCache::new());
    }
    CACHE.with_borrow_mut(|cache| {
        analyze_with(tokens, lemmas(), Catalogues::embedded(), cache)
    })
}

/// Analyze with a caller-owned word cache.
///
/// The cache key is the lowercased surface; morphology is independent of the
/// document, so a cache may outlive one text. It is capped and clears itself
/// when full.
pub fn analyze_with_cache(tokens: &[Token], cache: &mut AnalysisCache) -> SyntaxAnalysis {
    analyze_with(tokens, lemmas(), Catalogues::embedded(), cache)
}

/// The same analysis against a caller-supplied lexicon and catalogue.
///
/// Kept private for now: the crate's embedded data is the only production
/// source, and a public override would freeze an API before a caller needs it.
fn analyze_with(
    tokens: &[Token],
    lexicon: &[crate::kotus::Lemma<'_>],
    cat: &Catalogues,
    cache: &mut AnalysisCache,
) -> SyntaxAnalysis {
    let contexts = sentence::analyze(tokens);
    let sentences = sentence::spans(tokens);

    let mut out = vec![TokenSyntax::default(); tokens.len()];
    let mut nominal_lemmas: Vec<Option<String>> = vec![None; tokens.len()];

    // Morphology first: every later rule reads the same resolved readings.
    for (index, token) in tokens.iter().enumerate() {
        if !token.is_word() {
            continue;
        }
        let lower = token.text.to_lowercase();
        let analysis = cache.resolve_word(lexicon, cat, &lower);
        let mut readings = analysis.readings;
        let mut lemma = analysis.lemma.clone();
        nominal_lemmas[index] = analysis.nominal_lemma.clone();
        if readings.is_empty()
            && token.is_capitalised()
            && let Some(name) = crate::expand::proper_names::lemma(&token.text)
        {
            // The name table carries the case the generator saw
            // (`Helsingissä` is inessive). A curated extra or an unknown
            // name has no recorded case; the fallback gives the citation
            // spelling a nominative and leaves an inflected form case-less,
            // so it cannot become a subject or object by accident.
            add_reading(
                &mut readings,
                proper_name_reading(
                    &token.text,
                    name,
                    crate::expand::proper_names::case(&token.text),
                ),
            );
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

    let groups = clause_groups(tokens, &contexts);

    let mut clauses: Vec<Clause> = Vec::new();
    let mut clause_groups_of: Vec<usize> = Vec::new();
    for (group_index, words) in groups.iter().enumerate() {
        if words.is_empty() {
            continue;
        }
        for segment in segment_clause_words(words, tokens, &out, &contexts) {
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
                complement: None,
                negative: false,
                existential: false,
            });
            clause_groups_of.push(group_index);
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
    share_clause_roles(&mut clauses, &clause_groups_of, tokens, &out);

    let lists = crate::syntax_list::parse(tokens, &clauses, &out);
    SyntaxAnalysis {
        sentences,
        clauses,
        lists,
        tokens: out,
    }
}





#[cfg(test)]
mod tests {
    use super::*;
    use crate::tags::PartOfSpeech;
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

    fn group_count(text: &str) -> usize {
        let tokens = tokenise(text);
        let contexts = sentence::analyze(&tokens);
        clause_groups(&tokens, &contexts).len()
    }

    #[test]
    fn bracket_only_boundaries_merge_into_one_clause_group() {
        for text in [
            "Ostin omenoita, appelsiineja (pieniä, makeita) ja banaaneja.",
            "Teksti (tämä) jatkuu.",
        ] {
            assert_eq!(group_count(text), 1, "{text}");
        }
    }

    #[test]
    fn a_terminal_mark_after_a_bracket_still_ends_a_group() {
        for text in [
            "(Tämä on uusi virke.) Toinen virke.",
            "Teksti (tämä). Jatkuu.",
            "Teksti (tämä) jatkuu. Toinen virke.",
        ] {
            assert_eq!(group_count(text), 2, "{text}");
        }
    }

    /// The syntax grouping merges the bracket boundary, but the sentence spans
    /// the case layer sees must stay as they were.
    #[test]
    fn a_parenthetical_keeps_the_sentence_spans_but_not_the_clause_split() {
        let (_, analysis) =
            analyzed("Ostin omenoita, appelsiineja (pieniä, makeita) ja banaaneja.");
        assert_eq!(analysis.sentences.len(), 3);
        assert_eq!(analysis.clauses.len(), 1);
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
        assert_eq!(analysis.clauses[0].complement, Some(kaunis));
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
    fn a_top_level_semicolon_starts_a_clause() {
        let (_, analysis) = analyzed("Hän tuli; minä lähdin.");
        assert_eq!(analysis.clauses.len(), 2);
    }

    #[test]
    fn a_semicolon_inside_a_parenthetical_does_not_split() {
        let (_, analysis) = analyzed("Hän tuli (nopeasti; hiljaa) kotiin.");
        assert_eq!(analysis.clauses.len(), 1);
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
        let on = word_index(&tokens, "on");
        let rakennettu = word_index(&tokens, "rakennettu");
        assert_eq!(analysis.clauses[0].subject, None);
        assert_eq!(analysis.tokens[talo].role, SyntacticRole::Object);
        assert_eq!(analysis.tokens[on].role, SyntacticRole::Auxiliary);
        assert_eq!(analysis.tokens[on].head, Some(rakennettu));
        assert_eq!(analysis.tokens[rakennettu].role, SyntacticRole::Predicate);
    }

    #[test]
    fn a_passive_has_no_subject_but_keeps_the_logical_object() {
        let (tokens, analysis) = analyzed("Talo rakennettiin.");
        let talo = word_index(&tokens, "Talo");
        assert_eq!(analysis.clauses[0].subject, None);
        assert_eq!(analysis.tokens[talo].role, SyntacticRole::Object);
    }

    /// The name table records the case the generator saw; `Helsingissä` is an
    /// inessive adverbial, not a nominative subject.
    #[test]
    fn the_analysis_cache_returns_the_same_roles_and_reuses_words() {
        let tokens = tokenise("Sami söi puuron. Sami söi puuron.");
        let mut cache = AnalysisCache::new();
        let first = analyze_with_cache(&tokens, &mut cache);
        let words = cache.len();
        let second = analyze_with_cache(&tokens, &mut cache);
        assert_eq!(first, second);
        assert_eq!(cache.len(), words, "the second pass added no words");
        assert_eq!(words, 3, "sami, söi, puuron");
    }

    #[test]
    fn an_inflected_name_keeps_its_recorded_case() {
        let (tokens, analysis) = analyzed("Helsingissä asuva mies on opettaja.");
        let helsinki = word_index(&tokens, "Helsingissä");
        let mies = word_index(&tokens, "mies");
        assert_eq!(analysis.tokens[helsinki].case, Some(Case::Inessive));
        assert_eq!(analysis.tokens[helsinki].role, SyntacticRole::Adverbial);
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
