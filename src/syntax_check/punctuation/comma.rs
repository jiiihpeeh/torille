//! Clause-boundary comma diagnostics.

use crate::syntax::{Clause, SyntacticRole, SyntaxAnalysis};
use crate::tokenise::Token;

use super::super::analysis::{comma_between, first_word, is_finite, last_word};
use super::super::diagnostic::{
    Confidence, Correction, CorrectionLicense, DiagnosticKind, Edit, Severity, SyntaxDiagnostic,
};
use super::clause::{ClauseKind, clause_kind};

pub(super) fn clause_diagnostics(
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
) -> Vec<SyntaxDiagnostic> {
    let mut out = Vec::new();
    for (index, clause) in analysis.clauses.iter().enumerate() {
        let kind = clause_kind(clause, tokens, analysis);
        if kind == ClauseKind::Subordinate {
            out.extend(subordinate_comma(index, clause, kind, tokens, analysis));
        }
        if kind == ClauseKind::Relative {
            out.extend(relative_comma(clause, tokens, analysis));
        }
    }
    out.extend(extra_comma(tokens, analysis));
    out
}

/// The personal-pronoun subjects where a comma before the predicate is not an
/// apposition or a parenthetical.
const PERSONAL_SUBJECTS: &[&str] = &["minä", "sinä", "hän", "me", "te", "he"];

/// Fixed multiword conjunctions and adverbials whose halves take no comma.
///
/// `niin että` is the consecutive conjunction (`..., niin että kaikki
/// sanoivat`); `sen kun` is the fixed adverbial “kept on …”. `siten, että` is
/// deliberately absent: it does take the comma.
const NO_COMMA_PAIRS: &[(&str, &str)] = &[("niin", "että"), ("sen", "kun")];

/// `Minä, menen kotiin.`: a comma directly between a personal-pronoun subject
/// and its finite predicate.
///
/// The pattern is deliberately narrow. A noun subject can be an apposition or
/// an address (`Liisa, tule tänne`), and any intervening word (`Minä, kuten
/// tiedät, menen`) makes the comma a parenthetical. A deliberate pause is the
/// remaining competitor, so the finding is a `Warning` and the removal is a
/// `Likely` correction, never an `Error`.
fn extra_comma(tokens: &[Token], analysis: &SyntaxAnalysis) -> Vec<SyntaxDiagnostic> {
    let mut out = Vec::new();
    for clause in &analysis.clauses {
        let (Some(subject), Some(predicate)) = (clause.subject, clause.predicate) else {
            continue;
        };
        if subject >= predicate
            || analysis.tokens[subject].role != SyntacticRole::Subject
            || !PERSONAL_SUBJECTS.contains(&analysis.tokens[subject].key.as_str())
            || !is_finite(&analysis.tokens[predicate])
        {
            continue;
        }
        let Some(comma) = comma_between_index(tokens, subject, predicate) else {
            continue;
        };
        if tokens[subject + 1..predicate].iter().any(Token::is_word) {
            continue;
        }
        let token = &tokens[comma];
        let mut diagnostic = SyntaxDiagnostic::new(
            DiagnosticKind::CommaExtra,
            Severity::Warning,
            Confidence::Likely,
            token.start..token.end,
            "comma_extra",
        );
        diagnostic.expected = Some("no comma".to_string());
        diagnostic.corrections = vec![
            Correction::new(
                vec![Edit::new(token.start, token.end, "")],
                0.0,
                "comma_extra",
            )
            .licensed_by(CorrectionLicense::Punctuation)
            .with_confidence(Confidence::Likely),
        ];
        out.push(diagnostic);
    }
    out
}

fn comma_between_index(tokens: &[Token], left: usize, right: usize) -> Option<usize> {
    let (start, end) = if left < right {
        (left + 1, right)
    } else {
        (right + 1, left)
    };
    (start..end).find(|&index| !tokens[index].is_word() && tokens[index].text.contains(','))
}

