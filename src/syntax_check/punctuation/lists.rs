//! List structure and list-separator diagnostics.
//!
//! The comma-only rules (duplicate separator, comma before conjunction,
//! introducer comma) read the comma characters in the gap between two items.
//! A semicolon-separated list has no comma in that gap by construction, so
//! those rules stay quiet there without a separate guard.

use crate::syntax::{SyntacticRole, SyntaxAnalysis, TokenSyntax};
use crate::syntax_list::{List, ListItem, ListItemKind, ListKind, ListSeparator};
use crate::tags::{Case, PartOfSpeech};
use crate::tokenise::Token;

use super::super::analysis::{is_finite, nominal_case};
use super::super::diagnostic::{
    Confidence, Correction, CorrectionLicense, DiagnosticKind, Edit, Severity, SyntaxDiagnostic,
};

pub(super) fn list_diagnostics(tokens: &[Token], analysis: &SyntaxAnalysis) -> Vec<SyntaxDiagnostic> {
    let mut out = Vec::new();
    for list in analysis.lists.iter().filter(|list| list.kind == ListKind::Nominal) {
        let Some(first_item) = list.items.first() else {
            continue;
        };

        if let Some(colon) = list.introducer {
            let commas = comma_positions(tokens, colon, first_item.tokens.start);
            if let Some(&(start, _)) = commas.first() {
                out.push(remove_comma(
                    DiagnosticKind::ListIntroducerComma,
                    Severity::Error,
                    start,
                    "list_introducer_comma",
                ));
            }
        }

        for pair in list.items.windows(2) {
            let [left, right] = pair else {
                continue;
            };
            let commas = comma_positions(tokens, left.tokens.end, right.tokens.start);
            if commas.len() >= 2 {
                let (start, _) = commas[1];
                out.push(remove_comma(
                    DiagnosticKind::ListSeparatorDuplicate,
                    Severity::Error,
                    start,
                    "list_separator_duplicate",
                ));
            }
        }

        for item in &list.items {
            out.extend(missing_separators(
                item,
                tokens,
                &analysis.tokens,
                &analysis.lists,
            ));
        }

        if let Some(conjunction) = list.conjunction {
            let previous = list
                .items
                .iter()
                .rev()
                .find(|item| item.tokens.end <= conjunction);
            if let Some(previous) = previous {
                let commas = comma_positions(tokens, previous.tokens.end, conjunction);
                if commas.len() == 1 {
                    let (start, _) = commas[0];
                    out.push(remove_comma(
                        DiagnosticKind::ListCommaBeforeConjunction,
                        Severity::Suggestion,
                        start,
                        "list_comma_before_conjunction",
                    ));
                }
            }
        }

        if let Some(diagnostic) = simple_semicolon_list(tokens, analysis, list) {
            out.push(diagnostic);
        }
        if let Some(diagnostic) = missing_semicolons(tokens, analysis, list) {
            out.push(diagnostic);
        }
    }
    out
}

