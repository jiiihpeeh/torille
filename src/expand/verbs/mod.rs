//! Hand-written per-class verb engines.
//!
//! Each module inflects one Kotus verb type from its lemma, the way the
//! `expand::nominals` modules inflect a noun class. A class module works out
//! the stems its type gives (`sanoa` -> `sano-`) and hands them to
//! [`utility::VerbBuilder`], which applies the endings every verb shares. The
//! dispatcher here routes a class string to its module.
//!
//! A class whose module is not written yet returns `None`, and the older
//! type-driven engine in `grammar::verb` still inflects it; a class that is
//! written is authoritative, because the modules reproduce the whole paradigm
//! their class gives.

#![allow(dead_code)]

use crate::kotus::parse_single;
use crate::tags::Form;

pub mod utility;

pub mod class_52;
pub mod class_53;
pub mod class_54;
pub mod class_55;
pub mod class_56;
pub mod class_57;
pub mod class_58;
pub mod class_59;
pub mod class_60;
pub mod class_61;
pub mod class_62;
pub mod class_63;
pub mod class_64;
pub mod class_65;
pub mod class_66;
pub mod class_67;
pub mod class_68;
pub mod class_69;
pub mod class_70;
pub mod class_71;
pub mod class_72;
pub mod class_73;
pub mod class_74;
pub mod class_75;
pub mod class_76;
pub mod class_77;
pub mod class_78;

pub mod compound;

/// Lemmas the sanalista gives both a 72 and a 74 reading, where only one
/// reading is the verb the reference confirms.
///
/// The two types are close -- both are `-ta/-tä` change-of-state verbs -- and
/// the sanalista lists some members under both. `kotus_verbit.sqlite` decides
/// which one is real: for `oieta`, `raueta`, `isota`, `sietä` and `sueta` the
/// class-72 paradigm matches 982 of 986 `both` forms (or all of the union),
/// while class 74 invents ~680 forms; for `aueta`, `tyvetä`, `iljetä` and
/// `juljeta` it is the other way round. Suppressing the losing reading keeps
/// the index from naming `oikeaa` as a form of `oieta`.
const CLASS_72_WINS: &[&str] = &["oieta", "raueta", "isota", "sietä", "sueta"];
const CLASS_74_WINS: &[&str] = &["aueta", "tyvetä", "iljetä", "juljeta"];

/// Whether `class`'s paradigm for `lemma` is the one the reference rejects.
///
/// This is a lexical decision, not a stem rule: the two classes use the same
/// stem and differ only in the theme vowel. The list is the nine dual-reading
/// lemmas the class-72 module names in its own documentation.
pub fn suppresses_reading(lemma: &str, class: u16) -> bool {
    match class {
        72 => CLASS_74_WINS.contains(&lemma),
        74 => CLASS_72_WINS.contains(&lemma),
        _ => false,
    }
}

/// Inflect `lemma` as `class`, or `None` if no hand-written module covers it.
pub fn dispatch(class: &str, lemma: &str) -> Option<Vec<Form>> {
    // A class string carries the lemma's gradation code after the number
    // (`53*C`), which is a fact about the lemma rather than about the type:
    // type 1 (classes 52 and 53) gradates in a fixed pattern of cells, so the
    // code alone decides every alternation in the paradigm. The caller passes
    // one class at a time, so a lemma the sanalista gives two readings for
    // (`53*F, (54*F)`) arrives here as `53*F` and again as `54*F`, and the
    // second is what the reference inflects.
    let inflection = parse_single(class)?;
    match inflection.class {
        52 => Some(class_52::inflect(lemma, inflection.gradation)),
        53 => Some(class_53::inflect(lemma, inflection.gradation)),
        54 => Some(class_54::inflect(lemma, inflection.gradation)),
        55 => Some(class_55::inflect(lemma, inflection.gradation)),
        56 => Some(class_56::inflect(lemma, inflection.gradation)),
        57 => Some(class_57::inflect(lemma, inflection.gradation)),
        58 => Some(class_58::inflect(lemma, inflection.gradation)),
        59 => Some(class_59::inflect(lemma, inflection.gradation)),
        60 => Some(class_60::inflect(lemma, inflection.gradation)),
        61 => Some(class_61::inflect(lemma, inflection.gradation)),
        62 => Some(class_62::inflect(lemma)),
        63 => Some(class_63::inflect(lemma)),
        64 => Some(class_64::inflect(lemma)),
        65 => Some(class_65::inflect(lemma)),
        66 => Some(class_66::inflect(lemma, inflection.gradation)),
        67 => Some(class_67::inflect(lemma, inflection.gradation)),
        68 => Some(class_68::inflect(lemma)),
        69 => Some(class_69::inflect(lemma)),
        70 => Some(class_70::inflect(lemma)),
        71 => Some(class_71::inflect(lemma)),
        72 => Some(class_72::inflect(lemma, inflection.gradation)),
        73 => Some(class_73::inflect(lemma, inflection.gradation)),
        74 => Some(class_74::inflect(lemma, inflection.gradation)),
        75 => Some(class_75::inflect(lemma, inflection.gradation)),
        76 => Some(class_76::inflect(lemma, inflection.gradation)),
        77 => Some(class_77::inflect(lemma)),
        78 => Some(class_78::inflect(lemma)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::suppresses_reading;

    /// The dual 72/74 lemmas keep one reading only; a single-class lemma of
    /// either type keeps its own.
    #[test]
    fn a_dual_72_74_lemma_keeps_the_reference_reading() {
        for lemma in ["oieta", "raueta", "isota", "sietä", "sueta"] {
            assert!(suppresses_reading(lemma, 74), "{lemma} is not class 74");
            assert!(!suppresses_reading(lemma, 72), "{lemma} is class 72");
        }
        for lemma in ["aueta", "tyvetä", "iljetä", "juljeta"] {
            assert!(suppresses_reading(lemma, 72), "{lemma} is not class 72");
            assert!(!suppresses_reading(lemma, 74), "{lemma} is class 74");
        }
        assert!(!suppresses_reading("vanheta", 72));
        assert!(!suppresses_reading("katketa", 74));
    }
}
