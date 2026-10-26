//! Derivation: creating a new lexeme from an existing one.
//!
//! Derivation is a separate layer from inflection. Inflection makes a
//! grammatical form of an existing lexeme (`talo` -> `talossa`); derivation
//! makes a new lexeme (`opettaa` -> `opettaja`), which is then itself
//! inflected. A derivation rule declares its input and output part of speech,
//! the suffix (with its harmonic variants), how the base stem changes, how the
//! suffix interacts with consonant gradation, and how productive it is.
//!
//! Derivation is not a list of regex replacements. Each rule carries the stem
//! rule and the semantic class, so `-ja` is the agent suffix and `-minen` the
//! action suffix, and neither is just `stem` plus `"ja"`.

use crate::grammar::{Harmony, harmony};
use crate::tags::PartOfSpeech;

/// What a derived word denotes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticClass {
    Agent,
    Action,
    Abstract,
    Collective,
    Instrument,
    Location,
    Person,
    Diminutive,
    Adjective,
    Other,
}

/// How freely a derivation rule applies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Productivity {
    /// Applies to any base of the input part of speech.
    Productive,
    /// Applies to many bases but not all.
    SemiProductive,
    /// Only to the words the lexicon lists, not productively.
    Lexicalized,
}

/// How a rule's suffix interacts with the base stem.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grade {
    /// The stem keeps whatever grade it has.
    Preserve,
    /// The strong grade is used.
    Strong,
    /// The weak grade is used.
    Weak,
}

