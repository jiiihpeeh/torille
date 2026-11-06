//! Generated per-class inflection rules.
//!
//! Produced by tools/oracle/extract_class_rules.py --rust from the noun
//! lexicon. do not edit by hand; regenerate.
#![allow(dead_code)]

use crate::tags::Form;
pub mod utility;
pub mod gradated;
pub mod class_1;
pub mod harmony;
pub mod class_1A;
pub mod class_1B;
pub mod class_1C;
pub mod class_1D;
pub mod class_1E;
pub mod class_1F;
pub mod class_1G;
pub mod class_1H;
pub mod class_1M;
pub mod class_1K;
pub mod class_1J;
pub mod class_1I;
pub mod class_10;
pub mod class_10A;
pub mod class_10B;
pub mod class_10C;
pub mod class_10D;
pub mod class_10E;
pub mod class_10F;
pub mod class_10G;
pub mod class_10H;
pub mod class_10I;
pub mod class_10J;
pub mod class_10L;
pub mod class_101;
pub mod class_11;
pub mod class_12;
pub mod class_13;
pub mod class_14A;
pub mod class_14B;
pub mod class_14C;
pub mod class_15;
pub mod class_16H;
pub mod class_17;
pub mod class_18;
pub mod class_19;
pub mod class_2;
pub mod class_20;
pub mod class_21;
pub mod class_22;
pub mod class_23;
pub mod class_24;
pub mod class_25;
pub mod class_26;
pub mod class_27;
pub mod class_28I;
pub mod class_28J;
pub mod class_28K;
pub mod class_29;
pub mod class_3;
pub mod class_30;
pub mod class_31;
pub mod class_32;
pub mod class_32C;
pub mod class_32D;
pub mod class_33;
pub mod class_33B;
pub mod class_33C;
pub mod class_33D;
pub mod class_33E;
pub mod class_33F;
pub mod class_33I;
pub mod class_33J;
pub mod class_33K;
pub mod class_33L;
pub mod class_34;
pub mod class_34C;
pub mod class_35H;
pub mod class_36;
pub mod class_37;
pub mod class_38;
pub mod class_39;
pub mod class_40;
pub mod class_41;
pub mod class_41A;
pub mod class_41B;
pub mod class_41C;
pub mod class_41D;
pub mod class_41E;
pub mod class_41F;
pub mod class_41G;
pub mod class_41H;
pub mod class_41I;
pub mod class_41J;
pub mod class_41K;
pub mod class_42;
pub mod class_43;
pub mod class_44;
pub mod class_45;
pub mod class_46;
pub mod class_47;
pub mod class_48;
pub mod class_48A;
pub mod class_48B;
pub mod class_48C;
pub mod class_48D;
pub mod class_48E;
pub mod class_48F;
pub mod class_48H;
pub mod class_48I;
pub mod class_48J;
pub mod class_48K;
pub mod class_48L;
pub mod class_49;
pub mod class_49D;
pub mod class_49E;
pub mod class_49F;
pub mod class_49G;
pub mod class_49H;
pub mod class_49J;
pub mod class_4A;
pub mod class_5;
pub mod class_5A;
pub mod class_5B;
pub mod class_5C;
pub mod class_5D;
pub mod class_5E;
pub mod class_5F;
pub mod class_5G;
pub mod class_5I;
pub mod class_5J;
pub mod class_50;
pub mod class_51;
pub mod class_6;
pub mod class_7;
pub mod class_7B;
pub mod class_7D;
pub mod class_7E;
pub mod class_7F;
pub mod class_7G;
pub mod class_7H;
pub mod class_7L;
pub mod class_8;
pub mod class_8A;
pub mod class_8B;
pub mod class_8C;
pub mod class_9;
pub mod class_9A;
pub mod class_9B;
pub mod class_9C;
pub mod class_9D;
pub mod class_9E;
pub mod class_9F;
pub mod class_9G;
pub mod class_9H;
pub mod class_9I;
pub mod class_9J;
pub mod class_9K;
pub mod class_99;

