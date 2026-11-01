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
//! already handles. The output here is the analysis: the parts, the head,
//! whether the word is a simple word, a compound, or unknown, and -- for a
//! compound -- which parts inflect ([`CompoundKind`], [`ComponentInflection`]).
//! That distribution is a separate question from the split: `nuoripari` and
//! `suomalais-ruotsalainen` both inflect two parts, and `linja-auto` inflects
//! one, although all three are compounds.
//!
//! [`explain`] is the same analysis with the action on each part spelled out
//! (`talon` is a frozen genitive, `miehen` is an inflected head), which is what
//! the `check` tool prints.

use crate::expand::FormBuilder;
use crate::tags::{Case, PartOfSpeech};

/// The result of analysing a word.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Analysis {
    /// A word the lexicon lists and this analyzer does not decompose: either
    /// it is not a compound, or the dictionary calls it one but no split was
    /// found.
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
    /// Which parts inflect. This is a fact about the word, not about the
    /// writing: `nuoripari` and `suomalais-ruotsalainen` are both
    /// [`CompoundKind::BothParts`], and `linja-auto` is ordinary.
    pub kind: CompoundKind,
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
    /// How the component takes part in the compound's inflection.
    pub inflection: ComponentInflection,
    /// The case a frozen modifier is in (`talon` is genitive). `None` for a
    /// bare lemma, a bound form, and the head.
    pub case: Option<Case>,
}

/// Whether a component modifies the head or is the head.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Modifier,
    Head,
}

/// How a component takes part in the compound's inflection.
///
/// This is deliberately not a boolean: a modifier is not simply "fixed" or
/// "inflecting". `talon` in `talonmies` is frozen in a case, `aamu` in
/// `aamukahvi` is a bare lemma that stays as written, and `nuori` in
/// `nuoripari` inflects when the whole word does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComponentInflection {
    /// A bare lemma used as a modifier: it stays as written in every cell
    /// (`aamu` in `aamukahvi`).
    Invariant,
    /// The head: it carries the case, the number and the owner suffix
    /// (`kahvi` in `aamukahvi`).
    Inflect,
    /// A modifier the compound froze in an inflected or bound form (`talon` of
    /// `talo`, `esi-` of `esipuhe`): it keeps that surface in every cell.
    Lexical,
    /// A first part that inflects only when the whole expression does
    /// (`nuori` in `nuoripari`, `suomalais-` in `suomalais-ruotsalainen`).
    Conditional,
}

