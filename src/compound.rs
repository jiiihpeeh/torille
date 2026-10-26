//! Compound analysis: splitting an orthographic word into its components.
//!
//! A Finnish compound is one orthographic word made of two or more lexical
//! parts, the rightmost of which is the head: `aamukahvi` is `aamu` + `kahvi`,
//! a type of coffee. Compound analysis is a separate lexical layer from
//! inflection: the whole compound inflects as one word (its head's paradigm),
//! and clitics and possessive suffixes attach to the whole compound, not to a
//! component.
//!
//! The analyzer works on the bare word. Inflection, clitics and possessives
//! attach downstream to the whole compound, which the rest of the engine
//! already handles. The output here is the analysis: the parts, the head, and
//! whether the word is a simple word, a compound, or unknown.

use crate::expand::FormBuilder;
use crate::tags::PartOfSpeech;

/// The result of analysing a word.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Analysis {
    /// A word the lexicon lists outright.
    Simple(String),
    /// A compound of two or more components.
    Compound(Compound),
    /// Not a known word and not a valid compound.
    Unknown,
}

/// One compound, as an ordered list of components with the head identified.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Compound {
    /// The orthographic surface, the whole word.
    pub surface: String,
    /// The components, left to right.
    pub parts: Vec<Component>,
    /// The index into `parts` of the head, which is the rightmost part.
    pub head_index: usize,
}

/// One component of a compound.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Component {
    /// The part as it appears in the word.
    pub surface: String,
    /// The lemma it resolves to in the lexicon.
    pub lemma: String,
    /// Whether it is the modifier or the head.
    pub role: Role,
}

/// Whether a component modifies the head or is the head.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Modifier,
    Head,
}

/// Why a hyphen is allowed between the parts of a compound.
///
/// The rules are orthographic: they say what may be separated in writing, not
/// whether the parts are real words (the caller resolves those). `closed`
/// compounds (`kirjahylly`) stay closed; a hyphen is only licensed by one of
/// these exceptions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HyphenRule {
    /// An acronym or single letter: `EU-maat`, `TV-ohjelma`, `A-vitamiini`.
    Abbreviation,
    /// A number: `20-vuotias`.
    Number,
    /// A proper name as the first part: `Suomi-ottelu`, `Matti-setä`.
    ProperName,
    /// The parts meet at the same vowel: `linja-auto`, `maa-alue`.
    VowelBoundary,
    /// A contrast or paired expression: `suomalais-ruotsalainen`,
    /// `itä-länsi-suhteet`.
    PairedExpression,
}

/// Split a word at its hyphens, rejecting empty pieces (`--`) and a word
/// without a hyphen.
pub(crate) fn split_hyphenated(word: &str) -> Option<Vec<&str>> {
    if !word.contains('-') {
        return None;
    }
    let parts: Vec<&str> = word.split('-').collect();
    if parts.len() < 2 || parts.iter().any(|part| part.is_empty()) {
        return None;
    }
    Some(parts)
}

/// `20`, `1 000`, `3.5` and the like. Only the first part can use this rule.
pub(crate) fn is_number_part(part: &str) -> bool {
    !part.is_empty()
        && part
            .chars()
            .all(|c| c.is_numeric() || c.is_whitespace() || c == '.')
        && part.chars().any(|c| c.is_numeric())
}

/// An all-capitals acronym or a single capital letter. `TV`, `EU`, `A`.
pub(crate) fn is_abbreviation_part(part: &str) -> bool {
    let mut has_letter = false;
    for c in part.chars() {
        if c.is_alphabetic() {
            has_letter = true;
            if !c.is_uppercase() {
                return false;
            }
        } else if !c.is_numeric() && c != '.' {
            return false;
        }
    }
    has_letter
}

/// A capitalised proper name from the embedded name set.
pub(crate) fn is_proper_name_part(part: &str) -> bool {
    crate::expand::proper_names::contains_capitalised(part)
}

/// Whether the boundary between `left` and `right` is the same vowel.
pub(crate) fn same_vowel_boundary(left: &str, right: &str) -> bool {
    let Some(left) = left.chars().next_back() else {
        return false;
    };
    let Some(right) = right.chars().next() else {
        return false;
    };
    left == right && crate::is_vowel(left)
}

/// The `suomalais-` / `-ruotsalainen` pair. The left side is a combining
/// stem rather than a standalone lemma, so it is not caught by the ordinary
/// same-vowel rule.
fn is_lainen_pair(left: &str, right: &str) -> bool {
    (left.ends_with("lais") || left.ends_with("läis"))
        && (right.ends_with("lainen") || right.ends_with("läinen"))
}