/// `Ostin omenoita; banaaneja; appelsiineja.` — a short parallel list should
/// use commas.
///
/// Only three or more short, comma-free, same-case nominal items after a verb
/// or a colon qualify. A complex list (an item contains its own comma) and a
/// list of clauses keep the semicolon hierarchy, and a valid main-main
/// semicolon is not a list at all.
fn simple_semicolon_list(
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
    list: &List,
) -> Option<SyntaxDiagnostic> {
    if list.kind != ListKind::Nominal || list.items.len() < 3 {
        return None;
    }
    if list.separator_kinds.len() != list.items.len() - 1
        || list
            .separator_kinds
            .iter()
            .any(|kind| *kind != ListSeparator::Semicolon)
    {
        return None;
    }
    let governed = list.introducer.is_some()
        || matches!(
            list.role,
            Some(SyntacticRole::Object | SyntacticRole::Complement | SyntacticRole::Adverbial)
        );
    if !governed {
        return None;
    }

    // Every item must be short, comma-free and case-compatible with the rest.
    let mut shared: Option<Vec<Case>> = None;
    for item in &list.items {
        let words: Vec<usize> = item
            .tokens
            .clone()
            .filter(|&index| tokens[index].is_word())
            .collect();
        if words.is_empty() || words.len() > 2 || item.kind == ListItemKind::Clause {
            return None;
        }
        if tokens[item.tokens.clone()]
            .iter()
            .any(|token| !token.is_word() && token.text.contains(','))
        {
            return None;
        }
        let cases = nominal_cases(&analysis.tokens[words[0]]);
        if cases.is_empty() {
            return None;
        }
        shared = Some(match shared {
            Some(previous) => previous
                .into_iter()
                .filter(|case| cases.contains(case))
                .collect(),
            None => cases,
        });
        if shared.as_ref().is_some_and(Vec::is_empty) {
            return None;
        }
    }
    shared.as_ref()?;

    let mut edits = Vec::new();
    for &index in &list.separators {
        let offset = tokens[index].text.find(';')?;
        let start = tokens[index].start + offset;
        edits.push(Edit::new(start, start + 1, ","));
    }
    let first = edits.first()?;
    let last = edits.last()?;
    let mut diagnostic = SyntaxDiagnostic::new(
        DiagnosticKind::SemicolonMisuse,
        Severity::Suggestion,
        Confidence::Likely,
        first.start..last.end,
        "semicolon_simple_list",
    );
    diagnostic.corrections = vec![
        Correction::new(edits, 0.0, "semicolon_simple_list")
            .licensed_by(CorrectionLicense::Punctuation)
            .with_confidence(Confidence::Likely),
    ];
    Some(diagnostic)
}

/// `Virtanen, toimitusjohtaja; Laine, projektipäällikkö, Korhonen,
/// talousjohtaja.` — a mixed complex list where one group already uses a
/// semicolon, so the repeated group shape is known.
///
/// The first item must be the model group (`k` words). Every item must be a
/// whole multiple of `k`, every chunk must be nominal and predicate-free, and
/// each missing outer boundary must be exactly one comma. A fully
/// comma-separated list has no model group and stays quiet; a group that does
/// not fit the model stays quiet. The correction replaces every outer comma
/// with a semicolon in one edit set.
fn missing_semicolons(
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
    list: &List,
) -> Option<SyntaxDiagnostic> {
    if list.kind != ListKind::Nominal || list.items.len() < 2 {
        return None;
    }
    if !list.separator_kinds.contains(&ListSeparator::Semicolon) {
        return None;
    }
    if list.introducer.is_none() && list.role.is_none() {
        return None;
    }
    if list
        .items
        .iter()
        .any(|item| matches!(item.kind, ListItemKind::List(_)))
    {
        return None;
    }

    let first = item_words(tokens, list.items.first()?);
    let width = first.len();
    if width < 2 {
        return None;
    }

    let mut edits = Vec::new();
    let mut internal_comma = false;
    for item in &list.items {
        let words = item_words(tokens, item);
        if words.is_empty() || !words.len().is_multiple_of(width) {
            return None;
        }
        if words.iter().any(|&word| is_coordinator(word, tokens)) {
            return None;
        }
        let commas =
            crate::syntax_list::top_level_commas(tokens, item.tokens.start, item.tokens.end);
        if !commas.is_empty() {
            internal_comma = true;
        }
        let chunks: Vec<&[usize]> = words.chunks(width).collect();
        for pair in chunks.windows(2) {
            let left = *pair[0].last()?;
            let right = *pair[1].first()?;
            if !chunk_is_nominal(pair[0], analysis) || !chunk_is_nominal(pair[1], analysis) {
                return None;
            }
            let boundary: Vec<usize> = commas
                .iter()
                .copied()
                .filter(|&index| index > left && index < right)
                .collect();
            if boundary.len() != 1 {
                return None;
            }
            let offset = tokens[boundary[0]].text.find(',')?;
            let start = tokens[boundary[0]].start + offset;
            edits.push(Edit::new(start, start + 1, ";"));
        }
    }
    if !internal_comma || edits.is_empty() {
        return None;
    }

    let first = edits.first()?;
    let last = edits.last()?;
    let mut diagnostic = SyntaxDiagnostic::new(
        DiagnosticKind::SemicolonMissing,
        Severity::Suggestion,
        Confidence::Likely,
        first.start..last.end,
        "semicolon_list_hierarchy",
    );
    diagnostic.corrections = vec![
        Correction::new(edits, 0.0, "semicolon_list_hierarchy")
            .licensed_by(CorrectionLicense::Punctuation)
            .with_confidence(Confidence::Likely),
    ];
    Some(diagnostic)
}

