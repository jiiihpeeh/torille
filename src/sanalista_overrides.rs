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
    // Wiktionary `kynkkä` (Kotus 10/koira, kk:k): the source row carries
    // only the bare class, so the hand engine would keep the strong `kk`
    // in `kynkän`. `käsikynkkä` is the same head (`käsi` + `kynkkä`) and
    // was marked 99 only because Kotus had no separate compound entry.
    ("kynkkä", "10*A"),
    ("käsikynkkä", "10*A"),
    // Wiktionary `vikunja` is `kulkija` (Kotus 12): `vikunjoiden`,
    // `vikunjoitten`, `vikunjoita`, not the class-9 `vikunjojen`/`vikunjoja`.
    ("vikunja", "12"),
];

/// Corrections that apply to one homonym of a lemma.
///
/// The source row for `live` is the dialectal 48-E noun (`lipeä`) and the
/// adjective is a separate homonym. The common English loanword is class 8
/// (`livelähetys`): Wiktionary gives `liven`, `liveä`, `livejä`, `liveen`
/// and `livenne`, and the oracle confirms `livenne`. The 48-E reading stays;
/// the class-8 reading joins it. The adjective (`live 2`, class 99) is not
/// touched.
pub(crate) const CLASS_BY_HOMONYM: &[(&str, &str, &str)] = &[
    ("live", "1", "48*E, 8"),
];

/// The class expression to use instead of the source expression, when one
/// is known.
///
/// A homonym-specific correction wins over the lemma-level table; the two
/// tables stay separate because a lemma-level `CLASS` entry applies to every
/// homonym by design.
pub(crate) fn class_for(lemma: &str, homonymy: &str) -> Option<&'static str> {
    CLASS_BY_HOMONYM
        .iter()
        .find(|(word, homonym, _)| *word == lemma && *homonym == homonymy)
        .map(|(_, _, class)| *class)
        .or_else(|| {
            CLASS
                .iter()
                .find(|(word, _)| *word == lemma)
                .map(|(_, class)| *class)
        })
}