/// The shape of a compound as a whole.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompoundKind {
    /// Only the head inflects (`kirjakauppa`, `linja-auto`).
    Ordinary,
    /// The modifier is already in the genitive and stays frozen (`talonmies`,
    /// `viikonloppu`).
    GenitiveInitial,
    /// More than one part inflects: a class-51 word (`nuoripari`) or a
    /// coordinated hyphenated pair (`suomalais-ruotsalainen`). The hyphen is
    /// one way to get here, not the definition.
    BothParts,
    /// The first part is bound or combining (`esi-`, `‑kulmio`), so it has no
    /// paradigm of its own.
    Lexicalised,
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
    // A dictionary word is simple unless the dictionary itself says it is a
    // compound: a class 50/51 marker, or no class at all (Kotus omits the type
    // of a compound because it declines like its final component). The other
    // rows are simple words, and splitting them would be guessing. A
    // dictionary compound whose split this analyzer cannot find is still a
    // known word, so it falls back to `Simple` below rather than `Unknown`.
    let is_lemma = builder.bases_of(word).iter().any(|b| **b == *word);
    let dictionary_compound =
        is_lemma && (is_class_50(word) || is_class_51(word) || is_classless(word));
    if is_lemma && !dictionary_compound {
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
            return fallback(word, dictionary_compound);
        };
        let first_is_special = matches!(
            rule,
            HyphenRule::Abbreviation | HyphenRule::Number | HyphenRule::ProperName
        );
        let head_index = pieces.len() - 1;
        let mut parts = Vec::with_capacity(pieces.len());
        for (i, piece) in pieces.iter().enumerate() {
            let lower = piece.to_lowercase();
            let is_head = i == head_index;
            // A modifier that is only a bound prefix or a combining form is
            // lexicalised: `esi-` of `esipuhe` has no paradigm of its own.
            let mut bound = false;
            let resolved = if let Some(lemma) =
                lemma_of(piece, builder).or_else(|| lemma_of(&lower, builder))
            {
                Some(lemma)
            } else if crate::expand::modifier_is_known(&lower) {
                bound = true;
                Some(lower.clone())
            } else if is_head && is_proper_name_part(piece) {
                Some(lower.clone())
            } else {
                None
            };
            let lemma = match resolved {
                Some(lemma) => lemma,
                None if (i == 0 && first_is_special)
                    || (is_head
                        && rule == HyphenRule::Number
                        && piece.chars().next().is_some_and(char::is_alphabetic)) =>
                {
                    lower
                }
                None => return fallback(word, dictionary_compound),
            };
            // A function word is not a compound head: `ennen-kuin` is two
            // words, not a hyphenated compound. A proper name used as the
            // head has no sanalista row to check.
            if is_head && !is_content_head(&lemma) && !is_proper_name_part(piece) {
                return Analysis::Unknown;
            }
            let role = if is_head { Role::Head } else { Role::Modifier };
            parts.push(component(piece, lemma, role, bound, builder));
        }
        let kind = classify(&mut parts, head_index, word, Some(rule), builder);
        return Analysis::Compound(Compound {
            surface: word.to_string(),
            head_index,
            parts,
            kind,
        });
    }
    // Recursive split. Try the boundaries longest-modifier first, so the
    // modifier is the longest possible prefix; the rest is analysed the same
    // way. The head is the rightmost component, which is the head of the
    // deepest rest.
    //
    // The direction is not cosmetic. Scanning left to right accepts the first
    // split that resolves, and a short prefix is often a word of its own:
    // `nuoriparin` came back as `nuo` + `ripa` + `ry`, `isotaivot` as `iso` +
    // `taivo`, and `särkynytsydän` as `särky` + `nyt` + `sydän`. Scanning from
    // the right finds `nuoren` + `parin`, `isot` + `aivot` and `särkynyt` +
    // `sydän`, which are the real parts.
    let n = word.len();
    for split in (1..n).rev() {
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
                (vec![component(right, rl, Role::Head, false, builder)], 0)
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
                (vec![component(right, rl, Role::Head, false, builder)], 0)
            }
        };
        let mut parts = Vec::with_capacity(rest_parts.len() + 1);
        parts.push(component(left, left_lemma, Role::Modifier, false, builder));
        parts.extend(rest_parts);
        let head_index = rest_head + 1;
        let kind = classify(&mut parts, head_index, word, None, builder);
        return Analysis::Compound(Compound {
            surface: word.to_string(),
            head_index,
            parts,
            kind,
        });
    }
    fallback(word, dictionary_compound)
}

/// What to answer when the split fails.
///
/// A dictionary compound whose split this analyzer cannot find is still a known
/// word (`aakkosnumeerinen` has a derived head), so it is `Simple`; anything
/// else is `Unknown`.
fn fallback(word: &str, dictionary_compound: bool) -> Analysis {
    if dictionary_compound {
        Analysis::Simple(word.to_string())
    } else {
        Analysis::Unknown
    }
}

/// What happened to one component in the analysed word.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// The component is as the dictionary spells it.
    Unchanged,
    /// The component carries an ending in this word (`miehen`, `talossa`).
    Inflected,
    /// The compound froze an inflected or bound form (`talon`, `esi-`).
    Frozen,
}

/// One component's part in the analysed word, for [`explain`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComponentAction {
    /// The part as it appears in the word.
    pub surface: String,
    /// The lemma it resolves to.
    pub lemma: String,
    /// Whether it is the modifier or the head.
    pub role: Role,
    /// How it takes part in the inflection.
    pub inflection: ComponentInflection,
    /// The case a frozen modifier is in.
    pub case: Option<Case>,
    /// What happened to it in this word.
    pub action: Action,
}

/// Why a compound is spelled the way it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Explanation {
    /// The orthographic surface, the whole word.
    pub surface: String,
    /// Which parts inflect.
    pub kind: CompoundKind,
    /// The components, left to right.
    pub components: Vec<ComponentAction>,
}

/// Explains `word` component by component, or `None` when it is not a compound.
///
/// The action is relative to the dictionary: a component whose surface is the
/// lemma is `Unchanged`, one carrying an ending is `Inflected`, and a frozen
/// modifier (`talon`, `esi-`) is `Frozen`. That is the distinction a reader
/// needs: `talonmiehen` is not `talo` + `miehen` with both parts inflected, it
/// is a frozen genitive plus an inflected head.
pub fn explain(word: &str, builder: &FormBuilder) -> Option<Explanation> {
    let Analysis::Compound(c) = analyze(word, builder) else {
        return None;
    };
    let components = c
        .parts
        .iter()
        .map(|p| {
            let action = match p.inflection {
                ComponentInflection::Lexical => Action::Frozen,
                ComponentInflection::Invariant => Action::Unchanged,
                ComponentInflection::Conditional | ComponentInflection::Inflect => {
                    if same_word(&p.surface, &p.lemma) {
                        Action::Unchanged
                    } else {
                        Action::Inflected
                    }
                }
            };
            ComponentAction {
                surface: p.surface.clone(),
                lemma: p.lemma.clone(),
                role: p.role,
                inflection: p.inflection,
                case: p.case,
                action,
            }
        })
        .collect();
    Some(Explanation {
        surface: c.surface,
        kind: c.kind,
        components,
    })
}

