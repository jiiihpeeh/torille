//! Sanalista rows whose inflection expression needs correcting.
//!
//! Two kinds live here:
//!
//! * a class-`100` row (Kotus's "no declension type") that has a paradigm a
//!   neighbouring class describes exactly;
//! * a row whose parenthesised optional gradation is the standard reading
//!   (`9*(D)` for `nahka`, where the standard genitive is `nahan`).
//!
//! Naming the correction here keeps the raw sanalista untouched and gives
//! the same answer to the runtime parser and the build-script parser, which
//! is what keeps `parse_sanalista` and the embedded blob identical.
//!
//! Everything else stays as the source says; rows that are not words at all
//! belong in [`crate::rejected`], not here.

/// `(lemma, class expression)` for a row that needs a corrected expression.
///
/// Each entry is justified against the standard lemma or dictionary reading,
/// so a reader can check it without an oracle:
///
/// * `hälyyttää` declines like `hälyttää` (`53*C`, `hälyytän`), the standard
///   spelling; Omorfi analyses it as a verb without a non-standard marker.
/// * `rationoida` is an `-oida` verb like `kontrolloida`/`rationalisoida`
///   (`62`), the class every other `-oida` verb in the sanalista uses.
/// * `nahka` is `9*(D)`: the parenthesised gradation is the standard one
///   (`nahan`; also `nahkan`), and the plain class alone never emits it.
pub(crate) const CLASS: &[(&str, &str)] = &[
    ("hälyyttää", "53*C"),
    ("rationoida", "62"),
    ("nahka", "9*D"),
];

/// The class expression to use instead of the source expression, when one
/// is known.
pub(crate) fn class_for(lemma: &str) -> Option<&'static str> {
    CLASS
        .iter()
        .find(|(word, _)| *word == lemma)
        .map(|(_, class)| *class)
}