// One row per class the engine knows about, in the order the sanalista
// records it, with each entry's `inflect(lemma, possessives) -> Vec<Form>`
// function. Class 101 is the irregular pronoun paradigm -- it has no
// possessive column, so its `inflect` has a different signature and
// sits in its own early return inside `dispatch` below.
const DISPATCH: &[(&str, fn(&str, bool) -> Vec<Form>)] = &[
    ("1", class_1::inflect),
    ("1*A", class_1A::inflect),
    ("1*B", class_1B::inflect),
    ("1*C", class_1C::inflect),
    ("1*D", class_1D::inflect),
    ("1*E", class_1E::inflect),
    ("1*F", class_1F::inflect),
    ("1*G", class_1G::inflect),
    ("1*H", class_1H::inflect),
    ("1*I", class_1I::inflect),
    ("1*J", class_1J::inflect),
    ("1*K", class_1K::inflect),
    ("1*M", class_1M::inflect),
    ("10", class_10::inflect),
    ("10*A", class_10A::inflect),
    ("10*B", class_10B::inflect),
    ("10*C", class_10C::inflect),
    ("10*D", class_10D::inflect),
    ("10*E", class_10E::inflect),
    ("10*F", class_10F::inflect),
    ("10*G", class_10G::inflect),
    ("10*H", class_10H::inflect),
    ("10*I", class_10I::inflect),
    ("10*J", class_10J::inflect),
    ("10*L", class_10L::inflect),
    ("11", class_11::inflect),
    ("12", class_12::inflect),
    ("13", class_13::inflect),
    ("14*A", class_14A::inflect),
    ("14*B", class_14B::inflect),
    ("14*C", class_14C::inflect),
    ("15", class_15::inflect),
    ("16*H", class_16H::inflect),
    ("17", class_17::inflect),
    ("18", class_18::inflect),
    ("19", class_19::inflect),
    ("2", class_2::inflect),
    ("20", class_20::inflect),
    ("21", class_21::inflect),
    ("22", class_22::inflect),
    ("23", class_23::inflect),
    ("24", class_24::inflect),
    ("25", class_25::inflect),
    ("26", class_26::inflect),
    ("27", class_27::inflect),
    ("28*I", class_28I::inflect),
    ("28*J", class_28J::inflect),
    ("28*K", class_28K::inflect),
    ("29", class_29::inflect),
    ("3", class_3::inflect),
    ("30", class_30::inflect),
    ("31", class_31::inflect),
    ("32", class_32::inflect),
    ("32*C", class_32C::inflect),
    ("32*D", class_32D::inflect),
    ("33", class_33::inflect),
    ("33*B", class_33B::inflect),
    ("33*C", class_33C::inflect),
    ("33*D", class_33D::inflect),
    ("33*E", class_33E::inflect),
    ("33*F", class_33F::inflect),
    ("33*I", class_33I::inflect),
    ("33*J", class_33J::inflect),
    ("33*K", class_33K::inflect),
    ("33*L", class_33L::inflect),
    ("34", class_34::inflect),
    ("34*C", class_34C::inflect),
    ("35*H", class_35H::inflect),
    ("36", class_36::inflect),
    ("37", class_37::inflect),
    ("38", class_38::inflect),
    ("39", class_39::inflect),
    ("4*A", class_4A::inflect),
    ("40", class_40::inflect),
    ("41", class_41::inflect),
    ("41*A", class_41A::inflect),
    ("41*B", class_41B::inflect),
    ("41*C", class_41C::inflect),
    ("41*D", class_41D::inflect),
    ("41*E", class_41E::inflect),
    ("41*F", class_41F::inflect),
    ("41*G", class_41G::inflect),
    ("41*H", class_41H::inflect),
    ("41*I", class_41I::inflect),
    ("41*J", class_41J::inflect),
    ("41*K", class_41K::inflect),
    ("42", class_42::inflect),
    ("43", class_43::inflect),
    ("44", class_44::inflect),
    ("45", class_45::inflect),
    ("46", class_46::inflect),
    ("47", class_47::inflect),
    ("48", class_48::inflect),
    ("48*A", class_48A::inflect),
    ("48*B", class_48B::inflect),
    ("48*C", class_48C::inflect),
    ("48*D", class_48D::inflect),
    ("48*E", class_48E::inflect),
    ("48*F", class_48F::inflect),
    ("48*H", class_48H::inflect),
    ("48*I", class_48I::inflect),
    ("48*J", class_48J::inflect),
    ("48*K", class_48K::inflect),
    ("48*L", class_48L::inflect),
    ("49", class_49::inflect),
    ("49*D", class_49::inflect_d),
    ("49*E", class_49::inflect_e),
    ("49*F", class_49::inflect_f),
    ("49*G", class_49::inflect_g),
    ("49*H", class_49H::inflect),
    ("49*J", class_49J::inflect),
    ("5", class_5::inflect),
    ("5*A", class_5A::inflect),
    ("5*B", class_5B::inflect),
    ("5*C", class_5C::inflect),
    ("5*D", class_5D::inflect),
    ("5*E", class_5E::inflect),
    ("5*F", class_5F::inflect),
    ("5*G", class_5G::inflect),
    ("5*I", class_5I::inflect),
    ("5*J", class_5J::inflect),
    ("50", class_50::inflect),
    ("51", class_51::inflect),
    ("6", class_6::inflect),
    ("7", class_7::inflect),
    ("7*B", class_7B::inflect),
    ("7*D", class_7D::inflect),
    ("7*E", class_7E::inflect),
    ("7*F", class_7F::inflect),
    ("7*G", class_7G::inflect),
    ("7*H", class_7H::inflect),
    ("7*L", class_7L::inflect),
    ("8", class_8::inflect),
    ("8*(C)", class_8C::inflect),
    ("8*A", class_8A::inflect),
    ("8*B", class_8B::inflect),
    ("9", class_9::inflect),
    ("9*A", class_9A::inflect),
    ("9*B", class_9B::inflect),
    ("9*C", class_9C::inflect),
    ("9*D", class_9D::inflect),
    ("9*E", class_9E::inflect),
    ("9*F", class_9F::inflect),
    ("9*G", class_9G::inflect),
    ("9*H", class_9H::inflect),
    ("9*I", class_9I::inflect),
    ("9*J", class_9J::inflect),
    ("9*K", class_9K::inflect),
    ("99", class_99::inflect),
];

