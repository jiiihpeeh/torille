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