/// The hyphen rule licensed by the shape of `word`, if any.
///
/// `closed_known` says the hyphen-stripped spelling is already a known word.
/// In that case ordinary compounds stay closed: `kirja-hylly` must be
/// `kirjahylly`. Abbreviations, numbers and proper names are different --
/// their hyphen is a convention, not a readability choice.
pub(crate) fn hyphen_rule(word: &str, parts: &[&str], closed_known: bool) -> Option<HyphenRule> {
    if parts.len() < 2 || !word.contains('-') {
        return None;
    }
    let first = parts[0];
    if is_number_part(first) {
        return Some(HyphenRule::Number);
    }
    if is_abbreviation_part(first) {
        return Some(HyphenRule::Abbreviation);
    }
    if is_proper_name_part(first) {
        return Some(HyphenRule::ProperName);
    }
    if closed_known {
        return None;
    }
    if parts.len() >= 3 {
        return Some(HyphenRule::PairedExpression);
    }
    if is_lainen_pair(parts[0], parts[1]) {
        return Some(HyphenRule::PairedExpression);
    }
    if same_vowel_boundary(parts[0], parts[1]) {
        return Some(HyphenRule::VowelBoundary);
    }
    None
}

/// The hyphenated spelling licensed by the closed word's shape, if any.
///
/// This is the productive counterpart to the lexicalised spellings found in
/// the sanalista: `suomalaisruotsalainen` must be written
/// `suomalais-ruotsalainen`, while an ordinary compound such as
/// `kirjahylly` stays closed. Only the writing rule is applied here; whether
/// the parts themselves are words is the caller's concern.
pub(crate) fn closed_hyphenated_spelling<F>(word: &str, is_known: F) -> Option<String>
where
    F: Fn(&str) -> bool,
{
    if word.contains('-') {
        return None;
    }
    let lower = word.to_lowercase();
    for (byte, _) in lower.char_indices().skip(1) {
        let (left, right) = lower.split_at(byte);
        if left.is_empty() || right.is_empty() {
            continue;
        }
        let candidate = format!("{left}-{right}");
        // This is the shape rule alone: the caller already knows the closed
        // spelling is a compound. Passing `closed_known = true` would stop
        // at the ordinary-compound guard before the `-lainen` pair rule, and
        // `suomalaisruotsalainen` would wrongly stay closed.
        let Some(rule) = hyphen_rule(&candidate, &[left, right], false) else {
            continue;
        };
        // A same-vowel boundary or a `-lainen` pair only licenses the hyphen
        // when both sides are lexical. Without that check any word with two
        // adjacent vowels acquires a hyphenated spelling at that arbitrary
        // cut: `henkiruumiiseenne` would become `henkiruumiise-enne` and the
        // compound fast path would reject the real word.
        match rule {
            HyphenRule::VowelBoundary => {
                // A same-vowel boundary licenses the hyphen only for a
                // lexicalised pair (`linja-auto`). Without the lexeme check
                // any ordinary compound with adjacent vowels would be
                // rejected at an arbitrary cut (`päivänpa|ahtamia`).
                if is_known(left)
                    && is_known(right)
                    && crate::kotus::sanalista()
                        .by_word(&candidate)
                        .next()
                        .is_some()
                {
                    return Some(candidate);
                }
            }
            HyphenRule::PairedExpression => {
                // The `-lainen` pair rule is productive: the standard
                // spelling is hyphenated even when the pair is not a
                // sanalista lemma (`suomalais-ruotsalainen`).
                if is_known(left) && is_known(right) {
                    return Some(candidate);
                }
            }
            _ => return Some(candidate),
        }
    }
    None
}

