//! Generation of the forms the Kotus catalogue leaves out.
//!
//! The catalogue in `data/kotus-tyypit.txt` gives eight forms per class: for a
//! noun the nominative, genitive, partitive and illative, singular and plural.
//! Finnish has fourteen cases, and the inessive alone is far more common than
//! the four the catalogue does carry, so this module fills in the rest from the
//! stem rather than from a word list.
//!
//! Two things make that possible without a table of irregular forms. Vowel
//! harmony is applied per candidate stem, and a class contributes more than one
//! candidate stem wherever the members disagree about where the stem ends:
//! class 7 covers both `ovi`, whose stem is `ove`, and `joki`, whose stem is
//! `jo`. Every candidate is generated, so the union covers the class even though
//! no single rule does.
//!
//! The classes whose model word is unlike its own members do need a table, and
//! that is what [`irregular`] is for; see its module docs.
//!
//! These are generated forms, not words from the list. Callers should keep them
//! out of the suggestion set; see `FormBuilder` in `expand.rs`.

mod harmony;
mod irregular;
mod noun;
mod stem;
mod verb;

pub(crate) use harmony::harmonise;
pub use harmony::{Harmony, harmony, reading};
pub use irregular::{
    Form, Irregular, Step, WordForm, cases, cases_by_slot, irregular, realise, word_cases,
    word_irregular,
};
pub use noun::{class_stem, oblique};
pub use stem::{
    STEM_ALTERNATIONS, Stem, StemRule, apply_stem, apply_stem_of, fixed_stem, noun_class,
    stem_alternation,
};
pub use verb::{
    Ending3sg, GradationSite, KOTUS_VERBS, KotusVerb, MODEL_VERBS, Passive, PresentStem,
    VERB_TYPES, VerbSubtype, VerbType, is_verb_class, kotus_verb, passive_of, passive_stem,
    past_stem, present_stem, verb, verb_from_infinitive, verb_with_passive,
};
