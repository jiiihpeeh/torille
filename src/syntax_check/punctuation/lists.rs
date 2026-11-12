//! List structure and list-separator diagnostics.

use crate::syntax::{SyntaxAnalysis, TokenSyntax};
use crate::syntax_list::{List, ListItem, ListItemKind, ListKind};
use crate::tags::PartOfSpeech;
use crate::tokenise::Token;

use super::super::diagnostic::{
    Correction, DiagnosticConfidence, DiagnosticKind, Edit, Severity, SyntaxDiagnostic,
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
    }
    out
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
        if is_common_noun(*left, tokens, out) && is_common_noun(*right, tokens, out) {
            let boundary = tokens[*left].end;
            let mut diagnostic = SyntaxDiagnostic::new(
                DiagnosticKind::ListSeparatorMissing,
                Severity::Error,
                DiagnosticConfidence::High,
                boundary..boundary,
                "list_separator_missing",
            );
            diagnostic.corrections = vec![Correction::new(
                vec![Edit::insertion(boundary, ",")],
                0.0,
                "list_separator_missing",
            )];
            diagnostics.push(diagnostic);
        }
    }
    diagnostics
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
        DiagnosticConfidence::High,
        start..start + 1,
        message,
    );
    diagnostic.corrections = vec![Correction::new(
        vec![Edit::new(start, start + 1, "")],
        0.0,
        message,
    )];
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