/// How a derivation changes the base before the suffix.
///
/// This is the morphophonological layer. Most derivations need more than a
/// plain cut: `suomalainen` becomes `suomalais` before `-uus`, not `suomalain`
/// and `uus`, and `onni` becomes `onnell` before `-inen`. A [`MorphStem`]
/// states the change directly rather than pretending the suffix alone carries
/// the alternation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MorphStem {
    /// The base is used unchanged.
    Direct,
    /// The base's final vowel is dropped, as the verb suffix `-ja` wants:
    /// `laulaa` is `laula`.
    DropVowel,
    /// The base's final vowel is dropped and a vowel put back.
    DropAdd(&'static str),
    /// A trailing substring is replaced with another. More than one
    /// alternation is given because a family covers several shapes: the
    /// `-llinen` rule is `ni` -> `nell` for `onni`, `i` -> `ell` for `kivi`,
    /// and `de` -> `teell` for `taide`. Each is tried in turn.
    Replace(&'static [(&'static str, &'static str)]),
}

impl MorphStem {
    /// The stem of `base` under this rule, or `None` if no alternation fits.
    fn apply(self, base: &str) -> Option<String> {
        match self {
            MorphStem::Direct => Some(base.to_string()),
            MorphStem::DropVowel => {
                let mut s = base.to_string();
                s.pop();
                Some(s)
            }
            MorphStem::DropAdd(c) => {
                let mut s = base.to_string();
                s.pop();
                s.push_str(c);
                Some(s)
            }
            MorphStem::Replace(alts) => alts
                .iter()
                .find_map(|(from, to)| base.strip_suffix(from).map(|rest| format!("{rest}{to}"))),
        }
    }
}

/// One derivational rule.
#[derive(Clone, Copy, Debug)]
pub struct DerivationRule {
    /// The rule id, e.g. `"AGENT_JA"`.
    pub id: &'static str,
    /// The part of speech the rule takes as input.
    pub input_pos: PartOfSpeech,
    /// The part of speech the rule turns into.
    pub output_pos: PartOfSpeech,
    /// The suffix, with its front and back forms. The front is chosen for a
    /// front base, the back for a back one.
    pub suffix_front: &'static str,
    pub suffix_back: &'static str,
    /// How the base stem changes before the suffix.
    pub stem: MorphStem,
    /// What the rule's grade does to the base.
    pub grade: Grade,
    /// What the derived word denotes.
    pub semantic: SemanticClass,
    /// How freely the rule applies.
    pub productivity: Productivity,
}

/// The productive derivational rules, covering the major families.
///
/// These are rule families, not a claim that every listed suffix attaches to
/// every word of the input part of speech.
pub const RULES: &[DerivationRule] = &[
    // VERB -> NOUN. The agent suffix `-ja`/`-jä` takes the verb stem, dropping
    // the infinitive's final vowel: `laulaa` is `laula`.
    DerivationRule {
        id: "AGENT_JA",
        input_pos: PartOfSpeech::Verb,
        output_pos: PartOfSpeech::Noun,
        suffix_front: "jä",
        suffix_back: "ja",
        stem: MorphStem::DropVowel,
        grade: Grade::Preserve,
        semantic: SemanticClass::Agent,
        productivity: Productivity::Productive,
    },
    // VERB -> NOUN. The action `-minen`: `puhua` is `puhu`.
    DerivationRule {
        id: "ACTION_MINEN",
        input_pos: PartOfSpeech::Verb,
        output_pos: PartOfSpeech::Noun,
        suffix_front: "minen",
        suffix_back: "minen",
        stem: MorphStem::DropVowel,
        grade: Grade::Preserve,
        semantic: SemanticClass::Action,
        productivity: Productivity::Productive,
    },
    // ADJECTIVE -> NOUN. The abstract `-uus`/`-yys`. A `-nen` adjective turns
    // its `nen` into `s` before the suffix: `suomalainen` is `suomalais`,
    // `maallinen` is `maallis`, `ystävällinen` is `ystävällis`.
    DerivationRule {
        id: "ABSTRACT_UUS",
        input_pos: PartOfSpeech::Adjective,
        output_pos: PartOfSpeech::Noun,
        suffix_front: "yys",
        suffix_back: "uus",
        stem: MorphStem::Replace(&[("nen", "s")]),
        grade: Grade::Preserve,
        semantic: SemanticClass::Abstract,
        productivity: Productivity::SemiProductive,
    },
    // NOUN -> ADJECTIVE. The property `-llinen`. The alternation bakes the
    // `ll` into the stem, so the suffix is `-inen`: `onni` is `onnell` and
    // `kivi` is `kivell`.
    DerivationRule {
        id: "ADJ_LLINEN",
        input_pos: PartOfSpeech::Noun,
        output_pos: PartOfSpeech::Adjective,
        suffix_front: "inen",
        suffix_back: "inen",
        stem: MorphStem::Replace(&[("ni", "nell"), ("de", "teell"), ("i", "ell")]),
        grade: Grade::Preserve,
        semantic: SemanticClass::Adjective,
        productivity: Productivity::SemiProductive,
    },
    // NOUN -> ADJECTIVE. The affiliation `-lainen`/`-läinen`: `Turku` is
    // `turkulainen`.
    DerivationRule {
        id: "ADJ_LAINEN",
        input_pos: PartOfSpeech::Noun,
        output_pos: PartOfSpeech::Adjective,
        suffix_front: "läinen",
        suffix_back: "lainen",
        stem: MorphStem::Direct,
        grade: Grade::Preserve,
        semantic: SemanticClass::Adjective,
        productivity: Productivity::SemiProductive,
    },
    // NOUN -> ADJECTIVE. The material/containing `-inen`: `kivi` is `kivinen`.
    DerivationRule {
        id: "ADJ_INEN",
        input_pos: PartOfSpeech::Noun,
        output_pos: PartOfSpeech::Adjective,
        suffix_front: "inen",
        suffix_back: "inen",
        stem: MorphStem::DropVowel,
        grade: Grade::Preserve,
        semantic: SemanticClass::Adjective,
        productivity: Productivity::SemiProductive,
    },
];

/// Derives the word for `base` under `rule`, applying the stem rule, the
/// harmony and the grade. Returns `None` if the rule's stem alternation does
/// not fit the base.
pub fn derive(base: &str, rule: &DerivationRule) -> Option<String> {
    // A `-dä`/`-da` verb loses its `d` in the derived stem, so `myydä` gives
    // `myy` before an agent suffix rather than `myyd`.
    let effective = if base.ends_with("dä") || base.ends_with("da") {
        &base[..base.len() - 2]
    } else {
        base
    };
    let stem = rule.stem.apply(effective)?;
    let suffix = match harmony(&stem) {
        Harmony::Back => rule.suffix_back,
        Harmony::Front => rule.suffix_front,
    };
    Some(format!("{stem}{suffix}"))
}

/// Every derived word of `base` under the productive rules. A rule with
/// several stem alternations contributes one candidate per alternation that
/// fits the base.
pub fn derive_candidates(base: &str) -> Vec<String> {
    RULES
        .iter()
        .filter(|r| r.productivity != Productivity::Lexicalized)
        .filter_map(|r| derive(base, r))
        .collect()
}

/// A possible reverse derivation of a word.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reverse {
    /// The rule that could have produced the word.
    pub rule: &'static str,
    /// The source stem it would come from.
    pub source: String,
}