/// Route a lemma to its class's generated module, by the class
/// value the sanalista records (`9` and `9*B` are two keys).
pub fn dispatch(class: &str, lemma: &str, possessives: bool) -> Option<Vec<Form>> {
    // Class 101 is the irregular pronoun class, whose paradigm takes no
    // possessive column, so its `inflect` has a different signature and
    // sits in its own early return.
    if class == "101" {
        return Some(class_101::inflect(lemma));
    }
    DISPATCH.iter()
        .find(|(key, _)| *key == class)
        .map(|(_, inflect)| inflect(lemma, possessives))
}

// The classes whose hand-written engine has been verified against the
// sanalista ground truth. A verified engine replaces the heuristic
// engine's forms; an unverified engine is additive -- its forms join
// the heuristic's without replacing them. The set lives here, next to
// the dispatch table it qualifies, rather than in `expand::mod`, so
// "is this class authoritative?" stays a property of the dispatch
// tree rather than of the heuristic that depends on it.
const AUTHORITATIVE_CLASSES: &[&str] = &[
    "1", "1*A", "1*B", "1*C", "1*D", "1*E", "1*F", "1*G", "1*H",
    "1*I", "1*J", "1*K", "1*M", "2", "3", "4*A", "5",
    "5*A", "5*B", "5*C", "5*D", "5*E", "5*F", "5*G", "5*I", "5*J",
    "6", "7",
    "7*B", "7*D", "7*E", "7*F", "7*G", "7*H", "7*L", "8",
    "8*A", "8*B", "8*(C)",
    "9",
    "9*A", "9*B", "9*C", "9*D", "9*E", "9*F", "9*G", "9*H", "9*I", "9*J", "9*K",
    "10",
    "10*A", "10*B", "10*C", "10*D", "10*E", "10*F", "10*G", "10*H", "10*I", "10*J", "10*L",
    "11", "12", "13", "14*A", "14*B", "14*C",
    "15", "16*H", "17", "18", "19", "20",
    "21", "22", "23", "24", "25", "26", "27",
    "28*I", "28*J", "28*K",
    "29", "30",
    "31", "32", "32*C", "32*D",
    "33", "33*B", "33*C", "33*D", "33*E", "33*F", "33*I", "33*J", "33*K", "33*L",
    "34", "34*C", "35*H",
    "36", "37",
    "38",
    "39",
    "40",
    "41", "41*A", "41*B", "41*C", "41*D", "41*E", "41*F", "41*G", "41*H", "41*I", "41*J", "41*K",
    "42",
    "43", "44", "45", "46", "47", "49", "49*D", "49*E", "49*F", "49*G", "49*H", "49*J",
    "48", "48*(E)", "48*A", "48*B", "48*C", "48*D", "48*E", "48*F", "48*H", "48*I", "48*J", "48*K", "48*L",
    "99", "101",
];

