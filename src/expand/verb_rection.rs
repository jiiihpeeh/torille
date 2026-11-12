//! Verb rection: the case a verb governs.
//!
//! This is a small curated table, not a semantic valency model. A row is
//! `lemma,case,frame,priority,note` in `data/verb_rection.csv`; the build
//! script keeps the case names in the order the rows were written, so the
//! first correction is the highest-priority frame.

use crate::expand::static_table::StemMap;
use crate::tags::Case;

static RECTION: StemMap = StemMap::new(include_bytes!(concat!(
    env!("OUT_DIR"),
    "/verb_rection.bin",
)));

/// The governed cases for a verb lemma, highest priority first.
pub fn cases(lemma: &str) -> Vec<Case> {
    RECTION
        .bases(lemma)
        .filter_map(|name| Case::ALL.into_iter().find(|case| case.name() == name))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_known_verb_lists_its_frames_in_order() {
        assert_eq!(cases("pitää"), vec![Case::Elative, Case::Partitive]);
        assert_eq!(cases("mennä"), vec![Case::Illative, Case::Allative]);
        assert_eq!(cases("luottaa"), vec![Case::Illative]);
    }

    #[test]
    fn an_unknown_verb_has_no_frames() {
        assert!(cases("syödä").is_empty());
    }
}