/// A subordinate clause after a main clause, or a main clause after an initial
/// subordinate clause, needs a comma at the boundary. A merged initial
/// subordinate clause (`Koska olin väsynyt menin kotiin`) is split at its last
/// finite predicate because the clause parser could not see the boundary.
fn subordinate_comma(
    index: usize,
    clause: &Clause,
    _kind: ClauseKind,
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
) -> Vec<SyntaxDiagnostic> {
    let mut out = Vec::new();
    // A `kun`/`jos` clause whose boundary is already marked by a comma, a
    // dash, or a sentence-initial `ja` needs no second comma inside it. This
    // is the prose pattern `..., jos ...`, `--kun ...`, and `Ja kun ...`.
    if let Some(first) = first_word(clause, tokens) {
        let concord =
            |index: usize| matches!(tokens[index].text.to_lowercase().as_str(), "kun" | "jos");
        let marked = |index: usize| {
            previous_word(tokens, index).is_some_and(|previous| {
                comma_between(tokens, previous, index)
                    || dash_between(tokens, previous, index)
                    || tokens[previous].text.eq_ignore_ascii_case("ja")
            })
        };
        if (concord(first) && marked(first))
            || previous_word(tokens, first)
                .is_some_and(|previous| concord(previous) && marked(previous))
        {
            return out;
        }
    }
    if index > 0 && same_sentence(&analysis.clauses[index - 1], clause, tokens, analysis) {
        let previous = &analysis.clauses[index - 1];
        if let (Some(left), Some(right)) = (last_word(previous, tokens), first_word(clause, tokens))
        {
            let previous_kind = clause_kind(previous, tokens, analysis);
            let boundary_needs_comma = previous_kind != ClauseKind::Subordinate
                || clause_kind(clause, tokens, analysis) == ClauseKind::Subordinate;
            if boundary_needs_comma
                && !is_no_comma_pair(tokens, left, right)
                && !comparison_kuin_between(tokens, left, right)
                && !comma_between(tokens, left, right)
                && !dash_between(tokens, left, right)
            {
                out.push(comma_missing(
                    tokens[left].end,
                    DiagnosticKind::CommaMissing,
                ));
            }
        }
    }
    if let Some(next) = analysis.clauses.get(index + 1)
        && same_sentence(clause, next, tokens, analysis)
        && let (Some(left), Some(right)) = (last_word(clause, tokens), first_word(next, tokens))
        && clause_kind(next, tokens, analysis) == ClauseKind::Main
        && !is_no_comma_pair(tokens, left, right)
        && !comparison_kuin_between(tokens, left, right)
        && !comma_between(tokens, left, right)
        && !dash_between(tokens, left, right)
    {
        out.push(comma_missing(
            tokens[left].end,
            DiagnosticKind::CommaMissing,
        ));
    }

    // The subordinate clause may still be merged with its main clause in one
    // `Clause`: `Koska olin väsynyt menin kotiin`. Use the last finite
    // predicate as the main clause's head and insert the comma before it.
    let finite = finite_words(clause, tokens, analysis);
    if finite.len() >= 2
        && let Some(&main) = finite.last()
        && let Some(&previous) = finite.get(finite.len() - 2)
        // A comma between the two finite words already marks the boundary
        // (`Kun se aamu valkeni, jolloin …`, `..., sen kun Anni vain kertoi,
        // koko ajan …`). The merged-subordinate path must not add another.
        && !comparison_kuin_between(tokens, previous, main)
        && !comma_between(tokens, previous, main)
        && let Some(before) = previous_word(tokens, main)
        && !comma_between(tokens, before, main)
        && !dash_between(tokens, before, main)
    {
        out.push(comma_missing(
            tokens[before].end,
            DiagnosticKind::CommaMissing,
        ));
    }
    out
}

/// A relative clause that already has an opening comma and is followed by its
/// host predicate needs the closing comma. A restrictive relative without an
/// opening comma is left quiet.
fn relative_comma(
    clause: &Clause,
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
) -> Vec<SyntaxDiagnostic> {
    let Some(first) = first_word(clause, tokens) else {
        return Vec::new();
    };
    let Some(antecedent) = previous_word(tokens, first) else {
        return Vec::new();
    };
    if !comma_between(tokens, antecedent, first) {
        return Vec::new();
    }
    let finite = finite_predicate_words(clause, tokens, analysis);
    let Some(&first_predicate) = finite.first() else {
        return Vec::new();
    };
    // Without the closing comma the parser keeps the host predicate inside
    // the relative clause (`Mies, joka tuli söi`). The relative's own subject
    // is then the finite word before that predicate; the missing delimiter
    // belongs after it.
    if let Some(before) = previous_word(tokens, first_predicate)
        && before > first
        && is_finite(&analysis.tokens[before])
        && !matches!(
            analysis.tokens[before].role,
            SyntacticRole::Predicate | SyntacticRole::Auxiliary
        )
        && !comma_between(tokens, before, first_predicate)
        && !dash_between(tokens, before, first_predicate)
        && !line_break_between(tokens, before, first_predicate)
    {
        return vec![comma_missing(
            tokens[before].end,
            DiagnosticKind::CommaPairUnbalanced,
        )];
    }
    if finite.len() < 2 {
        return Vec::new();
    }
    let first_finite = finite[0];
    let host = *finite.last().unwrap_or(&first_finite);
    if host == first_finite || coordinated_between(tokens, first_finite, host) {
        return Vec::new();
    }
    let Some(before) = previous_word(tokens, host) else {
        return Vec::new();
    };
    if comma_between(tokens, before, host) {
        return Vec::new();
    }
    vec![comma_missing(
        tokens[before].end,
        DiagnosticKind::CommaPairUnbalanced,
    )]
}

/// A dash is a clause boundary in the same places a comma is; do not ask for a
/// comma after one.
fn dash_between(tokens: &[Token], left: usize, right: usize) -> bool {
    if left == right {
        return false;
    }
    let (start, end) = if left < right {
        (left + 1, right)
    } else {
        (right + 1, left)
    };
    tokens[start..end]
        .iter()
        .filter(|token| !token.is_word())
        .any(|token| {
            token.text.contains("--") || token.text.contains('—') || token.text.contains('–')
        })
}

