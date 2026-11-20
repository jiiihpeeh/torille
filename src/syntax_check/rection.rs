//! Verb rection and case selection from a curated valency table.
//!
//! Only verbs in `data/verb_rection.csv` are checked. A candidate argument is
//! the first nominal after the predicate that is not a complement, relative
//! pronoun or conjunction. If its case matches any frame the verb accepts,
//! there is no error; otherwise each frame contributes a correction, in table
//! priority order.

use crate::syntax::{SyntacticRole, SyntaxAnalysis};
use crate::tags::{Case, Mood, Person};
use crate::tokenise::Token;

use super::analysis::{
    candidate_number, clause_words, comma_between, is_imperative, is_marker, looks_like_address,
    nominal_case, nominal_or_numeral_pos,
};
use super::correction;
use super::diagnostic::{
    Confidence, DiagnosticKind, Severity, SyntaxDiagnostic,
};

pub(super) fn check(tokens: &[Token], analysis: &SyntaxAnalysis) -> Vec<SyntaxDiagnostic> {
    let mut diagnostics = Vec::new();
    for (clause_index, clause) in analysis.clauses.iter().enumerate() {
        let Some(predicate) = clause.predicate else {
            continue;
        };
        let Some(lemma) = analysis.tokens[predicate].lemma.as_deref() else {
            continue;
        };
        let frames = crate::expand::verb_rection::cases(lemma);
        if frames.is_empty() {
            continue;
        }
        let Some((argument, case)) =
            argument(clause, clause_index, predicate, tokens, analysis)
        else {
            continue;
        };
        if frames.contains(&case) {
            continue;
        }
        // The frame is already satisfied by another nominal in the clause:
        // `uskon sinut … haltuun` has the illative goal even though `haltuun`
        // is a postposition with no case reading. A closed set keeps the
        // transfer goal from being mistaken for a bad object.
        if frames.iter().any(|frame| {
            clause.tokens.clone().any(|index| {
                index != argument
                    && index != predicate
                    && (analysis.tokens[index].readings.iter().any(|reading| {
                        reading.case == Some(*frame)
                            && crate::syntax::is_nominal_pos(reading.part_of_speech)
                    }) || (frame == &Case::Illative
                        && analysis.tokens[index].key == "haltuun"))
            })
        }) {
            continue;
        }
        // A partitive object frame for these verbs is aspectual, not lexical:
        // `Katson elokuvaa` and `Katson elokuvan` are both correct, and
        // `Hän pitää puheen` is a total-object frame the table does not list.
        // A genitive/accusative argument is therefore not evidence of a
        // rection error. A bare nominative object stays an error because it
        // needs an imperative/passive/necessive context.
        if aspectual_total_object(lemma, case) {
            continue;
        }
        let syntax = &analysis.tokens[argument];
        if nominal_or_numeral_pos(syntax).is_none() {
            continue;
        }
        let number = candidate_number(syntax, case);
        let mut corrections = Vec::new();
        for (lemma_position, (lemma, base_pos)) in correction_bases(syntax).iter().enumerate() {
            for (priority, frame) in frames.iter().enumerate() {
                for mut correction in correction::case_corrections(
                    lemma,
                    &tokens[argument],
                    *frame,
                    number,
                    *base_pos,
                    crate::spec::Catalogues::embedded(),
                ) {
                    // Keep the CSV frame order ahead of edit distance, and the
                    // chosen lemma's forms ahead of a homonym's.
                    correction.score +=
                        priority as f32 * 100.0 + lemma_position as f32 * 1000.0;
                    corrections.push(correction);
                }
            }
        }
        corrections.sort_by(|a, b| {
            a.score
                .total_cmp(&b.score)
                .then_with(|| a.edits[0].replacement.cmp(&b.edits[0].replacement))
        });
        corrections.dedup_by(|a, b| a.edits[0].replacement == b.edits[0].replacement);
        let confident = analysis.tokens[predicate].confidence
            == crate::syntax::RoleConfidence::Likely;
        let (severity, confidence) = if confident {
            (Severity::Error, Confidence::Certain)
        } else {
            (Severity::Warning, Confidence::Likely)
        };
        let mut diagnostic = SyntaxDiagnostic::new(
            DiagnosticKind::VerbRection,
            severity,
            confidence,
            tokens[argument].start..tokens[argument].end,
            "verb_rection",
        );
        diagnostic.expected = Some(
            frames
                .iter()
                .map(|case| case.name())
                .collect::<Vec<_>>()
                .join(" / "),
        );
        diagnostic.corrections = corrections;
        diagnostics.push(diagnostic);
    }
    diagnostics
}