/// Analyses `word` against the built index.
///
/// A word the index lists is simple. Otherwise it is split into components,
/// either at hyphens or by trying every boundary left to right. A component
/// resolves to a lemma either directly (it is the lemma) or as an inflected
/// form of one (it is the genitive `talon`, whose lemma is `talo`), which is
/// how a compound modifier keeps its case: `talonmies` is `talo-n` + `mies`.
/// The head is the rightmost component. The split is recursive: a compound of
/// three or more parts is found by keeping the leftmost component and analysing
/// the rest, so `tietokoneohjelmointi` is `tieto` + `kone` + `ohjelmointi`. A
/// word that neither the index nor any split accounts for is unknown.
pub fn analyze(word: &str, builder: &FormBuilder) -> Analysis {
    if builder.bases_of(word).iter().any(|b| **b == *word) {
        return Analysis::Simple(word.to_string());
    }
    // A possessive suffix belongs to the whole compound, not to its head:
    // `todistelukyvyllänsä` is `todistelu` + `kyky`, with `-nsä` on the
    // outside. Strip it and analyse the host so the parts are lexical.
    if let Some(host) = strip_possessive(word)
        && let Analysis::Compound(mut c) = analyze(host, builder)
    {
        c.surface = word.to_string();
        return Analysis::Compound(c);
    }
    // A hyphenated compound is split at every hyphen, but only when a
    // writing rule licenses the hyphen. The ordinary compound `kirjahylly`
    // must not be accepted as `kirja-hylly`.
    if let Some(pieces) = split_hyphenated(word) {
        let closed = pieces.concat().to_lowercase();
        let closed_known = !builder.bases_of(&closed).is_empty();
        let Some(rule) = hyphen_rule(word, &pieces, closed_known) else {
            return Analysis::Unknown;
        };
        let first_is_special = matches!(
            rule,
            HyphenRule::Abbreviation | HyphenRule::Number | HyphenRule::ProperName
        );
        let head_index = pieces.len() - 1;
        let mut parts = Vec::with_capacity(pieces.len());
        for (i, piece) in pieces.iter().enumerate() {
            let lower = piece.to_lowercase();
            let resolved = lemma_of(piece, builder)
                .or_else(|| lemma_of(&lower, builder))
                .or_else(|| crate::expand::modifier_is_known(&lower).then_some(lower.clone()))
                .or_else(|| {
                    (i == head_index && is_proper_name_part(piece)).then_some(lower.clone())
                });
            let is_head = i == head_index;
            let lemma = match resolved {
                Some(lemma) => lemma,
                None if (i == 0 && first_is_special)
                    || (is_head
                        && rule == HyphenRule::Number
                        && piece.chars().next().is_some_and(char::is_alphabetic)) =>
                {
                    lower
                }
                None => return Analysis::Unknown,
            };
            // A function word is not a compound head: `ennen-kuin` is two
            // words, not a hyphenated compound. A proper name used as the
            // head has no sanalista row to check.
            if is_head && !is_content_head(&lemma) && !is_proper_name_part(piece) {
                return Analysis::Unknown;
            }
            parts.push(Component {
                surface: piece.to_string(),
                lemma,
                role: if is_head { Role::Head } else { Role::Modifier },
            });
        }
        return Analysis::Compound(Compound {
            surface: word.to_string(),
            head_index,
            parts,
        });
    }
    // Recursive split. Try every boundary from the left, so the modifier is
    // the longest possible prefix; the rest is analysed the same way. The head
    // is the rightmost component, which is the head of the deepest rest.
    let n = word.len();
    for split in 1..n {
        if !word.is_char_boundary(split) {
            continue;
        }
        let (left, right) = word.split_at(split);
        let Some(left_lemma) = lemma_of(left, builder) else {
            continue;
        };
        let head = analyze(right, builder);
        // The rest must itself be a compound (or a single lemma), so the whole
        // thing is a nested compound headed by the rest's head.
        let (rest_parts, rest_head) = match head {
            Analysis::Compound(c) => (c.parts, c.head_index),
            Analysis::Simple(_) => {
                let Some(rl) = lemma_of(right, builder) else {
                    continue;
                };
                if !is_content_head(&rl) {
                    continue;
                }
                (
                    vec![Component {
                        surface: right.to_string(),
                        lemma: rl,
                        role: Role::Head,
                    }],
                    0,
                )
            }
            Analysis::Unknown => {
                // The right half can be an inflected form rather than a
                // lemma (`todistelukyvyn` = `todistelu` + `kyvyn`); the
                // recursive analysis rejects it because it is not simple,
                // so resolve the head directly.
                let Some(rl) = lemma_of(right, builder) else {
                    continue;
                };
                if !is_content_head(&rl) {
                    continue;
                }
                (
                    vec![Component {
                        surface: right.to_string(),
                        lemma: rl,
                        role: Role::Head,
                    }],
                    0,
                )
            }
        };
        let mut parts = Vec::with_capacity(rest_parts.len() + 1);
        parts.push(Component {
            surface: left.to_string(),
            lemma: left_lemma,
            role: Role::Modifier,
        });
        parts.extend(rest_parts);
        return Analysis::Compound(Compound {
            surface: word.to_string(),
            head_index: rest_head + 1,
            parts,
        });
    }
    Analysis::Unknown
}