fn item_words(tokens: &[Token], item: &ListItem) -> Vec<usize> {
    item.tokens
        .clone()
        .filter(|&index| tokens[index].is_word())
        .collect()
}

fn chunk_is_nominal(chunk: &[usize], analysis: &SyntaxAnalysis) -> bool {
    chunk.iter().any(|&index| {
        analysis.tokens[index]
            .readings
            .iter()
            .any(|reading| crate::syntax::is_nominal_pos(reading.part_of_speech))
    }) && chunk
        .iter()
        .all(|&index| !is_finite(&analysis.tokens[index]))
}

fn is_coordinator(index: usize, tokens: &[Token]) -> bool {
    crate::syntax::is_coordinating(&tokens[index].text.to_lowercase())
}

/// The nominal readings' cases, without the verb homonym's case.
///
/// `osaamista` is a noun partitive and a verb elative; the syntax layer may
/// choose the verb reading's case, so the list comparison reads the nominal
/// readings only.
fn nominal_cases(token: &TokenSyntax) -> Vec<Case> {
    let mut cases = Vec::new();
    for reading in &token.readings {
        if !matches!(
            reading.part_of_speech,
            Some(
                PartOfSpeech::Noun
                    | PartOfSpeech::Adjective
                    | PartOfSpeech::Pronoun
                    | PartOfSpeech::Numeral
                    | PartOfSpeech::Ordinal
            )
        ) {
            continue;
        }
        if let Some(case) = reading.case
            && !cases.contains(&case)
        {
            cases.push(case);
        }
    }
    cases
}

fn missing_separators(
    item: &ListItem,
    tokens: &[Token],
    out: &[TokenSyntax],
    lists: &[List],
) -> Vec<SyntaxDiagnostic> {
    let words: Vec<usize> = item
        .tokens
        .clone()
        .filter(|&index| tokens[index].is_word())
        .collect();
    // A nested item's own words are checked by the child list. The words of
    // the host item before the bracket still need the parent-level check.
    let children: Vec<_> = match &item.kind {
        ListItemKind::List(indices) => indices
            .iter()
            .filter_map(|&index| lists.get(index))
            .map(|list| list.tokens.clone())
            .collect(),
        _ => Vec::new(),
    };
    let mut diagnostics = Vec::new();
    for pair in words.windows(2) {
        let [left, right] = pair else {
            continue;
        };
        if children
            .iter()
            .any(|range| range.contains(left) || range.contains(right))
        {
            continue;
        }
        if is_common_noun(*left, tokens, out)
            && is_common_noun(*right, tokens, out)
            && same_list_case(*left, *right, out)
        {
            let boundary = tokens[*left].end;
            let mut diagnostic = SyntaxDiagnostic::new(
                DiagnosticKind::ListSeparatorMissing,
                Severity::Error,
                Confidence::Certain,
                boundary..boundary,
                "list_separator_missing",
            );
            diagnostic.corrections = vec![Correction::new(
                vec![Edit::insertion(boundary, ",")],
                0.0,
                "list_separator_missing",
            )
            .licensed_by(CorrectionLicense::Punctuation)
            .with_confidence(Confidence::Certain)];
            diagnostics.push(diagnostic);
        }
    }
    diagnostics
}