/// Whether this class's hand-written engine has been verified against
/// the sanalista ground truth, and therefore replaces the heuristic
/// engine's forms rather than joining them. The caller in `expand::mod`
/// uses this to decide whether to drop the heuristic output before
/// appending the hand-written engine's forms.
pub fn is_authoritative(class: &str) -> bool {
    AUTHORITATIVE_CLASSES.contains(&class)
}

#[cfg(test)]
mod tests {
    use super::{class_1, dispatch};
    use crate::tags::{Confidence, Number};

    // The generated rule was read off the model word `äly`, so it
    // reproduces `äly`'s paradigm, and dispatch routes a class
    // value to it. Reproducing *other* words of the class needs the
    // harmony step (a/ä, o/ö), which is the next piece.
    #[test]
    fn class_1_reproduces_its_model() {
        let forms = class_1::inflect("äly", true);
        let words: std::collections::HashSet<String> =
            forms.iter().map(|f| f.word.clone()).collect();
        for want in ["äly", "älyn", "älyä", "älyyn", "älyssä", "älyksi",
                     "älyt", "älyjen", "älyihin", "älyni", "älyssäni"] {
            assert!(words.contains(want), "class 1 model missing {want}");
        }
        for f in forms.iter().filter(|f| f.word == "älyksi") {
            assert_eq!(f.confidence, Confidence::Plain);
        }
    }

    #[test]
    fn dispatch_routes_a_class_to_its_module() {
        // class 1 routes to class_1; an unknown class is None.
        let via = dispatch("1", "talo", true);
        assert!(via.is_some(), "dispatch(\"1\", talo) should route");
        assert!(dispatch("9999", "talo", true).is_none());
        assert!(dispatch("27", "käsi", true).is_some());
    }

    /// A `-t`-final lemma is a plural-only citation, so the class engine
    /// declines the dropped-`t` base and keeps only its plural cells.
    #[test]
    fn a_plural_only_t_lemma_keeps_only_plural_cells() {
        for (class, lemma) in [
            ("20", "bileet"),
            ("3", "kapiot"),
            ("2", "illatsut"),
            ("18", "häät"),
            ("5*G", "juomingit"),
        ] {
            let forms = dispatch(class, lemma, false).expect("routes");
            assert!(!forms.is_empty(), "{lemma} produced no forms");
            assert!(
                forms
                    .iter()
                    .all(|form| form.union().number == Some(Number::Plural)),
                "{lemma} produced a non-plural cell"
            );
            assert!(
                forms.iter().any(|form| form.word == lemma),
                "{lemma} is not its own nominative plural"
            );
        }
    }

    /// A two-character lemma keeps its nominative. The 16 two-character forms
    /// the class engines emit are all lemmas' own nominatives (`se`, `yö`,
    /// `ja`, ...), not fragments, so the verb side's three-character guard has
    /// no counterpart here: applying it would delete real words.
    #[test]
    fn a_two_character_lemma_keeps_its_nominative() {
        for (class, lemma) in [("101", "se"), ("99", "ja"), ("18", "EU")] {
            let forms = dispatch(class, lemma, true).expect("routes");
            assert!(
                forms.iter().any(|f| f.word == lemma),
                "{class} {lemma}: nominative lost"
            );
        }
    }
}