fn line_break_between(tokens: &[Token], left: usize, right: usize) -> bool {
    if left == right {
        return false;
    }
    let (start, end) = if left < right {
        (left + 1, right)
    } else {
        (right + 1, left)
    };
    tokens[start..end]
        .iter()
        .filter(|token| !token.is_word())
        .any(|token| token.text.contains('\n'))
}

fn comma_missing(at: usize, kind: DiagnosticKind) -> SyntaxDiagnostic {
    let message = if kind == DiagnosticKind::CommaPairUnbalanced {
        "comma_pair_unbalanced"
    } else {
        "comma_missing"
    };
    let mut diagnostic =
        SyntaxDiagnostic::new(kind, Severity::Error, Confidence::Certain, at..at, message);
    diagnostic.corrections = vec![
        Correction::new(vec![Edit::insertion(at, ",")], 0.0, "comma_missing")
            .licensed_by(CorrectionLicense::Punctuation)
            .with_confidence(Confidence::Certain),
    ];
    diagnostic
}

/// Whether a comparison `kuin` lies between two finite words.
///
/// `niin kauan kuin kaikki ovat yhtä köyhiä` is one clause to the parser: the
/// `kuin` is an adverbial, so the clause has two finite predicates (`on`,
/// `ovat`) and the merged-subordinate heuristic would insert a comma before
/// `ovat`. The `kuin` is a comparison boundary, not a main-clause boundary,
/// and Finnish does not put a comma there.
fn comparison_kuin_between(tokens: &[Token], left: usize, right: usize) -> bool {
    if left == right {
        return false;
    }
    let (start, end) = if left < right {
        (left + 1, right)
    } else {
        (right + 1, left)
    };
    tokens[start..end]
        .iter()
        .any(|token| token.text.eq_ignore_ascii_case("kuin"))
}

fn previous_word(tokens: &[Token], index: usize) -> Option<usize> {
    (0..index)
        .rev()
        .find(|&candidate| tokens[candidate].is_word())
}

fn finite_predicate_words(
    clause: &Clause,
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
) -> Vec<usize> {
    clause
        .tokens
        .clone()
        .filter(|&index| {
            tokens[index].is_word()
                && matches!(
                    analysis.tokens[index].role,
                    SyntacticRole::Predicate | SyntacticRole::Auxiliary
                )
                && is_finite(&analysis.tokens[index])
        })
        .collect()
}

fn finite_words(clause: &Clause, tokens: &[Token], analysis: &SyntaxAnalysis) -> Vec<usize> {
    clause
        .tokens
        .clone()
        .filter(|&index| tokens[index].is_word() && is_finite(&analysis.tokens[index]))
        .collect()
}

fn coordinated_between(tokens: &[Token], left: usize, right: usize) -> bool {
    let (start, end) = if left < right {
        (left + 1, right)
    } else {
        (right + 1, left)
    };
    tokens[start..end]
        .iter()
        .filter(|token| token.is_word())
        .any(|token| {
            matches!(
                token.text.to_lowercase().as_str(),
                "ja" | "sekä"
                    | "tai"
                    | "vai"
                    | "eikä"
                    | "enkä"
                    | "etkä"
                    | "emme"
                    | "ette"
                    | "eivätkä"
            )
        })
}

fn same_sentence(
    left: &Clause,
    right: &Clause,
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
) -> bool {
    if sentence_index(left.start, analysis) != sentence_index(right.start, analysis) {
        return false;
    }
    // The sentence layer does not end a sentence at a terminal mark followed
    // by an apostrophe quote (`. 'Jos …`), so the spans can merge two
    // sentences. The comma rules must not reach across a terminal mark
    // between the clauses even when the spans did.
    !terminal_between(left, right, tokens)
}

fn terminal_between(left: &Clause, right: &Clause, tokens: &[Token]) -> bool {
    let (Some(left_word), Some(right_word)) = (last_word(left, tokens), first_word(right, tokens))
    else {
        return false;
    };
    let (start, end) = if left_word < right_word {
        (left_word + 1, right_word)
    } else {
        (right_word + 1, left_word)
    };
    tokens[start..end]
        .iter()
        .filter(|token| !token.is_word())
        .any(|token| token.text.chars().any(|c| matches!(c, '.' | '!' | '?')))
}

/// A fixed multiword pair whose halves take no comma (`niin että`,
/// `sen kun`).
fn is_no_comma_pair(tokens: &[Token], left: usize, right: usize) -> bool {
    let left = tokens[left].text.to_lowercase();
    let right = tokens[right].text.to_lowercase();
    NO_COMMA_PAIRS
        .iter()
        .any(|&(first, second)| left == first && right == second)
}

fn sentence_index(byte: usize, analysis: &SyntaxAnalysis) -> Option<usize> {
    analysis
        .sentences
        .iter()
        .find(|sentence| byte >= sentence.start && byte < sentence.end)
        .map(|sentence| sentence.index)
}