/// Every nominal lemma behind the argument surface, chosen lemma first.
///
/// A homonymous surface (`kirjoja`, `tuli`) belongs to more than one base. The
/// syntax layer records them in [`crate::syntax::TokenSyntax::lemmas`], so the
/// rection rule can offer the right paradigm without re-running the base
/// search. Only rows the sanalista marks nominal or numeral contribute.
fn correction_bases(syntax: &crate::syntax::TokenSyntax) -> Vec<(&str, crate::tags::PartOfSpeech)> {
    use crate::tags::PartOfSpeech;

    let mut ordered: Vec<&str> = Vec::new();
    if let Some(lemma) = syntax.lemma.as_deref() {
        ordered.push(lemma);
    }
    for lemma in &syntax.lemmas {
        let lemma = lemma.as_str();
        if !ordered.contains(&lemma) {
            ordered.push(lemma);
        }
    }

    let mut out = Vec::new();
    for lemma in ordered {
        for row in crate::kotus::sanalista().by_word(lemma) {
            let Some(pos) = PartOfSpeech::parse_all(row.part_of_speech())
                .first()
                .copied()
            else {
                continue;
            };
            if matches!(
                pos,
                PartOfSpeech::Noun
                    | PartOfSpeech::Adjective
                    | PartOfSpeech::Pronoun
                    | PartOfSpeech::Numeral
                    | PartOfSpeech::Ordinal
            ) {
                out.push((lemma, pos));
                break;
            }
        }
    }
    out
}

/// Is this adjective a postposed attribute of the nominal before it?
///
/// Finnish allows the quantifier construction `jotain erinomaista`, where the
/// partitive adjective follows the indefinite pronoun it modifies. A governed
/// argument is a nominal in its own right, so an adjective directly after a
/// nominal is that nominal's attribute (or complement) and never the
/// argument: `Lapsesta tulee varmaan jotain erinomaista` is not a rection
/// error on `erinomaista`.
fn is_postposed_attribute(
    index: usize,
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
) -> bool {
    use crate::tags::PartOfSpeech;

    if !analysis.tokens[index]
        .readings
        .iter()
        .any(|reading| reading.part_of_speech == Some(PartOfSpeech::Adjective))
    {
        return false;
    }
    let previous = (0..index).rev().find(|&word| tokens[word].is_word());
    previous.is_some_and(|previous| nominal_or_numeral_pos(&analysis.tokens[previous]).is_some())
}

/// Is this `Subject`-role token really the clause subject rather than a
/// mislabelled governed argument?
///
/// The test is agreement, not the role alone. A role the pass assigned from a
/// nominative fallback is not evidence that the word is the subject, and a
/// rection rewrite of a real subject is always a false positive.
///
/// - an existential's postverbal nominative/partitive is its subject even
///   though the verb does not agree (`Pöydällä on kirjat`);
/// - more than one subject role means coordinated subjects, where the verb is
///   plural and the clause names only the first (`Naapurissa asuvat Liisa ja
///   Pekka`);
/// - a personal pronoun must agree in person and number (`Tuleeko hän?`);
/// - a nominal needs a 3rd-person verb and the same number. `Sitten auttoi
///   kreivi ...` and `alkoi tulla ... puute` are subjects this removes, while
///   `Menen koulu` is not: a 1st-person verb cannot take a noun subject, so
///   the nominative there is the rection error the rule exists to catch.
fn is_genuine_subject(
    clause: &crate::syntax::Clause,
    clause_index: usize,
    index: usize,
    predicate: usize,
    analysis: &SyntaxAnalysis,
) -> bool {
    use super::agreement::{finite_reading, pronoun_person_number};

    if clause.existential {
        return true;
    }
    let subjects = clause
        .tokens
        .clone()
        .filter(|&token| analysis.tokens[token].role == SyntacticRole::Subject)
        .count();
    if subjects > 1 {
        return true;
    }
    // An infinitive clause has no finite predicate of its own; its subject is
    // the one the governing verb already agreed with. `alkoi tulla ...
    // puute` agrees with `alkoi`, and `Haluan mennä koulu` does not: a
    // 1st-person verb cannot take the noun `koulu` as its subject, so the
    // nominative there is the rection error.
    let reading = finite_reading(&analysis.tokens[predicate]).or_else(|| {
        clause_index
            .checked_sub(1)
            .and_then(|previous| analysis.clauses.get(previous))
            .and_then(|previous| previous.predicate)
            .and_then(|previous| finite_reading(&analysis.tokens[previous]))
    });
    let Some(reading) = reading else {
        return false;
    };
    if let Some((person, number)) = pronoun_person_number(&analysis.tokens[index].key) {
        return reading.person == Some(person) && reading.number == Some(number);
    }
    // The agreed number of the surface, from the case the role licensed. An
    // ambiguous number stays a candidate: the rule offers a rewrite, so the
    // ambiguous direction is silence.
    let number = analysis.tokens[index]
        .case
        .and_then(|case| candidate_number(&analysis.tokens[index], case));
    match (reading.person, number) {
        (Some(Person::Third), Some(number)) => reading.number == Some(number),
        _ => false,
    }
}

