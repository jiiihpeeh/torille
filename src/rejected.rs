//! Sanalista rows this checker deliberately rejects.
//!
//! The sanalista is the dictionary, but it is not infallible. A row listed
//! here is removed from every generated artifact: the embedded lexicon, the
//! fast form set, and the stem maps. The runtime parser skips the same rows so
//! `parse_sanalista` and the embedded blob stay identical.
//!
//! Keep the list small and documented. A row belongs here only when the word
//! itself must not be accepted, not when one of its inflected forms is wrong;
//! the latter is a class-engine fix.

/// Lemmas rejected outright.
pub(crate) const LEMMAS: &[&str] = &[
    // `viti` is a sanalista row with no accepted use in the checker; the
    // repository's language review rejects it outright.
    "viti",
    // `vinka` is Swedish, not a Finnish noun. The sanalista has two rows
    // for it, and the English/Finnish Wiktionary entry has only the Swedish
    // verb section.
    "vinka",
    // Class-100 rows that are variant spellings or non-words, not a paradigm.
    // Voikko rejects all six and Omorfi either marks them non-standard or
    // guesses `UNKNOWN`; accepting them would also hide the standard
    // spelling from the suggester (`kehoittaa` -> `kehottaa`).
    "hälyytys", // `hälytys` (39)
    "kehoittaa", // `kehottaa` (53*C)
    "kuullostaa", // `kuulostaa` (53)
    "vaakita",   // no standard lemma; Omorfi GUESS=UNKNOWN
    "verytellä", // `venytellä` (67*C)
    "veryttely", // `venyttely`
];

/// Whether a sanalista lemma is rejected.
pub(crate) fn contains(lemma: &str) -> bool {
    LEMMAS.contains(&lemma)
}