/// Whether `word` may head a compound. A pronoun or a function word is
/// not a content head: `ennen` + `kuin` is the two-word phrase
/// `ennen kuin`, and `talon` + `mukaan` is not a compound either. The
/// check looks only at the sanalista lemma; an inflected form is left to
/// the ordinary resolution.
pub(crate) fn is_content_head(word: &str) -> bool {
    let mut rows = crate::kotus::sanalista().by_word(word).peekable();
    if rows.peek().is_none() {
        // A derived or otherwise classless head is not a function word.
        return true;
    }
    // A homonym can be a function word in one reading and a content word in
    // another: `puoli` is an adverb/postposition row and a noun/numeral row.
    // The compound head is the content reading, so accept the word when any
    // row is nominal or verbal rather than judging the first row alone.
    rows.any(|row| {
        PartOfSpeech::parse_all(row.part_of_speech())
            .iter()
            .any(|pos| {
                matches!(
                    pos,
                    PartOfSpeech::Noun
                        | PartOfSpeech::Adjective
                        | PartOfSpeech::Verb
                        | PartOfSpeech::Numeral
                )
            })
    })
}

/// The possessive suffixes that attach to the whole word, longest first.
const POSSESSIVE_SUFFIXES: &[&str] = &["nsa", "nsä", "nne", "mme", "si", "ni", "an", "än", "en"];

/// Strip one possessive suffix, or `None` when the word does not end in
/// one (or the remainder would be empty).
fn strip_possessive(word: &str) -> Option<&str> {
    POSSESSIVE_SUFFIXES.iter().find_map(|suffix| {
        let host = word.strip_suffix(suffix)?;
        (!host.is_empty()).then_some(host)
    })
}