/// A list item pair has the same case. `lapsineen suojassa sateelta ja
/// tuulelta` is four obliques, not a list; `omenoita appelsiineja` is a list
/// whose separator is missing. An unknown case stays quiet.
fn same_list_case(left: usize, right: usize, out: &[TokenSyntax]) -> bool {
    let left_case = nominal_case(&out[left]);
    let right_case = nominal_case(&out[right]);
    left_case.is_some() && left_case == right_case
}

fn is_common_noun(index: usize, tokens: &[Token], out: &[TokenSyntax]) -> bool {
    if tokens[index].is_capitalised() {
        return false;
    }
    let readings = &out[index].readings;
    let noun = readings
        .iter()
        .any(|reading| reading.part_of_speech == Some(PartOfSpeech::Noun));
    let competing = readings.iter().any(|reading| {
        matches!(
            reading.part_of_speech,
            Some(
                PartOfSpeech::Adjective
                    | PartOfSpeech::Numeral
                    | PartOfSpeech::Ordinal
                    | PartOfSpeech::Verb
                    | PartOfSpeech::NegativeVerb
            )
        )
    });
    noun && !competing
}

fn remove_comma(
    kind: DiagnosticKind,
    severity: Severity,
    start: usize,
    message: &'static str,
) -> SyntaxDiagnostic {
    let mut diagnostic = SyntaxDiagnostic::new(
        kind,
        severity,
        Confidence::Certain,
        start..start + 1,
        message,
    );
    diagnostic.corrections = vec![Correction::new(
        vec![Edit::new(start, start + 1, "")],
        0.0,
        message,
    )
    .licensed_by(CorrectionLicense::Punctuation)
    .with_confidence(Confidence::Certain)];
    diagnostic
}

fn comma_positions(tokens: &[Token], start: usize, end: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for token in tokens.iter().take(end).skip(start).filter(|token| !token.is_word()) {
        for (offset, character) in token.text.char_indices() {
            if character == ',' {
                let start = token.start + offset;
                out.push((start, start + 1));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_list(text: &str) -> Option<SyntaxDiagnostic> {
        let tokens = crate::tokenise::tokenise(text);
        let analysis = crate::syntax::analyze(&tokens);
        list_diagnostics(&tokens, &analysis)
            .into_iter()
            .find(|diagnostic| diagnostic.rule_id.as_str() == "semicolon_simple_list")
    }

    #[test]
    fn a_short_semicolon_list_becomes_one_comma_correction() {
        let diagnostic =
            simple_list("Ostin omenoita; banaaneja; appelsiineja.").expect("a suggestion");
        assert_eq!(diagnostic.corrections.len(), 1);
        let edits = &diagnostic.corrections[0].edits;
        assert_eq!(edits.len(), 2, "one edit per semicolon");
        assert!(edits.iter().all(|edit| edit.replacement == ","));
    }

    #[test]
    fn a_colon_introduced_semicolon_list_becomes_one_comma_correction() {
        let diagnostic = simple_list("Tarvitsemme kolme asiaa: aikaa; rahaa; osaamista.")
            .expect("a suggestion");
        assert_eq!(diagnostic.corrections[0].edits.len(), 2);
        assert_eq!(
            diagnostic.corrections[0].edits[0].start,
            "Tarvitsemme kolme asiaa: aikaa".len(),
            "the first semicolon"
        );
    }

    #[test]
    fn a_complex_semicolon_list_keeps_its_hierarchy() {
        assert!(
            simple_list(
                "Kokoukseen osallistuivat Virtanen, toimitusjohtaja; \
                 Laine, projektipäällikkö; Korhonen, talousjohtaja."
            )
            .is_none()
        );
    }

    #[test]
    fn a_semicolon_list_of_main_clauses_is_not_a_simple_list() {
        assert!(simple_list("Hän tuli; minä lähdin; he jäivät.").is_none());
    }

    #[test]
    fn an_elliptical_pair_is_not_a_simple_list() {
        assert!(simple_list("Hän on opettaja; minä lääkäri.").is_none());
    }
}
