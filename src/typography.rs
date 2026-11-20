//! Typography: punctuation spacing, separate from the grammar checker.
//!
//! The grammar pass in [`crate::syntax_check`] never returns these; a UI calls
//! [`check`] behind its own toggle or panel section. Every finding is a purely
//! mechanical edit around a semicolon, and nothing inside a protected context
//! (fenced or inline code, a URL, a numeric run, a quotation) is touched.
//!
//! The range in a [`TypographyDiagnostic`] is the exact edit surface: a
//! deletion spans the whitespace that goes away, a replacement spans the
//! duplicated marks, and an insertion is an empty range at the insertion point.

use std::ops::Range;

/// The mechanical semicolon rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypographyRule {
    /// No whitespace before `;`, one space after it.
    SemicolonSpacing,
    /// `;;` collapses to one `;`.
    SemicolonDuplicate,
}

impl TypographyRule {
    pub fn name(self) -> &'static str {
        match self {
            TypographyRule::SemicolonSpacing => "typography_semicolon_spacing",
            TypographyRule::SemicolonDuplicate => "typography_semicolon_duplicate",
        }
    }
}

/// One mechanical typography edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypographyDiagnostic {
    pub rule: TypographyRule,
    /// Byte span to replace. An insertion has `span.start == span.end`.
    pub span: Range<usize>,
    /// Text the span becomes.
    pub replacement: String,
    /// Stable message key; equals [`TypographyRule::name`].
    pub message: &'static str,
}

/// Semicolon spacing and duplicate diagnostics for `text`.
///
/// Every range is a byte range into `text`; [`apply`] turns the findings into
/// corrected text.
pub fn check(text: &str) -> Vec<TypographyDiagnostic> {
    let protected = protected_ranges(text);
    let is_protected = |byte: usize| protected.iter().any(|range| range.contains(&byte));

    let mut diagnostics = Vec::new();
    let mut cursor = 0;
    while let Some(offset) = text[cursor..].find(';') {
        let at = cursor + offset;
        cursor = at + 1;
        if is_protected(at) {
            continue;
        }

        // A run of adjacent semicolons is one duplicate finding.
        let mut run_end = at + 1;
        while text[run_end..].starts_with(';') {
            run_end += 1;
        }
        cursor = run_end;
        if run_end > at + 1 {
            diagnostics.push(diagnostic(
                TypographyRule::SemicolonDuplicate,
                at..run_end,
                ";",
            ));
        }

        // No whitespace before the semicolon, but never join two lines.
        let mut whitespace = at;
        while let Some((before, character)) = text[..whitespace].char_indices().next_back() {
            if character == ' ' || character == '\t' {
                whitespace = before;
            } else {
                break;
            }
        }
        if whitespace < at {
            diagnostics.push(diagnostic(
                TypographyRule::SemicolonSpacing,
                whitespace..at,
                "",
            ));
        }

        // One space after it unless the line, the text or a closing mark ends it.
        if let Some((after, character)) = text[run_end..].char_indices().next() {
            let after = run_end + after;
            if character.is_whitespace() || is_closing(character) {
                continue;
            }
            diagnostics.push(diagnostic(
                TypographyRule::SemicolonSpacing,
                after..after,
                " ",
            ));
        }
    }
    diagnostics
}

/// Apply `diagnostics` to `text`, skipping any overlapping edit.
pub fn apply(text: &str, diagnostics: &[TypographyDiagnostic]) -> String {
    let mut ordered: Vec<&TypographyDiagnostic> = diagnostics.iter().collect();
    ordered.sort_by_key(|diagnostic| (diagnostic.span.start, diagnostic.span.end));
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0;
    for diagnostic in ordered {
        if diagnostic.span.start < cursor || diagnostic.span.end > text.len() {
            continue;
        }
        out.push_str(&text[cursor..diagnostic.span.start]);
        out.push_str(&diagnostic.replacement);
        cursor = diagnostic.span.end;
    }
    out.push_str(&text[cursor..]);
    out
}

fn diagnostic(rule: TypographyRule, span: Range<usize>, text: &str) -> TypographyDiagnostic {
    TypographyDiagnostic {
        rule,
        span,
        replacement: text.to_string(),
        message: rule.name(),
    }
}

/// A closing quote or bracket takes the space instead of following it.
fn is_closing(character: char) -> bool {
    matches!(character, '"' | '\'' | '”' | '’' | '»' | ')' | ']' | '}')
}

/// Byte ranges that typography must not edit.
///
/// The scan is deliberately small: fenced code, inline code, URLs, numeric
/// runs and quoted passages. Anything else is ordinary prose.
pub fn protected_ranges(text: &str) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    fenced_code(text, &mut ranges);
    inline_code(text, &mut ranges);
    urls(text, &mut ranges);
    numeric_runs(text, &mut ranges);
    quotations(text, &mut ranges);
    ranges
}

fn fenced_code(text: &str, ranges: &mut Vec<Range<usize>>) {
    let mut offset = 0;
    let mut open: Option<usize> = None;
    for line in text.split_inclusive('\n') {
        let start = offset;
        offset += line.len();
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            match open.take() {
                Some(from) => ranges.push(from..offset),
                None => open = Some(start),
            }
        }
    }
    if let Some(from) = open {
        ranges.push(from..text.len());
    }
}