/// One component, with the inflection mode and any frozen case worked out.
///
/// A modifier is [`ComponentInflection::Invariant`] when it is the lemma itself
/// (`aamu`), [`ComponentInflection::Lexical`] when the compound froze a form of
/// it (`talon`) or only a bound prefix survives (`esi-`), and the head is
/// [`ComponentInflection::Inflect`]. The case is recorded only for a frozen
/// form: `talon` is genitive, and saying so is what distinguishes `talonmies`
/// from a typo.
fn component(
    surface: &str,
    lemma: String,
    role: Role,
    bound: bool,
    builder: &FormBuilder,
) -> Component {
    let is_head = role == Role::Head;
    let inflection = if is_head {
        ComponentInflection::Inflect
    } else if bound || !same_word(surface, &lemma) {
        ComponentInflection::Lexical
    } else {
        ComponentInflection::Invariant
    };
    let case = if inflection == ComponentInflection::Lexical {
        frozen_case(surface, builder)
    } else {
        None
    };
    Component {
        surface: surface.to_string(),
        lemma,
        role,
        inflection,
        case,
    }
}

/// Whether the surface is the lemma as written, ignoring capitalisation.
fn same_word(surface: &str, lemma: &str) -> bool {
    surface == lemma || surface.to_lowercase() == lemma.to_lowercase()
}

/// The case a frozen surface is in, if the index reads it as a case form.
fn frozen_case(surface: &str, builder: &FormBuilder) -> Option<Case> {
    builder
        .readings_of(surface)
        .and_then(|readings| readings.iter().find_map(|r| r.case))
}

/// Whether the sanalista marks `word` as a class-51 compound.
///
/// Class 51 is the dictionary's way of saying that the first part inflects as
/// well as the head (`nuoripari : nuorenparin`). It is a lexical fact, not an
/// orthographic one: `suomalais-ruotsalainen` has the same distribution and is
/// not in the sanalista at all.
fn is_class_51(word: &str) -> bool {
    crate::kotus::sanalista()
        .by_word(word)
        .any(|row| row.inflections().any(|i| i.class == 51))
}

/// Whether the sanalista marks `word` as a class-50 compound.
fn is_class_50(word: &str) -> bool {
    crate::kotus::sanalista()
        .by_word(word)
        .any(|row| row.inflections().any(|i| i.class == 50))
}

/// Whether the sanalista gives `word` no inflection class at all.
///
/// Kotus writes a compound without a type number because it declines like its
/// final component, so a classless row is a compound too.
fn is_classless(word: &str) -> bool {
    let mut rows = crate::kotus::sanalista().by_word(word).peekable();
    rows.peek().is_some() && rows.all(|row| row.inflections().next().is_none())
}

/// The kind of a compound, and the inflection mode that follows from it.
fn classify(
    parts: &mut [Component],
    head_index: usize,
    word: &str,
    rule: Option<HyphenRule>,
    builder: &FormBuilder,
) -> CompoundKind {
    // The citation is the lemmas joined the way the word joins them: a
    // hyphenated pair keeps its hyphen, a closed compound does not. This is
    // what finds the class-51 row behind an inflected form (`nuorenparin` ->
    // `nuoripari`).
    let hyphenated = word.contains('-');
    let citation: String = {
        let lemmas: Vec<&str> = parts.iter().map(|p| p.lemma.as_str()).collect();
        if hyphenated {
            lemmas.join("-")
        } else {
            lemmas.concat()
        }
    };
    // A participle first part agrees with the head and inflects with it:
    // `särkynyt` in `särkynytsydän` is `särkyneen` + `sydämen`. It is not a
    // lemma, so the class-51 row cannot name it.
    let participle_first = parts
        .first()
        .is_some_and(|p| is_participle(&p.surface, builder));
    let kind = if rule == Some(HyphenRule::PairedExpression)
        || participle_first
        || is_class_51(&citation)
        || is_class_51(word)
    {
        CompoundKind::BothParts
    } else if parts.first().is_some_and(|p| {
        p.inflection == ComponentInflection::Lexical && p.case == Some(Case::Genitive)
    }) {
        CompoundKind::GenitiveInitial
    } else if parts
        .iter()
        .enumerate()
        .any(|(i, p)| i != head_index && p.inflection == ComponentInflection::Lexical)
    {
        CompoundKind::Lexicalised
    } else {
        CompoundKind::Ordinary
    };
    // A class-51 or coordinated compound inflects its first part along with
    // the head. The middle parts of a three-part compound do not (`viini` in
    // `mustaviinimarja`), so only the first is promoted.
    if kind == CompoundKind::BothParts
        && let Some(first) = parts.first_mut()
        && first.role == Role::Modifier
    {
        first.inflection = ComponentInflection::Conditional;
    }
    kind
}

