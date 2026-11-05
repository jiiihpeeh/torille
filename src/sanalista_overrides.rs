//! Sanalista rows whose inflection class the source leaves as `100`.
//!
//! Kotus uses class `100` for a word it gives no declension type. For most of
//! those rows that is the end of the story (they are compounds or unknown
//! types), but a few have a paradigm that a neighbouring class describes
//! exactly. Naming it here keeps the raw sanalista untouched and gives the
//! same answer to the runtime parser and the build-script parser, which is
//! what keeps `parse_sanalista` and the embedded blob identical.
//!
//! Everything else with class `100` stays classless and generates nothing;
//! rows that are not words at all belong in [`crate::rejected`], not here.

/// `(lemma, class expression)` for a class-100 row that has a real paradigm.
///
/// Each entry is justified against the standard lemma it shares a paradigm
/// with, so a reader can check it without an oracle:
///
/// * `hälyyttää` declines like `hälyttää` (`53*C`, `hälyytän`), the standard
///   spelling; Omorfi analyses it as a verb without a non-standard marker.
/// * `rationoida` is an `-oida` verb like `kontrolloida`/`rationalisoida`
///   (`62`), the class every other `-oida` verb in the sanalista uses.
pub(crate) const CLASS: &[(&str, &str)] = &[
    ("hälyyttää", "53*C"),
    ("rationoida", "62"),
];

/// The class expression to use instead of `100`, when one is known.
pub(crate) fn class_for(lemma: &str) -> Option<&'static str> {
    CLASS
        .iter()
        .find(|(word, _)| *word == lemma)
        .map(|(_, class)| *class)
}
