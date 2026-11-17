//! Adposition case government: the case a closed-class adposition governs.
//!
//! This is a small curated table, not a semantic model. A row is
//! `adposition,case,example` in `data/adposition_cases.csv`; the example is
//! documentation only. The build script keeps the case names in row order, so
//! a future multi-case adposition keeps its priority.

use crate::expand::static_table::StemMap;
use crate::tags::Case;

static CASES: StemMap = StemMap::new(include_bytes!(concat!(
    env!("OUT_DIR"),
    "/adposition_cases.bin",
)));

/// The cases an adposition governs, in table order.
pub fn cases(adposition: &str) -> Vec<Case> {
    CASES
        .bases(adposition)
        .filter_map(|name| Case::ALL.into_iter().find(|case| case.name() == name))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_known_adposition_lists_its_case() {
        assert_eq!(cases("alla"), vec![Case::Genitive]);
        assert_eq!(cases("jälkeen"), vec![Case::Genitive]);
        assert_eq!(cases("ilman"), vec![Case::Partitive]);
        assert_eq!(cases("ennen"), vec![Case::Partitive]);
    }

    #[test]
    fn an_unknown_word_has_no_case() {
        assert!(cases("pöytä").is_empty());
    }
}
