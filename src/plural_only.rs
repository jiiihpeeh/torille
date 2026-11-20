//! The committed plural-only table.
//!
//! The embedded lexicon carries the same fact as a bit in the sanalista blob
//! (see [`crate::kotus::EmbeddedLemma::plural_only`]). This module exists for
//! [`crate::kotus::parse_sanalista`], which reads an arbitrary text lexicon and
//! has no blob bit to consult. The table is small and the lookup is one static
//! hash set, so the duplication costs a few kilobytes and keeps the text parser
//! consistent with the embedded one.

use std::collections::HashSet;
use std::sync::OnceLock;

const TABLE: &str = include_str!("../data/plural_only.tsv");

fn table() -> &'static HashSet<(&'static str, &'static str)> {
    static TABLE_SET: OnceLock<HashSet<(&'static str, &'static str)>> = OnceLock::new();
    TABLE_SET.get_or_init(|| {
        let mut out = HashSet::new();
        for line in TABLE.lines() {
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }
            let mut fields = line.split('\t');
            let Some(word) = fields.next() else { continue };
            let homonymy = fields.next().unwrap_or("");
            if !word.is_empty() {
                out.insert((word, homonymy));
            }
        }
        out
    })
}

/// Whether `(word, homonymy)` is in the committed plural-only table.
pub(crate) fn contains(word: &str, homonymy: &str) -> bool {
    table().contains(&(word, homonymy))
}