fn inline_code(text: &str, ranges: &mut Vec<Range<usize>>) {
    let mut index = 0;
    while let Some(offset) = text[index..].find('`') {
        let open = index + offset;
        let mut width = 1;
        while text[open + width..].starts_with('`') {
            width += 1;
        }
        let delimiter = &text[open..open + width];
        let after = open + width;
        match text[after..].find(delimiter) {
            Some(close) if !text[open..after + close].contains('\n') => {
                let end = after + close + width;
                ranges.push(open..end);
                index = end;
            }
            _ => index = after,
        }
    }
}

fn urls(text: &str, ranges: &mut Vec<Range<usize>>) {
    for marker in ["http://", "https://", "www."] {
        let mut cursor = 0;
        while let Some(offset) = text[cursor..].find(marker) {
            let start = cursor + offset;
            let end = text[start..]
                .char_indices()
                .find(|(_, character)| character.is_whitespace())
                .map_or(text.len(), |(index, _)| start + index);
            ranges.push(start..end);
            cursor = end;
        }
    }
}

fn numeric_runs(text: &str, ranges: &mut Vec<Range<usize>>) {
    let characters: Vec<(usize, char)> = text.char_indices().collect();
    let mut index = 0;
    while index < characters.len() {
        let (start, character) = characters[index];
        if !character.is_ascii_digit() {
            index += 1;
            continue;
        }
        let mut end = index + 1;
        while end < characters.len() && is_numeric_separator(characters[end].1) {
            end += 1;
        }
        let last = characters[end - 1];
        ranges.push(start..last.0 + last.1.len_utf8());
        index = end;
    }
}

fn is_numeric_separator(character: char) -> bool {
    character.is_ascii_digit() || matches!(character, '.' | ',' | ':' | '/' | '-')
}

fn quotations(text: &str, ranges: &mut Vec<Range<usize>>) {
    quoted(text, '"', '"', ranges);
    quoted(text, '»', '«', ranges);
}

/// Pair every opener with the next closer; an unbalanced passage is protected
/// to the end of the text.
fn quoted(text: &str, open: char, close: char, ranges: &mut Vec<Range<usize>>) {
    let mut opener: Option<usize> = None;
    for (index, character) in text.char_indices() {
        if character != open && (opener.is_none() || character != close) {
            continue;
        }
        match opener.take() {
            None => opener = Some(index),
            Some(start) => ranges.push(start..index + character.len_utf8()),
        }
    }
    if let Some(start) = opener {
        ranges.push(start..text.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed(text: &str) -> String {
        apply(text, &check(text))
    }

    #[test]
    fn a_space_before_the_semicolon_is_removed() {
        assert_eq!(fixed("sana ; toinen"), "sana; toinen");
        let diagnostics = check("sana ; toinen");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].rule, TypographyRule::SemicolonSpacing);
        assert_eq!(diagnostics[0].span, 4..5);
        assert_eq!(diagnostics[0].replacement, "");
    }

    #[test]
    fn a_missing_space_after_the_semicolon_is_inserted() {
        let diagnostics = check("sana;toinen");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].span, 5..5);
        assert_eq!(diagnostics[0].replacement, " ");
        assert_eq!(fixed("sana;toinen"), "sana; toinen");
    }

    #[test]
    fn a_duplicated_semicolon_collapses() {
        let diagnostics = check("sana;;toinen");
        assert_eq!(diagnostics.len(), 2, "{diagnostics:#?}");
        assert_eq!(diagnostics[0].rule, TypographyRule::SemicolonDuplicate);
        assert_eq!(fixed("sana;;toinen"), "sana; toinen");
    }

    #[test]
    fn a_correct_semicolon_is_quiet() {
        assert!(check("sana; toinen").is_empty());
        assert!(check("Hän tuli; minä lähdin.").is_empty());
        assert!(check("Hän lähti kotiin;").is_empty());
        assert!(check("Hän tuli;\nminä lähdin.").is_empty());
    }

    #[test]
    fn a_closing_mark_takes_the_space() {
        assert!(check("sana;\"").is_empty());
        assert!(check("sana;)").is_empty());
    }

    #[test]
    fn code_and_urls_are_protected() {
        assert!(check("`let x = 1;let y = 2;`").is_empty());
        assert!(check("```\nlet x = 1;let y = 2;\n```").is_empty());
        assert!(check("https://example.com/a;b").is_empty());
    }

    #[test]
    fn a_quotation_is_protected() {
        assert!(check("Hän sanoi: \"Tulen huomenna;minä lähden.\"").is_empty());
        assert!(check("Hän sanoi: »Tulen huomenna;minä lähden.»").is_empty());
    }

    #[test]
    fn a_numeric_run_is_protected() {
        assert!(check("3,14").is_empty());
        assert!(check("20.11.2026").is_empty());
        assert!(check("12:30").is_empty());
    }

    #[test]
    fn edits_are_applied_without_shifting() {
        let text = "sana ;toinen;;kolmas";
        let diagnostics = check(text);
        assert_eq!(apply(text, &diagnostics), "sana; toinen; kolmas");
    }
}