/// Verbs whose table frame is aspectual rather than lexical.
///
/// The table records the partitive object these verbs commonly take, but
/// Finnish also allows the total object when the action is completive:
/// `Katson elokuvaa` / `Katson elokuvan`, `Hän pitää kirjaa` /
/// `Hän pitää kirjan`. A genitive or accusative argument is therefore not a
/// rection error. This is a stopgap until the CSV carries a frame kind.
const ASPECTUAL_OBJECT_VERBS: &[&str] = &[
    "auttaa",
    "etsiä",
    "katsoa",
    "kuunnella",
    "käyttää",
    "odottaa",
    "pitää",
    "tarvita",
];

fn aspectual_total_object(lemma: &str, case: Case) -> bool {
    matches!(case, Case::Genitive | Case::Accusative)
        && ASPECTUAL_OBJECT_VERBS.contains(&lemma)
}

/// Is the first word after this clause an infinitive? The segmenter gives an
/// infinitive its own clause, so the matrix verb's last nominal is the
/// infinitive's argument.
fn next_word_is_infinitive(
    clause: &crate::syntax::Clause,
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
) -> bool {
    (clause.tokens.end..tokens.len())
        .find(|&index| tokens[index].is_word())
        .is_some_and(|index| {
            analysis.tokens[index]
                .readings
                .iter()
                .any(|reading| reading.mood == Some(Mood::Infinitive))
        })
}

/// The demonstrative pronouns that can open a clause after a comma.
const DEMONSTRATIVE_PRONOUNS: &[&str] = &["se", "tämä", "tuo", "ne", "nämä", "nuo"];

/// The first post-predicate nominal that can be the governed argument.
///
/// A *genuine* subject is never the governed argument, so it is skipped; see
/// [`is_genuine_subject`]. The subject role is still a candidate when the role
/// pass mislabelled a required argument as one, which is exactly the error this
/// rule exists to catch (`Menen koulu`, `Pidän sinä`).
fn argument(
    clause: &crate::syntax::Clause,
    clause_index: usize,
    predicate: usize,
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
) -> Option<(usize, Case)> {
    let words = clause_words(clause, tokens);
    let predicate_position = words.iter().position(|&index| index == predicate)?;
    for &index in words.iter().skip(predicate_position + 1) {
        let syntax = &analysis.tokens[index];
        if !matches!(
            syntax.role,
            SyntacticRole::Object | SyntacticRole::Subject | SyntacticRole::Unknown
        ) || is_marker(&analysis.tokens[index].key)
        {
            continue;
        }
        if syntax.role == SyntacticRole::Subject
            && is_genuine_subject(clause, clause_index, index, predicate, analysis)
        {
            continue;
        }
        // An address after an imperative is not the verb's argument:
        // `Tule tänne Pekka!` must not be rewritten as an illative complement.
        if syntax.role == SyntacticRole::Unknown
            && is_imperative(&analysis.tokens[predicate])
            && looks_like_address(index, tokens, analysis)
        {
            continue;
        }
        // A comma before a demonstrative pronoun starts a new clause
        // (`... katsokaa, se peitäkse ...`); the pronoun is that clause's
        // subject, not this verb's object. The predicate is separated from
        // the candidate by the comma, so it cannot govern it.
        if DEMONSTRATIVE_PRONOUNS.contains(&syntax.key.as_str())
            && comma_between(tokens, predicate, index)
        {
            continue;
        }
        if is_postposed_attribute(index, tokens, analysis) {
            continue;
        }
        let Some(case) = nominal_case(syntax) else {
            continue;
        };
        if nominal_or_numeral_pos(syntax).is_none() {
            continue;
        }
        // A clause-final nominal before an infinitive clause belongs to the
        // infinitive (`lähti niitä seuraamaan`), not to the matrix verb.
        if words.last() == Some(&index) && next_word_is_infinitive(clause, tokens, analysis) {
            continue;
        }
        return Some((index, case));
    }
    None
}