/// The lemma a surface form resolves to, if the index knows it.
///
/// The form is a lemma when the index lists it as its own base, and an
/// inflected form of a lemma otherwise: the genitive `talon` is a form of
/// `talo`, so a compound modifier `talon` resolves to `talo`. The first base
/// named is the one chosen, which is the primary reading.
fn lemma_of(word: &str, builder: &FormBuilder) -> Option<String> {
    let bases = builder.bases_of(word);
    if bases.is_empty() {
        return None;
    }
    // A word that is its own base is the lemma; otherwise the first base is.
    if bases.contains(&word) {
        Some(word.to_string())
    } else {
        Some(bases[0].to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kotus::{Inflection, Lemma};
    use crate::spec::Catalogues;

    fn noun(word: &str, class: u16) -> Lemma<'static> {
        Lemma {
            word: Box::leak(word.to_string().into_boxed_str()),
            homonymy: "",
            part_of_speech: "substantiivi",
            inflections: vec![Inflection {
                class,
                gradation: None,
            }],
        }
    }

    fn word_with_pos(word: &str, class: u16, pos: &'static str) -> Lemma<'static> {
        Lemma {
            word: Box::leak(word.to_string().into_boxed_str()),
            homonymy: "",
            part_of_speech: pos,
            inflections: vec![Inflection {
                class,
                gradation: None,
            }],
        }
    }

    /// `kyky` is class `1*M`: the genitive is `kyvyn`, not `kykyn`.
    fn graded_noun(word: &str, class: u16) -> Lemma<'static> {
        Lemma {
            word: Box::leak(word.to_string().into_boxed_str()),
            homonymy: "",
            part_of_speech: "substantiivi",
            inflections: vec![Inflection {
                class,
                gradation: Some(crate::kotus::Gradation(13)),
            }],
        }
    }

    fn builder(lemmas: Vec<Lemma<'static>>) -> FormBuilder {
        let cat = Catalogues::embedded().clone();
        crate::expand::build(&lemmas, &cat)
    }

    #[test]
    fn a_known_word_is_simple() {
        let b = builder(vec![noun("aamu", 1), noun("kahvi", 1)]);
        assert_eq!(analyze("aamu", &b), Analysis::Simple("aamu".to_string()));
    }

    #[test]
    fn a_compound_is_split_into_modifier_and_head() {
        let b = builder(vec![noun("aamu", 1), noun("kahvi", 1)]);
        let got = analyze("aamukahvi", &b);
        assert_eq!(
            got,
            Analysis::Compound(Compound {
                surface: "aamukahvi".to_string(),
                parts: vec![
                    Component {
                        surface: "aamu".to_string(),
                        lemma: "aamu".to_string(),
                        role: Role::Modifier,
                    },
                    Component {
                        surface: "kahvi".to_string(),
                        lemma: "kahvi".to_string(),
                        role: Role::Head,
                    },
                ],
                head_index: 1,
            })
        );
    }

    #[test]
    fn the_rightmost_component_is_the_head() {
        let b = builder(vec![
            noun("koti", 1),
            noun("maa", 18),
            noun("kahvi", 1),
            noun("kuppi", 1),
        ]);
        // `kahvikuppi` is `kahvi` + `kuppi`, headed by `kuppi`.
        let got = analyze("kahvikuppi", &b);
        assert!(
            matches!(got, Analysis::Compound(c) if c.head_index == 1 && c.parts[1].lemma == "kuppi")
        );
    }

    #[test]
    fn an_unknown_word_is_not_a_compound() {
        let b = builder(vec![noun("aamu", 1), noun("kahvi", 1)]);
        assert_eq!(analyze("xyzabc", &b), Analysis::Unknown);
    }

    #[test]
    fn a_word_ending_in_a_non_lemma_is_unknown() {
        // `aamukah` ends in `kah`, which is not a lemma, so it is not a
        // compound of `aamu` and `kah`.
        let b = builder(vec![noun("aamu", 1), noun("kahvi", 1)]);
        assert_eq!(analyze("aamukah", &b), Analysis::Unknown);
    }

    #[test]
    fn a_recursive_compound_has_more_than_two_parts() {
        let b = builder(vec![
            noun("tieto", 1),
            noun("kone", 48),
            noun("ohjelmointi", 5),
            noun("kurssi", 5),
        ]);
        // `tietokoneohjelmointikurssi` is four parts headed by `kurssi`.
        let got = analyze("tietokoneohjelmointikurssi", &b);
        match got {
            Analysis::Compound(c) => {
                let lemmas: Vec<&str> = c.parts.iter().map(|p| p.lemma.as_str()).collect();
                assert_eq!(lemmas, vec!["tieto", "kone", "ohjelmointi", "kurssi"]);
                assert_eq!(c.head_index, 3);
                assert_eq!(c.parts[3].lemma, "kurssi");
            }
            other => panic!("expected a compound, got {other:?}"),
        }
    }

    #[test]
    fn a_genitive_modifier_resolves_to_its_lemma() {
        // `talonmies` is `talo-n` (the genitive) + `mies`. The modifier `talon`
        // is not the lemma but a form of it, and resolves to `talo`.
        let b = builder(vec![noun("talo", 1), noun("mies", 42)]);
        let got = analyze("talonmies", &b);
        match got {
            Analysis::Compound(c) => {
                assert_eq!(c.parts[0].lemma, "talo");
                assert_eq!(c.parts[0].surface, "talon");
                assert_eq!(c.parts[1].lemma, "mies");
                assert_eq!(c.head_index, 1);
            }
            other => panic!("expected a compound, got {other:?}"),
        }
    }

    #[test]
    fn an_inflected_head_resolves_to_its_lemma() {
        let b = builder(vec![noun("todistelu", 2), graded_noun("kyky", 1)]);
        let got = analyze("todistelukyvyn", &b);
        match got {
            Analysis::Compound(c) => {
                let lemmas: Vec<&str> = c.parts.iter().map(|p| p.lemma.as_str()).collect();
                assert_eq!(lemmas, vec!["todistelu", "kyky"]);
            }
            other => panic!("expected a compound, got {other:?}"),
        }
    }

    #[test]
    fn a_possessive_on_a_compound_is_not_a_component() {
        let b = builder(vec![noun("todistelu", 2), graded_noun("kyky", 1)]);
        for word in ["todistelukykynsä", "todistelukyvyllänsä"] {
            let got = analyze(word, &b);
            match got {
                Analysis::Compound(c) => {
                    assert_eq!(c.surface, word);
                    let lemmas: Vec<&str> = c.parts.iter().map(|p| p.lemma.as_str()).collect();
                    assert_eq!(lemmas, vec!["todistelu", "kyky"]);
                }
                other => panic!("{word}: expected a compound, got {other:?}"),
            }
        }
    }

    #[test]
    fn a_function_word_is_not_a_compound_head() {
        // `ennenkuin` is the standard two-word phrase `ennen kuin`, not
        // an adverb+conjunction compound.
        let b = builder(vec![
            word_with_pos("ennen", 1, "adverbi"),
            word_with_pos("kuin", 1, "konjunktio"),
        ]);
        assert_eq!(analyze("ennenkuin", &b), Analysis::Unknown);
    }

    #[test]
    fn a_hyphenated_compound_is_split_on_the_hyphen() {
        let b = builder(vec![noun("Helsinki", 1), noun("Vantaa", 9)]);
        let got = analyze("Helsinki-Vantaa", &b);
        match got {
            Analysis::Compound(c) => {
                assert_eq!(c.parts.len(), 2);
                assert_eq!(c.parts[0].lemma, "Helsinki");
                assert_eq!(c.parts[1].lemma, "Vantaa");
                assert_eq!(c.head_index, 1);
            }
            other => panic!("expected a compound, got {other:?}"),
        }
    }
}