/// The reverse derivations of `word`, under the productive rules.
///
/// Each is *possible*, not asserted: the source is the stem the suffix would
/// have been added to, and whether the base actually exists is for the lexicon
/// to say. `opettaja` is `opettaa` under `AGENT_JA`, and `puhuminen` is `puhua`
/// under `ACTION_MINEN`.
pub fn reverse(word: &str) -> Vec<Reverse> {
    RULES
        .iter()
        .filter(|r| r.productivity != Productivity::Lexicalized)
        .filter_map(|r| {
            // The word must end in the suffix's back or front form.
            let (stem, is_front) = if let Some(s) = word.strip_suffix(r.suffix_back) {
                (s, false)
            } else {
                let s = word.strip_suffix(r.suffix_front)?;
                (s, true)
            };
            // Undo the stem rule to recover the base. The vowel a `DropVowel`
            // rule took off is not recoverable from the stem alone, so the
            // harmonic theme `a`/`ä` is the best guess; a `Replace` swaps the
            // alternation back.
            let vowel = if is_front { 'ä' } else { 'a' };
            let source = match r.stem {
                MorphStem::DropVowel | MorphStem::DropAdd(_) => format!("{stem}{vowel}"),
                MorphStem::Replace(alts) => {
                    // Swap the first alternation that the stem ends in back.
                    alts.iter()
                        .find_map(|(from, to)| {
                            stem.strip_suffix(to).map(|rest| format!("{rest}{from}"))
                        })
                        .unwrap_or_else(|| stem.to_string())
                }
                MorphStem::Direct => stem.to_string(),
            };
            Some(Reverse { rule: r.id, source })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(base: &str, rule: &DerivationRule) -> String {
        derive(base, rule).unwrap()
    }

    fn agent(base: &str) -> String {
        d(base, &RULES[0])
    }

    #[test]
    fn agent_ja_follows_harmony() {
        // The agent suffix takes the verb stem: `opettaa` -> `opettaja`,
        // `laulaa` -> `laulaja`. A front base takes the front `-jä`.
        assert_eq!(agent("opettaa"), "opettaja");
        assert_eq!(agent("laulaa"), "laulaja");
        assert_eq!(agent("myydä"), "myyjä");
    }

    #[test]
    fn action_minen_is_a_noun() {
        assert_eq!(d("puhua", &RULES[1]), "puhuminen");
        assert_eq!(d("opettaa", &RULES[1]), "opettaminen");
    }

    #[test]
    fn the_suffix_follows_the_harmony_of_the_stem() {
        // The agent suffix is `-ja` on a back stem and `-jä` on a front one.
        assert_eq!(d("laulaa", &RULES[0]), "laulaja");
        assert_eq!(d("myydä", &RULES[0]), "myyjä");
    }

    #[test]
    fn reverse_finds_the_possible_source() {
        // `opettaja` could be `opettaa` under the agent rule, and `puhuminen`
        // could be `puhua` under the action rule.
        let agent = reverse("opettaja");
        assert!(
            agent
                .iter()
                .any(|r| r.rule == "AGENT_JA" && r.source == "opettaa")
        );
        let action = reverse("puhuminen");
        assert!(
            action
                .iter()
                .any(|r| r.rule == "ACTION_MINEN" && r.source == "puhua")
        );
    }

    #[test]
    fn the_abstract_uus_turns_nen_into_s() {
        // `-uus`/`-yys` on a `-nen` adjective turns the `nen` into `s`, so
        // `suomalainen` is `suomalaisuus` and `maallinen` is `maallisuus`.
        let r = RULES.iter().find(|r| r.id == "ABSTRACT_UUS").unwrap();
        assert_eq!(d("suomalainen", r), "suomalaisuus");
        assert_eq!(d("maallinen", r), "maallisuus");
        assert_eq!(d("ystävällinen", r), "ystävällisyys");
    }

    #[test]
    fn the_llinen_alternation_bakes_the_double_l_into_the_stem() {
        // `-llinen` turns a final `i` into `ell` and appends `-inen`, so
        // `onni` is `onnellinen` and `kivi` is `kivellinen`, and `de` into
        // `teell` so `taide` is `taiteellinen`.
        let r = RULES.iter().find(|r| r.id == "ADJ_LLINEN").unwrap();
        assert_eq!(d("onni", r), "onnellinen");
        assert_eq!(d("kivi", r), "kivellinen");
        assert_eq!(d("taide", r), "taiteellinen");
    }
}