/// Whether `surface` is a participle: a verb reading that carries a case.
fn is_participle(surface: &str, builder: &FormBuilder) -> bool {
    builder.readings_of(surface).is_some_and(|readings| {
        readings
            .iter()
            .any(|r| r.part_of_speech == Some(PartOfSpeech::Verb) && r.case.is_some())
    })
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
    // A word that is its own base is the lemma; otherwise the best base is.
    // The choice matters for a compound head: `parin` is both `pari` and the
    // golf term `par`, and only `pari` makes `nuorenparin` a pair.
    if builder.bases_of(word).contains(&word) {
        return Some(word.to_string());
    }
    builder.best_base(word).map(str::to_string)
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
                    part(
                        "aamu",
                        "aamu",
                        Role::Modifier,
                        ComponentInflection::Invariant
                    ),
                    part("kahvi", "kahvi", Role::Head, ComponentInflection::Inflect),
                ],
                head_index: 1,
                kind: CompoundKind::Ordinary,
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

    /// One expected component, with no frozen case.
    fn part(surface: &str, lemma: &str, role: Role, inflection: ComponentInflection) -> Component {
        Component {
            surface: surface.to_string(),
            lemma: lemma.to_string(),
            role,
            inflection,
            case: None,
        }
    }

    #[test]
    fn a_genitive_modifier_is_recorded_as_frozen() {
        let b = builder(vec![noun("talo", 1), noun("mies", 42)]);
        let got = analyze("talonmies", &b);
        let Analysis::Compound(c) = got else {
            panic!("expected a compound, got {got:?}");
        };
        assert_eq!(c.kind, CompoundKind::GenitiveInitial);
        assert_eq!(c.parts[0].inflection, ComponentInflection::Lexical);
        assert_eq!(c.parts[0].case, Some(Case::Genitive));
        assert_eq!(c.parts[1].inflection, ComponentInflection::Inflect);
    }

    #[test]
    fn a_class_51_word_inflects_its_first_part() {
        // The classifier is asked directly: a class-51 row cannot live in a
        // test lexicon, because `LexiconIndex` indexes the embedded orders.
        // `nuoripari` is a real sanalista row, so the class check sees it.
        let b = builder(vec![noun("nuori", 1), noun("pari", 1)]);
        let mut parts = vec![
            part(
                "nuori",
                "nuori",
                Role::Modifier,
                ComponentInflection::Invariant,
            ),
            part("pari", "pari", Role::Head, ComponentInflection::Inflect),
        ];
        let kind = classify(&mut parts, 1, "nuoripari", None, &b);
        assert_eq!(kind, CompoundKind::BothParts);
        assert_eq!(parts[0].inflection, ComponentInflection::Conditional);
        assert_eq!(parts[1].inflection, ComponentInflection::Inflect);
    }

    #[test]
    fn a_coordinated_pair_inflects_both_parts_without_a_class() {
        // The hyphen is not the signal: `linja-auto` is ordinary. The pair is
        // `-lainen`/`-lais-`, and neither half is a class-51 row.
        let b = builder(vec![
            noun("suomalais", 1),
            noun("ruotsalainen", 1),
            noun("linja", 1),
            noun("auto", 1),
        ]);
        let got = analyze("suomalais-ruotsalainen", &b);
        let Analysis::Compound(c) = got else {
            panic!("expected a compound, got {got:?}");
        };
        assert_eq!(c.kind, CompoundKind::BothParts);
        assert_eq!(c.parts[0].inflection, ComponentInflection::Conditional);

        let got = analyze("linja-auto", &b);
        let Analysis::Compound(c) = got else {
            panic!("expected a compound, got {got:?}");
        };
        assert_eq!(c.kind, CompoundKind::Ordinary);
        assert_eq!(c.parts[0].inflection, ComponentInflection::Invariant);
    }

    #[test]
    fn explain_separates_a_frozen_modifier_from_an_inflected_head() {
        let b = builder(vec![noun("talo", 1), noun("mies", 42)]);
        let got = explain("talonmiehen", &b).expect("talonmiehen is a compound");
        assert_eq!(got.kind, CompoundKind::GenitiveInitial);
        assert_eq!(got.components[0].action, Action::Frozen);
        assert_eq!(got.components[1].action, Action::Inflected);
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
